//! Explicit, bounded collection of unreferenced native library content.
//! Unknown entries and pending imports are retained; no automatic collection.
use crate::{Database,util::{storage_error,sync_directory}};
use omniplay_contracts::{ContentHash,ErrorCode,GameManifest,GameSystem,PlatformError,Result};
use rusqlite::{Transaction,TransactionBehavior};
use serde::Serialize;
use sha2::{Digest,Sha256};
use std::{collections::{BTreeMap,BTreeSet},fs::{self,File,OpenOptions},path::{Path,PathBuf},time::{Instant,Duration,UNIX_EPOCH}};

const BATCH:usize=64;
fn invalid()->PlatformError{PlatformError::new(ErrorCode::CorruptData,"Library content ownership needs recovery before cleanup")}
fn bounded()->PlatformError{PlatformError::new(ErrorCode::ResourceLimit,"Library ownership exceeds this cleanup pass; no content was removed")}
fn ordinary(path:&Path,directory:bool)->Result<fs::Metadata>{
    let metadata=fs::symlink_metadata(path).map_err(storage_error)?;
    let mut linked=metadata.file_type().is_symlink();
    #[cfg(windows)]{use std::os::windows::fs::MetadataExt;linked|=metadata.file_attributes()&0x400!=0;}
    let matches_kind=if directory{metadata.is_dir()}else{metadata.is_file()};
    if linked||!matches_kind{return Err(invalid());}Ok(metadata)
}
/// Every native asset writer holds this lease through manifest publication.
pub(crate) struct AssetLease{_file:File}
impl AssetLease{
    pub(crate) fn acquire(database:&Database)->Result<Self>{
        ordinary(&database.root,true)?;let path=database.root.join("library-assets.lock");
        match fs::symlink_metadata(&path){Ok(_)=>{ordinary(&path,false)?;},Err(error)if error.kind()==std::io::ErrorKind::NotFound=>{},Err(error)=>return Err(storage_error(error))}
        let mut options=OpenOptions::new();options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]{use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
        let file=options.open(&path).map_err(storage_error)?;let metadata=file.metadata().map_err(storage_error)?;
        if !metadata.is_file()||metadata.len()!=0{return Err(invalid());}
        file.try_lock().map_err(|_|PlatformError::new(ErrorCode::Busy,"A library import or cleanup is still finishing"))?;
        let assets=database.root.join("assets");match fs::symlink_metadata(&assets){Ok(_)=>{ordinary(&assets,true)?;},Err(error)if error.kind()==std::io::ErrorKind::NotFound=>{},Err(error)=>return Err(storage_error(error))}
        Ok(Self{_file:file})
    }
}
#[derive(Serialize)]
pub struct Preview{pub schema_version:u32,pub revision:ContentHash,pub after_hash:Option<String>,pub scanned_assets:usize,pub unused_assets:usize,pub unused_bytes:u64,pub next_after:Option<String>,pub more_available:bool,pub library_games:usize,pub retained_unknown_entries:usize}
#[derive(Serialize)]
pub struct Removed{pub removed_assets:usize,pub removed_blob_reference_bytes:u64,pub retained_assets:usize,pub durability_confirmed:bool,pub next_after:Option<String>,pub more_available:bool}
#[derive(Clone,PartialEq,Eq)]
struct Identity{bytes:u64,modified:u128,created:u128}
fn identity(path:&Path)->Result<Identity>{let value=ordinary(path,false)?;Ok(Identity{bytes:value.len(),modified:value.modified().map_err(storage_error)?.duration_since(UNIX_EPOCH).map_err(storage_error)?.as_nanos(),created:value.created().ok().and_then(|time|time.duration_since(UNIX_EPOCH).ok()).map_or(0,|time|time.as_nanos())})}
fn frame(hash:&mut Sha256,value:&[u8]){hash.update((value.len()as u64).to_le_bytes());hash.update(value);}
struct Snapshot{preview:Preview,unused:Vec<(String,Identity)>,directory:PathBuf}
fn snapshot(database:&Database,after:Option<&str>)->Result<Snapshot>{
    if let Some(value)=after{ContentHash::parse(value.to_owned())?;}
    let until=Instant::now()+Duration::from_secs(15);let directory=database.root.join("assets");
    let mut names=BTreeSet::new();let mut unknown=0usize;
    let mut hash=Sha256::new();frame(&mut hash,b"omniplay.native-unused-assets.v1");frame(&mut hash,database.device_id.as_bytes());frame(&mut hash,database.root.as_os_str().to_string_lossy().as_bytes());frame(&mut hash,after.unwrap_or("").as_bytes());
    match fs::symlink_metadata(&directory){
        Ok(_)=>{
            ordinary(&directory,true)?;
            for(index,entry)in fs::read_dir(&directory).map_err(storage_error)?.enumerate(){
                if index>=20000||Instant::now()>=until{return Err(bounded());}
                let entry=entry.map_err(storage_error)?;let name=entry.file_name();
                let Some(name)=name.to_str()else{unknown+=1;continue;};
                if ContentHash::parse(name.to_owned()).is_err(){unknown+=1;continue;}
                // A hash-shaped link or directory is uncertainty, never a file
                // to traverse or a successful deletion candidate.
                ordinary(&entry.path(),false)?;names.insert(name.to_owned());
            }
        },
        Err(error)if error.kind()==std::io::ErrorKind::NotFound=>{},Err(error)=>return Err(storage_error(error)),
    }
    frame(&mut hash,&unknown.to_le_bytes());
    let selected:Vec<_>=names.iter().filter(|name|after.is_none_or(|after|name.as_str()>after)).take(BATCH+1).cloned().collect();
    let more=selected.len()>BATCH;let mut candidates=BTreeMap::new();
    for name in selected.into_iter().take(BATCH){let value=identity(&directory.join(&name))?;frame(&mut hash,name.as_bytes());frame(&mut hash,&value.bytes.to_le_bytes());frame(&mut hash,&value.modified.to_le_bytes());frame(&mut hash,&value.created.to_le_bytes());candidates.insert(name,(value,false));}
    let mut games=BTreeSet::new();let mut json_bytes=0usize;
    let mut statement=database.connection.prepare("SELECT id,content_hash,manifest_json FROM games ORDER BY id LIMIT 20001").map_err(storage_error)?;
    let mut rows=statement.query([]).map_err(storage_error)?;
    while let Some(row)=rows.next().map_err(storage_error)?{
        if games.len()>=20000||Instant::now()>=until{return Err(bounded());}
        let id:String=row.get(0).map_err(storage_error)?;let content:String=row.get(1).map_err(storage_error)?;let raw:String=row.get(2).map_err(storage_error)?;
        json_bytes=json_bytes.checked_add(raw.len()).ok_or_else(bounded)?;if raw.len()>256*1024||json_bytes>32*1024*1024{return Err(bounded());}
        let game:GameManifest=serde_json::from_str(&raw).map_err(|_|invalid())?;game.validate()?;
        if game.id.to_string()!=id||game.content_hash.0!=content{return Err(invalid());}
        frame(&mut hash,id.as_bytes());frame(&mut hash,raw.as_bytes());games.insert(id);
        // Retain the content identity even for a non-ROM entry, conservatively.
        if let Some(value)=candidates.get_mut(&content){value.1=true;}
        if !matches!(game.system,GameSystem::Pc|GameSystem::Web){if let Some(media)=game.media{for asset in media.assets{if let Some(value)=candidates.get_mut(&asset.content_hash.0){value.1=true;}}}}
    }
    drop(rows);drop(statement);
    let active:bool=database.connection.query_row("SELECT EXISTS(SELECT 1 FROM sessions WHERE state NOT IN ('stopped','failed'))",[],|row|row.get(0)).map_err(storage_error)?;
    if active{return Err(PlatformError::new(ErrorCode::Busy,"Stop active games before collecting unused content"));}
    let mut statement=database.connection.prepare("SELECT id,game_id,metadata_json FROM saves ORDER BY id LIMIT 20001").map_err(storage_error)?;let mut rows=statement.query([]).map_err(storage_error)?;let mut count=0;
    while let Some(row)=rows.next().map_err(storage_error)?{
        count+=1;if count>20000||Instant::now()>=until{return Err(bounded());}
        let id:String=row.get(0).map_err(storage_error)?;let game:String=row.get(1).map_err(storage_error)?;let raw:String=row.get(2).map_err(storage_error)?;
        if raw.len()>65536{return Err(invalid());}let save:omniplay_contracts::SaveMetadata=serde_json::from_str(&raw).map_err(|_|invalid())?;save.validate()?;
        if save.id.to_string()!=id||save.game_id.to_string()!=game||!games.contains(&game){return Err(invalid());}
        frame(&mut hash,id.as_bytes());frame(&mut hash,raw.as_bytes());if let Some(value)=candidates.get_mut(&save.game_content_hash.0){value.1=true;}
    }
    let next=if more{candidates.keys().next_back().cloned()}else{None};let scanned=candidates.len();let mut unused=Vec::new();let mut bytes=0u64;
    for(name,(value,referenced))in candidates{frame(&mut hash,if referenced{b"retained"}else{b"unused"});if !referenced{bytes=bytes.checked_add(value.bytes).filter(|bytes|*bytes<=9_007_199_254_740_991).ok_or_else(bounded)?;unused.push((name,value));}}
    frame(&mut hash,if more{b"more"}else{b"end"});
    Ok(Snapshot{preview:Preview{schema_version:1,revision:ContentHash(format!("{:x}",hash.finalize())),after_hash:after.map(str::to_owned),scanned_assets:scanned,unused_assets:unused.len(),unused_bytes:bytes,next_after:next,more_available:more,library_games:games.len(),retained_unknown_entries:unknown},unused,directory})
}
pub fn preview(database:&Database,after:Option<&str>)->Result<Preview>{
    let _lease=AssetLease::acquire(database)?;let tx=Transaction::new_unchecked(&database.connection,TransactionBehavior::Immediate).map_err(storage_error)?;
    let value=snapshot(database,after)?;tx.commit().map_err(storage_error)?;Ok(value.preview)
}
pub fn remove(database:&Database,after:Option<&str>,expected:&ContentHash)->Result<Removed>{
    ContentHash::parse(expected.0.clone())?;let _lease=AssetLease::acquire(database)?;let tx=Transaction::new_unchecked(&database.connection,TransactionBehavior::Immediate).map_err(storage_error)?;let value=snapshot(database,after)?;
    if &value.preview.revision!=expected{return Err(PlatformError::new(ErrorCode::Conflict,"Library content changed; refresh its cleanup preview"));}
    let(mut removed,mut bytes,mut retained)=(0usize,0u64,0usize);
    // Native unlink is not an atomic multi-file transaction. Report partial
    // results honestly; the next preview independently observes remaining files.
    for(name,observed)in value.unused{let path=value.directory.join(name);if identity(&path).is_ok_and(|current|current==observed)&&fs::remove_file(&path).is_ok(){removed+=1;bytes+=observed.bytes;}else{retained+=1;}}
    let durable=removed==0||sync_directory(&value.directory).is_ok();
    // Keep the database write lock until all unlinks finish. No visible save or
    // manifest can acquire ownership between the complete scan and removal.
    let audited=removed==0||tx.execute("INSERT INTO audit(event,entity_id,created_at_ms) VALUES('library.assets_collect',NULL,?1)",[crate::now_ms()]).is_ok();
    let committed=if audited{tx.commit().is_ok()}else{drop(tx);false};
    Ok(Removed{removed_assets:removed,removed_blob_reference_bytes:bytes,retained_assets:retained,durability_confirmed:durable&&committed&&cfg!(unix),next_after:value.preview.next_after,more_available:value.preview.more_available})
}

#[cfg(test)]
#[path="asset_storage_tests.rs"]
mod tests;
