use super::{GuardedStore,SyncLifecycle,unavailable,stopped};
use omniplay_contracts::{ErrorCode,PlatformError,Result,SyncAccountAction};
use omniplay_core::{Database,now_ms};
use omniplay_sync_client::CredentialStore;
use serde::Serialize;
use serde_json::{Value,json};
use std::{path::Path,sync::{Arc,Mutex,Condvar,atomic::{AtomicBool,Ordering}},time::{Duration,Instant}};
use uuid::Uuid;
use zeroize::{Zeroize,Zeroizing};

const RESULT_TTL:Duration=Duration::from_secs(120);
const MAX_RESULT:usize=256*1024;
#[derive(Clone,Serialize)]pub(crate) struct ActionSnapshot{pub id:Uuid,pub kind:String,pub state:String,pub started_at_ms:u64,pub finished_at_ms:Option<u64>,pub result_available:bool,pub result_expired:bool,pub stop_requested:bool,pub error:Option<PlatformError>}
struct State{snapshot:ActionSnapshot,result:Option<Zeroizing<Vec<u8>>>,completed:Option<Instant>,abandoned:bool}
struct Shared{state:Mutex<State>,wake:Condvar,alive:AtomicBool}
struct Job{owner:Uuid,shared:Arc<Shared>,cancel:Arc<AtomicBool>}
struct Live(Arc<Shared>);
impl Drop for Live{fn drop(&mut self){self.0.alive.store(false,Ordering::Release);self.0.wake.notify_all();}}
#[derive(Default)]pub(super) struct Actions{current:Option<Job>}

struct SecretAction(SyncAccountAction);
impl Drop for SecretAction{fn drop(&mut self){match &mut self.0{
    SyncAccountAction::Register{recovery_key,registration_token}=>{recovery_key.zeroize();registration_token.zeroize();},
    SyncAccountAction::Recover{recovery_key,..}|SyncAccountAction::Renew{recovery_key}|SyncAccountAction::RevokeDevice{recovery_key,..}|SyncAccountAction::DeleteAccount{recovery_key}=>recovery_key.zeroize(),_=>{}
}}}
impl SecretAction{
    fn validate(&self)->Result<()>{
        let bounded=|value:&str,max:usize|!value.is_empty()&&value.len()<=max;
        let valid=match &self.0{SyncAccountAction::Create{audience}=>bounded(audience,512),SyncAccountAction::Register{recovery_key,registration_token}=>bounded(recovery_key,256)&&bounded(registration_token,256),SyncAccountAction::Recover{audience,recovery_key}=>bounded(audience,512)&&bounded(recovery_key,256),SyncAccountAction::Renew{recovery_key}|SyncAccountAction::RevokeDevice{recovery_key,..}|SyncAccountAction::DeleteAccount{recovery_key}=>bounded(recovery_key,256),_=>true};
        if !valid{return Err(PlatformError::new(ErrorCode::InvalidRequest,"The account action exceeds its input bounds"));}Ok(())
    }
    fn execute(&self,database:&Database,profile:Uuid,store:&dyn CredentialStore)->Result<Zeroizing<Vec<u8>>>{
        use omniplay_sync_client as client;store.check_access()?;
        let result=match &self.0{
            SyncAccountAction::Create{audience}=>{let mut value=client::create_account_with_store(database,profile,audience,store)?;let raw=serde_json::to_vec(&value);value.recovery_key.zeroize();return encoded(raw);},
            SyncAccountAction::Register{recovery_key,registration_token}=>{client::register_account_with_store(database,recovery_key,registration_token,store)?;json!({"registered":true})},
            SyncAccountAction::Recover{audience,recovery_key}=>serde_json::to_value(client::recover_account_with_store(database,profile,audience,recovery_key,store)?).map_err(|_|unavailable())?,
            SyncAccountAction::Renew{recovery_key}=>{client::renew_certificate_with_store(database,recovery_key,store)?;json!({"renewed":true})},
            SyncAccountAction::Devices=>serde_json::to_value(client::SyncClient::open_with_store(database,store)?.devices()?).map_err(|_|unavailable())?,
            SyncAccountAction::RevokeDevice{recovery_key,device_id}=>{client::revoke_device_with_store(database,recovery_key,*device_id,store)?;json!({"revoked":true})},
            SyncAccountAction::DeleteAccount{recovery_key}=>{client::delete_account_with_store(database,recovery_key,store)?;json!({"deleted":true})},
            SyncAccountAction::Disconnect=>{client::disconnect_with_store(database,store)?;json!({"disconnected":true})},
        };encoded(serde_json::to_vec(&result))
    }
}
fn encoded(raw:std::result::Result<Vec<u8>,serde_json::Error>)->Result<Zeroizing<Vec<u8>>>{let raw=Zeroizing::new(raw.map_err(|_|unavailable())?);if raw.len()>MAX_RESULT{return Err(PlatformError::new(ErrorCode::ResourceLimit,"The account result exceeds its bound"));}Ok(raw)}

impl Actions{
    pub fn ensure_clear(&self)->Result<()>{if let Some(job)=&self.current{let mut state=job.shared.state.lock().map_err(|_|unavailable())?;expire(&mut state);job.shared.wake.notify_all();if job.shared.alive.load(Ordering::Acquire)||state.result.is_some(){return Err(PlatformError::new(ErrorCode::Busy,"Collect or dismiss the current account result and wait for its worker to finish"));}}Ok(())}
    pub fn start(&mut self,root:&Path,profile:Uuid,owner:Uuid,provider:Arc<dyn CredentialStore>,lifecycle:&Arc<SyncLifecycle>,action:SyncAccountAction)->Result<ActionSnapshot>{
        let action=SecretAction(action);action.validate()?;self.ensure_clear()?;let lease=lifecycle.begin()?;
        let snapshot=ActionSnapshot{id:Uuid::new_v4(),kind:action.0.kind().into(),state:"running".into(),started_at_ms:now_ms(),finished_at_ms:None,result_available:false,result_expired:false,stop_requested:false,error:None};
        let shared=Arc::new(Shared{state:Mutex::new(State{snapshot:snapshot.clone(),result:None,completed:None,abandoned:false}),wake:Condvar::new(),alive:AtomicBool::new(true)});let worker=shared.clone();let cancel=lease.cancel.clone();let stop=cancel.clone();let root=root.to_owned();let lifecycle=lifecycle.clone();
        std::thread::Builder::new().name("omniplay-account-action".into()).spawn(move||{
            let _live=Live(worker.clone());let store=GuardedStore{inner:provider,lifecycle,cancel:stop.clone()};
            let result=(||{store.check_access()?;let database=Database::open(&root)?;action.execute(&database,profile,&store)})();
            drop(action);drop(store);
            {
                let mut state=worker.state.lock().unwrap_or_else(|error|error.into_inner());state.completed=Some(Instant::now());state.snapshot.finished_at_ms=Some(now_ms());state.snapshot.stop_requested=stop.load(Ordering::Acquire);
                if state.abandoned{state.snapshot.state="discarded".into();drop(result);}else{match result{Ok(raw)=>{state.result=Some(raw);state.snapshot.result_available=true;state.snapshot.state="complete".into();},Err(error)=>{let cancelled=error.code==ErrorCode::Cancelled||stop.load(Ordering::Acquire);state.snapshot.state=if cancelled{"cancelled"}else{"failed"}.into();state.snapshot.error=Some(if cancelled{stopped()}else{error});}}}
            }
            // No database, credential owner or operation lease is held while a
            // completed result waits for collection/expiry. This same worker
            // supplies the deadline; another expiry thread is never spawned.
            drop(lease);wait_for_result(&worker);
        }).map_err(|_|PlatformError::new(ErrorCode::ResourceLimit,"The native account worker could not start"))?;
        self.current=Some(Job{owner,shared,cancel});Ok(snapshot)
    }
    fn job(&self,owner:Uuid,id:Uuid)->Result<&Job>{self.current.as_ref().filter(|job|job.owner==owner&&job.shared.state.lock().map(|state|state.snapshot.id==id).unwrap_or(false)).ok_or_else(not_found)}
    pub fn status(&self,owner:Uuid,id:Uuid)->Result<ActionSnapshot>{let job=self.job(owner,id)?;let mut state=job.shared.state.lock().map_err(|_|unavailable())?;expire(&mut state);job.shared.wake.notify_all();let mut value=state.snapshot.clone();value.stop_requested=job.cancel.load(Ordering::Acquire);Ok(value)}
    pub fn current(&self,owner:Uuid)->Option<ActionSnapshot>{let job=self.current.as_ref().filter(|job|job.owner==owner)?;let value=job.shared.state.lock().ok()?.snapshot.clone();self.status(owner,value.id).ok()}
    pub fn collect(&self,owner:Uuid,id:Uuid)->Result<Value>{
        let job=self.job(owner,id)?;let raw={let mut state=job.shared.state.lock().map_err(|_|unavailable())?;expire(&mut state);job.shared.wake.notify_all();if state.snapshot.state=="running"{return Err(PlatformError::new(ErrorCode::Busy,"The account operation is still running"));}if let Some(error)=&state.snapshot.error{return Err(error.clone());}let raw=state.result.take().ok_or_else(||PlatformError::new(ErrorCode::NotFound,"The account result was collected, dismissed or expired"))?;state.snapshot.result_available=false;job.shared.wake.notify_all();raw};
        serde_json::from_slice(&raw).map_err(|_|unavailable())
    }
    pub fn cancel(&self,owner:Uuid,id:Uuid)->Result<ActionSnapshot>{let job=self.job(owner,id)?;job.cancel.store(true,Ordering::Release);self.status(owner,id)}
    pub fn discard(&self,owner:Uuid,id:Uuid)->Result<ActionSnapshot>{let job=self.job(owner,id)?;{let mut state=job.shared.state.lock().map_err(|_|unavailable())?;if state.snapshot.state=="running"{return Err(PlatformError::new(ErrorCode::Busy,"Cancel and wait for the account operation before dismissing it"));}state.result.take();state.abandoned=true;state.snapshot.result_available=false;state.snapshot.state="discarded".into();job.shared.wake.notify_all();}self.status(owner,id)}
    pub fn abandon(&self,owner:Uuid){if let Some(job)=self.current.as_ref().filter(|job|job.owner==owner){job.cancel.store(true,Ordering::Release);let mut state=job.shared.state.lock().unwrap_or_else(|error|error.into_inner());state.abandoned=true;state.result.take();state.snapshot.result_available=false;job.shared.wake.notify_all();}}
}
impl Drop for Actions{fn drop(&mut self){if let Some(job)=&self.current{self.abandon(job.owner);}}}
fn wait_for_result(shared:&Shared){
    let mut state=shared.state.lock().unwrap_or_else(|error|error.into_inner());
    while state.result.is_some()&&!state.abandoned{
        let remaining=state.completed.and_then(|at|RESULT_TTL.checked_sub(at.elapsed())).unwrap_or(Duration::ZERO);
        if remaining.is_zero(){expire(&mut state);break;}
        let(next,_)=shared.wake.wait_timeout(state,remaining).unwrap_or_else(|error|error.into_inner());state=next;
    }
}
fn expire(state:&mut State){if state.result.is_some()&&state.completed.is_some_and(|at|at.elapsed()>=RESULT_TTL){state.result.take();state.snapshot.result_available=false;state.snapshot.result_expired=true;state.snapshot.state="expired".into();}}
fn not_found()->PlatformError{PlatformError::new(ErrorCode::NotFound,"The native account action was not found")}
