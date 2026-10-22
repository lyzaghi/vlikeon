use omniplay_contracts::*;
use omniplay_core::{Database,now_ms,sessions::{RuntimeAdapter,SessionManager}};
use serde::Serialize;
use std::{fs::File,sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}}};
use uuid::Uuid;

#[derive(Clone,Serialize)]
pub struct PreparationSnapshot {pub id:Uuid,pub game_id:Uuid,pub runtime_id:String,pub state:String,pub started_at_ms:u64,pub finished_at_ms:Option<u64>,pub session:Option<SessionInfo>,pub error:Option<PlatformError>}
struct Prepared {game:GameManifest,config:SessionConfig,adapter:Box<dyn RuntimeAdapter>}
struct State {snapshot:PreparationSnapshot,prepared:Option<Prepared>}
struct Active {state:Arc<Mutex<State>>,cancel:Arc<AtomicBool>}
#[derive(Default)]pub struct Preparations {active:Option<Active>}
impl Preparations {
    pub fn busy(&self)->bool{self.active.as_ref().is_some_and(|j|j.state.lock().map(|s|matches!(s.snapshot.state.as_str(),"preparing"|"ready"|"adopting")).unwrap_or(true))}
    pub fn start(&mut self,game:GameManifest,mut adapter:Box<dyn RuntimeAdapter>,asset:File,config:SessionConfig)->Result<PreparationSnapshot>{
        if self.busy(){return Err(PlatformError::new(ErrorCode::Busy,"Another native runtime is being prepared"));}
        game.validate()?;config.validate()?;adapter.manifest().validate()?;
        if !adapter.manifest().systems.contains(&game.system){return Err(PlatformError::new(ErrorCode::UnsupportedCapability,"This runtime does not support the selected system"));}
        let snapshot=PreparationSnapshot{id:Uuid::new_v4(),game_id:game.id,runtime_id:adapter.manifest().id.clone(),state:"preparing".into(),started_at_ms:now_ms(),finished_at_ms:None,session:None,error:None};
        let state=Arc::new(Mutex::new(State{snapshot:snapshot.clone(),prepared:None}));let cancel=Arc::new(AtomicBool::new(false));let status=Arc::clone(&state);let stop=Arc::clone(&cancel);
        std::thread::Builder::new().name("omniplay-native-prepare".into()).spawn(move||{
            let result=adapter.prepare_cancellable(&game,asset,&config,Arc::clone(&stop));
            if let Ok(mut state)=status.lock(){
                state.snapshot.finished_at_ms=Some(now_ms());
                match result{
                    Ok(())if !stop.load(Ordering::Acquire)=>{state.prepared=Some(Prepared{game,config,adapter});state.snapshot.state="ready".into();}
                    Ok(())=>{let _=adapter.stop();state.snapshot.state="cancelled".into();}
                    Err(error)=>{let _=adapter.stop();state.snapshot.state=if error.code==ErrorCode::Cancelled{"cancelled"}else{"failed"}.into();state.snapshot.error=Some(error);}
                }
            }
        }).map_err(|_|PlatformError::new(ErrorCode::ResourceLimit,"Native preparation worker could not start"))?;
        self.active=Some(Active{state,cancel});Ok(snapshot)
    }
    pub fn status(&mut self,id:Uuid,database:&Database,sessions:&mut SessionManager)->Result<PreparationSnapshot>{
        let active=self.active.as_ref().ok_or_else(||PlatformError::new(ErrorCode::NotFound,"Native preparation was not found"))?;
        let prepared={let mut state=active.state.lock().map_err(|_|PlatformError::new(ErrorCode::Internal,"Preparation status is unavailable"))?;if state.snapshot.id!=id{return Err(PlatformError::new(ErrorCode::NotFound,"Native preparation was not found"));}let value=state.prepared.take();if value.is_some(){state.snapshot.state="adopting".into();}value};
        if let Some(prepared)=prepared{
            let result=sessions.adopt_prepared(database,prepared.game,prepared.adapter,prepared.config);
            let mut state=active.state.lock().map_err(|_|PlatformError::new(ErrorCode::Internal,"Preparation status is unavailable"))?;
            match result{Ok(session)=>{state.snapshot.session=Some(session);state.snapshot.state="complete".into();},Err(error)=>{state.snapshot.state="failed".into();state.snapshot.error=Some(error);}}
        }
        self.snapshot(id)
    }
    fn snapshot(&self,id:Uuid)->Result<PreparationSnapshot>{let state=self.active.as_ref().ok_or_else(||PlatformError::new(ErrorCode::NotFound,"Native preparation was not found"))?.state.lock().map_err(|_|PlatformError::new(ErrorCode::Internal,"Preparation status is unavailable"))?;if state.snapshot.id!=id{return Err(PlatformError::new(ErrorCode::NotFound,"Native preparation was not found"));}Ok(state.snapshot.clone())}
    pub fn cancel(&mut self,id:Uuid)->Result<PreparationSnapshot>{
        self.snapshot(id)?;if let Some(active)=&self.active{active.cancel.store(true,Ordering::Release);let prepared={let mut state=active.state.lock().map_err(|_|PlatformError::new(ErrorCode::Internal,"Preparation status is unavailable"))?;let prepared=state.prepared.take();if prepared.is_some(){state.snapshot.state="cancelled".into();}prepared};drop(prepared);}self.snapshot(id)
    }
    pub fn cancel_all(&mut self){if let Some(active)=&self.active{active.cancel.store(true,Ordering::Release);if let Ok(mut state)=active.state.lock(){state.prepared.take();}}}
}
impl Drop for Preparations{fn drop(&mut self){self.cancel_all();}}
