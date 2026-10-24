//! Independent private observation lane. No default observer returns true and no
//! renderer/network/media activity is native authority. Product wiring is due.
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_stream_input::video_client::{VideoAuthorityLease,VideoAuthorityObservation,VideoOwner};
use std::{sync::{Arc,atomic::{AtomicBool,Ordering}},thread::JoinHandle,time::Duration};

/// Implement in the admitted native host, using current native document/capture
/// registration and verified pairing revocation/lifetime. A signed certificate
/// alone cannot prove that a pairing is still authorized. Never derive this
/// observation from a last-known renderer flag, automatic Poll or media output.
pub trait NativeVideoAuthority:Send+'static{
    fn observe(&mut self,expected:VideoOwner)->Result<VideoAuthorityObservation>;
}
static RUNNING:AtomicBool=AtomicBool::new(false);
struct Permit;
impl Drop for Permit{fn drop(&mut self){RUNNING.store(false,Ordering::Release);}}
pub struct VideoAuthorityLane{
    thread:Option<JoinHandle<()>>,cancel:Arc<AtomicBool>,join_failed:bool,
}
// Field drop order retains the permit through actual observer destruction.
// Source Drop may itself stall; that must not admit a replacement lane.
struct ObserverOwner<S>{source:S,lease:VideoAuthorityLease,_permit:Permit}
impl<S> Drop for ObserverOwner<S>{fn drop(&mut self){self.lease.close();}}
impl VideoAuthorityLane{
    pub fn start(source:impl NativeVideoAuthority,lease:VideoAuthorityLease)->Result<Self>{
        if RUNNING.compare_exchange(false,true,Ordering::AcqRel,Ordering::Acquire).is_err(){
            // The consumed lease drops/invalidates its helper on this failure.
            return Err(PlatformError::new(ErrorCode::Busy,"A native video authority lane is still retiring"));
        }
        let observer=ObserverOwner{source,lease,_permit:Permit};let owner=observer.lease.owner();
        let cancel=Arc::new(AtomicBool::new(false));let worker_cancel=cancel.clone();
        let thread=std::thread::Builder::new().name("vlikeon-video-authority".into()).spawn(move||{
            let mut observer=observer;
            let mut sequence=0u64;
            while !worker_cancel.load(Ordering::Acquire){
                // checked_at belongs to the actual source observation. Never
                // replace its timestamp with now after a slow source returns.
                let observation=match observer.source.observe(owner){Ok(value)=>value,Err(_)=>break};
                if worker_cancel.load(Ordering::Acquire){break;}
                sequence=match sequence.checked_add(1){Some(value)=>value,None=>break};
                if observer.lease.observe(sequence,observation).is_err(){break;}
                std::thread::sleep(Duration::from_millis(25));
            }
            // Also runs via lease Drop on panic. If observe blocks, the helper's
            // independent native lease expires without this thread's cooperation.
            observer.lease.close();
        }).map_err(|_|PlatformError::new(ErrorCode::ResourceLimit,"The native video authority observer could not start"))?;
        Ok(Self{thread:Some(thread),cancel,join_failed:false})
    }
    /// Cooperative cancellation only; never claim source/OS-call preemption.
    /// To earn graceful Stop receipts, stop the video worker before canceling
    /// this lane; hard loss of authority deliberately closes transport instead.
    pub fn close(&mut self){
        self.cancel.store(true,Ordering::Release);
        if self.thread.as_ref().is_some_and(|thread|thread.is_finished()){
            if let Some(thread)=self.thread.take(){self.join_failed|=thread.join().is_err();}
        }
    }
    pub fn finished(&self)->bool{self.thread.as_ref().is_none_or(|thread|thread.is_finished())}
    pub fn retirement_confirmed(&self)->bool{self.thread.is_none()&&!self.join_failed}
}
impl Drop for VideoAuthorityLane{fn drop(&mut self){self.close();}}
