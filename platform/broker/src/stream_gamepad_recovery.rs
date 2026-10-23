//! One bounded native recovery lane. Feature-off builds never start it. This
//! lane does not create controllers or authorize input and exposes no renderer
//! API. OS calls can still block; cancellation is cooperative, never a receipt.
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use std::{path::Path,sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}},thread::JoinHandle};
use uuid::Uuid;
static RUNNING:AtomicBool=AtomicBool::new(false);
struct Permit;
impl Drop for Permit{fn drop(&mut self){RUNNING.store(false,Ordering::Release);}}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum State{Idle,Running,Complete,Retained,Failed,Cancelled}
pub struct RecoveryLane{thread:Option<JoinHandle<()>>,cancel:Arc<AtomicBool>,state:Arc<Mutex<State>>}
impl Default for RecoveryLane{
    fn default()->Self{Self{thread:None,cancel:Arc::new(AtomicBool::new(false)),state:Arc::new(Mutex::new(State::Idle))}}
}
pub fn enabled()->bool{cfg!(all(feature="windows-gamepad",target_os="windows",target_env="msvc",any(target_arch="x86_64",target_arch="aarch64")))}
fn unavailable()->PlatformError{PlatformError::new(ErrorCode::RuntimeUnavailable,"Native gamepad recovery is unavailable or already running")}
impl RecoveryLane{
    pub fn start(&mut self,root:&Path,profile:Uuid,device:Uuid)->Result<()>{
        if !enabled()||profile.is_nil()||device.is_nil()||!root.is_absolute()||
           self.thread.as_ref().is_some_and(|thread|!thread.is_finished()){return Err(unavailable());}
        if let Some(thread)=self.thread.take(){let _=thread.join();}
        RUNNING.compare_exchange(false,true,Ordering::AcqRel,Ordering::Acquire).map_err(|_|unavailable())?;
        let permit=Permit;let root=root.to_owned();let cancel=Arc::new(AtomicBool::new(false));self.cancel=cancel.clone();
        let state=self.state.clone();*state.lock().map_err(|_|unavailable())?=State::Running;
        let started=std::thread::Builder::new().name("vlikeon-pad-recovery".into()).spawn(move||{
            let _permit=permit;
            let result=run(&root,profile,device,&cancel);
            let next=if cancel.load(Ordering::Acquire){State::Cancelled}else{match result{
                Ok(true)=>State::Complete,Ok(false)=>State::Retained,Err(_)=>State::Failed,
            }};
            if let Ok(mut state)=state.lock(){*state=next;}
        });
        match started{Ok(thread)=>{self.thread=Some(thread);Ok(())},Err(_)=>{
            if let Ok(mut state)=self.state.lock(){*state=State::Failed;}Err(unavailable())
        }}
    }
    pub fn state(&self)->State{
        let state=self.state.lock().map(|state|*state).unwrap_or(State::Failed);
        if state==State::Running&&self.thread.as_ref().is_some_and(JoinHandle::is_finished){State::Failed}else{state}
    }
    pub fn cancel(&mut self){
        self.cancel.store(true,Ordering::Release);
        if self.thread.as_ref().is_some_and(JoinHandle::is_finished){if let Some(thread)=self.thread.take(){let _=thread.join();}}
        // A blocked thread retains the single process-local permit until actual
        // completion. Drop never waits forever or spawns a replacement for it.
    }
}
impl Drop for RecoveryLane{fn drop(&mut self){self.cancel();}}
#[cfg(all(feature="windows-gamepad",target_os="windows",target_env="msvc",any(target_arch="x86_64",target_arch="aarch64")))]
fn run(root:&Path,profile:Uuid,device:Uuid,cancel:&AtomicBool)->Result<bool>{
    if cancel.load(Ordering::Acquire){return Err(unavailable());}
    let mut database=omniplay_core::stream_gamepad_journal::open_recovery(root,device)?;
    crate::stream_gamepad::recover_windows_gamepad(&mut database,profile,cancel)
}
#[cfg(not(all(feature="windows-gamepad",target_os="windows",target_env="msvc",any(target_arch="x86_64",target_arch="aarch64"))))]
fn run(_root:&Path,_profile:Uuid,_device:Uuid,_cancel:&AtomicBool)->Result<bool>{Err(unavailable())}
