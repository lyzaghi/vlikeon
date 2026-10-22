use base64::{Engine,engine::general_purpose::STANDARD};
use omniplay_contracts::{ContentHash,ErrorCode,PlatformError,Result,SaveMetadata};
use omniplay_core::util::{PendingFile,bounded_read,storage_error};
use serde::Serialize;
use sha2::{Digest,Sha256};
use std::{collections::HashMap,fs::File,io::{Read,Seek,SeekFrom,Write},path::Path};
use uuid::Uuid;

const CHUNK_BYTES:u32=48*1024;
const TRANSFER_TTL_MS:u64=120_000;

struct Download { file:File,expected_hash:ContentHash,length:u64,offset:u64,hasher:Sha256,expires_at_ms:u64 }
struct Upload { pending:PendingFile,metadata:SaveMetadata,expected_parent_id:Option<Uuid>,offset:u64,expires_at_ms:u64 }

#[derive(Default)]
pub struct Transfers { downloads:HashMap<Uuid,Download>,uploads:HashMap<Uuid,Upload>,exports:crate::export_stream::ExportStreams }

#[derive(Serialize)]
pub struct TransferInfo { pub transfer_id:Uuid,pub byte_length:u64,pub sha256:ContentHash,pub chunk_bytes:u32 }

#[derive(Serialize)]
pub struct ReadChunk { pub offset:u64,pub payload_base64:String,pub finished:bool }

impl Transfers {
    pub fn start_export(&mut self,metadata:SaveMetadata,grant:Uuid,file:File)->Result<Uuid>{self.exports.start(metadata,grant,file)}
    pub fn export_chunk(&mut self,id:Uuid,offset:u64,encoded:&str)->Result<u64>{self.exports.chunk(id,offset,encoded)}
    pub fn commit_export(&mut self,id:Uuid,grant:Uuid)->Result<()>{self.exports.commit(id,grant)}
    pub fn clear(&mut self){self.downloads.clear();self.uploads.clear();self.exports.clear();}
    pub fn open_download(&mut self,mut file:File,hash:ContentHash,length:u64)->Result<TransferInfo>{
        self.expire();
        if self.downloads.len()>=4||length==0||length>omniplay_contracts::MAX_ROM_BYTES {return Err(PlatformError::new(ErrorCode::ResourceLimit,"Download exceeds the transfer policy"));}
        ContentHash::parse(hash.0.clone())?;
        if file.metadata().map_err(storage_error)?.len()!=length{return Err(PlatformError::new(ErrorCode::ContentMismatch,"Stored content length changed"));}
        file.seek(SeekFrom::Start(0)).map_err(storage_error)?;
        let id=Uuid::new_v4();
        self.downloads.insert(id,Download{file,expected_hash:hash.clone(),length,offset:0,hasher:Sha256::new(),expires_at_ms:omniplay_core::now_ms().saturating_add(TRANSFER_TTL_MS)});
        Ok(TransferInfo{transfer_id:id,byte_length:length,sha256:hash,chunk_bytes:CHUNK_BYTES})
    }

    pub fn read(&mut self,id:Uuid,offset:u64,limit:u32)->Result<ReadChunk>{
        self.expire();
        let download=self.downloads.get_mut(&id).ok_or_else(||PlatformError::new(ErrorCode::NotFound,"Transfer is missing or expired"))?;
        if offset!=download.offset||limit==0||limit>CHUNK_BYTES{return Err(PlatformError::new(ErrorCode::InvalidRequest,"Transfer chunk offset or size is invalid"));}
        let count=(download.length-download.offset).min(limit as u64) as usize;
        let mut bytes=vec![0u8;count];download.file.read_exact(&mut bytes).map_err(storage_error)?;
        download.hasher.update(&bytes);download.offset+=count as u64;
        download.expires_at_ms=omniplay_core::now_ms().saturating_add(TRANSFER_TTL_MS);
        let finished=download.offset==download.length;
        if finished{
            let actual=ContentHash(format!("{:x}",download.hasher.clone().finalize()));
            if actual!=download.expected_hash{self.downloads.remove(&id);return Err(PlatformError::new(ErrorCode::CorruptData,"Stored content integrity check failed"));}
        }
        let result=ReadChunk{offset,payload_base64:STANDARD.encode(bytes),finished};
        if finished{self.downloads.remove(&id);}
        Ok(result)
    }

    pub fn start_upload(&mut self,root:&Path,metadata:SaveMetadata,expected_parent_id:Option<Uuid>)->Result<Uuid>{
        self.expire();metadata.validate()?;
        if self.uploads.len()>=2{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Too many pending save transfers"));}
        if metadata.parent_id!=expected_parent_id{return Err(PlatformError::new(ErrorCode::InvalidRequest,"Save upload parent is inconsistent"));}
        let pending=PendingFile::create(&root.join("transfers"))?;
        let id=Uuid::new_v4();
        self.uploads.insert(id,Upload{pending,metadata,expected_parent_id,offset:0,expires_at_ms:omniplay_core::now_ms().saturating_add(TRANSFER_TTL_MS)});
        Ok(id)
    }

    pub fn upload_chunk(&mut self,id:Uuid,offset:u64,encoded:&str)->Result<u64>{
        self.expire();
        if encoded.len()>CHUNK_BYTES as usize*2{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Save transfer chunk is too large"));}
        let upload=self.uploads.get_mut(&id).ok_or_else(||PlatformError::new(ErrorCode::NotFound,"Save transfer is missing or expired"))?;
        let bytes=STANDARD.decode(encoded).map_err(|_|PlatformError::new(ErrorCode::InvalidRequest,"Save chunk encoding is invalid"))?;
        if offset!=upload.offset||bytes.is_empty()||bytes.len()>CHUNK_BYTES as usize||upload.offset.saturating_add(bytes.len() as u64)>upload.metadata.payload_bytes{return Err(PlatformError::new(ErrorCode::InvalidRequest,"Save chunk offset or size is invalid"));}
        upload.pending.file.write_all(&bytes).map_err(storage_error)?;upload.offset+=bytes.len() as u64;
        upload.expires_at_ms=omniplay_core::now_ms().saturating_add(TRANSFER_TTL_MS);
        Ok(upload.offset)
    }

    pub fn finish_upload(&mut self,id:Uuid)->Result<(SaveMetadata,Vec<u8>,Option<Uuid>)>{
        self.expire();
        let mut upload=self.uploads.remove(&id).ok_or_else(||PlatformError::new(ErrorCode::NotFound,"Save transfer is missing or expired"))?;
        if upload.offset!=upload.metadata.payload_bytes{return Err(PlatformError::new(ErrorCode::InvalidRequest,"Save transfer is incomplete"));}
        upload.pending.file.seek(SeekFrom::Start(0)).map_err(storage_error)?;
        let payload=bounded_read(&mut upload.pending.file,omniplay_contracts::MAX_SAVE_BYTES)?;
        if omniplay_core::util::digest(&payload)!=upload.metadata.payload_hash{return Err(PlatformError::new(ErrorCode::ContentMismatch,"Save upload integrity check failed"));}
        Ok((upload.metadata,payload,upload.expected_parent_id))
    }

    pub fn abort(&mut self,id:Uuid){self.downloads.remove(&id);self.uploads.remove(&id);self.exports.abort(id);}
    pub fn expire(&mut self){let now=omniplay_core::now_ms();self.downloads.retain(|_,v|v.expires_at_ms>now);self.uploads.retain(|_,v|v.expires_at_ms>now);self.exports.expire();}
}
