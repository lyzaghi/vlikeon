//! Adopt only actual-byte matches for an existing save's trusted local metadata.
//! Unknown payloads never supply metadata, lineage, provenance or deletion rights.
use crate::{Database,save_payloads,save_origins,util::{digest,storage_error,hash_file}};
use omniplay_contracts::{ContentHash,ErrorCode,PlatformError,Result,SaveMetadata,SaveRepairResult,save_origin_hash};
use rusqlite::{Transaction,TransactionBehavior};
use serde::Serialize;
use std::{collections::BTreeMap,fs,io::{Read,Seek,SeekFrom},path::PathBuf,time::{Duration,Instant,UNIX_EPOCH}};
use uuid::Uuid;
#[derive(Serialize)]pub struct Candidate{pub candidate_id:String,pub revision:ContentHash}
#[derive(Serialize)]pub struct Page{pub matches:Vec<Candidate>,pub scanned:usize,pub next_after:Option<String>}
fn invalid()->PlatformError{PlatformError::new(ErrorCode::CorruptData,"Orphan payload cannot be safely matched to saved progress")}
fn bounded()->PlatformError{PlatformError::new(ErrorCode::ResourceLimit,"This orphan-recovery group exceeds its byte, entry or time bound")}
fn token(raw:&str)->Result<(&str,Uuid)>{let(kind,id)=raw.split_once(':').ok_or_else(invalid)?;let parsed=Uuid::parse_str(id).map_err(|_|invalid())?;if !matches!(kind,"legacy"|"repair")||parsed.to_string()!=id{return Err(invalid());}Ok((kind,parsed))}
fn path(database:&Database,metadata:&SaveMetadata,id:&str)->Result<PathBuf>{let(kind,id)=token(id)?;let base=if kind=="repair"{database.root.join("repaired-saves-v1")}else{database.root.join("saves").join(metadata.game_id.to_string())};if kind=="legacy"{save_payloads::directory(&database.root.join("saves"))?;}save_payloads::directory(&base)?;Ok(base.join(format!("{id}.bin")))}
fn unknown(database:&Database,id:&str)->Result<bool>{let(kind,id)=token(id)?;let owned:bool=if kind=="repair"{database.connection.query_row("SELECT EXISTS(SELECT 1 FROM save_repair_files WHERE file_id=?1)",[id.to_string()],|row|row.get(0))}else{database.connection.query_row("SELECT EXISTS(SELECT 1 FROM saves WHERE id=?1) OR EXISTS(SELECT 1 FROM save_payload_gc WHERE save_id=?1) OR EXISTS(SELECT 1 FROM locally_removed_saves WHERE save_id=?1)",[id.to_string()],|row|row.get(0))}.map_err(storage_error)?;Ok(!owned)}
fn observe(database:&Database,metadata:&SaveMetadata,id:&str)->Result<(Candidate,std::fs::File)>{
    if !unknown(database,id)?{return Err(invalid());}let mut file=save_payloads::open_file(&path(database,metadata,id)?)?;let info=file.metadata().map_err(storage_error)?;
    if info.len()!=metadata.payload_bytes{return Err(invalid());}let(hash,size)=hash_file(&mut file,128*1024*1024)?;if hash!=metadata.payload_hash||size!=metadata.payload_bytes{return Err(invalid());}
    let modified=info.modified().map_err(storage_error)?.duration_since(UNIX_EPOCH).map_err(storage_error)?.as_nanos();let revision=digest(format!("Vlikeon orphan payload v1\n{}\n{id}\n{size}\n{modified}",save_origin_hash(metadata)?.0).as_bytes());file.seek(SeekFrom::Start(0)).map_err(storage_error)?;Ok((Candidate{candidate_id:id.to_owned(),revision},file))
}
pub fn preview(database:&Database,save_id:Uuid,after:Option<&str>)->Result<Page>{
    if let Some(after)=after{token(after)?;}let _lease=save_payloads::lease(&database.root)?;let tx=Transaction::new_unchecked(&database.connection,TransactionBehavior::Immediate).map_err(storage_error)?;let metadata=save_origins::local_metadata(&tx,save_id)?;
    if metadata.payload_bytes>128*1024*1024{return Err(bounded());}
    let until=Instant::now()+Duration::from_secs(30);let mut names=BTreeMap::new();let mut inventory=0usize;
    for(kind,directory)in[("legacy",database.root.join("saves").join(metadata.game_id.to_string())),("repair",database.root.join("repaired-saves-v1"))]{
        match fs::symlink_metadata(&directory){Err(e)if e.kind()==std::io::ErrorKind::NotFound=>continue,Err(e)=>return Err(storage_error(e)),Ok(_)=>{}}
        if kind=="legacy"{save_payloads::directory(&database.root.join("saves"))?;}save_payloads::directory(&directory)?;
        for entry in fs::read_dir(directory).map_err(storage_error)?{inventory+=1;if inventory>20000||Instant::now()>=until{return Err(bounded());}let entry=entry.map_err(storage_error)?;let raw=entry.file_name();let Some(raw)=raw.to_str().and_then(|raw|raw.strip_suffix(".bin"))else{continue;};let id=format!("{kind}:{raw}");if token(&id).is_ok()&&after.is_none_or(|after|id.as_str()>after){names.insert(id,entry.path());}}
    }
    let mut matches=Vec::new();let mut scanned=0;let mut bytes=0u64;let mut next=None;let mut last_processed=after.map(str::to_owned);let mut iter=names.into_iter().peekable();
    while let Some((id,path))=iter.next(){
        if Instant::now()>=until{return Err(bounded());}next=Some(id.clone());scanned+=1;
        if unknown(database,&id)?{if let Ok(info)=fs::symlink_metadata(path){if info.is_file()&&!info.file_type().is_symlink()&&info.len()==metadata.payload_bytes{if bytes+metadata.payload_bytes>128*1024*1024{next=last_processed;scanned-=1;break;}bytes+=metadata.payload_bytes;if let Ok((candidate,file))=observe(database,&metadata,&id){drop(file);matches.push(candidate);}}}}
        last_processed=Some(id);
        if scanned==64{if iter.peek().is_none(){next=None;}break;}if iter.peek().is_none(){next=None;}
    }
    tx.commit().map_err(storage_error)?;Ok(Page{matches,scanned,next_after:next})
}
pub fn recover(database:&mut Database,save_id:Uuid,id:&str,expected:&ContentHash)->Result<SaveRepairResult>{
    let lease=save_payloads::lease(&database.root)?;let(metadata,mut file)={let tx=Transaction::new_unchecked(&database.connection,TransactionBehavior::Immediate).map_err(storage_error)?;let metadata=save_origins::local_metadata(&tx,save_id)?;if metadata.payload_bytes>128*1024*1024{return Err(bounded());}let(candidate,file)=observe(database,&metadata,id)?;if &candidate.revision!=expected{return Err(PlatformError::new(ErrorCode::Conflict,"The orphan payload changed; search again"));}tx.commit().map_err(storage_error)?;(metadata,file)};
    let mut payload=Vec::new();file.take(metadata.payload_bytes+1).read_to_end(&mut payload).map_err(storage_error)?;if payload.len()as u64!=metadata.payload_bytes||digest(&payload)!=metadata.payload_hash{return Err(invalid());}drop(lease);
    // Existing recovery rechecks local metadata/session/location under its own
    // lease, journals a fresh copy and never overwrites the unknown source.
    save_payloads::repair(database,&metadata,&payload,None)
}
