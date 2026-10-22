//! Explicit recovery requests are local per-generation preferences. Their CAS
//! records allow cancellation while a transport worker owns the account lease.
//! No completed receipt is deleted, and no global list cursor is rewound.
use crate::{CredentialStore,SyncClient,client::load_configuration};
use omniplay_contracts::{CloudSaveRecoveryState as State,CloudSaveRecoveryStatus,ContentHash,ErrorCode,PlatformError,Result,SaveMetadata,UserSettings,save_origin_hash};
use omniplay_core::{Database,now_ms,util::storage_error};
use rusqlite::{Connection,OptionalExtension,params};
use serde::{Deserialize,Serialize};
use std::sync::atomic::{AtomicBool,Ordering};
use uuid::Uuid;

const PREFIX:&str="sync_save_recovery_v1:";
const GLOB:&str="sync_save_recovery_v1:*";
const LIMIT:usize=64;
const MAX_RECORD:usize=4096;
#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry{
    schema_version:u32,request_id:Uuid,account_id:String,game_id:Uuid,
    local_metadata_hash:ContentHash,canonical_hash:Option<ContentHash>,status:CloudSaveRecoveryStatus,
}
fn key(id:Uuid)->String{format!("{PREFIX}{id}")}
fn corrupt()->PlatformError{PlatformError::new(ErrorCode::CorruptData,"The local cloud recovery request needs recovery")}
fn cancelled()->PlatformError{PlatformError::new(ErrorCode::Cancelled,"This cloud recovery request was cancelled or replaced")}
fn parse(raw:&str)->Result<Entry>{
    if raw.len()>MAX_RECORD{return Err(corrupt());}let entry:Entry=serde_json::from_str(raw).map_err(|_|corrupt())?;
    if entry.schema_version!=1||entry.request_id.is_nil()||entry.account_id.is_empty()||entry.account_id.len()>128||entry.status.requested_at_ms>entry.status.updated_at_ms||entry.status.updated_at_ms>9_007_199_254_740_991{return Err(corrupt());}
    ContentHash::parse(entry.local_metadata_hash.0.clone())?;if let Some(hash)=&entry.canonical_hash{ContentHash::parse(hash.0.clone())?;}Ok(entry)
}
fn encode(value:&Entry)->Result<String>{let raw=serde_json::to_string(value).map_err(storage_error)?;if raw.len()>MAX_RECORD{return Err(corrupt());}Ok(raw)}
fn row(connection:&Connection,id:Uuid)->Result<Option<(String,Entry)>>{
    let raw:Option<String>=connection.query_row("SELECT value FROM preferences WHERE key=?1",[key(id)],|row|row.get(0)).optional().map_err(storage_error)?;
    raw.map(|raw|{let entry=parse(&raw)?;if entry.status.save_id!=id{return Err(corrupt());}Ok((raw,entry))}).transpose()
}
fn entries(connection:&Connection,account:&str)->Result<Vec<Entry>>{
    let mut query=connection.prepare("SELECT key,value FROM preferences WHERE key GLOB ?1 ORDER BY key LIMIT 65").map_err(storage_error)?;
    let values=query.query_map([GLOB],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?))).map_err(storage_error)?;
    let mut result=Vec::new();let mut count=0;
    for value in values{let(name,raw)=value.map_err(storage_error)?;count+=1;if count>LIMIT{return Err(corrupt());}let entry=parse(&raw)?;if name!=key(entry.status.save_id){return Err(corrupt());}if entry.account_id==account{result.push(entry);}}
    result.sort_by_key(|entry|(entry.status.requested_at_ms,entry.status.save_id));Ok(result)
}
fn cas(connection:&Connection,raw:&str,value:&Entry)->Result<()>{
    if connection.execute("UPDATE preferences SET value=?1 WHERE key=?2 AND value=?3",params![encode(value)?,key(value.status.save_id),raw]).map_err(storage_error)?!=1{return Err(cancelled());}Ok(())
}
fn account(database:&Database)->Result<String>{
    let configuration=load_configuration(database)?.ok_or_else(||PlatformError::new(ErrorCode::NotFound,"Connect the account holding this saved generation first"))?;
    if !configuration.registered||!database.settings()?.cloud_sync_opt_in{return Err(PlatformError::new(ErrorCode::PermissionDenied,"Enable synchronization for the connected account first"));}Ok(configuration.account_id)
}
pub fn request(database:&Database,id:Uuid,store:&dyn CredentialStore)->Result<CloudSaveRecoveryStatus>{
    store.check_access()?;let _lease=crate::transfers::profile_lease(database)?;
    let tx=database.connection.unchecked_transaction().map_err(storage_error)?;let account_id=account(database)?;
    if omniplay_core::save_maintenance::locally_removed(database,id)?{return Err(PlatformError::new(ErrorCode::NotFound,"This saved generation was intentionally removed locally"));}
    let(local,origin)=omniplay_core::save_origins::inspect(&tx,id)?;
    let mut existing=entries(&tx,&account_id)?;
    let total:u64=tx.query_row("SELECT count(*) FROM preferences WHERE key GLOB ?1",[GLOB],|row|row.get(0)).map_err(storage_error)?;
    if row(&tx,id)?.is_none()&&total>=LIMIT as u64{
        let expired=existing.iter().position(|entry|entry.status.state.terminal()).ok_or_else(||PlatformError::new(ErrorCode::ResourceLimit,"Finish or cancel an existing cloud recovery request first"))?;
        let old=existing.remove(expired);tx.execute("DELETE FROM preferences WHERE key=?1",[key(old.status.save_id)]).map_err(storage_error)?;
    }
    let at=now_ms();let entry=Entry{schema_version:1,request_id:Uuid::new_v4(),account_id,game_id:local.game_id,
        local_metadata_hash:save_origin_hash(&local)?,canonical_hash:origin.canonical_hash,
        status:CloudSaveRecoveryStatus{save_id:id,state:State::Queued,requested_at_ms:at,updated_at_ms:at,error:None}};
    store.check_access()?;tx.execute("INSERT INTO preferences(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key(id),encode(&entry)?]).map_err(storage_error)?;
    tx.commit().map_err(storage_error)?;Ok(entry.status)
}
pub fn list(database:&Database,game_id:Uuid)->Result<Vec<CloudSaveRecoveryStatus>>{
    let Some(configuration)=load_configuration(database)?else{return Ok(Vec::new());};
    Ok(entries(&database.connection,&configuration.account_id)?.into_iter().filter(|entry|entry.game_id==game_id).map(|entry|entry.status).collect())
}
pub fn cancel(database:&Database,id:Uuid,store:&dyn CredentialStore)->Result<CloudSaveRecoveryStatus>{
    store.check_access()?;let tx=database.connection.unchecked_transaction().map_err(storage_error)?;
    let account_id=load_configuration(database)?.ok_or_else(||PlatformError::new(ErrorCode::NotFound,"The synchronization account is disconnected"))?.account_id;
    let(raw,mut entry)=row(&tx,id)?.ok_or_else(||PlatformError::new(ErrorCode::NotFound,"Cloud recovery request was not found"))?;
    if entry.account_id!=account_id{return Err(cancelled());}
    if !entry.status.state.terminal(){entry.status.state=State::Cancelled;entry.status.updated_at_ms=now_ms().max(entry.status.updated_at_ms);entry.status.error=None;cas(&tx,&raw,&entry)?;}
    tx.commit().map_err(storage_error)?;Ok(entry.status)
}
pub(crate) fn clear(database:&Database)->Result<()>{database.connection.execute("DELETE FROM preferences WHERE key GLOB ?1",[GLOB]).map_err(storage_error)?;Ok(())}
fn current(connection:&Connection,expected:&Entry)->Result<(String,Entry)>{
    let(raw,entry)=row(connection,expected.status.save_id)?.ok_or_else(cancelled)?;
    if entry.request_id!=expected.request_id||entry.account_id!=expected.account_id||entry.status.state.terminal(){return Err(cancelled());}Ok((raw,entry))
}
fn guard(connection:&Connection,entry:&Entry,cancel:&AtomicBool,store:&dyn CredentialStore)->Result<()>{
    if cancel.load(Ordering::Acquire){return Err(cancelled());}store.check_access()?;
    let raw:Option<String>=connection.query_row("SELECT value FROM preferences WHERE key='settings'",[],|row|row.get(0)).optional().map_err(storage_error)?;
    let settings:UserSettings=match raw{Some(raw)if raw.len()<=64*1024=>serde_json::from_str(&raw).map_err(|_|corrupt())?,Some(_)=>return Err(corrupt()),None=>UserSettings::default()};settings.validate()?;
    if !settings.cloud_sync_opt_in{return Err(cancelled());}current(connection,entry)?;Ok(())
}
fn state(connection:&Connection,entry:&Entry,value:State,error:Option<PlatformError>)->Result<()>{let(raw,mut entry)=current(connection,entry)?;entry.status.state=value;entry.status.updated_at_ms=now_ms().max(entry.status.updated_at_ms);entry.status.error=error;cas(connection,&raw,&entry)}
fn complete(connection:&Connection,entry:&Entry,repaired:bool)->Result<()>{state(connection,entry,if repaired{State::Recovered}else{State::AlreadyIntact},None)}
fn local(database:&Database,entry:&Entry)->Result<SaveMetadata>{
    if omniplay_core::save_maintenance::locally_removed(database,entry.status.save_id)?{return Err(PlatformError::new(ErrorCode::NotFound,"This saved generation was removed locally"));}
    let(local,origin)=omniplay_core::save_origins::inspect(&database.connection,entry.status.save_id)?;
    if local.game_id!=entry.game_id||save_origin_hash(&local)?!=entry.local_metadata_hash{return Err(PlatformError::new(ErrorCode::Conflict,"This saved generation changed after cloud recovery was requested"));}
    if let Some(hash)=&entry.canonical_hash{if origin.canonical_hash.as_ref()!=Some(hash){return Err(corrupt());}}Ok(local)
}
fn failure(code:ErrorCode)->PlatformError{
    let message=match code{ErrorCode::NotFound=>"This account has no available copy of that saved generation",ErrorCode::Conflict|ErrorCode::ContentMismatch=>"The recovery source does not match this saved generation",ErrorCode::CorruptData=>"The recovery source or local metadata failed integrity checks",ErrorCode::PermissionDenied|ErrorCode::PermissionExpired=>"Review device access before retrying cloud recovery",ErrorCode::ResourceLimit=>"Cloud recovery needs free storage or an available service quota",_=>"Cloud recovery could not finish; local saved progress remains unchanged"};
    PlatformError::new(code,message)
}
/// At most eight explicit requests per run. A transport that becomes blocked
/// after download retains its current partial file and yields the rest of this
/// run; the next opted-in foreground/background job resumes that same request.
pub(crate) fn process(database:&mut Database,client:&SyncClient,summary:&mut crate::RunSummary,cancel:&AtomicBool,store:&dyn CredentialStore,
    on_progress:&impl Fn(&crate::RunSummary))->Result<bool>{
    let all=entries(&database.connection,&client.configuration.account_id)?;
    for entry in all.iter().filter(|entry|entry.status.state.terminal()){if crate::transfers::discard_download(database,entry.status.save_id).is_err(){summary.cloud_recovery_cleanup_pending+=1;}}
    summary.cloud_recovery_pending=all.iter().filter(|entry|!entry.status.state.terminal()).count()as u32;
    let resume=crate::transfers::recovery_download_id(database,&client.configuration.account_id)?;
    let mut pending=all.into_iter().filter(|entry|!entry.status.state.terminal()).collect::<Vec<_>>();
    pending.sort_by_key(|entry|(resume!=Some(entry.status.save_id),entry.status.updated_at_ms,entry.status.requested_at_ms,entry.status.save_id));
    pending.truncate(8);
    for entry in pending{
        match guard(&database.connection,&entry,cancel,store){Ok(())=>{},Err(error)if error.code==ErrorCode::Cancelled&&!cancel.load(Ordering::Acquire)=>continue,Err(error)=>return Err(error)}
        let outcome=(||->Result<bool>{
            let metadata=local(database,&entry)?;
            // A healthy request is completed in the same guarded transaction
            // that rechecks health; no remote download is needed for that case.
            if omniplay_core::save_payloads::intact_with_hooks(database,&metadata,|connection|guard(connection,&entry,cancel,store),|connection|complete(connection,&entry,false))?{return Ok(false);}
            let active:bool=database.connection.query_row("SELECT EXISTS(SELECT 1 FROM sessions WHERE game_id=?1 AND state NOT IN ('stopped','failed'))",[metadata.game_id.to_string()],|row|row.get(0)).map_err(storage_error)?;
            if active{return Err(PlatformError::new(ErrorCode::Busy,"Close the game before cloud recovery"));}
            state(&database.connection,&entry,State::Downloading,None)?;
            let baseline=summary.clone();let mut encrypted=crate::transfers::download_checked(database,client,metadata.id,cancel,store,&|direction,completed_bytes,total_bytes|{
                let mut value=baseline.clone();value.transfer=Some(crate::TransferSummary{direction:direction.into(),completed_bytes,total_bytes});on_progress(&value);
            },&||guard(&database.connection,&entry,cancel,store))?;
            guard(&database.connection,&entry,cancel,store)?;
            let (remote,payload)=client.cipher.decrypt_blob_reader_checked(&encrypted.header,&mut encrypted.file,&||guard(&database.connection,&entry,cancel,store))?;
            guard(&database.connection,&entry,cancel,store)?;
            if remote.id!=metadata.id{return Err(failure(ErrorCode::ContentMismatch));}
            let result=omniplay_core::save_payloads::repair_with_hooks(database,&metadata,&payload,Some(remote),
                |connection|guard(connection,&entry,cancel,store),|connection,repaired|complete(connection,&entry,repaired))?;
            drop(payload);if crate::transfers::finish_download(database,encrypted).is_err(){summary.cloud_recovery_cleanup_pending+=1;}Ok(result.repaired)
        })();
        match outcome{
            Ok(repaired)=>{summary.cloud_recovered+=u32::from(repaired);summary.cloud_recovery_intact+=u32::from(!repaired);summary.cloud_recovery_pending=summary.cloud_recovery_pending.saturating_sub(1);},
            Err(error)if error.code==ErrorCode::Busy=>{
                let _=state(&database.connection,&entry,State::WaitingForGameClose,None);summary.recovery_deferred+=1;on_progress(summary);
                if crate::transfers::has_recovery_download(database,&entry.account_id,entry.status.save_id)?{
                    summary.cloud_recovery_pending=entries(&database.connection,&entry.account_id)?.iter().filter(|entry|!entry.status.state.terminal()).count()as u32;
                    return Ok(true);
                }
            },
            Err(error)if error.code==ErrorCode::Cancelled=>{if cancel.load(Ordering::Acquire){return Err(error);}summary.cloud_recovery_pending=summary.cloud_recovery_pending.saturating_sub(1);},
            Err(error)if matches!(error.code,ErrorCode::NetworkUnavailable|ErrorCode::DeadlineExceeded)=>{let _=state(&database.connection,&entry,State::Queued,Some(failure(error.code)));return Err(error);},
            Err(error)=>{let _=state(&database.connection,&entry,State::Failed,Some(failure(error.code)));crate::transfers::discard_download(database,entry.status.save_id)?;summary.cloud_recovery_failed+=1;summary.cloud_recovery_pending=summary.cloud_recovery_pending.saturating_sub(1);},
        }
        on_progress(summary);
    }
    summary.cloud_recovery_pending=entries(&database.connection,&client.configuration.account_id)?.iter().filter(|entry|!entry.status.state.terminal()).count()as u32;
    Ok(false)
}
