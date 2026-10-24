use crate::{ApiError,ApiResult};
use omniplay_streaming::*;
use rusqlite::{Connection,OptionalExtension,Transaction,params};
use serde::Serialize;
use serde_json::{Value,json};
use sha2::{Digest,Sha256};
use std::path::Path;

pub struct Store {database:Connection}
fn text(value:&impl Serialize)->ApiResult<String>{serde_json::to_string(value).map_err(|_|ApiError::invalid())}
fn sql(_:impl std::fmt::Display)->ApiError{ApiError::storage()}
impl Store {
    pub fn open(root:&Path)->ApiResult<Self>{
        let path=root.join("relay.sqlite3");if std::fs::symlink_metadata(&path).is_ok_and(|m|m.file_type().is_symlink()){return Err(ApiError::invalid());}
        let database=Connection::open(path).map_err(sql)?;database.busy_timeout(std::time::Duration::from_secs(3)).map_err(sql)?;
        database.execute_batch("PRAGMA journal_mode=WAL;PRAGMA synchronous=FULL;PRAGMA foreign_keys=ON;PRAGMA trusted_schema=OFF;
          CREATE TABLE IF NOT EXISTS schema_info(version INTEGER NOT NULL);INSERT INTO schema_info SELECT 1 WHERE NOT EXISTS(SELECT 1 FROM schema_info);
          CREATE TABLE IF NOT EXISTS invites(id TEXT PRIMARY KEY,host TEXT NOT NULL,expires INTEGER NOT NULL,document TEXT NOT NULL,secret_hash TEXT NOT NULL);
          CREATE INDEX IF NOT EXISTS invites_expiry ON invites(expires);
          CREATE TABLE IF NOT EXISTS joins(invite TEXT NOT NULL REFERENCES invites(id) ON DELETE CASCADE,client TEXT NOT NULL,hello TEXT NOT NULL,state TEXT NOT NULL,PRIMARY KEY(invite,client));
          CREATE TABLE IF NOT EXISTS pairs(id TEXT PRIMARY KEY,host TEXT NOT NULL,client TEXT NOT NULL,invite TEXT NOT NULL,expires INTEGER NOT NULL,grant_hash TEXT NOT NULL,document TEXT NOT NULL,revoked INTEGER NOT NULL DEFAULT 0,host_seq INTEGER NOT NULL DEFAULT 0,client_seq INTEGER NOT NULL DEFAULT 0);
          CREATE INDEX IF NOT EXISTS pairs_endpoints ON pairs(host,client);
          CREATE INDEX IF NOT EXISTS pairs_client ON pairs(client);
          CREATE INDEX IF NOT EXISTS pairs_expiry ON pairs(expires);
          CREATE TABLE IF NOT EXISTS messages(cursor INTEGER PRIMARY KEY AUTOINCREMENT,pair TEXT NOT NULL REFERENCES pairs(id) ON DELETE CASCADE,sender TEXT NOT NULL,message_id TEXT NOT NULL UNIQUE,session TEXT NOT NULL,sequence INTEGER NOT NULL,expires INTEGER NOT NULL,iv TEXT NOT NULL,ciphertext TEXT NOT NULL,UNIQUE(pair,sender,sequence));
          CREATE INDEX IF NOT EXISTS messages_inbox ON messages(pair,cursor);
          CREATE INDEX IF NOT EXISTS messages_expiry ON messages(expires);
          CREATE TABLE IF NOT EXISTS nonces(signer TEXT NOT NULL,nonce TEXT NOT NULL,expires INTEGER NOT NULL,PRIMARY KEY(signer,nonce));
          CREATE INDEX IF NOT EXISTS nonces_expiry ON nonces(expires);") .map_err(sql)?;
        if database.query_row("SELECT version FROM schema_info",[],|r|r.get::<_,u32>(0)).map_err(sql)?!=1{return Err(ApiError::invalid());}Ok(Self{database})
    }
    pub fn execute(&mut self,sender:&str,request:RelayRequest,audience:&str,now:u64)->ApiResult<Value>{
        let tx=self.database.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(sql)?;
        tx.execute("DELETE FROM nonces WHERE expires<=?1",[now]).map_err(sql)?;
        tx.execute("DELETE FROM messages WHERE expires<=?1",[now]).map_err(sql)?;
        tx.execute("DELETE FROM invites WHERE expires<=?1",[now]).map_err(sql)?;
        tx.execute("DELETE FROM pairs WHERE expires<=?1",[now]).map_err(sql)?;
        let nonce_count:u64=tx.query_row("SELECT count(*) FROM nonces",[],|r|r.get(0)).map_err(sql)?;if nonce_count>=20000{return Err(ApiError::quota());}
        if tx.execute("INSERT INTO nonces(signer,nonce,expires) VALUES(?1,?2,?3)",params![sender,request.nonce.to_string(),now+240000]).is_err(){return Err(ApiError::unauthorized());}
        let result=Self::operation(&tx,sender,request.operation,audience,now)?;tx.commit().map_err(sql)?;Ok(result)
    }
    fn operation(tx:&Transaction<'_>,sender:&str,operation:RelayOperation,audience:&str,now:u64)->ApiResult<Value>{
        match operation {
            RelayOperation::CreateInvite{invite:document}=>{
                let invite:Invite=verify(sender,INVITE_DOMAIN,&document,4096).map_err(|_|ApiError::unauthorized())?;invite.validate(audience,now).map_err(|_|ApiError::invalid())?;if invite.host_public_key!=sender{return Err(ApiError::unauthorized());}
                let count:u64=tx.query_row("SELECT count(*) FROM invites",[],|r|r.get(0)).map_err(sql)?;
                let owned:u64=tx.query_row("SELECT count(*) FROM invites WHERE host=?1",[sender],|r|r.get(0)).map_err(sql)?;if count>=512||owned>=2{return Err(ApiError::quota());}
                tx.execute("INSERT INTO invites(id,host,expires,document,secret_hash) VALUES(?1,?2,?3,?4,?5)",params![invite.id.to_string(),sender,invite.expires_at_ms,text(&document)?,invite.secret_hash.0]).map_err(|_|ApiError::conflict())?;Ok(json!({"created":true}))
            }
            RelayOperation::JoinInvite{invite_id,secret,hello}=>{
                let(_,hash)=invite_row(tx,&invite_id.to_string(),None)?;
                let raw=decode(&secret,32).map_err(|_|ApiError::unauthorized())?;if raw.len()!=32||format!("{:x}",Sha256::digest(raw))!=hash{return Err(ApiError::unauthorized());}
                let request:ClientHello=verify(sender,HELLO_DOMAIN,&hello,4096).map_err(|_|ApiError::unauthorized())?;request.validate(invite_id,now).map_err(|_|ApiError::invalid())?;
                if request.client_public_key!=sender{return Err(ApiError::unauthorized());}
                let existing:Option<String>=tx.query_row("SELECT hello FROM joins WHERE invite=?1 AND client=?2",params![invite_id.to_string(),sender],|r|r.get(0)).optional().map_err(sql)?;
                let receipt=json!({"pending":true,"join_protocol":1,"invite_id":invite_id,"client_public_key":sender,"hello_hash":document_hash(&hello).map_err(|_|ApiError::invalid())?});
                if let Some(existing)=existing{if existing!=text(&hello)?{return Err(ApiError::conflict());}return Ok(receipt);}
                let count:u64=tx.query_row("SELECT count(*) FROM joins WHERE invite=?1",[invite_id.to_string()],|r|r.get(0)).map_err(sql)?;if count>=8{return Err(ApiError::quota());}
                tx.execute("INSERT INTO joins(invite,client,hello,state) VALUES(?1,?2,?3,'pending')",params![invite_id.to_string(),sender,text(&hello)?]).map_err(sql)?;Ok(receipt)
            }
            RelayOperation::PollInvite{invite_id}=>{
                invite_row(tx,&invite_id.to_string(),Some(sender))?;
                let mut statement=tx.prepare("SELECT hello FROM joins WHERE invite=?1 AND state='pending' ORDER BY client LIMIT 8").map_err(sql)?;
                let rows=statement.query_map([invite_id.to_string()],|r|r.get::<_,String>(0)).map_err(sql)?;let mut hellos=Vec::<SignedDocument>::new();for row in rows{hellos.push(serde_json::from_str(&row.map_err(sql)?).map_err(sql)?);}Ok(json!({"requests":hellos}))
            }
            RelayOperation::PairingStatus{invite_id}=>{
                let joined:Option<(String,String)>=tx.query_row("SELECT state,hello FROM joins WHERE invite=?1 AND client=?2",params![invite_id.to_string(),sender],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(sql)?;
                let Some((state,hello))=joined else{return Err(ApiError::not_found());};
                let hello:SignedDocument=serde_json::from_str(&hello).map_err(sql)?;
                let hello_hash=document_hash(&hello).map_err(|_|ApiError::invalid())?;
                if state=="approved"{
                    let document:String=tx.query_row("SELECT document FROM pairs WHERE invite=?1 AND client=?2 AND revoked=0 AND expires>?3",params![invite_id.to_string(),sender,now],|r|r.get(0)).optional().map_err(sql)?.ok_or_else(ApiError::unauthorized)?;
                    let grant:SignedDocument=serde_json::from_str(&document).map_err(sql)?;Ok(json!({"state":state,"grant":grant,"pairing_status_protocol":1,"invite_id":invite_id,"client_public_key":sender,"hello_hash":hello_hash}))
                }else{Ok(json!({"state":state,"grant":null,"pairing_status_protocol":1,"invite_id":invite_id,"client_public_key":sender,"hello_hash":hello_hash}))}
            }
            RelayOperation::ApprovePair{invite_id,client_public_key,grant:document}=>{
                invite_row(tx,&invite_id.to_string(),Some(sender))?;let grant=verify_grant(&document,audience,now).map_err(|_|ApiError::unauthorized())?;
                if grant.host_public_key!=sender||grant.client_public_key!=client_public_key||grant.invite_id!=invite_id{return Err(ApiError::unauthorized());}
                let hello:Option<(String,String)>=tx.query_row("SELECT hello,state FROM joins WHERE invite=?1 AND client=?2",params![invite_id.to_string(),client_public_key],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(sql)?;
                let Some((hello,state))=hello else{return Err(ApiError::not_found());};
                let signed:SignedDocument=serde_json::from_str(&hello).map_err(sql)?;let client:ClientHello=verify(&client_public_key,HELLO_DOMAIN,&signed,4096).map_err(|_|ApiError::unauthorized())?;
                if document_hash(&signed).map_err(|_|ApiError::invalid())?!=grant.client_hello_hash.0||client.client_ecdh!=grant.client_ecdh||client.client_nonce!=grant.client_nonce{return Err(ApiError::conflict());}
                let receipt=json!({"approved":true,"approval_protocol":1,"pair_id":grant.id,"grant_hash":document_hash(&document).map_err(|_|ApiError::invalid())?,"invite_id":invite_id,"client_public_key":client_public_key});
                if state=="approved"{let previous:Option<String>=tx.query_row("SELECT grant_hash FROM pairs WHERE id=?1 AND revoked=0",[grant.id.to_string()],|r|r.get(0)).optional().map_err(sql)?;if previous.as_deref()==Some(document_hash(&document).map_err(|_|ApiError::invalid())?.as_str()){return Ok(receipt);}return Err(ApiError::conflict());}
                if state!="pending"{return Err(ApiError::conflict());}
                let total:u64=tx.query_row("SELECT count(*) FROM pairs",[],|r|r.get(0)).map_err(sql)?;let owned:u64=tx.query_row("SELECT count(*) FROM pairs WHERE (host=?1 OR client=?1) AND revoked=0",[sender],|r|r.get(0)).map_err(sql)?;
                if total>=10000||owned>=32{return Err(ApiError::quota());}
                tx.execute("INSERT INTO pairs(id,host,client,invite,expires,grant_hash,document) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![grant.id.to_string(),sender,client_public_key,invite_id.to_string(),grant.expires_at_ms,document_hash(&document).map_err(|_|ApiError::invalid())?,text(&document)?]).map_err(|_|ApiError::conflict())?;
                tx.execute("UPDATE joins SET state='approved' WHERE invite=?1 AND client=?2",params![invite_id.to_string(),client_public_key]).map_err(sql)?;Ok(receipt)
            }
            RelayOperation::DeclinePair{invite_id,client_public_key}=>{invite_row(tx,&invite_id.to_string(),Some(sender))?;tx.execute("UPDATE joins SET state='declined' WHERE invite=?1 AND client=?2 AND state='pending'",params![invite_id.to_string(),client_public_key]).map_err(sql)?;Ok(json!({"declined":true}))}
            RelayOperation::CloseInvite{invite_id}=>{invite_row(tx,&invite_id.to_string(),Some(sender))?;tx.execute("DELETE FROM invites WHERE id=?1",[invite_id.to_string()]).map_err(sql)?;Ok(json!({"closed":true}))}
            RelayOperation::Send{grant:document,message_id,session_id,sequence,expires_at_ms,iv,ciphertext}=>{
                let grant=pair(tx,sender,&document,audience,now)?;let pair_id=grant.id.to_string();
                let previous:Option<(String,String,u64,u64,String,String,u64,String)>=tx.query_row("SELECT pair,session,sequence,expires,iv,ciphertext,cursor,sender FROM messages WHERE message_id=?1",[message_id.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?))).optional().map_err(sql)?;
                if let Some((p,s,q,e,i,c,cursor,author))=previous{if author!=sender||p!=pair_id||s!=session_id.to_string()||q!=sequence||e!=expires_at_ms||i!=iv||c!=ciphertext{return Err(ApiError::conflict());}return Ok(json!({"accepted":true,"cursor":cursor}));}
                let count:u64=tx.query_row("SELECT count(*) FROM messages",[],|r|r.get(0)).map_err(sql)?;let pending:u64=tx.query_row("SELECT count(*) FROM messages WHERE pair=?1",[&pair_id],|r|r.get(0)).map_err(sql)?;if count>=4096||pending>=256{return Err(ApiError::quota());}
                let column=if sender==grant.host_public_key{"host_seq"}else{"client_seq"};let statement=format!("UPDATE pairs SET {column}=?1 WHERE id=?2 AND {column}<?1");
                if tx.execute(&statement,params![sequence,pair_id]).map_err(sql)?!=1{return Err(ApiError::conflict());}
                tx.execute("INSERT INTO messages(pair,sender,message_id,session,sequence,expires,iv,ciphertext) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![pair_id,sender,message_id.to_string(),session_id.to_string(),sequence,expires_at_ms,iv,ciphertext]).map_err(|_|ApiError::conflict())?;Ok(json!({"accepted":true,"cursor":tx.last_insert_rowid()}))
            }
            RelayOperation::Receive{grant:document,after,limit}=>{
                let grant=pair(tx,sender,&document,audience,now)?;
                let mut statement=tx.prepare("SELECT cursor,sender,message_id,session,sequence,expires,iv,ciphertext FROM messages WHERE pair=?1 AND sender!=?2 AND cursor>?3 AND expires>?4 ORDER BY cursor LIMIT ?5").map_err(sql)?;
                let rows=statement.query_map(params![grant.id.to_string(),sender,after,now,limit],|r|Ok((r.get::<_,u64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,u64>(4)?,r.get::<_,u64>(5)?,r.get::<_,String>(6)?,r.get::<_,String>(7)?))).map_err(sql)?;
                let mut packets=Vec::new();for row in rows{let(cursor,sender_public_key,message,session,sequence,expires_at_ms,iv,ciphertext)=row.map_err(sql)?;packets.push(RelayPacket{cursor,sender_public_key,message_id:message.parse().map_err(sql)?,session_id:session.parse().map_err(sql)?,sequence,expires_at_ms,iv,ciphertext});}Ok(json!({"packets":packets}))
            }
            RelayOperation::ConfirmPair{grant:document}=>{let grant=pair(tx,sender,&document,audience,now)?;
                Ok(json!({"pair_confirmation_protocol":1,"approved":true,"pair_id":grant.id,"grant_hash":document_hash(&document).map_err(|_|ApiError::invalid())?,"expires_at_ms":grant.expires_at_ms}))}
            RelayOperation::IceCredentials{grant:document}=>{let grant=pair(tx,sender,&document,audience,now)?;Ok(json!({"pair_id":grant.id,"grant_hash":document_hash(&document).map_err(|_|ApiError::invalid())?,"expires_at_ms":grant.expires_at_ms.min(now+600000)}))}
            RelayOperation::RevokePair{grant:document}=>{
                let grant=verify_grant_for_retirement(&document,audience,now).map_err(|_|ApiError::unauthorized())?;
                if sender!=grant.host_public_key&&sender!=grant.client_public_key{return Err(ApiError::unauthorized());}
                let hash=document_hash(&document).map_err(|_|ApiError::invalid())?;
                let known:Option<String>=tx.query_row("SELECT grant_hash FROM pairs WHERE id=?1",[grant.id.to_string()],|row|row.get(0)).optional().map_err(sql)?;
                let reason=match known{
                    Some(known)=>{
                        if known!=hash{return Err(ApiError::conflict());}
                        if tx.execute("UPDATE pairs SET revoked=1 WHERE id=?1 AND grant_hash=?2",params![grant.id.to_string(),hash]).map_err(sql)?!=1{return Err(ApiError::conflict());}
                        tx.execute("DELETE FROM messages WHERE pair=?1",[grant.id.to_string()]).map_err(sql)?;"revoked"
                    },
                    // execute() already removed expired rows in this same
                    // transaction. Approval still requires a LIVE grant, so
                    // this exact expired grant cannot be activated afterward.
                    None if grant.expires_at_ms<=now=>"expired",
                    None=>return Err(ApiError::not_found()),
                };
                Ok(json!({"revoked":true,"retirement_protocol":1,"pair_id":grant.id,"grant_hash":hash,"reason":reason}))
            }
        }
    }
}
fn invite_row(tx:&Transaction<'_>,id:&str,host:Option<&str>)->ApiResult<(String,String)>{let row:Option<(String,String)>=tx.query_row("SELECT host,secret_hash FROM invites WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(sql)?;let Some(row)=row else{return Err(ApiError::not_found());};if host.is_some_and(|value|value!=row.0){return Err(ApiError::unauthorized());}Ok(row)}
fn pair(tx:&Transaction<'_>,sender:&str,document:&SignedDocument,audience:&str,now:u64)->ApiResult<PairGrant>{
    let grant=verify_grant(document,audience,now).map_err(|_|ApiError::unauthorized())?;
    if sender!=grant.host_public_key&&sender!=grant.client_public_key{return Err(ApiError::unauthorized());}
    let known:Option<String>=tx.query_row("SELECT grant_hash FROM pairs WHERE id=?1 AND revoked=0 AND expires>?2",params![grant.id.to_string(),now],|r|r.get(0)).optional().map_err(sql)?;
    if known.as_deref()!=Some(document_hash(document).map_err(|_|ApiError::invalid())?.as_str()){return Err(ApiError::unauthorized());}Ok(grant)
}
