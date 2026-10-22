use omniplay_contracts::{ErrorCode,NativeWorkRecoverySummary,PlatformError,Result};
use serde::Serialize;
use std::{path::Path,sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}},thread::JoinHandle};
use uuid::Uuid;

#[derive(Clone,Serialize)]
#[serde(rename_all="snake_case")]
pub enum JobState {Running,Stopping,Complete,Cancelled,Failed}

#[derive(Clone,Serialize)]
pub struct JobSnapshot {
    pub id:Uuid,
    pub state:JobState,
    pub summary:NativeWorkRecoverySummary,
    pub started_at_ms:u64,
    pub finished_at_ms:Option<u64>,
    pub error:Option<PlatformError>,
}

struct Active {snapshot:Arc<Mutex<JobSnapshot>>,cancel:Arc<AtomicBool>,thread:JoinHandle<()>}
#[derive(Default)]
pub struct NativeWorkJobs {current:Option<Active>}
pub fn enabled()->bool{cfg!(windows)}
fn unsupported()->PlatformError{PlatformError::new(ErrorCode::UnsupportedCapability,"Native temporary-session recovery is not available on this platform")}

impl NativeWorkJobs {
    pub fn busy(&self)->bool{self.current.as_ref().is_some_and(|active|!active.thread.is_finished())}
    pub fn start(&mut self,root:&Path,profile_id:Uuid)->Result<JobSnapshot>{
        if !enabled(){return Err(unsupported());}
        if profile_id.is_nil(){return Err(PlatformError::new(ErrorCode::PermissionDenied,"Native recovery requires an exact profile owner"));}
        if self.busy(){
            return Err(PlatformError::new(ErrorCode::Busy,"A native temporary-session recovery is still finishing").recover("Wait for the current recovery operation to finish"));
        }
        if let Some(previous)=self.current.take(){let _=previous.thread.join();}
        let started=JobSnapshot{id:Uuid::new_v4(),state:JobState::Running,summary:NativeWorkRecoverySummary::default(),started_at_ms:omniplay_core::now_ms(),finished_at_ms:None,error:None};
        let snapshot=Arc::new(Mutex::new(started.clone()));let cancel=Arc::new(AtomicBool::new(false));
        let worker_snapshot=snapshot.clone();let worker_cancel=cancel.clone();let root=root.to_owned();
        let thread=std::thread::Builder::new().name("omniplay-native-recovery".into()).spawn(move||{
            let result:Result<NativeWorkRecoverySummary>=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{
                #[cfg(windows)]{
                    omniplay_native_runtime::recover_windows_work(&root,profile_id,&worker_cancel,|summary|{
                        if let Ok(mut value)=worker_snapshot.lock(){value.summary=summary.clone();}
                    })
                }
                #[cfg(not(windows))]{let _=(&root,profile_id,&worker_cancel);Err(unsupported())}
            })).unwrap_or_else(|_|Err(PlatformError::new(ErrorCode::Internal,"Native temporary-session recovery stopped unexpectedly")));
            if let Ok(mut value)=worker_snapshot.lock(){
                value.finished_at_ms=Some(omniplay_core::now_ms());
                match result{
                    Ok(summary)=>{value.state=if summary.cancelled{JobState::Cancelled}else{JobState::Complete};value.summary=summary;}
                    Err(error)=>{value.state=JobState::Failed;value.error=Some(error);}
                }
            }
        }).map_err(|_|PlatformError::new(ErrorCode::ResourceLimit,"Native temporary-session recovery worker could not start"))?;
        self.current=Some(Active{snapshot,cancel,thread});Ok(started)
    }

    pub fn status(&self,id:Uuid)->Result<JobSnapshot>{
        if !enabled(){return Err(unsupported());}
        let active=self.current.as_ref().ok_or_else(||PlatformError::new(ErrorCode::NotFound,"Native temporary-session recovery was not found"))?;
        let value=active.snapshot.lock().map_err(|_|PlatformError::new(ErrorCode::Internal,"Native recovery status is unavailable"))?;
        if value.id!=id||id.is_nil(){return Err(PlatformError::new(ErrorCode::NotFound,"Native temporary-session recovery was not found"));}
        Ok(value.clone())
    }

    pub fn cancel(&self,id:Uuid)->Result<JobSnapshot>{
        let previous=self.status(id)?;
        if let Some(active)=&self.current{
            active.cancel.store(true,Ordering::Release);
            let mut value=active.snapshot.lock().map_err(|_|PlatformError::new(ErrorCode::Internal,"Native recovery status is unavailable"))?;
            if matches!(value.state,JobState::Running){value.state=JobState::Stopping;}
            return Ok(value.clone());
        }Ok(previous)
    }

    pub fn cancel_all(&self){if let Some(active)=&self.current{active.cancel.store(true,Ordering::Release);}}
}

impl Drop for NativeWorkJobs {fn drop(&mut self){self.cancel_all();}}
