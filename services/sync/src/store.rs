use omniplay_sync::{BlobHeader,DeviceCertificate,ObjectSummary};
use rusqlite::{Connection,OptionalExtension,TransactionBehavior,params};
use std::{path::{Path,PathBuf},collections::HashSet};
use uuid::Uuid;

use crate::{ApiError,ApiResult,now_ms};
mod transfers;

pub struct Store {connection:Connection,pub root:PathBuf,pub max_total_bytes:u64,pub max_accounts:u32,pub account_quota:u64,cleaned_uploads:HashSet<Uuid>}
pub enum Reservation {Existing,New(Uuid)}

impl Store {
    pub fn open(root:&Path,max_total_bytes:u64,max_accounts:u32,account_quota:u64)->ApiResult<Self> {
        std::fs::create_dir_all(root.join("objects")).map_err(ApiError::storage)?;
        std::fs::create_dir_all(root.join("pending")).map_err(ApiError::storage)?;
        let connection=Connection::open(root.join("sync.sqlite3")).map_err(ApiError::storage)?;
        connection.busy_timeout(std::time::Duration::from_secs(5)).map_err(ApiError::storage)?;
        let version:u32=connection.query_row("PRAGMA user_version",[],|row|row.get(0)).map_err(ApiError::storage)?;
        if version>2{return Err(ApiError::storage("Newer database schema"));}
        connection.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA trusted_schema=OFF;").map_err(ApiError::storage)?;
        if version==0{connection.execute_batch(include_str!("../schema.sql")).map_err(ApiError::storage)?;}
        if version<2{connection.execute_batch(include_str!("../schema2.sql")).map_err(ApiError::storage)?;}
        Ok(Self{connection,root:root.into(),max_total_bytes,max_accounts,account_quota,cleaned_uploads:HashSet::new()})
    }

    pub fn root_key(&self,account_id:&str)->ApiResult<String> {
        self.connection.query_row("SELECT root_public_key FROM accounts WHERE id=?1",[account_id],|row|row.get(0)).optional().map_err(ApiError::storage)?.ok_or_else(ApiError::unauthorized)
    }

    pub fn retired(&self,account:&str)->ApiResult<bool> {
        self.connection.query_row("SELECT EXISTS(SELECT 1 FROM retired_accounts WHERE id=?1)",[account],|row|row.get(0)).map_err(ApiError::storage)
    }

    pub fn create_account(&mut self,id:&str,key:&str,nonce:Uuid)->ApiResult<()> {
        let transaction=self.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(ApiError::storage)?;
        let retired:bool=transaction.query_row("SELECT EXISTS(SELECT 1 FROM retired_accounts WHERE id=?1)",[id],|row|row.get(0)).map_err(ApiError::storage)?;
        if retired{return Err(ApiError::conflict());}
        let existing:Option<(String,bool)>=transaction.query_row("SELECT root_public_key,deleting FROM accounts WHERE id=?1",[id],|row|Ok((row.get(0)?,row.get(1)?))).optional().map_err(ApiError::storage)?;
        if let Some((existing,deleting))=existing {if existing!=key||deleting{return Err(ApiError::unauthorized());}}
        else {
            let count:u32=transaction.query_row("SELECT count(*) FROM accounts",[],|row|row.get(0)).map_err(ApiError::storage)?;
            if count>=self.max_accounts{return Err(ApiError::quota());}
            transaction.execute("INSERT INTO accounts(id,root_public_key,quota_bytes,created_at_ms) VALUES(?1,?2,?3,?4)",params![id,key,self.account_quota,now_ms()]).map_err(ApiError::storage)?;
        }
        consume_nonce(&transaction,id,nonce)?;
        transaction.commit().map_err(ApiError::storage)
    }

    pub fn authorize(&mut self,certificate:&DeviceCertificate,nonce:Uuid)->ApiResult<()> {
        let transaction=self.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(ApiError::storage)?;
        let active:bool=transaction.query_row("SELECT EXISTS(SELECT 1 FROM accounts WHERE id=?1 AND deleting=0)",[&certificate.account_id],|row|row.get(0)).map_err(ApiError::storage)?;
        if !active{return Err(ApiError::unauthorized());}
        let existing:Option<(String,Option<u64>)>=transaction.query_row("SELECT device_public_key,revoked_at_ms FROM devices WHERE account_id=?1 AND device_id=?2",params![certificate.account_id,certificate.device_id.to_string()],|row|Ok((row.get(0)?,row.get(1)?))).optional().map_err(ApiError::storage)?;
        match existing {
            Some((key,revoked))=>if key!=certificate.device_public_key||revoked.is_some(){return Err(ApiError::unauthorized());},
            None=>{
                let count:u32=transaction.query_row("SELECT count(*) FROM devices WHERE account_id=?1",[&certificate.account_id],|row|row.get(0)).map_err(ApiError::storage)?;
                if count>=32{return Err(ApiError::quota());}
                transaction.execute("INSERT INTO devices(account_id,device_id,device_public_key,first_seen_at_ms,last_seen_at_ms) VALUES(?1,?2,?3,?4,?4)",params![certificate.account_id,certificate.device_id.to_string(),certificate.device_public_key,now_ms()]).map_err(ApiError::storage)?;
            }
        }
        consume_nonce(&transaction,&certificate.account_id,nonce)?;
        transaction.execute("UPDATE devices SET last_seen_at_ms=?1 WHERE account_id=?2 AND device_id=?3",params![now_ms(),certificate.account_id,certificate.device_id.to_string()]).map_err(ApiError::storage)?;
        transaction.commit().map_err(ApiError::storage)
    }

    pub fn revoke(&mut self,account:&str,device:Uuid,nonce:Uuid)->ApiResult<()> {
        let transaction=self.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(ApiError::storage)?;
        consume_nonce(&transaction,account,nonce)?;
        let changed=transaction.execute("UPDATE devices SET revoked_at_ms=?1 WHERE account_id=?2 AND device_id=?3",params![now_ms(),account,device.to_string()]).map_err(ApiError::storage)?;
        if changed==0{return Err(ApiError::not_found());}
        transaction.execute("DELETE FROM reservations WHERE account_id=?1",[account]).map_err(ApiError::storage)?;
        transaction.commit().map_err(ApiError::storage)
    }

    pub fn check_device_active(&self,certificate:&DeviceCertificate)->ApiResult<()> {
        let allowed:bool=self.connection.query_row("SELECT EXISTS(SELECT 1 FROM devices d JOIN accounts a ON a.id=d.account_id WHERE d.account_id=?1 AND d.device_id=?2 AND d.device_public_key=?3 AND d.revoked_at_ms IS NULL AND a.deleting=0)",params![certificate.account_id,certificate.device_id.to_string(),certificate.device_public_key],|row|row.get(0)).map_err(ApiError::storage)?;
        if !allowed||certificate.expires_at_ms<=now_ms(){return Err(ApiError::unauthorized());}
        Ok(())
    }

    pub fn reserve(&mut self,header:&BlobHeader)->ApiResult<Reservation> {
        let transaction=self.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(ApiError::storage)?;
        let existing:Option<String>=transaction.query_row("SELECT header_json FROM objects WHERE account_id=?1 AND object_id=?2",params![header.binding.account_id,header.binding.object_id.to_string()],|row|row.get(0)).optional().map_err(ApiError::storage)?;
        if let Some(json)=existing {
            let stored:BlobHeader=serde_json::from_str(&json).map_err(ApiError::storage)?;
            return if stored==*header {Ok(Reservation::Existing)}else{Err(ApiError::conflict())};
        }
        transaction.execute("DELETE FROM reservations WHERE expires_at_ms<=?1",[now_ms()]).map_err(ApiError::storage)?;
        let staged:bool=transaction.query_row("SELECT EXISTS(SELECT 1 FROM uploads WHERE account_id=?1 AND object_id=?2)",params![header.binding.account_id,header.binding.object_id.to_string()],|row|row.get(0)).map_err(ApiError::storage)?;
        if staged{return Err(ApiError::conflict());}
        let (quota,account_used):(u64,u64)=transaction.query_row("SELECT quota_bytes,(SELECT coalesce(sum(byte_length),0) FROM objects WHERE account_id=?1)+(SELECT coalesce(sum(byte_length),0) FROM reservations WHERE account_id=?1)+(SELECT coalesce(sum(byte_length*2),0) FROM uploads WHERE account_id=?1) FROM accounts WHERE id=?1 AND deleting=0",[&header.binding.account_id],|row|Ok((row.get(0)?,row.get(1)?))).map_err(ApiError::storage)?;
        let total:u64=transaction.query_row("SELECT (SELECT coalesce(sum(byte_length),0) FROM objects)+(SELECT coalesce(sum(byte_length),0) FROM reservations)+(SELECT coalesce(sum(byte_length*2),0) FROM uploads)",[],|row|row.get(0)).map_err(ApiError::storage)?;
        if account_used.saturating_add(header.ciphertext_bytes)>quota||total.saturating_add(header.ciphertext_bytes)>self.max_total_bytes{return Err(ApiError::quota());}
        let id=Uuid::new_v4();
        transaction.execute("INSERT INTO reservations(id,account_id,object_id,byte_length,expires_at_ms) VALUES(?1,?2,?3,?4,?5)",params![id.to_string(),header.binding.account_id,header.binding.object_id.to_string(),header.ciphertext_bytes,now_ms().saturating_add(600_000)]).map_err(|_|ApiError::conflict())?;
        transaction.commit().map_err(ApiError::storage)?;
        Ok(Reservation::New(id))
    }

    pub fn cancel_reservation(&mut self,id:Uuid) {let _=self.connection.execute("DELETE FROM reservations WHERE id=?1",[id.to_string()]);}

    pub fn commit_object(&mut self,id:Uuid,header:&BlobHeader)->ApiResult<()> {
        let transaction=self.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(ApiError::storage)?;
        let exists:bool=transaction.query_row("SELECT EXISTS(SELECT 1 FROM reservations WHERE id=?1 AND account_id=?2 AND object_id=?3 AND expires_at_ms>?4)",params![id.to_string(),header.binding.account_id,header.binding.object_id.to_string(),now_ms()],|row|row.get(0)).map_err(ApiError::storage)?;
        if !exists{return Err(ApiError::conflict());}
        transaction.execute("INSERT INTO objects(account_id,object_id,header_json,ciphertext_sha256,byte_length,uploaded_at_ms) VALUES(?1,?2,?3,?4,?5,?6)",params![header.binding.account_id,header.binding.object_id.to_string(),serde_json::to_string(header).map_err(ApiError::storage)?,header.ciphertext_sha256,header.ciphertext_bytes,now_ms()]).map_err(ApiError::storage)?;
        transaction.execute("DELETE FROM reservations WHERE id=?1",[id.to_string()]).map_err(ApiError::storage)?;
        transaction.commit().map_err(ApiError::storage)
    }

    pub fn object_header(&self,account:&str,id:Uuid)->ApiResult<BlobHeader> {
        let json:Option<String>=self.connection.query_row("SELECT header_json FROM objects WHERE account_id=?1 AND object_id=?2",params![account,id.to_string()],|row|row.get(0)).optional().map_err(ApiError::storage)?;
        let json=json.ok_or_else(ApiError::not_found)?;if json.len()>4096{return Err(ApiError::storage("Invalid object header"));}
        let header:BlobHeader=serde_json::from_str(&json).map_err(ApiError::storage)?;header.validate().map_err(ApiError::storage)?;
        if header.binding.account_id!=account||header.binding.object_id!=id{return Err(ApiError::storage("Invalid object identity"));}Ok(header)
    }

    pub fn list(&self,account:&str,after:u64,limit:u16)->ApiResult<Vec<ObjectSummary>> {
        let mut query=self.connection.prepare("SELECT object_id,sequence,byte_length,uploaded_at_ms FROM objects WHERE account_id=?1 AND sequence>?2 ORDER BY sequence LIMIT ?3").map_err(ApiError::storage)?;
        let rows=query.query_map(params![account,after,limit],|row|Ok((row.get::<_,String>(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).map_err(ApiError::storage)?;
        rows.map(|row|{let (id,sequence,encrypted_bytes,uploaded_at_ms)=row.map_err(ApiError::storage)?;Ok(ObjectSummary{object_id:Uuid::parse_str(&id).map_err(ApiError::storage)?,sequence,encrypted_bytes,uploaded_at_ms})}).collect()
    }

    pub fn devices(&self,account:&str)->ApiResult<Vec<serde_json::Value>> {
        let mut query=self.connection.prepare("SELECT device_id,first_seen_at_ms,last_seen_at_ms,revoked_at_ms FROM devices WHERE account_id=?1 ORDER BY first_seen_at_ms").map_err(ApiError::storage)?;
        let rows=query.query_map([account],|row|Ok(serde_json::json!({"device_id":row.get::<_,String>(0)?,"first_seen_at_ms":row.get::<_,u64>(1)?,"last_seen_at_ms":row.get::<_,u64>(2)?,"revoked_at_ms":row.get::<_,Option<u64>>(3)?}))).map_err(ApiError::storage)?;
        rows.map(|row|row.map_err(ApiError::storage)).collect()
    }

    pub fn begin_delete(&mut self,account:&str,nonce:Uuid)->ApiResult<()> {
        let transaction=self.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(ApiError::storage)?;
        consume_nonce(&transaction,account,nonce)?;
        let changed=transaction.execute("UPDATE accounts SET deleting=1 WHERE id=?1",[account]).map_err(ApiError::storage)?;
        if changed==0{return Err(ApiError::unauthorized());}
        transaction.execute("DELETE FROM reservations WHERE account_id=?1",[account]).map_err(ApiError::storage)?;
        transaction.commit().map_err(ApiError::storage)
    }

    pub fn finish_delete(&mut self,account:&str)->ApiResult<()> {
        let transaction=self.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(ApiError::storage)?;
        let deleting:bool=transaction.query_row("SELECT EXISTS(SELECT 1 FROM accounts WHERE id=?1 AND deleting=1)",[account],|row|row.get(0)).map_err(ApiError::storage)?;
        if !deleting{return Err(ApiError::conflict());}
        transaction.execute("INSERT OR IGNORE INTO retired_accounts(id) VALUES(?1)",[account]).map_err(ApiError::storage)?;
        transaction.execute("DELETE FROM accounts WHERE id=?1 AND deleting=1",[account]).map_err(ApiError::storage)?;
        transaction.commit().map_err(ApiError::storage)
    }
}

fn consume_nonce(connection:&Connection,account:&str,nonce:Uuid)->ApiResult<()> {
    connection.execute("DELETE FROM nonces WHERE expires_at_ms<=?1",[now_ms()]).map_err(ApiError::storage)?;
    let count:u32=connection.query_row("SELECT count(*) FROM nonces WHERE account_id=?1",[account],|row|row.get(0)).map_err(ApiError::storage)?;
    if count>=2000{return Err(ApiError::quota());}
    connection.execute("INSERT INTO nonces(account_id,nonce,expires_at_ms) VALUES(?1,?2,?3)",params![account,nonce.to_string(),now_ms().saturating_add(300_000)]).map_err(|_|ApiError::unauthorized())?;
    Ok(())
}
