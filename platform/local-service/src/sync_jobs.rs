use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_core::{Database,now_ms};
use omniplay_sync_client::{CredentialStore,RunSummary};
use serde::Serialize;
use std::{path::Path,sync::{Arc,Mutex,Condvar,atomic::{AtomicBool,Ordering}},time::{Duration,Instant}};
use uuid::Uuid;
mod actions;
pub(crate) use actions::ActionSnapshot;

#[derive(Clone,Copy)]enum Mode{Foreground,Background(Instant)}
struct Running{cancel:Arc<AtomicBool>,mode:Mode}
#[derive(Default)]struct Activity{foreground:bool,current:Option<Running>}
#[derive(Default)]pub struct SyncLifecycle{activity:Mutex<Activity>}
impl SyncLifecycle{
    pub fn set_foreground(&self,value:bool){if let Ok(mut state)=self.activity.lock(){state.foreground=value;if let Some(run)=&state.current{if matches!((run.mode,value),(Mode::Foreground,false)|(Mode::Background(_),true)){run.cancel.store(true,Ordering::Release);}}}}
    pub fn cancel(&self){if let Ok(state)=self.activity.lock(){if let Some(run)=&state.current{run.cancel.store(true,Ordering::Release);}}}
    pub fn cancel_background(&self){if let Ok(state)=self.activity.lock(){if let Some(run)=&state.current{if matches!(run.mode,Mode::Background(_)){run.cancel.store(true,Ordering::Release);}}}}
    pub fn busy(&self)->bool{self.activity.lock().map(|state|state.current.is_some()).unwrap_or(true)}
    pub fn foreground(&self)->bool{self.activity.lock().map(|state|state.foreground).unwrap_or(false)}
    fn begin(self:&Arc<Self>)->Result<Lease>{
        self.begin_mode(Mode::Foreground)
    }
    fn begin_mode(self:&Arc<Self>,mode:Mode)->Result<Lease>{
        let mut state=self.activity.lock().map_err(|_|unavailable())?;
        if state.foreground!=matches!(mode,Mode::Foreground){return Err(PlatformError::new(ErrorCode::PermissionDenied,"The synchronization lifecycle changed; retry in the appropriate app state"));}
        if state.current.is_some(){return Err(PlatformError::new(ErrorCode::Busy,"Wait for the current synchronization operation to finish"));}
        let cancel=Arc::new(AtomicBool::new(false));
        let wake=if let Mode::Background(deadline)=mode{
            let wake=Arc::new((Mutex::new(false),Condvar::new()));let timer=wake.clone();let stop=cancel.clone();
            std::thread::Builder::new().name("omniplay-sync-deadline".into()).spawn(move||{
                let (lock,changed)=&*timer;
                if let Ok(finished)=lock.lock(){
                    match changed.wait_timeout_while(finished,deadline.saturating_duration_since(Instant::now()),|done|! *done){Ok((done,_)) if *done=>{},_=>stop.store(true,Ordering::Release)}
                }else{stop.store(true,Ordering::Release);}
            }).map_err(|_|PlatformError::new(ErrorCode::ResourceLimit,"The synchronization deadline could not be armed"))?;Some(wake)
        }else{None};
        state.current=Some(Running{cancel:cancel.clone(),mode});Ok(Lease{lifecycle:self.clone(),cancel,wake,deadline:match mode{Mode::Background(value)=>Some(value),Mode::Foreground=>None}})
    }
    fn check(&self,cancel:&Arc<AtomicBool>)->Result<()>{
        let state=self.activity.lock().map_err(|_|unavailable())?;
        let allowed=state.current.as_ref().is_some_and(|run|Arc::ptr_eq(&run.cancel,cancel)&&match run.mode{Mode::Foreground=>state.foreground,Mode::Background(deadline)=>!state.foreground&&Instant::now()<deadline});
        if !allowed||cancel.load(Ordering::Acquire){cancel.store(true,Ordering::Release);return Err(stopped());}Ok(())
    }
}
struct Lease{lifecycle:Arc<SyncLifecycle>,cancel:Arc<AtomicBool>,wake:Option<Arc<(Mutex<bool>,Condvar)>>,deadline:Option<Instant>}
impl Drop for Lease{fn drop(&mut self){if let Some(wake)=&self.wake{if let Ok(mut done)=wake.0.lock(){*done=true;wake.1.notify_all();}}if let Ok(mut state)=self.lifecycle.activity.lock(){if state.current.as_ref().is_some_and(|value|Arc::ptr_eq(&value.cancel,&self.cancel)){state.current=None;}}}}
struct GuardedStore{inner:Arc<dyn CredentialStore>,lifecycle:Arc<SyncLifecycle>,cancel:Arc<AtomicBool>}
impl GuardedStore{fn check(&self)->Result<()>{self.lifecycle.check(&self.cancel)}}
impl CredentialStore for GuardedStore{
    fn check_access(&self)->Result<()>{self.check()?;self.inner.check_access()}
    fn read(&self,profile:Uuid)->Result<zeroize::Zeroizing<Vec<u8>>>{self.check()?;let value=self.inner.read(profile)?;self.check()?;self.inner.check_access()?;Ok(value)}
    fn write(&self,profile:Uuid,secret:&[u8])->Result<()>{self.check()?;self.inner.write(profile,secret)}
    fn remove(&self,profile:Uuid)->Result<()>{self.check()?;self.inner.remove(profile)}
}
#[derive(Clone,Serialize)]pub struct SyncSnapshot{pub id:Uuid,pub state:String,pub summary:RunSummary,pub started_at_ms:u64,pub finished_at_ms:Option<u64>,pub error:Option<PlatformError>}
#[derive(Clone)]pub struct SyncJobHandle{snapshot:Arc<Mutex<SyncSnapshot>>,cancel:Arc<AtomicBool>}
impl SyncJobHandle{
    pub fn status(&self,id:Uuid)->Result<SyncSnapshot>{let value=self.snapshot.lock().map_err(|_|unavailable())?;if value.id!=id{return Err(not_found());}Ok(value.clone())}
    pub fn cancel(&self,id:Uuid)->Result<SyncSnapshot>{let value=self.status(id)?;self.cancel.store(true,Ordering::Release);Ok(value)}
}
type Job=SyncJobHandle;
pub(crate) struct SyncJobs{lifecycle:Arc<SyncLifecycle>,current:Option<Job>,actions:actions::Actions}
impl SyncJobs{
    pub fn new(lifecycle:Arc<SyncLifecycle>)->Self{Self{lifecycle,current:None,actions:actions::Actions::default()}}
    pub fn busy(&self)->bool{self.lifecycle.busy()}
    pub fn cancel_recovery(&self,database:&Database,id:Uuid,provider:Arc<dyn CredentialStore>)->Result<omniplay_contracts::CloudSaveRecoveryStatus>{
        if !self.lifecycle.foreground(){return Err(PlatformError::new(ErrorCode::PermissionDenied,"Open OmniPlay before changing a recovery request"));}
        omniplay_sync_client::cancel_save_recovery(database,id,provider.as_ref())
    }
    pub fn account<T>(&self,provider:Arc<dyn CredentialStore>,operation:impl FnOnce(&dyn CredentialStore)->Result<T>)->Result<T>{
        self.actions.ensure_clear()?;
        let lease=self.lifecycle.begin()?;let store=GuardedStore{inner:provider,lifecycle:self.lifecycle.clone(),cancel:lease.cancel.clone()};let value=operation(&store);drop(store);drop(lease);value
    }
    pub fn start(&mut self,root:&Path,provider:Arc<dyn CredentialStore>)->Result<SyncSnapshot>{
        self.actions.ensure_clear()?;
        let lease=self.lifecycle.begin()?;
        self.launch(root,provider,lease,false)
    }
    pub fn start_background(&mut self,root:&Path,provider:Arc<dyn CredentialStore>,budget_ms:u32)->Result<(SyncSnapshot,SyncJobHandle)>{
        if !(1_000..=25_000).contains(&budget_ms){return Err(PlatformError::new(ErrorCode::InvalidRequest,"Background synchronization budget is outside policy"));}
        self.actions.ensure_clear()?;
        let lease=self.lifecycle.begin_mode(Mode::Background(Instant::now()+Duration::from_millis(budget_ms.into())))?;
        let snapshot=self.launch(root,provider,lease,true)?;
        let observer=self.current.as_ref().ok_or_else(unavailable)?.clone();Ok((snapshot,observer))
    }
    fn launch(&mut self,root:&Path,provider:Arc<dyn CredentialStore>,lease:Lease,background:bool)->Result<SyncSnapshot>{
        let value=SyncSnapshot{id:Uuid::new_v4(),state:"running".into(),summary:RunSummary::default(),started_at_ms:now_ms(),finished_at_ms:None,error:None};
        let snapshot=Arc::new(Mutex::new(value.clone()));let worker_snapshot=snapshot.clone();let cancel=lease.cancel.clone();let worker_cancel=cancel.clone();let root=root.to_owned();let lifecycle=self.lifecycle.clone();
        std::thread::Builder::new().name("omniplay-mobile-sync".into()).spawn(move||{
            let store=GuardedStore{inner:provider,lifecycle,cancel:worker_cancel.clone()};
            let result=(||{store.check()?;let mut database=Database::open(&root)?;
                if background&&(!database.mobile_background_sync()?||!database.settings()?.cloud_sync_opt_in){return Err(stopped());}
                store.check()?;omniplay_sync_client::synchronize_with_store_deadline(&mut database,&worker_cancel,|summary|{if let Ok(mut value)=worker_snapshot.lock(){value.summary=summary.clone();}},&store,lease.deadline)})();
            drop(store);
            if let Ok(mut value)=worker_snapshot.lock(){value.finished_at_ms=Some(now_ms());match result{Ok(summary)=>{value.summary=summary;value.state=if worker_cancel.load(Ordering::Acquire){"cancelled"}else{"complete"}.into();},Err(error)=>{let cancelled=error.code==ErrorCode::Cancelled||worker_cancel.load(Ordering::Acquire);value.state=if cancelled{"cancelled"}else{"failed"}.into();value.error=Some(if cancelled{stopped()}else{error});}}}
            drop(lease);
        }).map_err(|_|PlatformError::new(ErrorCode::ResourceLimit,"The mobile synchronization worker could not start"))?;
        self.current=Some(Job{snapshot,cancel});Ok(value)
    }
    pub fn status(&self,id:Uuid)->Result<SyncSnapshot>{let job=self.current.as_ref().ok_or_else(not_found)?;let value=job.snapshot.lock().map_err(|_|unavailable())?;if value.id!=id{return Err(not_found());}Ok(value.clone())}
    pub fn current(&self)->Option<SyncSnapshot>{self.current.as_ref().and_then(|job|job.snapshot.lock().ok().map(|value|value.clone()))}
    pub fn cancel(&self,id:Uuid)->Result<SyncSnapshot>{let value=self.status(id)?;if let Some(job)=&self.current{job.cancel.store(true,Ordering::Release);}Ok(value)}
    pub fn cancel_all(&self){self.lifecycle.cancel();}
    pub fn start_action(&mut self,root:&Path,profile:Uuid,owner:Uuid,provider:Arc<dyn CredentialStore>,action:omniplay_contracts::SyncAccountAction)->Result<ActionSnapshot>{self.actions.start(root,profile,owner,provider,&self.lifecycle,action)}
    pub fn action_status(&self,owner:Uuid,id:Uuid)->Result<ActionSnapshot>{self.actions.status(owner,id)}
    pub fn current_action(&self,owner:Uuid)->Option<ActionSnapshot>{self.actions.current(owner)}
    pub fn collect_action(&self,owner:Uuid,id:Uuid)->Result<serde_json::Value>{if !self.lifecycle.foreground(){return Err(PlatformError::new(ErrorCode::PermissionDenied,"Reopen Game Hub before collecting the account result"));}self.actions.collect(owner,id)}
    pub fn cancel_action(&self,owner:Uuid,id:Uuid)->Result<ActionSnapshot>{self.actions.cancel(owner,id)}
    pub fn discard_action(&self,owner:Uuid,id:Uuid)->Result<ActionSnapshot>{self.actions.discard(owner,id)}
    pub fn abandon_actions(&self,owner:Uuid){self.actions.abandon(owner);}
}
impl Drop for SyncJobs{fn drop(&mut self){self.cancel_all();}}
fn stopped()->PlatformError{PlatformError::new(ErrorCode::Cancelled,"Synchronization stopped; local saves remain available")}
fn unavailable()->PlatformError{PlatformError::new(ErrorCode::Internal,"Mobile synchronization state is unavailable")}
fn not_found()->PlatformError{PlatformError::new(ErrorCode::NotFound,"The mobile synchronization job was not found")}
