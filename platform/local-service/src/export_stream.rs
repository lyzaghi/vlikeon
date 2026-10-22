use base64::{Engine,engine::general_purpose::STANDARD};
use omniplay_contracts::{ContentHash,ErrorCode,PlatformError,Result,SaveMetadata};
use omniplay_core::{saves::{export_prefix,finalize_export},util::storage_error};
use sha2::{Digest,Sha256};
use std::{collections::HashMap,fs::File,io::{Write,Seek,SeekFrom},time::{Duration,Instant}};
use uuid::Uuid;

const CHUNK:usize=48*1024;
struct Export {file:File,grant:Uuid,metadata:SaveMetadata,offset:u64,hash:Sha256,expires:Instant,committed:bool}
impl Drop for Export{fn drop(&mut self){if !self.committed{let _=self.file.set_len(0);let _=self.file.sync_all();}}}
#[derive(Default)]
pub struct ExportStreams{active:HashMap<Uuid,Export>}
fn missing()->PlatformError{PlatformError::new(ErrorCode::PermissionExpired,"Export is unavailable or expired; choose a new destination")}
impl ExportStreams{
    pub fn start(&mut self,metadata:SaveMetadata,grant:Uuid,mut file:File)->Result<Uuid>{
        self.expire();metadata.validate()?;if !self.active.is_empty(){return Err(PlatformError::new(ErrorCode::Busy,"Finish the active live-save export first"));}
        let prefix=export_prefix(&metadata)?;file.seek(SeekFrom::Start(0)).map_err(storage_error)?;file.set_len(0).map_err(storage_error)?;
        let mut export=Export{file,grant,metadata,offset:0,hash:Sha256::new(),expires:Instant::now()+Duration::from_secs(120),committed:false};export.file.write_all(&prefix).map_err(storage_error)?;
        let id=Uuid::new_v4();self.active.insert(id,export);Ok(id)
    }
    pub fn chunk(&mut self,id:Uuid,offset:u64,encoded:&str)->Result<u64>{
        self.expire();
        let result=(||{
            let export=self.active.get_mut(&id).ok_or_else(missing)?;
            if encoded.is_empty()||encoded.len()>CHUNK/3*4{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Export chunk exceeds its bound"));}
            let bytes=STANDARD.decode(encoded).map_err(|_|PlatformError::new(ErrorCode::InvalidRequest,"Export bytes are not canonical base64"))?;
            if bytes.is_empty()||bytes.len()>CHUNK||STANDARD.encode(&bytes)!=encoded||offset!=export.offset||offset.saturating_add(bytes.len()as u64)>export.metadata.payload_bytes{return Err(PlatformError::new(ErrorCode::InvalidRequest,"Export chunk offset, size or encoding is invalid"));}
            export.file.write_all(&bytes).map_err(storage_error)?;export.hash.update(&bytes);export.offset+=bytes.len()as u64;export.expires=Instant::now()+Duration::from_secs(120);Ok(export.offset)
        })();
        if result.is_err(){self.active.remove(&id);}result
    }
    pub fn commit(&mut self,id:Uuid,grant:Uuid)->Result<()>{
        self.expire();let export=self.active.get(&id).ok_or_else(missing)?;if export.grant!=grant{return Err(PlatformError::new(ErrorCode::PermissionDenied,"Export destination does not match its original selection"));}
        let mut export=self.active.remove(&id).ok_or_else(missing)?;
        if export.offset!=export.metadata.payload_bytes||ContentHash(format!("{:x}",export.hash.clone().finalize()))!=export.metadata.payload_hash{return Err(PlatformError::new(ErrorCode::ContentMismatch,"Export is incomplete or failed its integrity check"));}
        finalize_export(&mut export.file)?;export.committed=true;Ok(())
    }
    pub fn abort(&mut self,id:Uuid){self.active.remove(&id);}
    pub fn expire(&mut self){let now=Instant::now();self.active.retain(|_,value|value.expires>now);}
    pub fn clear(&mut self){self.active.clear();}
}
