use super::Store;
use crate::{ApiError,ApiResult,account_directory,now_ms};
use omniplay_sync::{BlobHeader,DeviceCertificate,UploadProgress,TRANSFER_PROTOCOL,TRANSFER_CHUNK_BYTES,TRANSFER_TTL_MS,MAX_ACCOUNT_TRANSFERS,MAX_SERVICE_TRANSFERS,chunk_length,transfer_files as files};
use rusqlite::{OptionalExtension,TransactionBehavior,params};
use sha2::{Digest,Sha256};
use std::{io::{Read,Seek,SeekFrom},path::{Path,PathBuf}};
use uuid::Uuid;

struct Upload{header:BlobHeader,received:u64,expiry:u64,complete:bool}
fn digest(bytes:&[u8])->String{format!("{:x}",Sha256::digest(bytes))}
fn part_name(offset:u64)->String{format!("{:08x}.chunk",offset/u64::from(TRANSFER_CHUNK_BYTES))}
fn verified(path:&Path,header:&BlobHeader)->ApiResult<()>{
    let mut file=files::open(path,false,header.ciphertext_bytes).map_err(ApiError::storage)?;
    let mut hash=Sha256::new();let mut buffer=[0u8;64*1024];let mut length=0u64;
    loop{let count=file.read(&mut buffer).map_err(ApiError::storage)?;if count==0{break;}length+=count as u64;hash.update(&buffer[..count]);}
    if length!=header.ciphertext_bytes||format!("{:x}",hash.finalize())!=header.ciphertext_sha256{return Err(ApiError::invalid());}Ok(())
}
impl Store{
    fn transfer_directory(&self,id:Uuid)->ApiResult<PathBuf>{let root=files::private_dir(&self.root,"transfers").map_err(ApiError::storage)?;files::private_dir(&root,&id.to_string()).map_err(ApiError::storage)}
    fn prepare_directory(&mut self,id:Uuid,bytes:u64)->ApiResult<PathBuf>{
        let directory=self.transfer_directory(id)?;
        if !self.cleaned_uploads.contains(&id){
            let mut count=0;let bound=bytes.div_ceil(u64::from(TRANSFER_CHUNK_BYTES))+33;
            for entry in std::fs::read_dir(&directory).map_err(ApiError::storage)?{
                count+=1;if count>bound{return Err(ApiError::storage("Transfer recovery bound exceeded"));}
                let entry=entry.map_err(ApiError::storage)?;let name=entry.file_name().into_string().map_err(|_|ApiError::storage("Unexpected transfer filename"))?;
                if let Some(value)=name.strip_prefix(".pending-"){
                    let token=Uuid::parse_str(value).map_err(ApiError::storage)?;if token.to_string()!=value{return Err(ApiError::storage("Unexpected transfer temporary file"));}
                    let file=files::open(&entry.path(),false,bytes).map_err(ApiError::storage)?;drop(file);files::remove(&entry.path()).map_err(ApiError::storage)?;
                }
            }
            files::sync_directory(&directory).map_err(ApiError::storage)?;self.cleaned_uploads.insert(id);
        }
        Ok(directory)
    }
    fn upload(&self,certificate:&DeviceCertificate,id:Uuid)->ApiResult<Upload>{
        self.check_device_active(certificate)?;
        let row:Option<(String,u64,u64,bool)>=self.connection.query_row("SELECT header_json,received_bytes,expires_at_ms,completed FROM uploads WHERE id=?1 AND account_id=?2 AND device_id=?3",params![id.to_string(),certificate.account_id,certificate.device_id.to_string()],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional().map_err(ApiError::storage)?;
        let (json,received,expiry,complete)=row.ok_or_else(ApiError::not_found)?;
        if json.len()>4096{return Err(ApiError::storage("Invalid staged header"));}
        let header:BlobHeader=serde_json::from_str(&json).map_err(ApiError::storage)?;header.validate().map_err(|_|ApiError::invalid())?;
        if header.binding.account_id!=certificate.account_id||received>header.ciphertext_bytes||(received<header.ciphertext_bytes&&received%u64::from(TRANSFER_CHUNK_BYTES)!=0){return Err(ApiError::storage("Invalid staged prefix"));}
        if expiry<=now_ms(){return Err(ApiError::not_found());}
        Ok(Upload{header,received,expiry,complete})
    }
    fn committed(&self,header:&BlobHeader)->ApiResult<bool>{
        let json:Option<String>=self.connection.query_row("SELECT header_json FROM objects WHERE account_id=?1 AND object_id=?2",params![header.binding.account_id,header.binding.object_id.to_string()],|row|row.get(0)).optional().map_err(ApiError::storage)?;
        match json{None=>Ok(false),Some(json)=>{let stored:BlobHeader=serde_json::from_str(&json).map_err(ApiError::storage)?;if stored!=*header{return Err(ApiError::conflict());}Ok(true)}}
    }
    pub fn upload_begin(&mut self,certificate:&DeviceCertificate,id:Uuid,header:BlobHeader)->ApiResult<UploadProgress>{
        self.check_device_active(certificate)?;header.validate().map_err(|_|ApiError::invalid())?;
        if id.is_nil()||header.binding.account_id!=certificate.account_id{return Err(ApiError::invalid());}
        self.collect_uploads()?;
        self.check_device_active(certificate)?;
        if self.committed(&header)?{return Ok(UploadProgress{protocol:TRANSFER_PROTOCOL,transfer_id:id,received_bytes:header.ciphertext_bytes,header,stored:true,expires_at_ms:0});}
        let existing:bool=self.connection.query_row("SELECT EXISTS(SELECT 1 FROM uploads WHERE id=?1)",[id.to_string()],|row|row.get(0)).map_err(ApiError::storage)?;
        if existing{let upload=self.upload(certificate,id)?;if upload.header!=header{return Err(ApiError::conflict());}return Ok(UploadProgress{protocol:TRANSFER_PROTOCOL,transfer_id:id,header,received_bytes:upload.received,stored:upload.complete,expires_at_ms:upload.expiry});}
        let transaction=self.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(ApiError::storage)?;
        let (account_count,total_count):(u32,u32)=transaction.query_row("SELECT (SELECT count(*) FROM uploads WHERE account_id=?1),(SELECT count(*) FROM uploads)",[&certificate.account_id],|row|Ok((row.get(0)?,row.get(1)?))).map_err(ApiError::storage)?;
        if account_count>=MAX_ACCOUNT_TRANSFERS||total_count>=MAX_SERVICE_TRANSFERS{return Err(ApiError::quota());}
        let reserved:bool=transaction.query_row("SELECT EXISTS(SELECT 1 FROM reservations WHERE account_id=?1 AND object_id=?2 AND expires_at_ms>?3)",params![certificate.account_id,header.binding.object_id.to_string(),now_ms()],|row|row.get(0)).map_err(ApiError::storage)?;
        if reserved{return Err(ApiError::conflict());}
        let (quota,used):(u64,u64)=transaction.query_row("SELECT quota_bytes,(SELECT coalesce(sum(byte_length),0) FROM objects WHERE account_id=?1)+(SELECT coalesce(sum(byte_length),0) FROM reservations WHERE account_id=?1)+(SELECT coalesce(sum(byte_length*2),0) FROM uploads WHERE account_id=?1) FROM accounts WHERE id=?1 AND deleting=0",[&certificate.account_id],|row|Ok((row.get(0)?,row.get(1)?))).map_err(ApiError::storage)?;
        let total:u64=transaction.query_row("SELECT (SELECT coalesce(sum(byte_length),0) FROM objects)+(SELECT coalesce(sum(byte_length),0) FROM reservations)+(SELECT coalesce(sum(byte_length*2),0) FROM uploads)",[],|row|row.get(0)).map_err(ApiError::storage)?;
        let reserve=header.ciphertext_bytes.saturating_mul(2);
        if used.saturating_add(reserve)>quota||total.saturating_add(reserve)>self.max_total_bytes{return Err(ApiError::quota());}
        let expiry=now_ms().saturating_add(TRANSFER_TTL_MS);
        transaction.execute("INSERT INTO uploads(id,account_id,device_id,object_id,header_json,byte_length,expires_at_ms) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![id.to_string(),certificate.account_id,certificate.device_id.to_string(),header.binding.object_id.to_string(),serde_json::to_string(&header).map_err(ApiError::storage)?,header.ciphertext_bytes,expiry]).map_err(|_|ApiError::conflict())?;
        transaction.commit().map_err(ApiError::storage)?;
        Ok(UploadProgress{protocol:TRANSFER_PROTOCOL,transfer_id:id,header,received_bytes:0,stored:false,expires_at_ms:expiry})
    }
    pub fn upload_chunk(&mut self,certificate:&DeviceCertificate,id:Uuid,offset:u64,sha256:&str,bytes:&[u8])->ApiResult<UploadProgress>{
        let upload=self.upload(certificate,id)?;
        if upload.complete||chunk_length(upload.header.ciphertext_bytes,offset)!=Some(bytes.len() as u32)||digest(bytes)!=sha256||offset>upload.received{return Err(ApiError::conflict());}
        let directory=self.prepare_directory(id,upload.header.ciphertext_bytes)?;let path=directory.join(part_name(offset));
        let publication=(||->ApiResult<()>{if offset<upload.received||path.try_exists().map_err(ApiError::storage)?{
            let mut file=files::open(&path,false,u64::from(TRANSFER_CHUNK_BYTES)).map_err(ApiError::storage)?;let mut existing=Vec::new();file.read_to_end(&mut existing).map_err(ApiError::storage)?;
            if existing!=bytes{return Err(ApiError::conflict());}
        }else{let mut pending=files::Pending::create(&directory).map_err(ApiError::storage)?;pending.write_all(bytes).map_err(ApiError::storage)?;pending.publish(&path).map_err(ApiError::storage)?;}Ok(())})();
        if let Err(error)=publication{self.cleaned_uploads.remove(&id);return Err(error);}
        let received=upload.received.max(offset+bytes.len() as u64);
        let changed=self.connection.execute("UPDATE uploads SET received_bytes=?1 WHERE id=?2 AND received_bytes=?3 AND expires_at_ms>?4 AND completed=0",params![received,id.to_string(),upload.received,now_ms()]).map_err(ApiError::storage)?;
        if changed!=1{return Err(ApiError::conflict());}
        Ok(UploadProgress{protocol:TRANSFER_PROTOCOL,transfer_id:id,header:upload.header,received_bytes:received,stored:false,expires_at_ms:upload.expiry})
    }
    pub fn upload_commit(&mut self,certificate:&DeviceCertificate,id:Uuid,header:BlobHeader)->ApiResult<UploadProgress>{
        self.check_device_active(certificate)?;
        header.validate().map_err(|_|ApiError::invalid())?;
        if header.binding.account_id!=certificate.account_id{return Err(ApiError::invalid());}
        if self.committed(&header)?{return Ok(UploadProgress{protocol:TRANSFER_PROTOCOL,transfer_id:id,received_bytes:header.ciphertext_bytes,header,stored:true,expires_at_ms:0});}
        let upload=self.upload(certificate,id)?;
        if upload.header!=header||upload.received!=header.ciphertext_bytes{return Err(ApiError::conflict());}
        let directory=self.prepare_directory(id,header.ciphertext_bytes)?;let assembled=directory.join("assembled.blob");
        let assembly=(||->ApiResult<()>{if !assembled.try_exists().map_err(ApiError::storage)?{
            let mut pending=files::Pending::create(&directory).map_err(ApiError::storage)?;let mut offset=0;let mut hash=Sha256::new();
            while let Some(length)=chunk_length(header.ciphertext_bytes,offset){
                let mut part=files::open(&directory.join(part_name(offset)),false,u64::from(length)).map_err(ApiError::storage)?;let mut bytes=vec![0;length as usize];part.read_exact(&mut bytes).map_err(ApiError::storage)?;
                hash.update(&bytes);pending.write_all(&bytes).map_err(ApiError::storage)?;offset+=u64::from(length);
            }
            if format!("{:x}",hash.finalize())!=header.ciphertext_sha256{return Err(ApiError::invalid());}
            pending.publish(&assembled).map_err(ApiError::storage)?;
        }else{verified(&assembled,&header)?;}Ok(())})();
        if let Err(error)=assembly{self.cleaned_uploads.remove(&id);return Err(error);}
        self.check_device_active(certificate)?;
        if upload.expiry<=now_ms(){return Err(ApiError::not_found());}
        let objects=files::private_dir(&self.root,"objects").map_err(ApiError::storage)?;
        let target_directory=files::private_dir(&objects,&certificate.account_id).map_err(ApiError::storage)?;
        let target=target_directory.join(format!("{}.blob",header.ciphertext_sha256));
        if target.try_exists().map_err(ApiError::storage)?{verified(&target,&header)?;}else{std::fs::hard_link(&assembled,&target).map_err(ApiError::storage)?;files::sync_directory(&target_directory).map_err(ApiError::storage)?;}
        let transaction=self.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(ApiError::storage)?;
        transaction.execute("INSERT INTO objects(account_id,object_id,header_json,ciphertext_sha256,byte_length,uploaded_at_ms) VALUES(?1,?2,?3,?4,?5,?6)",params![certificate.account_id,header.binding.object_id.to_string(),serde_json::to_string(&header).map_err(ApiError::storage)?,header.ciphertext_sha256,header.ciphertext_bytes,now_ms()]).map_err(ApiError::storage)?;
        transaction.execute("UPDATE uploads SET completed=1 WHERE id=?1",[id.to_string()]).map_err(ApiError::storage)?;
        transaction.commit().map_err(ApiError::storage)?;
        // Failed cleanup retains its row and conservative reserved-byte debt.
        let _=self.remove_upload(id);
        Ok(UploadProgress{protocol:TRANSFER_PROTOCOL,transfer_id:id,received_bytes:header.ciphertext_bytes,header,stored:true,expires_at_ms:0})
    }
    pub fn upload_abort(&mut self,certificate:&DeviceCertificate,id:Uuid)->ApiResult<()>{
        self.check_device_active(certificate)?;
        let owner:Option<(String,String)>=self.connection.query_row("SELECT account_id,device_id FROM uploads WHERE id=?1",[id.to_string()],|row|Ok((row.get(0)?,row.get(1)?))).optional().map_err(ApiError::storage)?;
        if let Some((account,device))=owner{if account!=certificate.account_id||device!=certificate.device_id.to_string(){return Err(ApiError::unauthorized());}self.remove_upload(id)?;}Ok(())
    }
    pub fn collect_uploads(&mut self)->ApiResult<()>{
        let ids={let mut query=self.connection.prepare("SELECT id FROM uploads WHERE expires_at_ms<=?1 OR completed=1 ORDER BY expires_at_ms,id LIMIT 8").map_err(ApiError::storage)?;let rows=query.query_map([now_ms()],|row|row.get::<_,String>(0)).map_err(ApiError::storage)?;rows.collect::<std::result::Result<Vec<_>,_>>().map_err(ApiError::storage)?};
        for id in ids{self.remove_upload(Uuid::parse_str(&id).map_err(ApiError::storage)?)?;}Ok(())
    }
    pub fn abort_account_uploads(&mut self,account:&str)->ApiResult<()>{
        let ids={let mut query=self.connection.prepare("SELECT id FROM uploads WHERE account_id=?1 LIMIT 32").map_err(ApiError::storage)?;let rows=query.query_map([account],|row|row.get::<_,String>(0)).map_err(ApiError::storage)?;rows.collect::<std::result::Result<Vec<_>,_>>().map_err(ApiError::storage)?};
        for id in ids{self.remove_upload(Uuid::parse_str(&id).map_err(ApiError::storage)?)?;}Ok(())
    }
    fn remove_upload(&mut self,id:Uuid)->ApiResult<()>{
        self.cleaned_uploads.remove(&id);
        let bytes:Option<u64>=self.connection.query_row("SELECT byte_length FROM uploads WHERE id=?1",[id.to_string()],|row|row.get(0)).optional().map_err(ApiError::storage)?;
        let Some(bytes)=bytes else{return Ok(());};
        if bytes>omniplay_sync::MAX_CIPHERTEXT_BYTES as u64{return Err(ApiError::storage("Invalid upload quota record"));}
        let root=files::private_dir(&self.root,"transfers").map_err(ApiError::storage)?;let directory=root.join(id.to_string());
        if directory.try_exists().map_err(ApiError::storage)?{
            let directory=self.transfer_directory(id)?;let chunks=bytes.div_ceil(u64::from(TRANSFER_CHUNK_BYTES));let mut count=0;
            for entry in std::fs::read_dir(&directory).map_err(ApiError::storage)?{
                count+=1;if count>chunks+32{return Err(ApiError::storage("Upload cleanup bound exceeded"));}
                let entry=entry.map_err(ApiError::storage)?;let name=entry.file_name().into_string().map_err(|_|ApiError::storage("Unexpected transfer filename"))?;
                let part=name.strip_suffix(".chunk").filter(|value|value.len()==8&&value.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))).and_then(|value|u64::from_str_radix(value,16).ok()).is_some_and(|index|index<chunks);
                let pending=name.strip_prefix(".pending-").and_then(|value|Uuid::parse_str(value).ok().map(|id|(value,id))).is_some_and(|(value,id)|id.to_string()==value);
                if name!="assembled.blob"&&!part&&!pending{return Err(ApiError::storage("Unexpected transfer entry"));}
                files::remove(&entry.path()).map_err(ApiError::storage)?;
            }
            std::fs::remove_dir(&directory).map_err(ApiError::storage)?;files::sync_directory(&root).map_err(ApiError::storage)?;
        }
        self.connection.execute("DELETE FROM uploads WHERE id=?1",[id.to_string()]).map_err(ApiError::storage)?;Ok(())
    }
    pub fn object_chunk(&self,account:&str,id:Uuid,offset:u64,length:u32)->ApiResult<(BlobHeader,Vec<u8>)>{
        let header=self.object_header(account,id)?;
        if chunk_length(header.ciphertext_bytes,offset)!=Some(length){return Err(ApiError::invalid());}
        let path=account_directory(&self.root,account)?.join(format!("{}.blob",header.ciphertext_sha256));
        let mut file=files::open(&path,false,header.ciphertext_bytes).map_err(ApiError::storage)?;
        if file.metadata().map_err(ApiError::storage)?.len()!=header.ciphertext_bytes{return Err(ApiError::storage("Encrypted object length mismatch"));}
        file.seek(SeekFrom::Start(offset)).map_err(ApiError::storage)?;let mut bytes=vec![0;length as usize];file.read_exact(&mut bytes).map_err(ApiError::storage)?;Ok((header,bytes))
    }
}
