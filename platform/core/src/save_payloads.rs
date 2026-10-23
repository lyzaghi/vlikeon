//! Immutable replacement payloads. A durable staged-file journal precedes I/O;
//! a SQLite pointer publication follows synced/verified bytes. Never overwrite
//! a generation, origin, head or the original potentially damaged file.
use crate::{Database,save_origins,util::{digest,hash_file,storage_error,sync_directory}};
use omniplay_contracts::{ErrorCode,PlatformError,Result,SaveMetadata,SaveRepairResult,save_origin_hash,MAX_SAVE_BYTES};
use rusqlite::{Connection,OptionalExtension,TransactionBehavior,params};
use std::{fs::{self,File,OpenOptions},io::{Seek,SeekFrom,Write},path::{Path,PathBuf}};
use uuid::Uuid;

fn corrupt()->PlatformError{PlatformError::new(ErrorCode::CorruptData,"Saved progress payload storage is inconsistent")}
fn conflict()->PlatformError{PlatformError::new(ErrorCode::Conflict,"Saved progress changed during payload recovery").recover("Refresh history and retry the same recovery file")}
fn storage_id(raw:&str)->Result<Uuid>{let id=Uuid::parse_str(raw).map_err(|_|corrupt())?;if id.to_string()!=raw{return Err(corrupt());}Ok(id)}
fn plain(metadata:&fs::Metadata,directory:bool)->bool{
    #[cfg(windows)]{use std::os::windows::fs::MetadataExt;if metadata.file_attributes()&0x400!=0{return false;}}
    !metadata.file_type().is_symlink()&&if directory{metadata.is_dir()}else{metadata.is_file()}
}
pub(crate) fn directory(path:&Path)->Result<()>{if !plain(&fs::symlink_metadata(path).map_err(storage_error)?,true){return Err(corrupt());}Ok(())}
pub(crate) fn open_file(path:&Path)->Result<File>{
    let mut options=OpenOptions::new();options.read(true);
    #[cfg(unix)]{use std::os::unix::fs::OpenOptionsExt;options.custom_flags(libc::O_NOFOLLOW|libc::O_CLOEXEC);}
    #[cfg(windows)]{use std::os::windows::fs::OpenOptionsExt;options.custom_flags(0x00200000).share_mode(1);}
    let file=options.open(path).map_err(|error|if error.kind()==std::io::ErrorKind::NotFound{PlatformError::new(ErrorCode::NotFound,"Saved progress payload is missing")}else{storage_error(error)})?;
    if !plain(&file.metadata().map_err(storage_error)?,false){return Err(corrupt());}Ok(file)
}
pub(crate) fn lease(root:&Path)->Result<File>{
    directory(root)?;let mut options=OpenOptions::new();options.read(true).write(true).create(true);
    #[cfg(unix)]{use std::os::unix::fs::OpenOptionsExt;options.mode(0o600).custom_flags(libc::O_NOFOLLOW|libc::O_CLOEXEC);}
    #[cfg(windows)]{use std::os::windows::fs::OpenOptionsExt;options.custom_flags(0x00200000).share_mode(3);}
    let file=options.open(root.join("save-payload-repair-v1.lock")).map_err(storage_error)?;
    let metadata=file.metadata().map_err(storage_error)?;if !plain(&metadata,false)||metadata.len()!=0{return Err(corrupt());}
    match file.try_lock(){Ok(())=>Ok(file),Err(std::fs::TryLockError::WouldBlock)=>Err(PlatformError::new(ErrorCode::Busy,"Saved progress storage is busy; retry after the current operation finishes")),Err(std::fs::TryLockError::Error(error))=>Err(storage_error(error))}
}
fn repair_directory(root:&Path,create:bool)->Result<PathBuf>{
    directory(root)?;let path=root.join("repaired-saves-v1");
    if create{let mut builder=fs::DirBuilder::new();
        #[cfg(unix)]{use std::os::unix::fs::DirBuilderExt;builder.mode(0o700);}
        match builder.create(&path){Ok(())=>sync_directory(root)?,Err(error)if error.kind()==std::io::ErrorKind::AlreadyExists=>{},Err(error)=>return Err(storage_error(error))}
    }
    directory(&path)?;Ok(path)
}
/// Logical history plus pending physical debt has one bounded admission policy.
/// Bytes reserve every known legacy original and every staged/live/retired file.
pub(crate) fn capacity(connection:&Connection,additional_count:u64,additional_bytes:u64)->Result<()>{
    let(count,bytes):(u64,u64)=connection.query_row("SELECT (SELECT count(*) FROM saves)+(SELECT count(*) FROM save_payload_gc)+(SELECT count(*) FROM save_repair_files WHERE state!='live'),(SELECT coalesce(sum(payload_bytes),0) FROM save_legacy_payloads)+(SELECT coalesce(sum(payload_bytes),0) FROM save_payload_gc)+(SELECT coalesce(sum(payload_bytes),0) FROM save_repair_files)",[],|row|Ok((row.get(0)?,row.get(1)?))).map_err(storage_error)?;
    if count.checked_add(additional_count).is_none_or(|n|n>crate::save_maintenance::MAX_LOCAL_SAVE_COUNT)||bytes.checked_add(additional_bytes).is_none_or(|n|n>crate::save_maintenance::MAX_LOCAL_SAVE_BYTES){return Err(PlatformError::new(ErrorCode::ResourceLimit,"Local saved progress storage is full").recover("Export progress, remove local history and finish pending cleanup before retrying"));}Ok(())
}
/// The lease stays owned through the caller's later metadata publication. Drop
/// leaves journaled staged bytes for bounded cleanup after rejection or a crash.
pub(crate) struct StagedPayload{file_id:Uuid,_lease:File}
impl StagedPayload{
    pub(crate) fn publish(&self,connection:&Connection,metadata:&SaveMetadata)->Result<()>{
        let matches:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM save_repair_files WHERE file_id=?1 AND save_id=?2 AND game_id=?3 AND payload_hash=?4 AND payload_bytes=?5 AND state='staged')",params![self.file_id.to_string(),metadata.id.to_string(),metadata.game_id.to_string(),metadata.payload_hash.0,metadata.payload_bytes],|row|row.get(0)).map_err(storage_error)?;
        if !matches{return Err(corrupt());}
        if connection.execute("UPDATE save_repair_files SET state='live' WHERE file_id=?1 AND state='staged'",[self.file_id.to_string()]).map_err(storage_error)?!=1{return Err(corrupt());}
        connection.execute("INSERT INTO save_payload_locations(save_id,file_id) VALUES(?1,?2)",params![metadata.id.to_string(),self.file_id.to_string()]).map_err(storage_error)?;
        // The save-insert fallback trigger is for original-path writers. This
        // save's actual bytes are in its journaled immutable location instead.
        if connection.execute("DELETE FROM save_legacy_payloads WHERE save_id=?1",[metadata.id.to_string()]).map_err(storage_error)?!=1{return Err(corrupt());}
        capacity(connection,0,0)
    }
}
pub(crate) fn stage(database:&mut Database,metadata:&SaveMetadata,payload:&[u8])->Result<StagedPayload>{
    metadata.validate()?;if payload.len()as u64!=metadata.payload_bytes||digest(payload)!=metadata.payload_hash{return Err(PlatformError::new(ErrorCode::ContentMismatch,"Staged save bytes do not match their metadata"));}
    let lease=lease(&database.root)?;let _=cleanup_locked(database,8);let file_id=Uuid::new_v4();
    let tx=database.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage_error)?;
    let game:Option<String>=tx.query_row("SELECT content_hash FROM games WHERE id=?1",[metadata.game_id.to_string()],|row|row.get(0)).optional().map_err(storage_error)?;
    if game.as_deref()!=Some(metadata.game_content_hash.0.as_str()){return Err(PlatformError::new(ErrorCode::ContentMismatch,"Save game content is not in this library"));}
    let excluded:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM saves WHERE id=?1) OR EXISTS(SELECT 1 FROM locally_removed_saves WHERE save_id=?1)",[metadata.id.to_string()],|row|row.get(0)).map_err(storage_error)?;
    if excluded{return Err(PlatformError::new(ErrorCode::Conflict,"This saved generation already exists or was intentionally removed"));}
    capacity(&tx,1,metadata.payload_bytes)?;
    tx.execute("INSERT INTO save_repair_files(file_id,save_id,game_id,payload_hash,payload_bytes,state,created_at_ms) VALUES(?1,?2,?3,?4,?5,'staged',?6)",params![file_id.to_string(),metadata.id.to_string(),metadata.game_id.to_string(),metadata.payload_hash.0,metadata.payload_bytes,crate::now_ms()]).map_err(storage_error)?;
    tx.commit().map_err(storage_error)?;
    let directory=repair_directory(&database.root,true)?;let path=directory.join(format!("{file_id}.bin"));
    let mut options=OpenOptions::new();options.read(true).write(true).create_new(true);
    #[cfg(unix)]{use std::os::unix::fs::OpenOptionsExt;options.mode(0o600).custom_flags(libc::O_NOFOLLOW|libc::O_CLOEXEC);}
    #[cfg(windows)]{use std::os::windows::fs::OpenOptionsExt;options.custom_flags(0x00200000).share_mode(0);}
    let mut file=options.open(path).map_err(storage_error)?;
    for chunk in payload.chunks(65536){file.write_all(chunk).map_err(storage_error)?;}
    file.sync_all().map_err(storage_error)?;file.seek(SeekFrom::Start(0)).map_err(storage_error)?;
    let file=verified(file,metadata)?;drop(file);sync_directory(&directory)?;
    Ok(StagedPayload{file_id,_lease:lease})
}
fn location(connection:&Connection,root:&Path,metadata:&SaveMetadata)->Result<(PathBuf,Option<Uuid>)>{
    type Row=(String,String,String,String,u64,String);
    let row:Option<Row>=connection.query_row("SELECT f.file_id,f.save_id,f.game_id,f.payload_hash,f.payload_bytes,f.state FROM save_payload_locations l LEFT JOIN save_repair_files f ON f.file_id=l.file_id WHERE l.save_id=?1",[metadata.id.to_string()],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?))).optional().map_err(storage_error)?;
    directory(root)?;
    if let Some((id,save,game,hash,bytes,state))=row{
        let id=storage_id(&id)?;
        if state!="live"||save!=metadata.id.to_string()||game!=metadata.game_id.to_string()||hash!=metadata.payload_hash.0||bytes!=metadata.payload_bytes{return Err(corrupt());}
        // The directory can be absent after data loss. Its absence is repairable;
        // a link, wrong type or denied path is never treated as a missing file.
        let path=root.join("repaired-saves-v1");
        match fs::symlink_metadata(&path){Ok(value)if plain(&value,true)=>{},Err(error)if error.kind()==std::io::ErrorKind::NotFound=>{},Ok(_)=>return Err(corrupt()),Err(error)=>return Err(storage_error(error))}
        Ok((path.join(format!("{id}.bin")),Some(id)))
    }else{
        let legacy:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM save_legacy_payloads WHERE save_id=?1 AND game_id=?2 AND payload_bytes=?3)",params![metadata.id.to_string(),metadata.game_id.to_string(),metadata.payload_bytes],|row|row.get(0)).map_err(storage_error)?;
        if !legacy{return Err(corrupt());}
        let base=root.join("saves");let game=base.join(metadata.game_id.to_string());
        for path in [&base,&game]{match fs::symlink_metadata(path){Ok(value)if plain(&value,true)=>{},Err(error)if error.kind()==std::io::ErrorKind::NotFound=>{},Ok(_)=>return Err(corrupt()),Err(error)=>return Err(storage_error(error))}}
        Ok((game.join(format!("{}.bin",metadata.id)),None))
    }
}
fn verified(mut file:File,metadata:&SaveMetadata)->Result<File>{
    if file.metadata().map_err(storage_error)?.len()!=metadata.payload_bytes{return Err(PlatformError::new(ErrorCode::CorruptData,"Saved progress payload length needs recovery"));}
    let(hash,bytes)=hash_file(&mut file,MAX_SAVE_BYTES)?;
    if hash!=metadata.payload_hash||bytes!=metadata.payload_bytes{return Err(PlatformError::new(ErrorCode::CorruptData,"Saved progress payload integrity needs recovery"));}
    file.seek(SeekFrom::Start(0)).map_err(storage_error)?;Ok(file)
}
fn healthy(connection:&Connection,root:&Path,metadata:&SaveMetadata)->Result<(bool,Option<Uuid>)>{
    let(path,id)=location(connection,root,metadata)?;
    match open_file(&path){
        Ok(file)=>match verified(file,metadata){Ok(_)=>Ok((true,id)),Err(error)if error.code==ErrorCode::CorruptData=>Ok((false,id)),Err(error)=>Err(error)},
        Err(error)if error.code==ErrorCode::NotFound=>Ok((false,id)),Err(error)=>Err(error),
    }
}
/// Every read/export/native transfer uses the committed location, with actual
/// byte integrity verified before returning an owned descriptor at offset zero.
pub fn open(database:&Database,metadata:&SaveMetadata)->Result<File>{
    // Fence location lookup/open against retirement collection. The returned
    // descriptor owns the selected inode after this short-lived lease ends.
    let _lease=lease(&database.root)?;
    let tx=database.connection.unchecked_transaction().map_err(storage_error)?;
    let current=save_origins::local_metadata(&tx,metadata.id)?;
    if save_origin_hash(&current)?!=save_origin_hash(metadata)?{return Err(conflict());}
    let(path,_)=location(&tx,&database.root,metadata)?;
    let file=verified(open_file(&path)?,metadata)?;tx.commit().map_err(storage_error)?;Ok(file)
}
fn stopped(connection:&Connection,game:Uuid)->Result<()>{
    let active:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM sessions WHERE game_id=?1 AND state NOT IN ('stopped','failed'))",[game.to_string()],|row|row.get(0)).map_err(storage_error)?;
    if active{return Err(PlatformError::new(ErrorCode::Busy,"Stop the game before recovering its saved progress"));}Ok(())
}
/// Complete a trusted request only if its actual current payload is intact.
/// A false result makes no request completion and authorizes no file change.
pub fn intact_with_hooks(database:&mut Database,expected:&SaveMetadata,guard:impl Fn(&Connection)->Result<()>,complete:impl Fn(&Connection)->Result<()>)->Result<bool>{
    let _lease=lease(&database.root)?;let root=database.root.clone();
    let tx=database.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage_error)?;guard(&tx)?;
    let local=save_origins::local_metadata(&tx,expected.id)?;if save_origin_hash(&local)?!=save_origin_hash(expected)?{return Err(conflict());}
    let(good,_)=healthy(&tx,&root,&local)?;
    if good{guard(&tx)?;complete(&tx)?;}tx.commit().map_err(storage_error)?;Ok(good)
}
/// `canonical` is Some only after authenticated synchronization decryption.
/// User-selected exports restore bytes only and cannot upgrade provenance.
pub fn repair(database:&mut Database,expected:&SaveMetadata,payload:&[u8],canonical:Option<SaveMetadata>)->Result<SaveRepairResult>{
    repair_with_hooks(database,expected,payload,canonical,|_|Ok(()),|_,_|Ok(()))
}
/// Trusted policy hooks run inside the actual publication transactions. A
/// caller can fence cancellation/consent and publish its request completion in
/// the same commit as the repaired pointer, without an acknowledgement gap.
pub fn repair_with_hooks(database:&mut Database,expected:&SaveMetadata,payload:&[u8],canonical:Option<SaveMetadata>,
    guard:impl Fn(&Connection)->Result<()>,complete:impl Fn(&Connection,bool)->Result<()>)->Result<SaveRepairResult>{
    guard(&database.connection)?;
    expected.validate()?;
    if payload.len()as u64!=expected.payload_bytes||digest(payload)!=expected.payload_hash{return Err(PlatformError::new(ErrorCode::ContentMismatch,"Recovery bytes do not match the existing saved progress"));}
    let _lease=lease(&database.root)?;
    // Reclaim only previously journaled, unreferenced files under the same lease.
    let _=cleanup_locked(database,8);
    let root=database.root.clone();let file_id=Uuid::new_v4();
    let tx=database.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage_error)?;
    guard(&tx)?;
    let local=save_origins::local_metadata(&tx,expected.id)?;
    if save_origin_hash(&local)?!=save_origin_hash(expected)?{return Err(conflict());}
    if let Some(metadata)=canonical{save_origins::attach_remote(&tx,&local,metadata)?;}
    let(good,previous)=healthy(&tx,&root,&local)?;
    if good{guard(&tx)?;complete(&tx,false)?;tx.commit().map_err(storage_error)?;return Ok(SaveRepairResult{metadata:local,repaired:false});}
    stopped(&tx,local.game_id)?;
    capacity(&tx,1,local.payload_bytes)?;
    tx.execute("INSERT INTO save_repair_files(file_id,save_id,game_id,payload_hash,payload_bytes,state,created_at_ms) VALUES(?1,?2,?3,?4,?5,'staged',?6)",params![file_id.to_string(),local.id.to_string(),local.game_id.to_string(),local.payload_hash.0,local.payload_bytes,crate::now_ms()]).map_err(storage_error)?;
    tx.commit().map_err(storage_error)?;
    // Publication is not a rename over an old save. A crash here leaves a known
    // unreferenced file; a later bounded collector owns it after this lease ends.
    let directory=repair_directory(&root,true)?;let path=directory.join(format!("{file_id}.bin"));
    let mut options=OpenOptions::new();options.write(true).read(true).create_new(true);
    #[cfg(unix)]{use std::os::unix::fs::OpenOptionsExt;options.mode(0o600).custom_flags(libc::O_NOFOLLOW|libc::O_CLOEXEC);}
    #[cfg(windows)]{use std::os::windows::fs::OpenOptionsExt;options.custom_flags(0x00200000).share_mode(0);}
    guard(&database.connection)?;
    let mut file=options.open(&path).map_err(storage_error)?;
    for chunk in payload.chunks(65536){guard(&database.connection)?;file.write_all(chunk).map_err(storage_error)?;}
    file.sync_all().map_err(storage_error)?;guard(&database.connection)?;
    file.seek(SeekFrom::Start(0)).map_err(storage_error)?;let file=verified(file,&local)?;drop(file);sync_directory(&directory)?;
    let tx=database.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage_error)?;
    guard(&tx)?;
    let current=save_origins::local_metadata(&tx,local.id)?;
    if save_origin_hash(&current)?!=save_origin_hash(&local)?{return Err(conflict());}
    stopped(&tx,local.game_id)?;
    let(good,selected)=healthy(&tx,&root,&local)?;
    if selected!=previous{return Err(conflict());}
    if good{tx.execute("UPDATE save_repair_files SET state='retired' WHERE file_id=?1",[file_id.to_string()]).map_err(storage_error)?;guard(&tx)?;complete(&tx,false)?;tx.commit().map_err(storage_error)?;return Ok(SaveRepairResult{metadata:local,repaired:false});}
    if tx.execute("UPDATE save_repair_files SET state='live' WHERE file_id=?1 AND state='staged'",[file_id.to_string()]).map_err(storage_error)?!=1{return Err(corrupt());}
    tx.execute("INSERT INTO save_payload_locations(save_id,file_id) VALUES(?1,?2) ON CONFLICT(save_id) DO UPDATE SET file_id=excluded.file_id",params![local.id.to_string(),file_id.to_string()]).map_err(storage_error)?;
    if let Some(previous)=previous{tx.execute("UPDATE save_repair_files SET state='retired' WHERE file_id=?1",[previous.to_string()]).map_err(storage_error)?;}
    tx.execute("INSERT INTO audit(event,entity_id,created_at_ms) VALUES('save.payload_repaired',?1,?2)",params![local.id.to_string(),crate::now_ms()]).map_err(storage_error)?;
    capacity(&tx,0,0)?;
    guard(&tx)?;complete(&tx,true)?;
    tx.commit().map_err(storage_error)?;
    Ok(SaveRepairResult{metadata:local,repaired:true})
}
fn cleanup_locked(database:&Database,limit:u16)->Result<()>{
    let directory=database.root.join("repaired-saves-v1");
    directory_check_optional(&directory)?;
    let rows={let mut statement=database.connection.prepare("SELECT file_id,payload_bytes FROM save_repair_files WHERE state IN ('staged','retired') ORDER BY attempt_count,created_at_ms,file_id LIMIT ?1").map_err(storage_error)?;
        let values=statement.query_map([limit],|row|Ok((row.get::<_,String>(0)?,row.get::<_,u64>(1)?))).map_err(storage_error)?;
        values.collect::<std::result::Result<Vec<_>,_>>().map_err(storage_error)?};
    for(id,bytes)in rows{
        let id=storage_id(&id)?;if bytes==0||bytes>MAX_SAVE_BYTES{return Err(corrupt());}
        let referenced:bool=database.connection.query_row("SELECT EXISTS(SELECT 1 FROM save_payload_locations WHERE file_id=?1)",[id.to_string()],|row|row.get(0)).map_err(storage_error)?;
        if referenced{return Err(corrupt());}
        let path=directory.join(format!("{id}.bin"));
        let removed=(||->Result<()>{
            match fs::symlink_metadata(&path){Ok(value)if plain(&value,false)=>fs::remove_file(&path).map_err(storage_error)?,Err(error)if error.kind()==std::io::ErrorKind::NotFound=>{},Ok(_)=>return Err(corrupt()),Err(error)=>return Err(storage_error(error))}
            if directory.exists(){sync_directory(&directory)?;}else{sync_directory(&database.root)?;}Ok(())
        })();
        if removed.is_ok(){database.connection.execute("DELETE FROM save_repair_files WHERE file_id=?1 AND NOT EXISTS(SELECT 1 FROM save_payload_locations WHERE file_id=?1)",[id.to_string()]).map_err(storage_error)?;}
        else{database.connection.execute("UPDATE save_repair_files SET attempt_count=MIN(attempt_count+1,1000000) WHERE file_id=?1",[id.to_string()]).map_err(storage_error)?;}
    }Ok(())
}
fn directory_check_optional(path:&Path)->Result<()>{match fs::symlink_metadata(path){Ok(value)if plain(&value,true)=>Ok(()),Err(error)if error.kind()==std::io::ErrorKind::NotFound=>Ok(()),Ok(_)=>Err(corrupt()),Err(error)=>Err(storage_error(error))}}
pub fn cleanup(database:&Database,limit:u16)->Result<()>{
    if limit==0||limit>256{return Err(PlatformError::new(ErrorCode::InvalidRequest,"Invalid repair cleanup bound"));}
    let _lease=lease(&database.root)?;cleanup_locked(database,limit)
}
