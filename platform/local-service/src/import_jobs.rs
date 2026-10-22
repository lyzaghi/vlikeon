use omniplay_contracts::*;
use omniplay_core::{Database,library::Library,media_import::{DiscScope,import_bundle},now_ms};
use serde::Serialize;
use std::{fs::File,path::Path,sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}}};
use uuid::Uuid;

pub enum ImportSource {Single{file:File,format_hint:Option<ContentFormat>},Bundle{scope:DiscScope,entry:String,encoding:CueEncoding}}
#[derive(Clone,Serialize)]
pub struct ImportJob {pub id:Uuid,pub state:String,pub started_at_ms:u64,pub finished_at_ms:Option<u64>,pub progress:ImportProgress,pub game:Option<GameManifest>,pub error:Option<PlatformError>}
struct Active {snapshot:Arc<Mutex<ImportJob>>,cancel:Arc<AtomicBool>,provider:Option<std::sync::Weak<dyn omniplay_core::media_import::DiscProvider>>}
#[derive(Default)]pub struct ImportJobs {active:Option<Active>}
impl ImportJobs {
    pub fn busy(&self)->bool{self.active.as_ref().is_some_and(|j|j.snapshot.lock().map(|s|s.state=="running").unwrap_or(true))}
    pub fn start(&mut self,root:&Path,title:String,system:GameSystem,source:ImportSource)->Result<ImportJob>{
        if self.busy(){return Err(PlatformError::new(ErrorCode::Busy,"A library import is already running"));}
        if title.trim().is_empty()||title.len()>512||matches!(system,GameSystem::Pc|GameSystem::Web){return Err(PlatformError::new(ErrorCode::InvalidRequest,"Import title or system is invalid"));}
        let value=ImportJob{id:Uuid::new_v4(),state:"running".into(),started_at_ms:now_ms(),finished_at_ms:None,progress:ImportProgress::default(),game:None,error:None};
        let provider=match &source{ImportSource::Bundle{scope,..}=>scope.cancellation(),ImportSource::Single{..}=>None};
        let snapshot=Arc::new(Mutex::new(value.clone()));let cancel=Arc::new(AtomicBool::new(false));let status=Arc::clone(&snapshot);let stop=Arc::clone(&cancel);let root=root.to_owned();
        std::thread::Builder::new().name("omniplay-library-import".into()).spawn(move||{
            let result=(||{let database=Database::open(&root)?;let progress=|progress|{if let Ok(mut job)=status.lock(){job.progress=progress;}};match source{ImportSource::Single{file,format_hint}=>Library::new(&database).import_controlled(file,title,system,format_hint,&stop,progress),ImportSource::Bundle{scope,entry,encoding}=>import_bundle(&database,scope,&entry,title,system,encoding,&stop,progress)}})();
            if let Ok(mut job)=status.lock(){job.finished_at_ms=Some(now_ms());match result{Ok(game)=>{job.state="complete".into();job.progress.phase="complete".into();job.game=Some(game);},Err(error)=>{job.state=if error.code==ErrorCode::Cancelled{"cancelled"}else{"failed"}.into();job.error=Some(error);}}}
        }).map_err(|_|PlatformError::new(ErrorCode::ResourceLimit,"The import worker could not start"))?;
        self.active=Some(Active{snapshot,cancel,provider});Ok(value)
    }
    pub fn status(&self,id:Uuid)->Result<ImportJob>{let job=self.active.as_ref().ok_or_else(||PlatformError::new(ErrorCode::NotFound,"Import job was not found"))?.snapshot.lock().map_err(|_|PlatformError::new(ErrorCode::Internal,"Import status is unavailable"))?;if job.id!=id{return Err(PlatformError::new(ErrorCode::NotFound,"Import job was not found"));}Ok(job.clone())}
    pub fn cancel(&self,id:Uuid)->Result<ImportJob>{let job=self.status(id)?;self.cancel_all();Ok(job)}
    pub fn cancel_all(&self){if let Some(active)=&self.active{active.cancel.store(true,Ordering::Release);if let Some(provider)=active.provider.as_ref().and_then(std::sync::Weak::upgrade){provider.cancel();}}}
}
impl Drop for ImportJobs{fn drop(&mut self){self.cancel_all();}}
