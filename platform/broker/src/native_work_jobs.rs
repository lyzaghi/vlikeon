use omniplay_contracts::{ErrorCode,NativeWorkRecoverySummary,PlatformError,Result};
use serde::Serialize;
use std::{path::Path,sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}}};
use omniplay_native_runtime::NativeRecoveryWorker;
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

struct Active {snapshot:Arc<Mutex<JobSnapshot>>,cancel:Arc<AtomicBool>,worker:NativeRecoveryWorker}
#[derive(Default)]
pub struct NativeWorkJobs {current:Option<Active>}
pub fn enabled()->bool{cfg!(windows)}
fn unsupported()->PlatformError{PlatformError::new(ErrorCode::UnsupportedCapability,"Native temporary-session recovery is not available on this platform")}

impl NativeWorkJobs {
    pub fn busy(&self)->bool{self.current.as_ref().map(|active|!active.worker.has_ended()).unwrap_or_else(NativeRecoveryWorker::pending)}
    pub fn start(&mut self,root:&Path,profile_id:Uuid)->Result<JobSnapshot>{
        if !enabled(){return Err(unsupported());}
        if profile_id.is_nil(){return Err(PlatformError::new(ErrorCode::PermissionDenied,"Native recovery requires an exact profile owner"));}
        if self.busy(){
            return Err(PlatformError::new(ErrorCode::Busy,"A native temporary-session recovery is still finishing").recover("Wait for the current recovery operation to finish"));
        }
        // Drop transfers the held handle into fixed retirement storage. The
        // process-wide owner reclaims it only after actual OS thread exit.
        drop(self.current.take());
        let started=JobSnapshot{id:Uuid::new_v4(),state:JobState::Running,summary:NativeWorkRecoverySummary::default(),started_at_ms:omniplay_core::now_ms(),finished_at_ms:None,error:None};
        let snapshot=Arc::new(Mutex::new(started.clone()));let cancel=Arc::new(AtomicBool::new(false));
        let worker_snapshot=snapshot.clone();let worker_cancel=cancel.clone();let root=root.to_owned();
        let worker=NativeRecoveryWorker::spawn(move||{
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
        })?;
        self.current=Some(Active{snapshot,cancel,worker});Ok(started)
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

    pub fn quarantine_legacy(&self,root:&Path,profile_id:Uuid,work_id:Uuid)->Result<omniplay_native_runtime::LegacyQuarantineReceipt>{
        if !enabled(){return Err(unsupported());}
        if profile_id.is_nil()||work_id.is_nil(){return Err(PlatformError::new(ErrorCode::PermissionDenied,"Legacy work quarantine requires an exact profile and work owner"));}
        if self.busy(){return Err(PlatformError::new(ErrorCode::Busy,"A native temporary-session recovery is still finishing"));}
        #[cfg(windows)]{omniplay_native_runtime::quarantine_unowned_windows_work(root,profile_id,work_id)}
        #[cfg(not(windows))]{let _=(root,profile_id,work_id);Err(unsupported())}
    }

    pub fn diagnostic(&self)->Result<Option<omniplay_core::diagnostics::NativeRecoveryDiagnostic>>{
        let Some(active)=&self.current else{return Ok(None)};
        // Export never waits on a worker-held snapshot lock and never exposes
        // its job cancellation capability or free-form error message.
        let value=active.snapshot.try_lock().map_err(|error|{
            let code=match error{std::sync::TryLockError::WouldBlock=>ErrorCode::Busy,std::sync::TryLockError::Poisoned(_)=>ErrorCode::Internal};
            PlatformError::new(code,"Native recovery diagnostic is unavailable")
        })?;
        let state=match value.state{JobState::Running=>"running",JobState::Stopping=>"stopping",JobState::Complete=>"complete",JobState::Cancelled=>"cancelled",JobState::Failed=>"failed"};
        Ok(Some(omniplay_core::diagnostics::NativeRecoveryDiagnostic{state,error_code:value.error.as_ref().map(|error|error.code),summary:value.summary.clone(),started_at_ms:value.started_at_ms,finished_at_ms:value.finished_at_ms}))
    }
}

impl Drop for NativeWorkJobs {fn drop(&mut self){self.cancel_all();}}

#[cfg(all(test,windows))]
mod tests{
    use super::*;
    #[test]
    fn recovery_diagnostic_redacts_capabilities_and_free_text_and_refuses_a_busy_snapshot(){
        let id=Uuid::new_v4();let(release,gate)=std::sync::mpsc::channel();
        let worker=NativeRecoveryWorker::spawn(move||{let _=gate.recv();}).expect("isolated recovery test worker");
        let snapshot=Arc::new(Mutex::new(JobSnapshot{id,state:JobState::Failed,
            summary:NativeWorkRecoverySummary::default(),started_at_ms:10,finished_at_ms:Some(20),
            error:Some(PlatformError::new(ErrorCode::StorageFailure,"PRIVATE_TEST_PATH_AND_ERROR").recover("PRIVATE_TEST_RECOVERY_ACTION"))}));
        let jobs=NativeWorkJobs{current:Some(Active{snapshot:snapshot.clone(),cancel:Arc::new(AtomicBool::new(false)),worker})};
        let diagnostic=jobs.diagnostic().expect("projection").expect("observed recovery");
        let encoded=serde_json::to_value(diagnostic).expect("diagnostic serialization");
        let text=encoded.to_string();
        let redacted=!text.contains(&id.to_string())&&!text.contains("PRIVATE_TEST_PATH_AND_ERROR")&&!text.contains("PRIVATE_TEST_RECOVERY_ACTION");
        let guard=snapshot.lock().expect("hold actual snapshot");
        let busy=jobs.diagnostic().err().map(|error|error.code);drop(guard);
        // Release before assertions, including when the projection is wrong.
        let _=release.send(());drop(jobs);
        assert!(redacted);assert_eq!(encoded["state"],"failed");assert_eq!(encoded["error_code"],"storage_failure");
        assert_eq!(encoded["started_at_ms"],10);assert_eq!(encoded["finished_at_ms"],20);
        assert_eq!(busy,Some(ErrorCode::Busy));
        assert_eq!(encoded.as_object().unwrap().len(),5);
    }
}
