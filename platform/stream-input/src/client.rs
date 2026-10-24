#[cfg(windows)]
mod native{
use crate::{failure,gamepad_child::{ChildProcess,HelperKind,SuspendedChild},protocol::{self,Action,Observation,Reply,Request,Target}};
use omniplay_contracts::{RemoteInputFrame,Result};
use std::{path::Path,os::windows::io::AsRawHandle,sync::{Arc,OnceLock,atomic::{AtomicBool,Ordering},mpsc::{self,SyncSender}},thread::JoinHandle,time::{Duration,Instant}};
use uuid::Uuid;

static LIVE:AtomicBool=AtomicBool::new(false);
static RETAINED:OnceLock<Arc<ChildProcess>>=OnceLock::new();
struct Permit;
impl Permit{fn acquire()->Result<Self>{LIVE.compare_exchange(false,true,Ordering::AcqRel,Ordering::Acquire).map_err(|_|failure())?;Ok(Self)}}
impl Drop for Permit{fn drop(&mut self){LIVE.store(false,Ordering::Release);}}
struct Shared{process:Arc<ChildProcess>,permit:Option<Permit>}
impl Drop for Shared{fn drop(&mut self){
    if !self.process.exited().unwrap_or(false){
        // Unknown retirement keeps the single transport permit and original
        // job/process alive. Neither a timeout nor cancellation proves exit.
        if let Some(permit)=self.permit.take(){std::mem::forget(permit);}
        if let Err(process)=RETAINED.set(self.process.clone()){std::mem::forget(process);}
    }
}}
struct Call{request:Request,reply:SyncSender<Result<Observation>>}
pub struct InputWorker{shared:Arc<Shared>,sender:Option<SyncSender<Call>>,thread:Option<JoinHandle<()>>,sequence:u64,grant_id:Uuid,session_id:Uuid,closed:bool,stop_observation:Option<Observation>,join_failed:bool}
impl InputWorker{
    pub fn spawn(executable:&Path,target:Target,grant_id:Uuid,session_id:Uuid,expires_at_ms:u64)->Result<Self>{
        let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|_|failure())?.as_millis()as u64;
        if !crate::supported()||!executable.is_absolute()||expires_at_ms<=now||expires_at_ms-now>30*60*1000{return Err(failure());}
        let permit=Permit::acquire()?;
        let child=SuspendedChild::launch(executable,HelperKind::KeyboardMouse)?;
        let(process,primary,mut input,mut output)=child.into_parts();
        let shared=Arc::new(Shared{process,permit:Some(permit)});
        let(sender,receive)=mpsc::sync_channel::<Call>(1);
        let retained=shared.clone();
        let thread=std::thread::Builder::new().name("omniplay-input-ipc".into()).spawn(move||{
            let _retained=retained;
            while let Ok(call)=receive.recv(){
                let result=(||{
                    protocol::write(&call.request,&mut input).map_err(|_|failure())?;let reply:Reply=protocol::read(&mut output).map_err(|_|failure())?.ok_or_else(failure)?;
                    if reply.protocol!=1||reply.id!=call.request.id||reply.status!="ok"||!["waiting_for_target","active","idle","revoked"].contains(&reply.observation.state.as_str())||reply.observation.reason.as_ref().is_some_and(|r|r.is_empty()||r.len()>64||!r.bytes().all(|c|c.is_ascii_lowercase()||c==b'_')){return Err(failure());}Ok(reply.observation)
                })();let terminal=result.is_err();let _=call.reply.send(result);if terminal{break;}
            }
            // EOF asks a live helper to release input. Job termination alone
            // cannot prove that the OS keyboard/mouse state was released.
            drop(input);
        }).map_err(|_|{let _=shared.process.stop(1000);failure()})?;
        let mut worker=Self{shared,sender:Some(sender),thread:Some(thread),sequence:0,grant_id,session_id,closed:false,stop_observation:None,join_failed:false};
        SuspendedChild::resume(&primary)?;
        worker.call(Action::Prepare{target,expires_at_ms},Duration::from_secs(5))?;Ok(worker)
    }
    fn call(&mut self,action:Action,timeout:Duration)->Result<Observation>{
        if self.closed||self.shared.process.exited()?{return Err(failure());}
        self.sequence=self.sequence.checked_add(1).ok_or_else(failure)?;let(reply,receive)=mpsc::sync_channel(1);let request=Request{protocol:1,id:self.sequence,grant_id:self.grant_id,session_id:self.session_id,action};
        self.sender.as_ref().ok_or_else(failure)?.try_send(Call{request,reply}).map_err(|_|failure())?;
        let observation=receive.recv_timeout(timeout).map_err(|_|failure())??;
        // Preserve a real terminal release observation even if the helper exits
        // before a later Stop RPC. Process/I/O retirement is checked separately.
        if observation.state=="revoked"&&observation.reason.as_deref()!=Some("release_failed"){
            self.stop_observation=Some(observation.clone());
        }
        Ok(observation)
    }
    pub fn input(&mut self,sequence:u64,frame:RemoteInputFrame)->Result<Observation>{frame.validate()?;self.call(Action::Input{sequence,frame},Duration::from_millis(750))}
    pub fn status(&mut self)->Result<Observation>{self.call(Action::Status,Duration::from_millis(750))}
    pub fn release(&mut self)->Result<Observation>{self.call(Action::Release,Duration::from_millis(750))}
    /// A process exit or a successful Stop RPC without a released-state
    /// observation is not proof that tracked keyboard/mouse input was released.
    pub fn stop_confirmed(&mut self)->Result<Observation>{
        self.stop_confirmed_until(Instant::now()+Duration::from_millis(3750))
    }
    /// Stop/release receipt, graceful exit, forced containment and I/O reaping
    /// consume one original caller budget. Missing release evidence stays debt.
    pub fn stop_confirmed_until(&mut self,deadline:Instant)->Result<Observation>{
        if !self.closed{
            let remaining=deadline.saturating_duration_since(Instant::now());
            if !remaining.is_zero(){
                let observed=self.call(Action::Stop,remaining.min(Duration::from_millis(750)));
                if let Ok(value)=observed{if value.state=="revoked"&&value.reason.as_deref()!=Some("release_failed"){self.stop_observation=Some(value);}}
            }
            self.closed=true;self.sender.take();
        }
        let graceful=deadline.min(Instant::now()+Duration::from_secs(2));
        while Instant::now()<graceful{
            if self.shared.process.exited().unwrap_or(false){break;}
            std::thread::sleep(graceful.saturating_duration_since(Instant::now()).min(Duration::from_millis(20)));
        }
        if let Some(thread)=self.thread.as_ref(){
            // Cancellation is only a request. A blocked thread retains Shared
            // and the singleton permit until its actual closure completes.
            unsafe{windows_sys::Win32::System::IO::CancelSynchronousIo(thread.as_raw_handle());}
        }
        let remaining=deadline.saturating_duration_since(Instant::now()).as_millis().min(1000)as u32;
        let retired=self.shared.process.stop(remaining).is_ok();
        if self.thread.as_ref().is_some_and(|thread|thread.is_finished()){
            if let Some(thread)=self.thread.take(){self.join_failed|=thread.join().is_err();}
        }
        if !retired||self.thread.is_some()||self.join_failed||Instant::now()>=deadline{return Err(failure());}
        // Retain only a real released-state receipt. Retry may reap an already
        // finished I/O thread, but cannot invent a lost helper acknowledgment.
        self.stop_observation.clone().ok_or_else(failure)
    }
    pub fn close(&mut self){let _=self.stop_confirmed();}
}
impl Drop for InputWorker{fn drop(&mut self){self.close();}}
}
#[cfg(windows)]pub use native::InputWorker;

#[cfg(not(windows))]
pub struct InputWorker;
#[cfg(not(windows))]
impl InputWorker{
    pub fn spawn(_: &std::path::Path,_:crate::protocol::Target,_:uuid::Uuid,_:uuid::Uuid,_:u64)->omniplay_contracts::Result<Self>{Err(crate::failure())}
    pub fn input(&mut self,_:u64,_:omniplay_contracts::RemoteInputFrame)->omniplay_contracts::Result<crate::protocol::Observation>{Err(crate::failure())}
    pub fn status(&mut self)->omniplay_contracts::Result<crate::protocol::Observation>{Err(crate::failure())}
    pub fn release(&mut self)->omniplay_contracts::Result<crate::protocol::Observation>{Err(crate::failure())}
    pub fn stop_confirmed(&mut self)->omniplay_contracts::Result<crate::protocol::Observation>{Err(crate::failure())}
    pub fn stop_confirmed_until(&mut self,_:std::time::Instant)->omniplay_contracts::Result<crate::protocol::Observation>{Err(crate::failure())}
    pub fn close(&mut self){}
}
