use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_core::{Database,now_ms};
use omniplay_sync_client::RunSummary;
use serde::Serialize;
use std::{path::Path,sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}}};
use uuid::Uuid;

#[derive(Clone,Serialize)]
#[serde(rename_all="snake_case")]
pub enum JobState {Running,Complete,Cancelled,Failed}

#[derive(Clone,Serialize)]
pub struct JobSnapshot {pub id:Uuid,pub state:JobState,pub summary:RunSummary,pub started_at_ms:u64,pub finished_at_ms:Option<u64>,pub error:Option<PlatformError>}

struct Job {snapshot:Arc<Mutex<JobSnapshot>>,cancel:Arc<AtomicBool>}

#[derive(Default)]
pub struct Jobs {current:Option<Job>}

impl Jobs {
    pub fn busy(&self)->bool {self.current.as_ref().is_some_and(|job|job.snapshot.lock().map(|value|matches!(value.state,JobState::Running)).unwrap_or(true))}

    pub fn start_sync(&mut self,root:&Path)->Result<JobSnapshot> {
        if self.busy(){return Err(PlatformError::new(ErrorCode::Busy,"Synchronization is already running"));}
        let started=JobSnapshot{id:Uuid::new_v4(),state:JobState::Running,summary:RunSummary::default(),started_at_ms:now_ms(),finished_at_ms:None,error:None};
        let snapshot=Arc::new(Mutex::new(started.clone()));let cancel=Arc::new(AtomicBool::new(false));
        let worker_snapshot=Arc::clone(&snapshot);let worker_cancel=Arc::clone(&cancel);let root=root.to_owned();
        std::thread::Builder::new().name("omniplay-save-sync".into()).spawn(move||{
            let result=Database::open(&root).and_then(|mut database|omniplay_sync_client::synchronize(&mut database,&worker_cancel,|summary|{if let Ok(mut status)=worker_snapshot.lock(){status.summary=summary.clone();}}));
            if let Ok(mut status)=worker_snapshot.lock(){
                status.finished_at_ms=Some(now_ms());
                match result {Ok(summary)=>{status.summary=summary;status.state=JobState::Complete;},Err(error)=>{status.state=if error.code==ErrorCode::Cancelled{JobState::Cancelled}else{JobState::Failed};status.error=Some(error);}}
            }
        }).map_err(|_|PlatformError::new(ErrorCode::ResourceLimit,"Synchronization worker could not start"))?;
        self.current=Some(Job{snapshot,cancel});
        Ok(started)
    }

    pub fn status(&self,id:Uuid)->Result<JobSnapshot> {
        let job=self.current.as_ref().ok_or_else(||PlatformError::new(ErrorCode::NotFound,"Synchronization job was not found"))?;
        let snapshot=job.snapshot.lock().map_err(|_|PlatformError::new(ErrorCode::Internal,"Synchronization worker state is unavailable"))?;
        if snapshot.id!=id{return Err(PlatformError::new(ErrorCode::NotFound,"Synchronization job was not found"));}
        Ok(snapshot.clone())
    }

    pub fn cancel(&self,id:Uuid)->Result<JobSnapshot> {
        let snapshot=self.status(id)?;
        if let Some(job)=&self.current {job.cancel.store(true,Ordering::Release);}
        Ok(snapshot)
    }

    pub fn cancel_all(&self){if let Some(job)=&self.current {job.cancel.store(true,Ordering::Release);}}
}

impl Drop for Jobs {fn drop(&mut self){self.cancel_all();}}
