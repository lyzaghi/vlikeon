//! Explicit reimport of retained content and interrupted raw-import snapshots.
//! Unknown names remain untouched. Recovery copies; it never moves or erases.
use crate::{Database,asset_storage::AssetLease,util::{hash_file,storage_error}};
use omniplay_contracts::{ContentHash,ErrorCode,GameManifest,GameSystem,PlatformError,Result};
use serde::Serialize;
use sha2::{Digest,Sha256};
use std::{fs::{self,File,OpenOptions},path::Path,time::{Duration,Instant,UNIX_EPOCH}};
use uuid::Uuid;
#[derive(Serialize)]pub struct Candidate{pub id:String,pub revision:ContentHash,pub bytes:u64,pub interrupted_import:bool}
#[derive(Serialize)]pub struct Page{pub candidates:Vec<Candidate>,pub next_after:Option<String>,pub retained_unknown:usize}
fn invalid()->PlatformError{PlatformError::new(ErrorCode::CorruptData,"Retained content needs manual recovery")}
fn name(id:&str)->Result<String>{
    if let Some(hash)=id.strip_prefix("content:"){ContentHash::parse(hash.to_owned())?;return Ok(hash.to_owned());}
    if let Some(raw)=id.strip_prefix("pending:"){let id=Uuid::parse_str(raw).map_err(|_|invalid())?;if id.to_string()==raw{return Ok(format!(".pending-{id}"));}}
    Err(invalid())
}
fn candidate(id:String,path:&Path)->Result<Candidate>{
    let metadata=fs::symlink_metadata(path).map_err(storage_error)?;
    #[cfg(windows)]{use std::os::windows::fs::MetadataExt;if metadata.file_attributes()&0x400!=0{return Err(invalid());}}
    if metadata.file_type().is_symlink()||!metadata.is_file()||metadata.len()==0||metadata.len()>omniplay_contracts::MAX_ROM_BYTES{return Err(invalid());}
    let modified=metadata.modified().map_err(storage_error)?.duration_since(UNIX_EPOCH).map_err(storage_error)?.as_nanos();
    let mut hash=Sha256::new();hash.update(b"Vlikeon retained raw content v1\0");hash.update(id.as_bytes());hash.update(metadata.len().to_le_bytes());hash.update(modified.to_le_bytes());
    Ok(Candidate{interrupted_import:id.starts_with("pending:"),id,revision:ContentHash(format!("{:x}",hash.finalize())),bytes:metadata.len()})
}
pub fn list(database:&Database,after:Option<&str>)->Result<Page>{
    if let Some(after)=after{name(after)?;}
    let _lease=AssetLease::acquire(database)?;let directory=database.root.join("assets");
    if !directory.exists(){return Ok(Page{candidates:Vec::new(),next_after:None,retained_unknown:0});}
    let until=Instant::now()+Duration::from_secs(15);let mut values=Vec::new();let mut unknown=0;
    for(index,entry)in fs::read_dir(&directory).map_err(storage_error)?.enumerate(){
        if index>=20000||Instant::now()>=until{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Retained-content inventory exceeds this recovery pass"));}
        let entry=entry.map_err(storage_error)?;let raw=entry.file_name();let Some(raw)=raw.to_str()else{unknown+=1;continue;};
        let id=if ContentHash::parse(raw.to_owned()).is_ok(){format!("content:{raw}")}else if let Some(raw)=raw.strip_prefix(".pending-"){let id=format!("pending:{raw}");if name(&id).is_err(){unknown+=1;continue;}id}else{unknown+=1;continue;};
        // Recovery inventories ordinary candidates only; uncertainty is retained.
        match candidate(id,&entry.path()){Ok(value)=>values.push(value),Err(_)=>unknown+=1}
    }
    values.sort_by(|a,b|a.id.cmp(&b.id));values.retain(|value|after.is_none_or(|after|value.id.as_str()>after));let more=values.len()>64;values.truncate(64);let next_after=if more{values.last().map(|value|value.id.clone())}else{None};Ok(Page{candidates:values,next_after,retained_unknown:unknown})
}
pub fn recover(database:&Database,id:&str,revision:&ContentHash,title:String,system:GameSystem)->Result<GameManifest>{
    let raw=name(id)?;let lease=AssetLease::acquire(database)?;let path=database.root.join("assets").join(raw);let reviewed=candidate(id.to_owned(),&path)?;
    if &reviewed.revision!=revision{return Err(PlatformError::new(ErrorCode::Conflict,"Retained content changed; review it again"));}
    let mut options=OpenOptions::new();options.read(true);
    #[cfg(unix)]{use std::os::unix::fs::OpenOptionsExt;options.custom_flags(libc::O_NOFOLLOW|libc::O_CLOEXEC);}
    #[cfg(windows)]{use std::os::windows::fs::OpenOptionsExt;options.custom_flags(0x00200000).share_mode(1);}
    let mut file:File=options.open(path).map_err(storage_error)?;if file.metadata().map_err(storage_error)?.len()!=reviewed.bytes{return Err(invalid());}
    if let Some(expected)=id.strip_prefix("content:"){let(hash,size)=hash_file(&mut file,omniplay_contracts::MAX_ROM_BYTES)?;if hash.0!=expected||size!=reviewed.bytes{return Err(invalid());}use std::io::{Seek,SeekFrom};file.seek(SeekFrom::Start(0)).map_err(storage_error)?;}
    // The owned descriptor survives a later collector unlink. Release before
    // normal import reacquires its own lease and performs copy/publication.
    drop(lease);crate::library::Library::new(database).import(file,title,system)
}
