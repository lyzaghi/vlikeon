use crate::{Database,now_ms,util::{storage_error,sync_directory}};
use omniplay_contracts::{ContentHash,ErrorCode,PlatformError,Result,SaveCleanupStatus,SavePurgePreview,SavePurgeResult};
use rusqlite::{Connection,OptionalExtension,TransactionBehavior,params};
use sha2::{Digest,Sha256};
use std::{fs,path::{Path,PathBuf}};
use uuid::Uuid;

fn busy()->PlatformError{PlatformError::new(ErrorCode::Busy,"Stop the game before removing its saved progress")}
fn conflict()->PlatformError{PlatformError::new(ErrorCode::Conflict,"Saved progress changed after the removal preview").recover("Refresh the preview and choose again")}
pub const MAX_LOCAL_SAVE_BYTES:u64=8*1024*1024*1024;
pub const MAX_LOCAL_SAVE_COUNT:u64=20000;
pub fn locally_removed(database:&Database,id:Uuid)->Result<bool>{database.connection.query_row("SELECT EXISTS(SELECT 1 FROM locally_removed_saves WHERE save_id=?1)",[id.to_string()],|row|row.get(0)).map_err(storage_error)}
fn frame(hash:&mut Sha256,value:&[u8]){hash.update((value.len()as u64).to_le_bytes());hash.update(value);}
fn preview(connection:&Connection,game_id:Uuid)->Result<SavePurgePreview>{
    let content:Option<String>=connection.query_row("SELECT content_hash FROM games WHERE id=?1",[game_id.to_string()],|row|row.get(0)).optional().map_err(storage_error)?;
    let content=content.ok_or_else(||PlatformError::new(ErrorCode::NotFound,"Game was not found"))?;ContentHash::parse(content.clone())?;
    let mut hash=Sha256::new();hash.update(b"omniplay.save-purge-preview.v1\0");frame(&mut hash,game_id.as_bytes());frame(&mut hash,content.as_bytes());
    let mut count=0u64;let mut bytes=0u64;
    let mut statement=connection.prepare("SELECT metadata_json,payload_bytes FROM saves WHERE game_id=?1 ORDER BY id").map_err(storage_error)?;
    let rows=statement.query_map([game_id.to_string()],|row|Ok((row.get::<_,String>(0)?,row.get::<_,u64>(1)?))).map_err(storage_error)?;
    for row in rows{
        let(raw,size)=row.map_err(storage_error)?;
        if raw.len()>64*1024||size==0||size>omniplay_contracts::MAX_SAVE_BYTES{return Err(PlatformError::new(ErrorCode::CorruptData,"Saved progress metadata needs recovery"));}
        count=count.checked_add(1).ok_or_else(conflict)?;bytes=bytes.checked_add(size).ok_or_else(conflict)?;frame(&mut hash,raw.as_bytes());frame(&mut hash,&size.to_le_bytes());
    }
    drop(statement);let mut heads=0u64;
    let mut statement=connection.prepare("SELECT kind,slot,save_id FROM save_heads WHERE game_id=?1 ORDER BY kind,slot").map_err(storage_error)?;
    let rows=statement.query_map([game_id.to_string()],|row|Ok((row.get::<_,String>(0)?,row.get::<_,u16>(1)?,row.get::<_,String>(2)?))).map_err(storage_error)?;
    for row in rows{let(kind,slot,id)=row.map_err(storage_error)?;Uuid::parse_str(&id).map_err(storage_error)?;frame(&mut hash,kind.as_bytes());frame(&mut hash,&slot.to_le_bytes());frame(&mut hash,id.as_bytes());heads=heads.checked_add(1).ok_or_else(conflict)?;}
    frame(&mut hash,&count.to_le_bytes());frame(&mut hash,&heads.to_le_bytes());
    Ok(SavePurgePreview{game_id,revision:ContentHash(format!("{:x}",hash.finalize())),save_count:count,payload_bytes:bytes,current_heads:heads})
}
pub fn purge_preview(database:&Database,game_id:Uuid)->Result<SavePurgePreview>{
    let tx=database.connection.unchecked_transaction().map_err(storage_error)?;let result=preview(&tx,game_id)?;tx.commit().map_err(storage_error)?;Ok(result)
}
pub fn purge(database:&mut Database,game_id:Uuid,expected:ContentHash)->Result<SavePurgeResult>{
    ContentHash::parse(expected.0.clone())?;
    let tx=database.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage_error)?;
    let active:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM sessions WHERE game_id=?1 AND state NOT IN ('stopped','failed'))",[game_id.to_string()],|row|row.get(0)).map_err(storage_error)?;if active{return Err(busy());}
    let state=preview(&tx,game_id)?;if state.revision!=expected{return Err(conflict());}
    let removed_at=now_ms();
    // Keep UUID-only local suppression so an existing cloud object does not
    // silently resurrect intentionally removed local progress. Cloud is untouched.
    tx.execute("INSERT INTO locally_removed_saves(save_id,game_content_hash,removed_at_ms) SELECT s.id,g.content_hash,?2 FROM saves s JOIN games g ON g.id=s.game_id WHERE s.game_id=?1 ON CONFLICT(save_id) DO NOTHING",params![game_id.to_string(),removed_at]).map_err(storage_error)?;
    tx.execute("INSERT INTO save_payload_gc(save_id,game_id,payload_bytes,retired_at_ms) SELECT s.id,s.game_id,l.payload_bytes,?2 FROM saves s JOIN save_legacy_payloads l ON l.save_id=s.id WHERE s.game_id=?1 ON CONFLICT(save_id) DO NOTHING",params![game_id.to_string(),removed_at]).map_err(storage_error)?;
    tx.execute("DELETE FROM sync_outbox WHERE save_id IN (SELECT id FROM saves WHERE game_id=?1)",[game_id.to_string()]).map_err(storage_error)?;
    tx.execute("DELETE FROM sync_receipts WHERE save_id IN (SELECT id FROM saves WHERE game_id=?1)",[game_id.to_string()]).map_err(storage_error)?;
    tx.execute("DELETE FROM preferences WHERE key IN (SELECT 'sync_save_recovery_v1:'||id FROM saves WHERE game_id=?1)",[game_id.to_string()]).map_err(storage_error)?;
    tx.execute("DELETE FROM save_heads WHERE game_id=?1",[game_id.to_string()]).map_err(storage_error)?;
    tx.execute("DELETE FROM saves WHERE game_id=?1",[game_id.to_string()]).map_err(storage_error)?;
    tx.execute("INSERT INTO audit(event,entity_id,created_at_ms) VALUES('save.purge_local',?1,?2)",params![game_id.to_string(),removed_at]).map_err(storage_error)?;
    tx.commit().map_err(storage_error)?;
    // Logical deletion has committed. Cleanup errors cannot roll back a reported
    // removal or discard the durable queue; return its actual remaining counts.
    let _=cleanup(database,128);let status=cleanup_status(database)?;
    Ok(SavePurgeResult{game_id,removed_count:state.save_count,retired_bytes:state.payload_bytes,cleanup:status})
}
pub fn cleanup_status(database:&Database)->Result<SaveCleanupStatus>{
    database.connection.query_row("SELECT (SELECT count(*) FROM save_payload_gc)+(SELECT count(*) FROM save_repair_files WHERE state!='live'),(SELECT coalesce(sum(payload_bytes),0) FROM save_payload_gc)+(SELECT coalesce(sum(payload_bytes),0) FROM save_repair_files WHERE state!='live')",[],|row|Ok(SaveCleanupStatus{pending_files:row.get(0)?,pending_bytes:row.get(1)?})).map_err(storage_error)
}
fn ordinary(path:&Path,directory:bool)->Result<bool>{
    let metadata=match fs::symlink_metadata(path){Ok(value)=>value,Err(error)if error.kind()==std::io::ErrorKind::NotFound=>return Ok(false),Err(error)=>return Err(storage_error(error))};
    let mut linked=metadata.file_type().is_symlink();
    #[cfg(windows)]{use std::os::windows::fs::MetadataExt;linked|=metadata.file_attributes()&0x400!=0;}
    let matches_kind=if directory{metadata.is_dir()}else{metadata.is_file()};
    if linked||!matches_kind{return Err(PlatformError::new(ErrorCode::StorageFailure,"A retired save path is not an ordinary owned entry"));}Ok(true)
}
fn remove_payload(database:&Database,game:Uuid,save:Uuid)->Result<()>{
    if !ordinary(&database.root,true)?{return Err(storage_error("profile root missing"));}
    let base=database.root.canonicalize().map_err(storage_error)?;let saves=base.join("saves");
    if !ordinary(&saves,true)?{sync_directory(&base)?;return Ok(());}let directory=saves.join(game.to_string());if !ordinary(&directory,true)?{sync_directory(&saves)?;return Ok(());}
    let target:PathBuf=directory.join(format!("{save}.bin"));
    if ordinary(&target,false)?{fs::remove_file(&target).map_err(storage_error)?;}sync_directory(&directory)?;
    // Keep the empty game directory: removing it could race a new save's
    // create-directory/open-pending-file sequence. Never traverse unknown files.
    Ok(())
}
pub fn cleanup(database:&Database,limit:u16)->Result<SaveCleanupStatus>{
    if limit==0||limit>256{return Err(PlatformError::new(ErrorCode::InvalidRequest,"Invalid save cleanup bound"));}
    let records={let mut statement=database.connection.prepare("SELECT game_id,save_id FROM save_payload_gc ORDER BY attempt_count,retired_at_ms,save_id LIMIT ?1").map_err(storage_error)?;let rows=statement.query_map([limit],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?))).map_err(storage_error)?;rows.collect::<std::result::Result<Vec<_>,_>>().map_err(storage_error)?};
    for(game,save)in records{
        let game=Uuid::parse_str(&game).map_err(storage_error)?;let save=Uuid::parse_str(&save).map_err(storage_error)?;
        // A live save must never be collected even if a queue record is corrupt.
        let live:bool=database.connection.query_row("SELECT EXISTS(SELECT 1 FROM saves WHERE id=?1)",[save.to_string()],|row|row.get(0)).map_err(storage_error)?;
        if !live&&locally_removed(database,save)?&&remove_payload(database,game,save).is_ok(){database.connection.execute("DELETE FROM save_payload_gc WHERE save_id=?1",[save.to_string()]).map_err(storage_error)?;}
        else{database.connection.execute("UPDATE save_payload_gc SET attempt_count=MIN(attempt_count+1,1000000) WHERE save_id=?1",[save.to_string()]).map_err(storage_error)?;}
    }
    let _=crate::save_payloads::cleanup(database,limit);
    cleanup_status(database)
}
