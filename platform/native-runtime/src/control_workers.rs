//! Process-wide ownership of active, reserved and retiring native IPC workers.
//! No timeout, cancelled request or dropped channel refunds a live worker slot.
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use std::{sync::{Arc,LazyLock,Mutex,atomic::{AtomicUsize,Ordering}},thread::JoinHandle};

const LIMIT:usize=16;
static WORKERS:LazyLock<Arc<WorkerPool>>=LazyLock::new(||Arc::new(WorkerPool::new()));
static RECOVERY_WORKERS:LazyLock<Arc<WorkerPool>>=LazyLock::new(||Arc::new(WorkerPool::with_limit(1)));

struct WorkerPool{
    limit:usize,
    used:Arc<AtomicUsize>,
    retired:Mutex<[Option<WorkerRecord>;LIMIT]>,
}
struct Permit{used:Arc<AtomicUsize>}
impl Drop for Permit{fn drop(&mut self){self.used.fetch_sub(1,Ordering::AcqRel);}}
struct WorkerRecord{thread:Option<JoinHandle<()>>,permit:Option<Permit>}
impl WorkerRecord{
    fn ready(&self)->bool{self.thread.as_ref().is_some_and(|thread|thread.thread().id()!=std::thread::current().id()&&omniplay_sandbox::owned_thread_ended(thread))}
    fn join_completed(mut self){
        // Only called after the completion check, outside the registry mutex.
        // The permit stays charged through actual join (including TLS cleanup).
        if let Some(thread)=self.thread.take(){let _=thread.join();drop(self.permit.take());}
    }
}
impl Drop for WorkerRecord{
    fn drop(&mut self){
        // Unexpected owner loss must fail closed. In particular, dropping a
        // JoinHandle alone detaches; it does not establish worker completion.
        drop(self.thread.take());
        if let Some(permit)=self.permit.take(){std::mem::forget(permit);}
    }
}
impl WorkerPool{
    fn new()->Self{Self::with_limit(LIMIT)}
    fn with_limit(limit:usize)->Self{Self{limit,used:Arc::new(AtomicUsize::new(0)),retired:Mutex::new([const{None};LIMIT])}}
    fn pending(&self)->bool{
        // A terminal result is not termination. Poll parked handles without
        // joining on a status query; attached/reserved/lost owners stay busy.
        let retired=self.retired.lock().unwrap_or_else(|error|error.into_inner());
        let ended=retired.iter().flatten().filter(|record|record.ready()).count();
        self.used.load(Ordering::Acquire)>ended
    }
    fn collect(&self){
        let mut completed:[Option<WorkerRecord>;LIMIT]=[const{None};LIMIT];
        {
            let mut retired=self.retired.lock().unwrap_or_else(|error|error.into_inner());
            for(slot,destination)in retired.iter_mut().zip(completed.iter_mut()){
                if slot.as_ref().is_some_and(WorkerRecord::ready){*destination=slot.take();}
            }
        }
        for record in completed.into_iter().flatten(){record.join_completed();}
    }
    fn reserve(self:&Arc<Self>)->Result<ChannelReservation>{
        self.collect();
        self.used.fetch_update(Ordering::AcqRel,Ordering::Acquire,|used|if used<self.limit{Some(used+1)}else{None}).map_err(|_|PlatformError::new(ErrorCode::ResourceLimit,"Native workers are active, reserved or retiring").recover("Close an active game or wait for earlier native work to finish, then retry"))?;
        Ok(ChannelReservation{pool:self.clone(),permit:Permit{used:self.used.clone()}})
    }
    fn park(&self,record:WorkerRecord){
        // Fixed slots: channel Drop neither allocates a growing retirement list
        // nor joins/does pipe I/O. A lost invariant retains the permit instead.
        let mut retired=self.retired.lock().unwrap_or_else(|error|error.into_inner());
        if let Some(slot)=retired.iter_mut().find(|slot|slot.is_none()){*slot=Some(record);}
    }
}
pub(crate) struct ChannelReservation{pool:Arc<WorkerPool>,permit:Permit}
pub(crate) struct OwnedControlWorker{pool:Arc<WorkerPool>,record:Option<WorkerRecord>}
impl ChannelReservation{
    pub(crate) fn spawn(self,task:impl FnOnce()+Send+'static)->Result<OwnedControlWorker>{
        self.spawn_named("omniplay-native-ipc",task)
    }
    fn spawn_named(self,name:&str,task:impl FnOnce()+Send+'static)->Result<OwnedControlWorker>{
        // The permit remains outside the thread closure. Returning from that
        // closure cannot free a slot before its OS thread and TLS have ended.
        let thread=std::thread::Builder::new().name(name.into()).stack_size(2*1024*1024).spawn(task).map_err(|_|PlatformError::new(ErrorCode::ResourceLimit,"The native worker could not be started"))?;
        Ok(OwnedControlWorker{pool:self.pool,record:Some(WorkerRecord{thread:Some(thread),permit:Some(self.permit)})})
    }
}
impl Drop for OwnedControlWorker{fn drop(&mut self){if let Some(record)=self.record.take(){self.pool.park(record);}}}
pub(crate) fn reserve()->Result<ChannelReservation>{WORKERS.reserve()}

/// The single process-wide native-work collector, retained across broker-owner
/// replacement. This is not a renderer-selected task or an actor/process proof.
pub struct NativeRecoveryWorker{owner:OwnedControlWorker}
impl NativeRecoveryWorker{
    pub fn spawn(task:impl FnOnce()+Send+'static)->Result<Self>{
        let reservation=RECOVERY_WORKERS.reserve().map_err(|_|PlatformError::new(ErrorCode::Busy,"An earlier native recovery worker is still finishing").recover("Wait for the earlier recovery worker to finish, then retry"))?;
        Ok(Self{owner:reservation.spawn_named("omniplay-native-recovery",task)?})
    }
    pub fn has_ended(&self)->bool{self.owner.record.as_ref().is_some_and(WorkerRecord::ready)}
    pub fn pending()->bool{RECOVERY_WORKERS.pending()}
}

#[cfg(test)]
#[path="control_workers_tests.rs"]
mod tests;
