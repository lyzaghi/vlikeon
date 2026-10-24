//! Candidate Windows private transport. No provider/native-consent admission is
//! implied by supplying an executable path/hash. The installed broker supplies
//! a verified immutable executable file guard, retained by all worker threads.
use crate::{failure,gamepad::Owner,gamepad_child::{ChildProcess,SuspendedChild},
    gamepad_process::ProcessWitness,gamepad_protocol::{self as wire,Action,Reply,Request},protocol};
use omniplay_contracts::{ContentHash,Result};
use std::{collections::BTreeSet,fs::File,os::windows::io::{AsRawHandle,FromRawHandle,OwnedHandle},
    path::Path,sync::{Arc,Mutex,OnceLock,atomic::{AtomicBool,Ordering},mpsc::{self,SyncSender}},
    thread::JoinHandle,time::{Duration,Instant}};
use windows_sys::Win32::{Foundation::*,System::{IO::CancelSynchronousIo,Threading::*}};

static LIVE:AtomicBool=AtomicBool::new(false);
struct Permit{release:bool}
impl Permit{
    fn acquire()->Result<Self>{LIVE.compare_exchange(false,true,Ordering::AcqRel,Ordering::Acquire).map_err(|_|failure())?;Ok(Self{release:true})}
}
impl Drop for Permit{fn drop(&mut self){if self.release{LIVE.store(false,Ordering::Release);}}}
// Bounded process-local tombstone if even termination could not be confirmed.
// It intentionally prevents another launch until process restart/recovery.
static RETAINED:OnceLock<(Arc<ChildProcess>,File)>=OnceLock::new();
struct Lease{sequence:u64,deadline:Instant,closed:bool}
struct Pending{id:u64,deadline:Instant,initial_deadline:Option<Instant>,prepare:bool,waiting:bool,refresh_input:bool}
struct Shared{
    _permit:Permit,_artifact:Option<File>,process:Arc<ChildProcess>,owner:Owner,deadline:Instant,lease:Mutex<Lease>,
    closed:AtomicBool,ready:AtomicBool,pending:Mutex<Option<Pending>>,
    writing:Mutex<Option<Instant>>,input_deadline:Mutex<Option<Instant>>,
    reader_thread:OnceLock<OwnedHandle>,writer_thread:OnceLock<OwnedHandle>,
}
impl Drop for Shared{
    fn drop(&mut self){
        if !self.process.exited().unwrap_or(false){
            self._permit.release=false;
            if let Some(artifact)=self._artifact.take(){
                // Only one transport can have held the global permit. Preserve
                // the original guard/job if the OS cannot prove process exit.
                if let Err(retained)=RETAINED.set((self.process.clone(),artifact)){
                    std::mem::forget(retained); // Fail closed on an impossible duplicate tombstone.
                }
            }
        }
    }
}
impl Shared{
    fn close(&self){
        self.closed.store(true,Ordering::Release);
        if let Ok(mut lease)=self.lease.lock(){lease.closed=true;}
        // Requests cancellation only: completion is still checked by the I/O
        // threads. Never free buffers or join a thread on the strength of this.
        for thread in [self.reader_thread.get(),self.writer_thread.get()].into_iter().flatten(){
            unsafe{CancelSynchronousIo(thread.as_raw_handle());}
        }
    }
    fn live(&self)->bool{
        !self.closed.load(Ordering::Acquire)&&Instant::now()<self.deadline&&
            self.lease.lock().is_ok_and(|lease|!lease.closed&&Instant::now()<lease.deadline)
    }
    fn deadlines_valid(&self)->bool{
        let now=Instant::now();
        self.pending.lock().is_ok_and(|pending|pending.as_ref().is_none_or(|pending|
            now<pending.deadline&&pending.initial_deadline.is_none_or(|deadline|now<deadline)))&&
            self.writing.lock().is_ok_and(|writing|writing.is_none_or(|deadline|now<deadline))&&
            self.input_deadline.lock().is_ok_and(|deadline|deadline.is_none_or(|deadline|now<deadline))
    }
    fn threads_alive(&self)->bool{
        [self.reader_thread.get(),self.writer_thread.get()].into_iter().all(|thread|
            thread.is_some_and(|thread|unsafe{WaitForSingleObject(thread.as_raw_handle(),0)}==WAIT_TIMEOUT))
    }
}
/// The broker's independent native authority lane owns this object. It must
/// refresh from fresh document/capture/pair checks even while calls are waiting.
/// Neither remote input nor a writer thread can renew this authority on its own.
pub struct AuthorityLease{shared:Arc<Shared>}
impl AuthorityLease{
    pub fn refresh(&self,owner:Owner,sequence:u64,checked_at:Instant,
        document_alive:bool,capture_alive:bool,pair_authorized:bool)->Result<()>{
        let now=Instant::now();let mut lease=self.shared.lease.lock().map_err(|_|failure())?;
        if self.shared.closed.load(Ordering::Acquire)||lease.closed||now>=lease.deadline||
           owner!=self.shared.owner||sequence==0||sequence>wire::MAX_ID||sequence<=lease.sequence||
           !document_alive||!capture_alive||!pair_authorized||
           !now.checked_duration_since(checked_at).is_some_and(|age|age<=Duration::from_millis(100)){
            lease.closed=true;drop(lease);self.shared.close();return Err(failure());
        }
        lease.sequence=sequence;lease.deadline=checked_at+Duration::from_millis(200);Ok(())
    }
    pub fn close(&self){self.shared.close();}
}
impl Drop for AuthorityLease{fn drop(&mut self){self.close();}}
struct Call{action:Action,timeout:Duration,assigned:SyncSender<Result<u64>>}

pub struct GamepadClient{
    shared:Arc<Shared>,commands:Option<SyncSender<Call>>,replies:mpsc::Receiver<Reply>,
    threads:Vec<JoinHandle<()>>,sha256:ContentHash,prepared:bool,join_failed:bool,
}
fn thread_handle(thread:&JoinHandle<()>)->Result<OwnedHandle>{
    let mut duplicate=std::ptr::null_mut();let current=unsafe{GetCurrentProcess()};
    if unsafe{DuplicateHandle(current,thread.as_raw_handle(),current,&mut duplicate,
        THREAD_TERMINATE|THREAD_SYNCHRONIZE,0,0)}==0||duplicate.is_null()||duplicate==INVALID_HANDLE_VALUE{return Err(failure());}
    // SAFETY: sole owner of the new non-inheritable thread handle.
    Ok(unsafe{OwnedHandle::from_raw_handle(duplicate)})
}
impl GamepadClient{
    /// Launch suspended in a creation-time kill-on-close job with one-process/
    /// 128-MiB limits and only private stdin/stdout/NUL handles inherited. The
    /// first command must be Prepare; this does not grant gamepad permission.
    pub fn spawn(executable:&Path,artifact:File,sha256:ContentHash,owner:Owner,remaining:Duration)->Result<(Self,AuthorityLease)>{
        if !owner.valid()||remaining.is_zero()||remaining>Duration::from_secs(30*60)||
            ContentHash::parse(sha256.0.clone()).is_err(){return Err(failure());}
        let permit=Permit::acquire()?;let now=Instant::now();
        let child=SuspendedChild::launch(executable,crate::gamepad_child::HelperKind::Gamepad)?;
        let(process,thread,input,output)=child.into_parts();
        let shared=Arc::new(Shared{_permit:permit,_artifact:Some(artifact),process,owner,deadline:now+remaining,
            lease:Mutex::new(Lease{sequence:0,deadline:Instant::now()+Duration::from_millis(300),closed:false}),
            closed:AtomicBool::new(false),ready:AtomicBool::new(false),pending:Mutex::new(None),
            writing:Mutex::new(None),input_deadline:Mutex::new(None),reader_thread:OnceLock::new(),writer_thread:OnceLock::new()});
        let(sender,receiver)=mpsc::sync_channel::<Call>(2);let(reply_sender,replies)=mpsc::sync_channel::<Reply>(2);
        let mut client=Self{shared:shared.clone(),commands:Some(sender),replies,threads:Vec::new(),sha256,prepared:false,join_failed:false};
        let reader_shared=shared.clone();
        let reader=std::thread::Builder::new().name("vlikeon-pad-read".into()).spawn(move||reader(output,reader_shared,reply_sender)).map_err(|_|failure())?;
        let cancel=thread_handle(&reader);client.threads.push(reader);
        shared.reader_thread.set(cancel?).map_err(|_|failure())?;
        let writer_shared=shared.clone();
        let writer=std::thread::Builder::new().name("vlikeon-pad-write".into()).spawn(move||writer(input,writer_shared,receiver)).map_err(|_|failure())?;
        let cancel=thread_handle(&writer);client.threads.push(writer);
        shared.writer_thread.set(cancel?).map_err(|_|failure())?;
        let watch_shared=shared.clone();
        let watchdog=std::thread::Builder::new().name("vlikeon-pad-watch".into()).spawn(move||{
            loop{
                if !watch_shared.live()||!watch_shared.deadlines_valid()||!watch_shared.threads_alive()||
                   watch_shared.process.exited().unwrap_or(true){break;}
                std::thread::sleep(Duration::from_millis(8));
            }
            watch_shared.close();let _=watch_shared.process.stop(1000);
        }).map_err(|_|failure())?;
        client.threads.push(watchdog);
        SuspendedChild::resume(&thread)?;
        if !shared.live(){return Err(failure());}
        Ok((client,AuthorityLease{shared}))
    }
    pub fn helper_witness(&self)->Result<ProcessWitness>{self.shared.process.witness()}
    pub fn helper_sha256(&self)->&ContentHash{&self.sha256}
    pub fn owner(&self)->Owner{self.shared.owner}
    /// Waits for the final prepared receipt while the independent lease owner
    /// refreshes native authority. No controller is created before activation.
    pub fn prepare(&mut self,slots:Vec<u8>,expires_at_ms:u64,remaining_ms:u32,
        system_visible_consent:bool,vendor:u16,product:u16)->Result<Reply>{
        if self.prepared||!system_visible_consent||vendor==0||product==0||
           remaining_ms==0||remaining_ms>30*60*1000||slots.is_empty()||slots.len()>4||
           slots.iter().any(|slot|*slot>3)||slots.iter().collect::<BTreeSet<_>>().len()!=slots.len(){
            self.shared.close();return Err(failure());
        }
        let(_,reply)=self.call(Action::Prepare{slots,remaining_ms,expires_at_ms,system_visible_consent,vendor,product},Duration::from_secs(35))?;
        if reply.status!="ok"||reply.phase!="prepared"{self.shared.close();return Err(failure());}
        self.prepared=true;Ok(reply)
    }
    pub fn exchange(&mut self,action:Action,timeout:Duration)->Result<(u64,Reply)>{
        if !self.prepared||matches!(&action,Action::Prepare{..}|Action::Heartbeat{..})||
           timeout.is_zero()||timeout>Duration::from_secs(5){self.shared.close();return Err(failure());}
        self.call(action,timeout)
    }
    fn call(&mut self,action:Action,timeout:Duration)->Result<(u64,Reply)>{
        let result=(||{
            if !self.shared.live(){return Err(failure());}
            let end=Instant::now()+timeout;let(sender,receive)=mpsc::sync_channel(1);
            self.commands.as_ref().ok_or_else(failure)?.try_send(Call{action,timeout,assigned:sender}).map_err(|_|failure())?;
            let id=receive.recv_timeout(end.saturating_duration_since(Instant::now())).map_err(|_|failure())??;
            let reply=self.replies.recv_timeout(end.saturating_duration_since(Instant::now())).map_err(|_|failure())?;
            if reply.id!=id||!reply.complete||reply.binding.owner()!=self.shared.owner{return Err(failure());}
            // A terminal receipt can still prove cleanup after authority loss.
            // A nonterminal reply cannot turn an already closed transport live.
            if !matches!(reply.phase.as_str(),"cleanup_required"|"revoked")&&
               (!self.shared.live()||!self.shared.deadlines_valid()){return Err(failure());}
            Ok((id,reply))
        })();
        if result.is_err(){self.shared.close();}result
    }
    /// Explicit heartbeat uses the same writer/id space as automatic heartbeats.
    /// It does not renew the independent native authority lease.
    pub fn heartbeat(&mut self)->Result<u64>{
        let result=(||{
            if !self.shared.live()||!self.shared.ready.load(Ordering::Acquire){return Err(failure());}
            let(sender,receive)=mpsc::sync_channel(1);
            self.commands.as_ref().ok_or_else(failure)?.try_send(Call{action:Action::Heartbeat{
                document_alive:true,capture_alive:true,pair_authorized:true},timeout:Duration::from_millis(200),assigned:sender}).map_err(|_|failure())?;
            receive.recv_timeout(Duration::from_millis(250)).map_err(|_|failure())?
        })();if result.is_err(){self.shared.close();}result
    }
    pub fn stop(&mut self)->Result<()>{
        self.shared.close();self.commands.take();let result=self.shared.process.stop(1000);
        // Never block forever joining an I/O thread whose cancellation is not
        // acknowledged. Shared permit remains held until every thread retires,
        // preventing repeated launches from accumulating abandoned threads.
        let mut retained=Vec::new();
        for thread in self.threads.drain(..){if thread.is_finished(){self.join_failed|=thread.join().is_err();}else{retained.push(thread);}}
        self.threads=retained;
        if !self.threads.is_empty()||self.join_failed{return Err(failure());}result
    }
}
impl Drop for GamepadClient{fn drop(&mut self){let _=self.stop();}}

fn valid_reply(reply:&Reply,owner:Owner)->bool{
    reply.protocol==wire::VERSION&&reply.binding.owner()==owner&&reply.id>0&&reply.id<=wire::MAX_ID&&
        matches!(reply.status.as_str(),"ok"|"error")&&
        matches!(reply.phase.as_str(),"waiting_for_target"|"prepared"|"active"|"cleanup_required"|"revoked")&&
        !reply.reason.as_ref().is_some_and(|reason|reason.is_empty()||reason.len()>64||!reason.bytes().all(|b|b.is_ascii_lowercase()||b==b'_'))&&
        reply.identities.as_ref().is_none_or(|ids|ids.len()<=4&&ids.iter().all(|id|id.slot<=3&&!id.container.is_nil())&&
            ids.iter().map(|id|id.slot).collect::<BTreeSet<_>>().len()==ids.len()&&
            ids.iter().map(|id|id.container).collect::<BTreeSet<_>>().len()==ids.len())
}
fn reader(mut output:File,shared:Arc<Shared>,sender:SyncSender<Reply>){
    let run=(||->Result<()>{
        while !shared.closed.load(Ordering::Acquire){
            let reply=protocol::read::<Reply>(&mut output).map_err(|_|failure())?.ok_or_else(failure)?;
            if !valid_reply(&reply,shared.owner){return Err(failure());}
            let mut pending=shared.pending.lock().map_err(|_|failure())?;
            let call=pending.as_mut().filter(|call|call.id==reply.id).ok_or_else(failure)?;
            if Instant::now()>=call.deadline||call.initial_deadline.is_some_and(|deadline|Instant::now()>=deadline){return Err(failure());}
            if !reply.complete{
                if !call.prepare||call.waiting||reply.status!="ok"||reply.phase!="waiting_for_target"||
                   reply.reason.is_some()||reply.identities.as_ref().is_none_or(|ids|!ids.is_empty()){return Err(failure());}
                call.waiting=true;call.initial_deadline=None;shared.ready.store(true,Ordering::Release);continue;
            }
            if call.prepare&&!call.waiting{return Err(failure());}
            let terminal=matches!(reply.phase.as_str(),"cleanup_required"|"revoked");
            if !terminal&&!shared.live(){return Err(failure());}
            if reply.phase=="active"&&reply.status=="ok"&&call.refresh_input{
                // Independent fallback if the helper's own 250-ms input-gap
                // cleanup blocks. Status/heartbeats cannot renew this deadline.
                let mut deadline=shared.input_deadline.lock().map_err(|_|failure())?;
                let now=Instant::now();
                if deadline.is_some_and(|deadline|now>=deadline){return Err(failure());}
                *deadline=Some(now+Duration::from_millis(350));
            }
            *pending=None;drop(pending);
            if sender.try_send(reply).is_err(){return Err(failure());}
            if terminal{break;}
        }
        Ok(())
    })();let _=run;shared.close();
}
fn writer(mut input:File,shared:Arc<Shared>,receiver:mpsc::Receiver<Call>){
    let mut sequence=0;let mut first=true;let mut last_heartbeat=Instant::now();
    let run=(||->Result<()>{
        while shared.live(){
            let call=match receiver.recv_timeout(Duration::from_millis(8)){
                Ok(call)=>Some(call),Err(mpsc::RecvTimeoutError::Timeout)=>None,Err(_)=>break,
            };
            // Heartbeats take priority even under continuous input commands.
            // A stream of user calls must not starve the independent lease.
            let automatic=shared.ready.load(Ordering::Acquire)&&last_heartbeat.elapsed()>=Duration::from_millis(50);
            if call.is_none()&&!automatic{continue;}
            let mut sends=Vec::with_capacity(2);
            if automatic{sends.push((Action::Heartbeat{document_alive:true,capture_alive:true,pair_authorized:true},Duration::from_millis(200),None));}
            if let Some(call)=call{sends.push((call.action,call.timeout,Some(call.assigned)));}
            for(action,timeout,assigned)in sends{
                let heartbeat=matches!(&action,Action::Heartbeat{..});let prepare=matches!(&action,Action::Prepare{..});
                if first!=prepare||heartbeat&&!shared.ready.load(Ordering::Acquire){return Err(failure());}
                if !shared.live()||!shared.deadlines_valid(){return Err(failure());}
                sequence+=1;if sequence>wire::MAX_ID{return Err(failure());}
                if !heartbeat{
                    let mut pending=shared.pending.lock().map_err(|_|failure())?;
                    if pending.is_some(){return Err(failure());}
                    *pending=Some(Pending{id:sequence,deadline:Instant::now()+timeout,
                        initial_deadline:prepare.then(||Instant::now()+Duration::from_secs(5)),prepare,waiting:false,
                        refresh_input:matches!(&action,Action::Activate{..}|Action::Input{..}|Action::Release{..})});
                }
                *shared.writing.lock().map_err(|_|failure())?=Some(Instant::now()+Duration::from_millis(200));
                protocol::write(&Request{protocol:wire::VERSION,id:sequence,binding:shared.owner.into(),action},&mut input).map_err(|_|failure())?;
                *shared.writing.lock().map_err(|_|failure())?=None;
                first=false;if heartbeat{last_heartbeat=Instant::now();}
                if let Some(assigned)=assigned{if assigned.try_send(Ok(sequence)).is_err(){return Err(failure());}}
            }
        }
        Ok(())
    })();let _=run;shared.close();
}
