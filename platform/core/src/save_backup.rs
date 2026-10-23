//! Bounded portable multi-save archives. No paths, executable data or automatic
//! head selection enter the archive. Restore is explicitly per-entry durable.
use crate::{Database,save_origins,saves::SaveStore,util::{digest,storage_error}};
use omniplay_contracts::{ContentHash,ErrorCode,PlatformError,Result,SaveMetadata,SaveOrigin,SaveOriginKind,save_origin_hash};
use rusqlite::OptionalExtension;
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
use std::{collections::BTreeSet,fs::File,io::{Read,Seek,SeekFrom,Write},time::{Duration,Instant}};
use uuid::Uuid;
const MAGIC:&[u8;8]=b"OPBK0001";
const MAX_BYTES:usize=128*1024*1024;
const MAX_JSON:usize=4*1024*1024;
const MAX_ENTRIES:usize=64;
#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchivedReference{pub metadata:SaveMetadata,pub origin:SaveOrigin,pub current:bool}
#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry{pub metadata:SaveMetadata,pub origin:SaveOrigin,pub current:bool,#[serde(default)]pub prior:Vec<ArchivedReference>}
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest{schema_version:u32,entries:Vec<Entry>}
#[derive(Serialize)]
pub struct Exported{pub exported:bool,pub delivery:&'static str,pub entries:usize,pub bytes:u64,pub archive_hash:ContentHash}
#[derive(Serialize)]
pub struct Outcome{pub source_id:Uuid,pub restored_id:Option<Uuid>,pub already_present:bool,pub error:Option<ErrorCode>}
#[derive(Serialize)]
pub struct Restored{pub archive_hash:ContentHash,pub entries:Vec<Outcome>}
fn corrupt()->PlatformError{PlatformError::new(ErrorCode::CorruptData,"The backup archive is incomplete or inconsistent")}
fn limit()->PlatformError{PlatformError::new(ErrorCode::ResourceLimit,"A backup group allows at most 64 saves and 128 MiB; export smaller groups for larger history")}
fn deadline(until:Instant)->Result<()>{if Instant::now()>=until{Err(PlatformError::new(ErrorCode::DeadlineExceeded,"The backup operation reached its time budget"))}else{Ok(())}}
fn same_payload(a:&SaveMetadata,b:&SaveMetadata)->bool{a.game_content_hash==b.game_content_hash&&a.kind==b.kind&&a.slot==b.slot&&a.core_id==b.core_id&&a.core_version==b.core_version&&a.state_format_version==b.state_format_version&&a.platform==b.platform&&a.core_config_hash==b.core_config_hash&&a.payload_hash==b.payload_hash&&a.payload_bytes==b.payload_bytes}
fn validate(entry:&Entry)->Result<()>{
    if entry.prior.len()>16{return Err(limit());}
    validate_origin(&entry.metadata,&entry.origin)?;for previous in &entry.prior{validate_origin(&previous.metadata,&previous.origin)?;}Ok(())
}
fn validate_origin(metadata:&SaveMetadata,origin:&SaveOrigin)->Result<()>{
    metadata.validate()?;
    if origin.schema_version!=1||origin.save_id!=metadata.id{return Err(corrupt());}
    if origin.kind==SaveOriginKind::LegacyProjection{
        if origin.canonical_metadata.is_some()||origin.canonical_hash.is_some()||origin.imported_from.is_some()||origin.imported_from_hash.is_some()||origin.missing_parent{return Err(corrupt());}
    }else{
        let canonical=origin.canonical_metadata.as_ref().ok_or_else(corrupt)?;canonical.validate()?;
        if origin.missing_parent&&canonical.parent_id.is_none(){return Err(corrupt());}
        if canonical.id!=metadata.id||!same_payload(canonical,metadata)||canonical.created_at_ms!=metadata.created_at_ms||canonical.device_id!=metadata.device_id||canonical.label!=metadata.label||origin.canonical_hash.as_ref()!=Some(&save_origin_hash(canonical)?){return Err(corrupt());}
    }
    if (origin.kind==SaveOriginKind::Imported)!=origin.imported_from.is_some()||origin.imported_from.is_some()!=origin.imported_from_hash.is_some(){return Err(corrupt());}
    if let Some(previous)=&origin.imported_from{previous.validate()?;if previous.id==metadata.id||!same_payload(previous,metadata)||origin.imported_from_hash.as_ref()!=Some(&save_origin_hash(previous)?){return Err(corrupt());}}
    Ok(())
}
fn inspect(database:&Database,ids:&[Uuid])->Result<Vec<Entry>>{
    let tx=database.connection.unchecked_transaction().map_err(storage_error)?;let mut entries=Vec::new();
    for id in ids{let(metadata,origin)=save_origins::inspect(&tx,*id)?;let current=tx.query_row("SELECT EXISTS(SELECT 1 FROM save_heads WHERE save_id=?1)",[id.to_string()],|row|row.get(0)).map_err(storage_error)?;let reference:Option<String>=tx.query_row("SELECT value FROM preferences WHERE key=?1",[format!("backup_reference.{id}")],|row|row.get(0)).optional().map_err(storage_error)?;let mut prior=Vec::new();if let Some(reference)=reference{if reference.len()>256*1024{return Err(limit());}let previous:Entry=serde_json::from_str(&reference).map_err(|_|corrupt())?;validate(&previous)?;prior.push(ArchivedReference{metadata:previous.metadata,origin:previous.origin,current:previous.current});prior.extend(previous.prior);}let entry=Entry{metadata,origin,current,prior};validate(&entry)?;entries.push(entry);}
    tx.commit().map_err(storage_error)?;Ok(entries)
}
fn archive_hash(bytes:&[u8])->ContentHash{let mut hash=Sha256::new();hash.update(b"Vlikeon portable saves archive v1\0");hash.update(bytes);ContentHash(format!("{:x}",hash.finalize()))}
pub fn export(database:&mut Database,ids:&[Uuid],destination:&mut File)->Result<Exported>{
    if ids.is_empty()||ids.len()>MAX_ENTRIES||ids.iter().collect::<BTreeSet<_>>().len()!=ids.len(){return Err(limit());}
    let until=Instant::now()+Duration::from_secs(90);let entries=inspect(database,ids)?;
    let json=serde_json::to_vec(&Manifest{schema_version:1,entries:entries.clone()}).map_err(storage_error)?;
    if json.len()>MAX_JSON{return Err(limit());}
    let total=entries.iter().try_fold(12usize+json.len()+32,|total,entry|total.checked_add(usize::try_from(entry.metadata.payload_bytes).map_err(|_|limit())?).filter(|n|*n<=MAX_BYTES).ok_or_else(limit))?;
    let mut bytes=Vec::with_capacity(total);bytes.extend_from_slice(MAGIC);bytes.extend_from_slice(&(json.len()as u32).to_le_bytes());bytes.extend_from_slice(&json);
    for entry in &entries{deadline(until)?;let mut file=crate::save_payloads::open(database,&entry.metadata)?;let start=bytes.len();let mut buffer=[0u8;65536];loop{deadline(until)?;let count=file.read(&mut buffer).map_err(storage_error)?;if count==0{break;}if bytes.len().checked_add(count).is_none_or(|n|n>total-32){return Err(corrupt());}bytes.extend_from_slice(&buffer[..count]);}if bytes.len()-start!=entry.metadata.payload_bytes as usize||digest(&bytes[start..])!=entry.metadata.payload_hash{return Err(corrupt());}}
    // Every selected metadata/origin/head must still match the reviewed capture.
    if serde_json::to_vec(&inspect(database,ids)?).map_err(storage_error)?!=serde_json::to_vec(&entries).map_err(storage_error)?{return Err(PlatformError::new(ErrorCode::Conflict,"Saved history changed during backup; retry the group"));}
    let hash=archive_hash(&bytes);let raw=hex_bytes(&hash)?;bytes.extend_from_slice(&raw);if bytes.len()!=total{return Err(corrupt());}
    destination.seek(SeekFrom::Start(0)).map_err(storage_error)?;destination.set_len(0).map_err(storage_error)?;destination.write_all(&[0;8]).map_err(storage_error)?;
    for chunk in bytes[8..].chunks(65536){deadline(until)?;destination.write_all(chunk).map_err(storage_error)?;}
    destination.sync_all().map_err(storage_error)?;deadline(until)?;destination.seek(SeekFrom::Start(0)).map_err(storage_error)?;destination.write_all(MAGIC).map_err(storage_error)?;destination.sync_all().map_err(storage_error)?;
    Ok(Exported{exported:true,delivery:"file_handle",entries:entries.len(),bytes:bytes.len()as u64,archive_hash:hash})
}
fn hex_bytes(hash:&ContentHash)->Result<Vec<u8>>{ContentHash::parse(hash.0.clone())?;(0..64).step_by(2).map(|i|u8::from_str_radix(&hash.0[i..i+2],16).map_err(|_|corrupt())).collect()}
fn restored_id(hash:&ContentHash,id:Uuid,attempt:u32)->Uuid{let mut value=Sha256::new();value.update(b"Vlikeon backup imported branch v1\0");value.update(hash.0.as_bytes());value.update(id.as_bytes());value.update(attempt.to_le_bytes());let bytes=value.finalize();let mut id=[0u8;16];id.copy_from_slice(&bytes[..16]);id[6]=(id[6]&15)|0x40;id[8]=(id[8]&63)|0x80;Uuid::from_bytes(id)}
fn available_id(database:&Database,hash:&ContentHash,source:Uuid)->Result<Uuid>{
    // A new explicit import may restore a previously purged branch under a new
    // identity. Never erase a suppression marker or reuse an intentionally
    // removed UUID. Retries choose the same first nonremoved derived identity.
    for attempt in 0..128{let id=restored_id(hash,source,attempt);let removed:bool=database.connection.query_row("SELECT EXISTS(SELECT 1 FROM locally_removed_saves WHERE save_id=?1)",[id.to_string()],|row|row.get(0)).map_err(storage_error)?;if !removed{return Ok(id);}}
    Err(PlatformError::new(ErrorCode::ResourceLimit,"This archive has reached its explicit reimport history limit"))
}
pub fn restore(database:&mut Database,source:&mut impl Read)->Result<Restored>{
    let until=Instant::now()+Duration::from_secs(90);let mut bytes=Vec::new();let mut buffer=[0u8;65536];loop{deadline(until)?;let n=source.read(&mut buffer).map_err(storage_error)?;if n==0{break;}if bytes.len()+n>MAX_BYTES{return Err(limit());}bytes.extend_from_slice(&buffer[..n]);}
    if bytes.len()<44||&bytes[..8]!=MAGIC{return Err(corrupt());}let size=u32::from_le_bytes(bytes[8..12].try_into().map_err(|_|corrupt())?)as usize;
    if size==0||size>MAX_JSON||12+size>bytes.len()-32{return Err(corrupt());}
    let hash=archive_hash(&bytes[..bytes.len()-32]);if hex_bytes(&hash)?.as_slice()!=&bytes[bytes.len()-32..]{return Err(corrupt());}
    let manifest:Manifest=serde_json::from_slice(&bytes[12..12+size]).map_err(|_|corrupt())?;
    if manifest.schema_version!=1||manifest.entries.is_empty()||manifest.entries.len()>MAX_ENTRIES{return Err(corrupt());}
    let mut seen=BTreeSet::new();let mut offsets=Vec::new();let mut offset=12+size;
    for entry in &manifest.entries{deadline(until)?;validate(entry)?;if !seen.insert(entry.metadata.id){return Err(corrupt());}let end=offset.checked_add(usize::try_from(entry.metadata.payload_bytes).map_err(|_|corrupt())?).filter(|end|*end<=bytes.len()-32).ok_or_else(corrupt)?;if digest(&bytes[offset..end])!=entry.metadata.payload_hash{return Err(corrupt());}offsets.push((offset,end));offset=end;}
    if offset!=bytes.len()-32{return Err(corrupt());}
    // Complete structural and actual-byte validation precedes the first write.
    // Each generation commits separately; return every outcome, never imply a
    // whole-archive atomic transaction or rollback of earlier successes.
    let mut outcomes=Vec::new();
    for(entry,(start,end))in manifest.entries.iter().zip(offsets){
        let selected=available_id(database,&hash,entry.metadata.id);let id=match selected{Ok(id)=>id,Err(error)=>{outcomes.push(Outcome{source_id:entry.metadata.id,restored_id:None,already_present:false,error:Some(error.code)});continue;}};let result=(||->Result<bool>{
            deadline(until)?;let original=entry.origin.canonical_metadata.as_ref().unwrap_or(&entry.metadata);if id==original.id{return Err(corrupt());}
            let reference=serde_json::to_string(entry).map_err(storage_error)?;
            let existing:Option<String>=database.connection.query_row("SELECT value FROM preferences WHERE key=?1",[format!("backup_reference.{id}")],|row|row.get(0)).optional().map_err(storage_error)?;
            if let Some(existing)=existing{if existing!=reference{return Err(corrupt());}let local=save_origins::local_metadata(&database.connection,id)?;if !same_payload(&local,original){return Err(corrupt());}drop(crate::save_payloads::open(database,&local)?);return Ok(true);}
            let game=crate::library::Library::new(database).by_hash(&original.game_content_hash)?.ok_or_else(||PlatformError::new(ErrorCode::NotFound,"Import the matching game content before restoring this save"))?;
            SaveStore::new(database).restore_backup(original.clone(),&bytes[start..end],game.id,id,&reference)?;Ok(false)
        })();
        outcomes.push(match result{Ok(already_present)=>Outcome{source_id:entry.metadata.id,restored_id:Some(id),already_present,error:None},Err(error)=>Outcome{source_id:entry.metadata.id,restored_id:None,already_present:false,error:Some(error.code)}});
    }
    Ok(Restored{archive_hash:hash,entries:outcomes})
}
