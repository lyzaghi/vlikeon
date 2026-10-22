use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_updater::{channel::Channel,store::{InstalledRelease,Store}};
use serde::Serialize;
use std::sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}};
use uuid::Uuid;

#[derive(Clone,Serialize)]
pub struct ReleaseSummary {pub id:Uuid,pub version:String,pub status:String,pub security_epoch:u64,pub healthy:bool}
impl From<InstalledRelease> for ReleaseSummary {fn from(value:InstalledRelease)->Self{Self{id:value.id,version:value.version.to_string(),status:value.status,security_epoch:value.security_epoch,healthy:value.healthy}}}
#[derive(Clone,Serialize)]
pub struct Job {pub id:Uuid,pub state:String,pub release:Option<ReleaseSummary>,pub error:Option<PlatformError>}
struct Active {value:Arc<Mutex<Job>>,cancel:Arc<AtomicBool>}
#[derive(Default)]
pub struct UpdateJobs {active:Option<Active>}

pub fn open()->Result<Store> {Store::open(&omniplay_updater::install_root()?,Channel::embedded()?)}
pub fn status()->Result<serde_json::Value> {
    let status=open()?.status()?;
    // Do not send file inventories or install paths through the control channel.
    Ok(serde_json::json!({"configured":status.configured,"channel":status.channel,"platform":status.platform,"minimum_security_epoch":status.minimum_security_epoch,"highest_metadata_version":status.highest_metadata_version,"releases":status.releases.into_iter().take(20).map(ReleaseSummary::from).collect::<Vec<_>>()}))
}
impl Drop for UpdateJobs{fn drop(&mut self){self.cancel_all();}}
impl UpdateJobs {
    pub fn busy(&self)->bool {self.active.as_ref().is_some_and(|a|a.value.lock().map(|v|v.state=="running").unwrap_or(true))}
    pub fn start(&mut self)->Result<Job> {
        if self.busy(){return Err(PlatformError::new(ErrorCode::Busy,"An update download is already running"));}
        Channel::embedded()?.require_enabled()?;
        let job=Job{id:Uuid::new_v4(),state:"running".into(),release:None,error:None};
        let value=Arc::new(Mutex::new(job.clone()));let cancel=Arc::new(AtomicBool::new(false));let shared=Arc::clone(&value);let stop=Arc::clone(&cancel);
        std::thread::Builder::new().name("omniplay-update".into()).spawn(move||{
            let result=(||{let mut store=open()?;store.refresh_roots(&stop)?;let root=store.trusted_root()?;let metadata=omniplay_updater::transport::metadata(&store.channel.metadata_url,&root)?;store.stage_download_cancellable(&metadata,&stop)})();
            if let Ok(mut job)=shared.lock(){match result {Ok(release)=>{job.state="complete".into();job.release=Some(release.into());},Err(error)=>{job.state=if error.code==ErrorCode::Cancelled{"cancelled"}else{"failed"}.into();job.error=Some(error);}}}
        }).map_err(|_|PlatformError::new(ErrorCode::ResourceLimit,"The update worker could not start"))?;
        self.active=Some(Active{value,cancel});Ok(job)
    }
    pub fn status(&self,id:Uuid)->Result<Job> {let active=self.active.as_ref().ok_or_else(||PlatformError::new(ErrorCode::NotFound,"No update download is active"))?;let job=active.value.lock().map_err(|_|PlatformError::new(ErrorCode::Internal,"Update status is unavailable"))?;if job.id!=id{return Err(PlatformError::new(ErrorCode::NotFound,"Update download was not found"));}Ok(job.clone())}
    pub fn cancel(&self,id:Uuid)->Result<Job> {let job=self.status(id)?;if let Some(active)=&self.active{active.cancel.store(true,Ordering::Relaxed);}Ok(job)}
    pub fn cancel_all(&self){if let Some(active)=&self.active{active.cancel.store(true,Ordering::Relaxed);}}
}
