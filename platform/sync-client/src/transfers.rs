//! Two bounded encrypted transport slots, persisted independently of a job's
//! lifetime. No plaintext payload, root key, device seed or service token is staged.
use crate::{SyncClient,CredentialStore};
use omniplay_contracts::{ErrorCode,PlatformError,Result,SaveMetadata};
use omniplay_core::{Database,saves::SaveStore,util::{storage_error,digest}};
use omniplay_sync::{BlobHeader,UploadProgress,TRANSFER_PROTOCOL,TRANSFER_CHUNK_BYTES,MAX_CIPHERTEXT_BYTES,chunk_length,transfer_files as files};
use rusqlite::{OptionalExtension,params};
use serde::{Serialize,Deserialize,de::DeserializeOwned};
use sha2::{Digest,Sha256};
use std::{fs::File,io::{Read,Write,Seek,SeekFrom},path::PathBuf,sync::atomic::{AtomicBool,Ordering}};
use uuid::Uuid;

const UP:&str="sync_transfer_upload_v1";
const DOWN:&str="sync_transfer_download_v1";
pub(crate) fn profile_lease(db:&Database)->Result<File>{files::lease(&db.root,"sync-transport-v1.lock").map_err(|error|if error.kind()==std::io::ErrorKind::WouldBlock{PlatformError::new(ErrorCode::Busy,"Another synchronization operation is still finishing for this profile")}else{storage_error(error)})}
#[derive(Serialize,Deserialize)]#[serde(deny_unknown_fields)]
struct Upload{protocol:u32,device:Uuid,id:Uuid,header:BlobHeader,metadata_hash:String,#[serde(default)]canonical:bool}
#[derive(Serialize,Deserialize)]#[serde(deny_unknown_fields)]
struct Download{protocol:u32,device:Uuid,id:Uuid,header:BlobHeader,offset:u64,prefix_sha256:String}
// The owned private file carries ciphertext between transport, inbox and decode.
// No whole-object allocation is necessary just to hand off a completed download.
pub(crate) struct Downloaded{pub header:BlobHeader,pub file:File,id:Uuid}
struct Guard<'a>{cancel:&'a AtomicBool,store:&'a dyn CredentialStore}
impl Guard<'_>{fn check(&self,db:&Database)->Result<()>{if self.cancel.load(Ordering::Acquire)||!db.settings()?.cloud_sync_opt_in{return Err(PlatformError::new(ErrorCode::Cancelled,"Synchronization stopped; local progress remains available"));}self.store.check_access()}}
fn corrupt()->PlatformError{PlatformError::new(ErrorCode::CorruptData,"Encrypted transfer staging needs recovery; original local saves remain available")}
fn directory(db:&Database)->Result<PathBuf>{files::private_dir(&db.root,"sync-transfers-v1").map_err(storage_error)}
fn load<T:DeserializeOwned>(db:&Database,key:&str)->Result<Option<T>>{
    let json:Option<String>=db.connection.query_row("SELECT value FROM preferences WHERE key=?1",[key],|row|row.get(0)).optional().map_err(storage_error)?;
    json.map(|value|{if value.len()>4096{return Err(corrupt());}serde_json::from_str(&value).map_err(|_|corrupt())}).transpose()
}
fn save(db:&Database,key:&str,value:&impl Serialize)->Result<()>{
    let json=serde_json::to_string(value).map_err(storage_error)?;if json.len()>4096{return Err(corrupt());}
    db.connection.execute("INSERT INTO preferences(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,json]).map_err(storage_error)?;Ok(())
}
fn validate(header:&BlobHeader,protocol:u32,id:Uuid)->Result<()>{header.validate()?;if protocol!=TRANSFER_PROTOCOL||id.is_nil(){return Err(corrupt());}Ok(())}
fn metadata_hash(metadata:&SaveMetadata,canonical:bool)->Result<String>{
    if canonical{return Ok(omniplay_contracts::save_origin_hash(metadata)?.0);}
    let mut value=serde_json::to_value(metadata).map_err(storage_error)?;
    // Local import maps game IDs/order/conflict state and can omit a missing local
    // parent. Match authenticated content/runtime identity across those projections;
    // an existing remote object is acknowledged, never overwritten or reparented.
    if let Some(value)=value.as_object_mut(){for key in ["game_id","conflict","generation","parent_id"]{value.remove(key);}}
    Ok(digest(&serde_json::to_vec(&value).map_err(storage_error)?).0)
}
fn clear_slot(db:&Database,key:&str,id:Uuid,extension:&str)->Result<()>{
    db.connection.execute("DELETE FROM preferences WHERE key=?1",[key]).map_err(storage_error)?;
    let dir=directory(db)?;files::remove(&dir.join(format!("{id}.{extension}"))).map_err(storage_error)?;files::sync_directory(&dir).map_err(storage_error)
}
pub(crate) fn clear(db:&Database)->Result<()>{
    if let Some(up)=load::<Upload>(db,UP)?{validate(&up.header,up.protocol,up.id)?;clear_slot(db,UP,up.id,"upload")?;}
    if let Some(down)=load::<Download>(db,DOWN)?{validate(&down.header,down.protocol,down.id)?;clear_slot(db,DOWN,down.id,"download")?;}
    sweep(db)
}
fn sweep(db:&Database)->Result<()>{
    let up=load::<Upload>(db,UP)?;let down=load::<Download>(db,DOWN)?;
    if let Some(value)=&up{validate(&value.header,value.protocol,value.id)?;}
    if let Some(value)=&down{validate(&value.header,value.protocol,value.id)?;}
    let keep_up=up.map(|value|format!("{}.upload",value.id));let keep_down=down.map(|value|format!("{}.download",value.id));
    let dir=directory(db)?;let mut count=0;
    for entry in std::fs::read_dir(&dir).map_err(storage_error)?{
        count+=1;if count>16{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Encrypted transfer cleanup exceeds its bound"));}
        let entry=entry.map_err(storage_error)?;let name=entry.file_name().into_string().map_err(|_|corrupt())?;
        let uuid=name.strip_prefix(".pending-").or_else(||name.strip_suffix(".upload")).or_else(||name.strip_suffix(".download"));
        if !uuid.and_then(|value|Uuid::parse_str(value).ok().map(|id|(value,id))).is_some_and(|(value,id)|id.to_string()==value){return Err(corrupt());}
        let file=files::open(&entry.path(),false,MAX_CIPHERTEXT_BYTES as u64).map_err(storage_error)?;drop(file);
        if keep_up.as_deref()!=Some(name.as_str())&&keep_down.as_deref()!=Some(name.as_str()){files::remove(&entry.path()).map_err(storage_error)?;}
    }
    files::sync_directory(&dir).map_err(storage_error)
}
pub(crate) fn cleanup_acknowledged(db:&Database,account:&str)->Result<()>{
    if let Some(up)=load::<Upload>(db,UP)?{
        validate(&up.header,up.protocol,up.id)?;
        let obsolete:bool=db.connection.query_row("SELECT NOT EXISTS(SELECT 1 FROM saves WHERE id=?1) OR EXISTS(SELECT 1 FROM sync_receipts WHERE account_id=?2 AND save_id=?1)",params![up.header.binding.object_id.to_string(),account],|row|row.get(0)).map_err(storage_error)?;
        if up.header.binding.account_id!=account||obsolete{clear_slot(db,UP,up.id,"upload")?;}
    }
    if let Some(down)=load::<Download>(db,DOWN)?{
        validate(&down.header,down.protocol,down.id)?;
        if down.header.binding.account_id!=account||omniplay_core::save_maintenance::locally_removed(db,down.header.binding.object_id)?{clear_slot(db,DOWN,down.id,"download")?;}
    }
    sweep(db)
}
fn hash_prefix(file:&mut File,length:u64,guard:&Guard<'_>,db:&Database)->Result<Sha256>{
    hash_prefix_checked(file,length,guard,db,&||Ok(()))
}
fn hash_prefix_checked(file:&mut File,length:u64,guard:&Guard<'_>,db:&Database,check:&dyn Fn()->Result<()>)->Result<Sha256>{
    file.seek(SeekFrom::Start(0)).map_err(storage_error)?;let mut left=length;let mut hash=Sha256::new();let mut buffer=[0u8;64*1024];
    while left>0{guard.check(db)?;check()?;let size=left.min(buffer.len() as u64) as usize;file.read_exact(&mut buffer[..size]).map_err(storage_error)?;hash.update(&buffer[..size]);left-=size as u64;}Ok(hash)
}
pub(crate) fn pending_upload_id(db:&Database,account:&str)->Result<Option<Uuid>>{
    let up=load::<Upload>(db,UP)?;
    if let Some(value)=up{validate(&value.header,value.protocol,value.id)?;if value.header.binding.account_id==account{return Ok(Some(value.header.binding.object_id));}}Ok(None)
}
fn progress(value:&UploadProgress,up:&Upload)->Result<()>{
    if value.protocol!=TRANSFER_PROTOCOL||value.transfer_id!=up.id||value.header!=up.header||value.received_bytes>up.header.ciphertext_bytes||(!value.stored&&value.received_bytes<up.header.ciphertext_bytes&&value.received_bytes%u64::from(TRANSFER_CHUNK_BYTES)!=0)||(value.stored&&value.received_bytes!=up.header.ciphertext_bytes){return Err(corrupt());}Ok(())
}
pub(crate) fn upload(db:&mut Database,client:&SyncClient,metadata:&SaveMetadata,cancel:&AtomicBool,store:&dyn CredentialStore,on_transfer:&dyn Fn(&str,u64,u64))->Result<()>{
    let guard=Guard{cancel,store};guard.check(db)?;sweep(db)?;
    let (current,canonical)=omniplay_core::save_origins::for_sync(db,metadata.id)?;let fingerprint=metadata_hash(&current,canonical)?;
    if metadata_hash(metadata,canonical)?!=fingerprint{return Err(PlatformError::new(ErrorCode::Conflict,"Save origin changed before synchronization"));}
    let previous=load::<Upload>(db,UP)?;
    let up=if let Some(up)=previous.filter(|up|up.canonical==canonical&&up.device==client.device_id()&&up.header.binding.account_id==client.configuration.account_id&&up.header.binding.object_id==metadata.id&&up.metadata_hash==fingerprint){up}else{
        if let Some(old)=load::<Upload>(db,UP)?{if old.device==client.device_id()&&old.header.binding.account_id==client.configuration.account_id{guard.check(db)?;client.upload_abort(old.id)?;}clear_slot(db,UP,old.id,"upload")?;}
        guard.check(db)?;let (captured,mut payload,known)=SaveStore::new(db).open_for_sync(metadata.id)?;guard.check(db)?;
        if known!=canonical||metadata_hash(&captured,known)?!=fingerprint{return Err(PlatformError::new(ErrorCode::Conflict,"Save origin changed while preparing synchronization"));}
        let (header,bytes)=client.cipher.encrypt_blob_reader_checked(&captured,&mut payload,&||guard.check(db))?;drop(payload);
        let id=Uuid::new_v4();let dir=directory(db)?;let mut pending=files::Pending::create(&dir).map_err(storage_error)?;
        for part in bytes.chunks(64*1024){guard.check(db)?;pending.write_all(part).map_err(storage_error)?;}drop(bytes);guard.check(db)?;
        pending.publish(&dir.join(format!("{id}.upload"))).map_err(storage_error)?;
        let up=Upload{protocol:TRANSFER_PROTOCOL,device:client.device_id(),id,header,metadata_hash:fingerprint,canonical};save(db,UP,&up)?;up
    };
    validate(&up.header,up.protocol,up.id)?;guard.check(db)?;
    let mut status=match client.upload_begin(up.id,&up.header){
        Ok(status)=>status,
        Err(error) if error.code==ErrorCode::Conflict=>{
            let mut remote=match download(db,client,metadata.id,cancel,store,on_transfer){Err(e) if e.code==ErrorCode::NotFound=>return Err(error),other=>other?};
            guard.check(db)?;let (remote_metadata,payload)=client.cipher.decrypt_blob_reader_checked(&remote.header,&mut remote.file,&||guard.check(db))?;drop(payload);guard.check(db)?;
            if metadata_hash(&remote_metadata,up.canonical)?!=up.metadata_hash{return Err(PlatformError::new(ErrorCode::Conflict,"The remote immutable save contains different progress"));}
            SaveStore::new(db).attach_authenticated_origin(remote_metadata)?;
            finish_download(db,remote)?;return Ok(());
        },Err(error)=>return Err(error),
    };
    progress(&status,&up)?;on_transfer("upload",status.received_bytes,up.header.ciphertext_bytes);if status.stored{return Ok(());}
    let opened=files::open(&directory(db)?.join(format!("{}.upload",up.id)),false,up.header.ciphertext_bytes);
    let mut file=match opened{Ok(file)=>file,Err(error) if error.kind()==std::io::ErrorKind::NotFound=>{guard.check(db)?;client.upload_abort(up.id)?;clear_slot(db,UP,up.id,"upload")?;return Err(corrupt());},Err(error)=>return Err(storage_error(error))};
    if file.metadata().map_err(storage_error)?.len()!=up.header.ciphertext_bytes{drop(file);guard.check(db)?;client.upload_abort(up.id)?;clear_slot(db,UP,up.id,"upload")?;return Err(corrupt());}
    let hash=hash_prefix(&mut file,up.header.ciphertext_bytes,&guard,db)?;
    if format!("{:x}",hash.finalize())!=up.header.ciphertext_sha256{
        drop(file);guard.check(db)?;client.upload_abort(up.id)?;clear_slot(db,UP,up.id,"upload")?;return Err(corrupt());
    }
    while let Some(length)=chunk_length(up.header.ciphertext_bytes,status.received_bytes){
        guard.check(db)?;let offset=status.received_bytes;file.seek(SeekFrom::Start(offset)).map_err(storage_error)?;let mut bytes=vec![0;length as usize];file.read_exact(&mut bytes).map_err(storage_error)?;guard.check(db)?;
        let next=client.upload_chunk(up.id,offset,bytes)?;progress(&next,&up)?;
        if next.stored||next.received_bytes!=offset+u64::from(length){return Err(corrupt());}status=next;on_transfer("upload",status.received_bytes,up.header.ciphertext_bytes);
    }
    drop(file);guard.check(db)?;let committed=client.upload_commit(up.id,&up.header)?;progress(&committed,&up)?;if !committed.stored{return Err(corrupt());}Ok(())
}
pub(crate) fn finish_upload(db:&Database,save_id:Uuid)->Result<()>{if let Some(up)=load::<Upload>(db,UP)?{if up.header.binding.object_id==save_id{clear_slot(db,UP,up.id,"upload")?;}}Ok(())}
pub(crate) fn download(db:&Database,client:&SyncClient,object_id:Uuid,cancel:&AtomicBool,store:&dyn CredentialStore,on_transfer:&dyn Fn(&str,u64,u64))->Result<Downloaded>{
    download_checked(db,client,object_id,cancel,store,on_transfer,&||Ok(()))
}
pub(crate) fn download_checked(db:&Database,client:&SyncClient,object_id:Uuid,cancel:&AtomicBool,store:&dyn CredentialStore,on_transfer:&dyn Fn(&str,u64,u64),check:&dyn Fn()->Result<()>)->Result<Downloaded>{
    check()?;let guard=Guard{cancel,store};guard.check(db)?;sweep(db)?;let header=client.object_header(object_id)?;guard.check(db)?;check()?;
    let previous=load::<Download>(db,DOWN)?;
    let mut state=if let Some(value)=previous.filter(|value|value.device==client.device_id()&&value.header==header){value}else{
        if let Some(old)=load::<Download>(db,DOWN)?{clear_slot(db,DOWN,old.id,"download")?;}
        let id=Uuid::new_v4();let dir=directory(db)?;files::Pending::create(&dir).map_err(storage_error)?.publish(&dir.join(format!("{id}.download"))).map_err(storage_error)?;
        let state=Download{protocol:TRANSFER_PROTOCOL,device:client.device_id(),id,header:header.clone(),offset:0,prefix_sha256:digest(&[]).0};save(db,DOWN,&state)?;state
    };
    validate(&state.header,state.protocol,state.id)?;
    if state.offset>header.ciphertext_bytes||(state.offset<header.ciphertext_bytes&&state.offset%u64::from(TRANSFER_CHUNK_BYTES)!=0){return Err(corrupt());}
    let path=directory(db)?.join(format!("{}.download",state.id));
    let mut file=match files::open(&path,true,header.ciphertext_bytes){Ok(file)=>file,Err(error) if error.kind()==std::io::ErrorKind::NotFound=>{clear_slot(db,DOWN,state.id,"download")?;return Err(corrupt());},Err(error)=>return Err(storage_error(error))};
    if file.metadata().map_err(storage_error)?.len()<state.offset{drop(file);clear_slot(db,DOWN,state.id,"download")?;return Err(corrupt());}
    file.set_len(state.offset).map_err(storage_error)?;file.sync_all().map_err(storage_error)?;
    check()?;let mut hash=hash_prefix_checked(&mut file,state.offset,&guard,db,check)?;check()?;
    if format!("{:x}",hash.clone().finalize())!=state.prefix_sha256{drop(file);clear_slot(db,DOWN,state.id,"download")?;return Err(corrupt());}
    on_transfer("download",state.offset,header.ciphertext_bytes);
    while let Some(length)=chunk_length(header.ciphertext_bytes,state.offset){
        guard.check(db)?;check()?;let bytes=client.object_chunk(&header,state.offset,length)?;guard.check(db)?;check()?;
        file.write_all(&bytes).map_err(storage_error)?;file.sync_all().map_err(storage_error)?;hash.update(&bytes);state.offset+=u64::from(length);state.prefix_sha256=format!("{:x}",hash.clone().finalize());save(db,DOWN,&state)?;on_transfer("download",state.offset,header.ciphertext_bytes);
    }
    if state.prefix_sha256!=header.ciphertext_sha256{drop(file);clear_slot(db,DOWN,state.id,"download")?;return Err(corrupt());}
    guard.check(db)?;check()?;file.seek(SeekFrom::Start(0)).map_err(storage_error)?;
    if file.metadata().map_err(storage_error)?.len()!=header.ciphertext_bytes{return Err(corrupt());}Ok(Downloaded{header,file,id:state.id})
}
pub(crate) fn finish_download(db:&Database,value:Downloaded)->Result<()>{let id=value.id;drop(value);if load::<Download>(db,DOWN)?.is_some_and(|state|state.id==id){clear_slot(db,DOWN,id,"download")?;}Ok(())}
pub(crate) fn discard_download(db:&Database,object_id:Uuid)->Result<()>{if let Some(state)=load::<Download>(db,DOWN)?{if state.header.binding.object_id==object_id{clear_slot(db,DOWN,state.id,"download")?;}}Ok(())}
pub(crate) fn has_recovery_download(db:&Database,account:&str,object_id:Uuid)->Result<bool>{Ok(load::<Download>(db,DOWN)?.is_some_and(|value|value.header.binding.account_id==account&&value.header.binding.object_id==object_id))}
pub(crate) fn recovery_download_id(db:&Database,account:&str)->Result<Option<Uuid>>{Ok(load::<Download>(db,DOWN)?.filter(|value|value.header.binding.account_id==account).map(|value|value.header.binding.object_id))}
