//! Durable staging-copy debt. Rows survive crashes and uncertain process exit;
//! only confirmed retirement followed by actual directory removal releases them.
use crate::{Database,now_ms,util::storage_error};
use omniplay_contracts::{ContentHash,ErrorCode,PlatformError,Result};
use rusqlite::{params,OptionalExtension,TransactionBehavior};
use std::{collections::BTreeSet,fs,path::Path};
use uuid::Uuid;

pub const METADATA_RESERVE:u64=8*1024*1024;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum RecoveryPhase {Reserved,Staged,Retiring}
#[derive(Clone,Copy,Debug)]
pub struct RecoveryReservation {pub phase:RecoveryPhase,pub windows_creation_protocol:u32}

/// Include ledger-only debts in the recovery inventory. Directory enumeration
/// alone misses a creator that died after reservation but before mkdir.
pub fn windows_recovery_candidates(database:&Database,profile:Uuid)->Result<Vec<Uuid>>{
    if profile.is_nil(){return Err(invalid());}
    let mut query=database.connection.prepare("SELECT work_id,profile_id FROM native_work_storage ORDER BY work_id LIMIT 65").map_err(storage_error)?;
    let rows=query.query_map([],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?))).map_err(storage_error)?;
    let mut result=Vec::new();
    for row in rows{let(work,owner)=row.map_err(storage_error)?;if result.len()>=64||id(&owner)?!=profile{return Err(invalid());}result.push(id(&work)?);}
    Ok(result)
}

pub fn windows_recovery_reservation(database:&Database,profile:Uuid,work:Uuid)->Result<Option<RecoveryReservation>>{
    if profile.is_nil()||work.is_nil(){return Err(invalid());}
    let row:Option<(String,String,u32)>=database.connection.query_row(
        "SELECT profile_id,phase,windows_creation_protocol FROM native_work_storage WHERE work_id=?1",
        [work.to_string()],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).optional().map_err(storage_error)?;
    let Some((owner,phase,protocol))=row else{return Ok(None)};
    if id(&owner)?!=profile||protocol>1{return Err(invalid());}
    let phase=match phase.as_str(){"reserved"=>RecoveryPhase::Reserved,"staged"=>RecoveryPhase::Staged,"retiring"=>RecoveryPhase::Retiring,_=>return Err(invalid())};
    Ok(Some(RecoveryReservation{phase,windows_creation_protocol:protocol}))
}
pub fn status(database:&Database,profile:Uuid)->Result<omniplay_contracts::NativeWorkStorageStatus>{
    if profile.is_nil(){return Err(invalid());}
    let(works,reserved,staged,retiring,bytes,files,foreign):(u32,u32,u32,u32,u64,u32,u32)=database.connection.query_row(
        "SELECT count(*),COALESCE(sum(phase='reserved'),0),COALESCE(sum(phase='staged'),0),COALESCE(sum(phase='retiring'),0),COALESCE(sum(reserved_bytes),0),COALESCE(sum(file_count),0),COALESCE(sum(profile_id!=?1),0) FROM native_work_storage",
        [profile.to_string()],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?))).map_err(storage_error)?;
    if foreign!=0||works>64||reserved+staged+retiring!=works||bytes>32*1024*1024*1024||files>4096{return Err(invalid());}
    Ok(omniplay_contracts::NativeWorkStorageStatus{schema_version:1,works,reserved_works:reserved,staged_works:staged,retiring_works:retiring,reserved_bytes:bytes,reserved_files:files,maximum_works:64,maximum_bytes:32*1024*1024*1024,maximum_files:4096,physical_bytes:None,recovery_running:false})
}
pub fn linux_recovery_cursor(database:&Database)->Result<Option<String>>{let value:Option<String>=database.connection.query_row("SELECT value FROM preferences WHERE key='linux_native_recovery_cursor_v1'",[],|row|row.get(0)).optional().map_err(storage_error)?;if value.as_ref().is_some_and(|value|value.len()>2048){return Err(invalid());}Ok(value)}
pub fn set_linux_recovery_cursor(database:&Database,value:&str)->Result<()>{if value.len()>2048{return Err(invalid());}database.connection.execute("INSERT INTO preferences(key,value) VALUES('linux_native_recovery_cursor_v1',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[value]).map_err(storage_error)?;Ok(())}
pub fn windows_recovery_cursor(database:&Database)->Result<Option<String>>{let value:Option<String>=database.connection.query_row("SELECT value FROM preferences WHERE key='windows_native_recovery_cursor_v1'",[],|row|row.get(0)).optional().map_err(storage_error)?;if let Some(value)=&value{id(value)?;}Ok(value)}
pub fn set_windows_recovery_cursor(database:&Database,value:Uuid)->Result<()>{if value.is_nil(){return Err(invalid());}database.connection.execute("INSERT INTO preferences(key,value) VALUES('windows_native_recovery_cursor_v1',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[value.to_string()]).map_err(storage_error)?;Ok(())}
fn invalid()->PlatformError{PlatformError::new(ErrorCode::StorageFailure,"Native work storage needs confirmed recovery before it can be reused")}
fn full()->PlatformError{PlatformError::new(ErrorCode::ResourceLimit,"Native session staging has reached its private storage quota").recover("Close earlier native sessions or finish recovering retained session work before retrying")}
fn linked(value:&fs::Metadata)->bool{let result=value.file_type().is_symlink();#[cfg(windows)]{use std::os::windows::fs::MetadataExt;return result||value.file_attributes()&0x400!=0;}#[cfg(not(windows))]{result}}
fn absent(path:&Path)->Result<bool>{match fs::symlink_metadata(path){Err(error)if error.kind()==std::io::ErrorKind::NotFound=>Ok(true),Err(error)=>Err(storage_error(error)),Ok(_)=>Ok(false)}}
fn id(raw:&str)->Result<Uuid>{let value=Uuid::parse_str(raw).map_err(|_|invalid())?;if value.is_nil()||value.to_string()!=raw{return Err(invalid());}Ok(value)}
fn known_tree(profile_root:&Path,known:&BTreeSet<String>,new:Uuid)->Result<()>{
    let root=profile_root.join("native-work");let shape=match fs::symlink_metadata(&root){Err(error)if error.kind()==std::io::ErrorKind::NotFound=>return Ok(()),Err(error)=>return Err(storage_error(error)),Ok(value)=>value};if !shape.is_dir()||linked(&shape){return Err(invalid());}
    let mut count=0;for entry in fs::read_dir(root).map_err(storage_error)?{count+=1;if count>256{return Err(full());}let entry=entry.map_err(storage_error)?;let name=entry.file_name().into_string().map_err(|_|invalid())?;let work=id(&name)?;let shape=fs::symlink_metadata(entry.path()).map_err(storage_error)?;if !shape.is_dir()||linked(&shape)||work!=new&&!known.contains(&name){return Err(invalid());}}Ok(())
}
pub fn reserve(database:&mut Database,profile:Uuid,work:Uuid,bytes:u64,files:u32)->Result<()>{reserve_with_protocol(database,profile,work,bytes,files,0)}
/// The Windows owner must hold its profile creation lease until owner.lease is
/// acquired. The protocol bit commits in the same transaction as the debt; it
/// is never inferred from an old directory name or added to a legacy row.
#[cfg(windows)]
pub fn reserve_windows(database:&mut Database,profile:Uuid,work:Uuid,bytes:u64,files:u32)->Result<()>{reserve_with_protocol(database,profile,work,bytes,files,1)}
fn reserve_with_protocol(database:&mut Database,profile:Uuid,work:Uuid,bytes:u64,files:u32,protocol:u32)->Result<()>{
    if profile.is_nil()||work.is_nil()||bytes==0||bytes>8*1024*1024*1024||!(1..=144).contains(&files){return Err(full());}
    let profile_root=database.root.clone();let tx=database.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage_error)?;
    let mut query=tx.prepare("SELECT work_id FROM native_work_storage LIMIT 65").map_err(storage_error)?;let known=query.query_map([],|row|row.get::<_,String>(0)).map_err(storage_error)?.collect::<std::result::Result<BTreeSet<_>,_>>().map_err(storage_error)?;drop(query);if known.len()>64{return Err(invalid());}for value in &known{id(value)?;}known_tree(&profile_root,&known,work)?;
    let(count,total,file_count):(u64,u64,u64)=tx.query_row("SELECT count(*),COALESCE(sum(reserved_bytes),0),COALESCE(sum(file_count),0) FROM native_work_storage",[],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).map_err(storage_error)?;
    if count>=64||total.checked_add(bytes).is_none_or(|value|value>32*1024*1024*1024)||file_count+u64::from(files)>4096{return Err(full());}tx.execute("INSERT INTO native_work_storage(work_id,profile_id,reserved_bytes,file_count,phase,manifest_sha256,created_at_ms,windows_creation_protocol) VALUES(?1,?2,?3,?4,'reserved',NULL,?5,?6)",params![work.to_string(),profile.to_string(),bytes,files,now_ms(),protocol]).map_err(storage_error)?;tx.commit().map_err(storage_error)
}
pub fn staged(database:&Database,profile:Uuid,work:Uuid,manifest:&ContentHash)->Result<()>{ContentHash::parse(manifest.0.clone())?;if database.connection.execute("UPDATE native_work_storage SET phase='staged',manifest_sha256=?3 WHERE work_id=?1 AND profile_id=?2 AND phase='reserved'",params![work.to_string(),profile.to_string(),manifest.0]).map_err(storage_error)?!=1{return Err(invalid());}Ok(())}
pub fn require_reserved(database:&Database,profile:Uuid,work:Uuid,minimum_bytes:u64,files:u32)->Result<()>{let admitted:bool=database.connection.query_row("SELECT EXISTS(SELECT 1 FROM native_work_storage WHERE work_id=?1 AND profile_id=?2 AND phase='reserved' AND reserved_bytes>=?3 AND file_count>=?4)",params![work.to_string(),profile.to_string(),minimum_bytes,files],|row|row.get(0)).map_err(storage_error)?;if !admitted{return Err(invalid());}Ok(())}
pub fn require_staged(database:&Database,profile:Uuid,work:Uuid,manifest:&ContentHash,minimum_bytes:u64,files:u32)->Result<()>{
    ContentHash::parse(manifest.0.clone())?;let value:Option<(u64,u32,String,String)>=database.connection.query_row("SELECT reserved_bytes,file_count,phase,COALESCE(manifest_sha256,'') FROM native_work_storage WHERE work_id=?1 AND profile_id=?2",params![work.to_string(),profile.to_string()],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional().map_err(storage_error)?;
    if !value.is_some_and(|(bytes,count,phase,hash)|bytes>=minimum_bytes&&count>=files&&phase=="staged"&&hash==manifest.0){return Err(invalid());}Ok(())
}
/// Only the process/work owner calls this after exact process/tree/profile
/// retirement. This transition itself never kills a process or deletes a file.
pub fn retiring(database:&Database,profile:Uuid,work:Uuid)->Result<()>{
    let existing:Option<String>=database.connection.query_row("SELECT profile_id FROM native_work_storage WHERE work_id=?1",[work.to_string()],|row|row.get(0)).optional().map_err(storage_error)?;if existing.as_deref().is_some_and(|value|value!=profile.to_string()){return Err(invalid());}database.connection.execute("UPDATE native_work_storage SET phase='retiring' WHERE work_id=?1 AND profile_id=?2",params![work.to_string(),profile.to_string()]).map_err(storage_error)?;Ok(())
}
pub fn require_retiring(database:&Database,profile:Uuid,work:Uuid)->Result<()>{
    let admitted:bool=database.connection.query_row("SELECT EXISTS(SELECT 1 FROM native_work_storage WHERE work_id=?1 AND profile_id=?2 AND phase='retiring')",params![work.to_string(),profile.to_string()],|row|row.get(0)).map_err(storage_error)?;if !admitted{return Err(invalid());}Ok(())
}
pub fn release_removed(database:&Database,profile:Uuid,work:Uuid)->Result<()>{
    if !absent(&database.root.join("native-work").join(work.to_string()))?{return Err(invalid());}let existing:Option<(String,String)>=database.connection.query_row("SELECT profile_id,phase FROM native_work_storage WHERE work_id=?1",[work.to_string()],|row|Ok((row.get(0)?,row.get(1)?))).optional().map_err(storage_error)?;
    if existing.is_some_and(|(owner,phase)|owner!=profile.to_string()||phase!="retiring"){return Err(invalid());}database.connection.execute("DELETE FROM native_work_storage WHERE work_id=?1 AND profile_id=?2 AND phase='retiring'",params![work.to_string(),profile.to_string()]).map_err(storage_error)?;Ok(())
}
/// A reserved row whose fresh UUID directory never existed cannot have reached
/// launch: all launch paths must first durably commit the staged transition.
pub fn release_uncreated(database:&Database,profile:Uuid,work:Uuid)->Result<()>{
    if !absent(&database.root.join("native-work").join(work.to_string()))?{return Err(invalid());}let changed=database.connection.execute("UPDATE native_work_storage SET phase='retiring' WHERE work_id=?1 AND profile_id=?2 AND phase='reserved'",params![work.to_string(),profile.to_string()]).map_err(storage_error)?;if changed!=1{return Err(invalid());}release_removed(database,profile,work)
}
pub fn collect_removed(database:&Database,limit:usize)->Result<usize>{
    let mut query=database.connection.prepare("SELECT profile_id,work_id FROM native_work_storage WHERE phase='retiring' ORDER BY created_at_ms,work_id LIMIT ?1").map_err(storage_error)?;let rows=query.query_map([limit.min(64)],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?))).map_err(storage_error)?.collect::<std::result::Result<Vec<_>,_>>().map_err(storage_error)?;drop(query);let mut released=0;for(profile,work)in rows{let(profile,work)=(id(&profile)?,id(&work)?);if absent(&database.root.join("native-work").join(work.to_string()))?{release_removed(database,profile,work)?;released+=1;}}Ok(released)
}

#[cfg(test)]
mod migration_tests {
    use rusqlite::Connection;

    #[test]
    fn schema_fourteen_debt_keeps_identity_and_legacy_protocol_after_upgrade(){
        let connection=Connection::open_in_memory().unwrap();
        connection.execute_batch(include_str!("../migrations/0014.sql")).unwrap();
        let work=uuid::Uuid::new_v4().to_string();let profile=uuid::Uuid::new_v4().to_string();
        connection.execute("INSERT INTO native_work_storage(work_id,profile_id,reserved_bytes,file_count,phase,manifest_sha256,created_at_ms) VALUES(?1,?2,4096,3,'reserved',NULL,123)",rusqlite::params![work,profile]).unwrap();
        connection.execute_batch(include_str!("../migrations/0015.sql")).unwrap();
        let observed:(String,String,u64,u32,String,u64,u32)=connection.query_row(
            "SELECT work_id,profile_id,reserved_bytes,file_count,phase,created_at_ms,windows_creation_protocol FROM native_work_storage",[],
            |row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?))).unwrap();
        assert_eq!(observed,(work.clone(),profile,4096,3,"reserved".into(),123,0));
        assert!(connection.execute("UPDATE native_work_storage SET windows_creation_protocol=1 WHERE work_id=?1",[work]).is_err());
        let version:u32=connection.query_row("PRAGMA user_version",[],|row|row.get(0)).unwrap();assert_eq!(version,15);
    }
}
