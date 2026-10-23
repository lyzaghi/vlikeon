//! Candidate private Windows video transport. Installed artifact admission,
//! actual capture/negotiation and live document/pair consent belong to the host.
//! No current product command invokes this module.
use crate::{failure,gamepad_child::{ChildProcess,HelperKind,SuspendedChild},gamepad_process::ProcessWitness};
use omniplay_contracts::{ContentHash,Result};
use std::{collections::VecDeque,fs::File,io::{Read,Write},os::windows::io::{AsRawHandle,FromRawHandle,OwnedHandle},
    path::Path,sync::{Arc,Mutex,OnceLock,atomic::{AtomicBool,Ordering},mpsc::{self,SyncSender}},
    thread::JoinHandle,time::{Duration,Instant}};
use uuid::Uuid;
use windows_sys::Win32::{Foundation::*,System::{IO::CancelSynchronousIo,Threading::*}};

const CONFIGURE:u16=1;const FRAME:u16=2;const POLL:u16=3;const STOP:u16=4;const KEYFRAME:u16=5;
static LIVE:AtomicBool=AtomicBool::new(false);
static RETAINED:OnceLock<(Arc<ChildProcess>,File,Vec<File>)>=OnceLock::new();
struct Permit{release:bool}
impl Permit{fn acquire()->Result<Self>{LIVE.compare_exchange(false,true,Ordering::AcqRel,Ordering::Acquire).map_err(|_|failure())?;Ok(Self{release:true})}}
impl Drop for Permit{fn drop(&mut self){if self.release{LIVE.store(false,Ordering::Release);}}}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct VideoOwner{pub capture:Uuid,pub game:Uuid,pub pair:Uuid,pub session:Uuid,pub generation:u64}
impl VideoOwner{fn valid(self)->bool{[self.capture,self.game,self.pair,self.session].iter().all(|id|!id.is_nil())&&(1..=1_000_000).contains(&self.generation)}}
#[derive(Clone,Copy)]
pub struct VideoProfile{pub width:u32,pub height:u32,pub fps:u32,pub bitrate:u32,pub require_hardware:bool}
impl VideoProfile{fn valid(self)->bool{matches!((self.width,self.height),(1280,720)|(1920,1080))&&matches!(self.fps,30|60)&&(500_000..=30_000_000).contains(&self.bitrate)}}
pub struct VideoPacket{pub timestamp_100ns:i64,pub keyframe:bool,pub source_sequence:Option<u64>,pub bytes:Vec<u8>,pub sequence_header:Vec<u8>}
struct Queued{packet:VideoPacket,deadline:Instant}
struct Packets{queue:VecDeque<Queued>,bytes:usize}
struct Lease{sequence:u64,deadline:Instant}
struct Shared{
    process:Arc<ChildProcess>,target:ProcessWitness,owner:VideoOwner,deadline:Instant,window:Option<u64>,
    _permit:Permit,artifact:Option<File>,installation_guards:Vec<File>,closed:AtomicBool,revoking:AtomicBool,
    lease:Mutex<Lease>,stop_deadline:Mutex<Option<Instant>>,pending:Mutex<Option<Instant>>,
    packets:Mutex<Packets>,io:OnceLock<OwnedHandle>,
}
impl Drop for Shared{fn drop(&mut self){
    if !self.process.exited().unwrap_or(false){
        self._permit.release=false;
        if let Some(artifact)=self.artifact.take(){if let Err(value)=RETAINED.set((self.process.clone(),artifact,std::mem::take(&mut self.installation_guards))){std::mem::forget(value);}}
    }
}}
impl Shared{
    fn authority_live(&self)->bool{
        !self.closed.load(Ordering::Acquire)&&!self.revoking.load(Ordering::Acquire)&&Instant::now()<self.deadline&&
        self.lease.lock().is_ok_and(|lease|Instant::now()<lease.deadline)&&
        self.target.exited(self.target.identity()).is_ok_and(|exited|!exited)
    }
    fn transport_live(&self)->bool{
        if self.closed.load(Ordering::Acquire){return false;}
        if self.revoking.load(Ordering::Acquire){return self.stop_deadline.lock().is_ok_and(|deadline|deadline.is_some_and(|value|Instant::now()<value));}
        self.authority_live()
    }
    fn close(&self){
        self.closed.store(true,Ordering::Release);
        if let Ok(mut packets)=self.packets.lock(){packets.queue.clear();packets.bytes=0;}
        if let Some(io)=self.io.get(){unsafe{CancelSynchronousIo(io.as_raw_handle());}}
    }
}
/// Only a trusted native observer refreshes this handle. A received media packet
/// or an automatic Poll never establishes document/pair/capture authority.
pub struct VideoAuthorityLease{shared:Arc<Shared>}
/// Native-host observation, never deserialize this from a renderer/peer. Each
/// source must recheck its own current state; caching a true bit is not evidence.
pub struct VideoAuthorityObservation{
    pub owner:VideoOwner,pub checked_at:Instant,
    pub document_alive:bool,pub capture_authorized:bool,pub pair_authorized:bool,
}
impl VideoAuthorityObservation{
    fn permits(&self,expected:VideoOwner,now:Instant)->bool{
        self.owner==expected&&self.document_alive&&self.capture_authorized&&self.pair_authorized&&
            now.checked_duration_since(self.checked_at).is_some_and(|age|age<=Duration::from_millis(100))
    }
}
impl VideoAuthorityLease{
    pub fn owner(&self)->VideoOwner{self.shared.owner}
    pub fn observe(&self,sequence:u64,observation:VideoAuthorityObservation)->Result<()>{
        let now=Instant::now();
        if !observation.permits(self.shared.owner,now)||!self.shared.authority_live(){self.shared.close();return Err(failure());}
        let mut lease=self.shared.lease.lock().map_err(|_|failure())?;
        if now>=lease.deadline||sequence<=lease.sequence||sequence>10_000_000{drop(lease);self.shared.close();return Err(failure());}
        lease.sequence=sequence;lease.deadline=observation.checked_at+Duration::from_millis(200);Ok(())
    }
    pub fn close(&self){self.shared.close();}
}
impl Drop for VideoAuthorityLease{fn drop(&mut self){self.close();}}

#[cfg(test)]
mod authority_tests{
    use super::*;
    fn owner()->VideoOwner{VideoOwner{capture:Uuid::from_u128(1),game:Uuid::from_u128(2),pair:Uuid::from_u128(3),session:Uuid::from_u128(4),generation:1}}
    fn observation(now:Instant)->VideoAuthorityObservation{VideoAuthorityObservation{owner:owner(),checked_at:now,document_alive:true,capture_authorized:true,pair_authorized:true}}
    #[test]fn current_exact_owner_is_admitted(){let now=Instant::now();assert!(observation(now).permits(owner(),now));}
    #[test]fn every_owner_component_is_bound(){
        let now=Instant::now();
        for index in 0..5{
            let mut value=observation(now);
            match index{0=>value.owner.capture=Uuid::from_u128(9),1=>value.owner.game=Uuid::from_u128(9),2=>value.owner.pair=Uuid::from_u128(9),3=>value.owner.session=Uuid::from_u128(9),_=>value.owner.generation+=1}
            assert!(!value.permits(owner(),now));
        }
    }
    #[test]fn each_native_authority_loss_is_denied(){
        let now=Instant::now();for index in 0..3{let mut value=observation(now);
            match index{0=>value.document_alive=false,1=>value.capture_authorized=false,_=>value.pair_authorized=false}
            assert!(!value.permits(owner(),now));
        }
    }
    #[test]fn stale_and_future_observations_are_denied(){
        let now=Instant::now();assert!(!observation(now).permits(owner(),now+Duration::from_millis(101)));
        assert!(!observation(now+Duration::from_millis(1)).permits(owner(),now));
    }
}
struct Call{kind:u16,payload:Vec<u8>,reply:SyncSender<Result<Vec<u8>>>,timeout:Duration}
pub struct VideoWorker{shared:Arc<Shared>,sender:Option<SyncSender<Call>>,threads:Vec<JoinHandle<()>>,profile:Option<VideoProfile>,sha256:ContentHash}
fn thread_handle(thread:&JoinHandle<()>)->Result<OwnedHandle>{
    let current=unsafe{GetCurrentProcess()};let mut handle=std::ptr::null_mut();
    if unsafe{DuplicateHandle(current,thread.as_raw_handle(),current,&mut handle,THREAD_TERMINATE|THREAD_SYNCHRONIZE,0,0)}==0||handle.is_null()||handle==INVALID_HANDLE_VALUE{return Err(failure());}
    Ok(unsafe{OwnedHandle::from_raw_handle(handle)})
}
impl VideoWorker{
    pub fn spawn(executable:&Path,artifact:File,sha256:ContentHash,owner:VideoOwner,target:ProcessWitness,remaining:Duration)->Result<(Self,VideoAuthorityLease)>{
        Self::spawn_mode(executable,artifact,sha256,owner,target,remaining,None,Vec::new())
    }
    /// The HWND is private native capture context, not renderer authority.
    /// Native WGC confirmation remains mandatory inside the v2 worker.
    pub fn spawn_owned(executable:&Path,artifact:File,sha256:ContentHash,owner:VideoOwner,target:ProcessWitness,window:u64,remaining:Duration)->Result<(Self,VideoAuthorityLease)>{
        Self::spawn_owned_guarded(executable,artifact,Vec::new(),sha256,owner,target,window,remaining)
    }
    /// Native installed admission supplies non-delete-sharing ancestor guards;
    /// retain them through I/O retirement, including an unknown-exit tombstone.
    pub fn spawn_owned_guarded(executable:&Path,artifact:File,guards:Vec<File>,sha256:ContentHash,owner:VideoOwner,target:ProcessWitness,window:u64,remaining:Duration)->Result<(Self,VideoAuthorityLease)>{
        let mut pid=0;
        if window==0||unsafe{windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(window as usize as HWND,&mut pid)}==0||pid!=target.identity().process_id{return Err(failure());}
        Self::spawn_mode(executable,artifact,sha256,owner,target,remaining,Some(window),guards)
    }
    fn spawn_mode(executable:&Path,artifact:File,sha256:ContentHash,owner:VideoOwner,target:ProcessWitness,remaining:Duration,window:Option<u64>,installation_guards:Vec<File>)->Result<(Self,VideoAuthorityLease)>{
        if !owner.valid()||remaining.is_zero()||remaining>Duration::from_secs(1800)||target.exited(target.identity())?||ContentHash::parse(sha256.0.clone()).is_err(){return Err(failure());}
        let permit=Permit::acquire()?;let now=Instant::now();
        let child=if window.is_some(){SuspendedChild::launch_owned_video(executable,target.native_handle())?}else{SuspendedChild::launch(executable,HelperKind::Video)?};
        let(process,primary,input,output)=child.into_parts();
        let shared=Arc::new(Shared{process,target,owner,deadline:now+remaining,window,_permit:permit,artifact:Some(artifact),installation_guards,
            closed:AtomicBool::new(false),revoking:AtomicBool::new(false),lease:Mutex::new(Lease{sequence:0,deadline:now+Duration::from_millis(300)}),
            stop_deadline:Mutex::new(None),pending:Mutex::new(None),packets:Mutex::new(Packets{queue:VecDeque::new(),bytes:0}),io:OnceLock::new()});
        let(sender,receiver)=mpsc::sync_channel(1);
        let mut client=Self{shared:shared.clone(),sender:Some(sender),threads:Vec::new(),profile:None,sha256};
        let io_shared=shared.clone();
        let io=std::thread::Builder::new().name("vlikeon-video-io".into()).spawn(move||io_loop(input,output,io_shared,receiver)).map_err(|_|failure())?;
        let handle=thread_handle(&io);client.threads.push(io);shared.io.set(handle?).map_err(|_|failure())?;
        let watch=shared.clone();
        client.threads.push(std::thread::Builder::new().name("vlikeon-video-watch".into()).spawn(move||{
            while watch.transport_live()&&watch.pending.lock().is_ok_and(|deadline|deadline.is_none_or(|value|Instant::now()<value))&&
                watch.packets.lock().is_ok_and(|packets|packets.queue.front().is_none_or(|packet|Instant::now()<packet.deadline))&&
                watch.io.get().is_some_and(|io|unsafe{WaitForSingleObject(io.as_raw_handle(),0)}==WAIT_TIMEOUT)&&watch.process.exited().is_ok_and(|value|!value){
                std::thread::sleep(Duration::from_millis(8));
            }
            watch.close();let _=watch.process.stop(1000);
        }).map_err(|_|failure())?);
        SuspendedChild::resume(&primary)?;
        Ok((client,VideoAuthorityLease{shared}))
    }
    pub fn owner(&self)->VideoOwner{self.shared.owner}
    pub fn owned_capture(&self)->bool{self.shared.window.is_some()}
    pub fn remaining_authorized(&self)->Result<Duration>{
        if !self.shared.authority_live(){return Err(failure());}
        Ok(self.shared.deadline.saturating_duration_since(Instant::now()))
    }
    pub fn helper_sha256(&self)->&ContentHash{&self.sha256}
    pub fn helper_witness(&self)->Result<ProcessWitness>{self.shared.process.witness()}
    fn call(&self,kind:u16,payload:Vec<u8>,timeout:Duration)->Result<Vec<u8>>{
        if !(if kind==STOP{self.shared.transport_live()}else{self.shared.authority_live()}){return Err(failure());}
        let(reply,receive)=mpsc::sync_channel(1);
        let result=(||{
            self.sender.as_ref().ok_or_else(failure)?.try_send(Call{kind,payload,reply,timeout}).map_err(|_|failure())?;
            receive.recv_timeout(timeout+Duration::from_millis(50)).map_err(|_|failure())?
        })();
        if result.is_err(){self.shared.close();}result
    }
    pub fn configure(&mut self,profile:VideoProfile)->Result<bool>{
        if self.profile.is_some()||!profile.valid(){return Err(failure());}
        let remaining=self.shared.deadline.saturating_duration_since(Instant::now()).as_millis();
        if remaining==0||remaining>1_800_000{return Err(failure());}
        let mut payload=Vec::with_capacity(61);payload.extend(self.shared.owner.capture.as_bytes());payload.extend(self.shared.owner.session.as_bytes());
        payload.extend(self.shared.owner.generation.to_le_bytes());
        for value in [profile.width,profile.height,profile.fps,profile.bitrate,remaining as u32]{payload.extend(value.to_le_bytes());}
        payload.push(u8::from(profile.require_hardware));
        if let Some(window)=self.shared.window{payload.extend(window.to_le_bytes());}
        let reply=self.call(CONFIGURE,payload.clone(),Duration::from_secs(if self.owned_capture(){45}else{5}))?;
        let end=4+payload.len();
        if reply.len()!=end+1||reply[..4]!=[0;4]||reply[4..end]!=payload[..]||reply[end]>1||profile.require_hardware&&reply[end]!=1{self.shared.close();return Err(failure());}
        self.profile=Some(profile);Ok(reply[end]!=0)
    }
    /// Tight NV12. False means the worker did not accept this raw frame.
    pub fn submit(&mut self,bytes:Vec<u8>,timestamp_100ns:i64,keyframe:bool)->Result<bool>{
        if self.owned_capture(){return Err(failure());}
        let profile=self.profile.ok_or_else(failure)?;
        if timestamp_100ns<0||bytes.len()!=(profile.width*profile.height*3/2)as usize{return Err(failure());}
        let mut payload=Vec::with_capacity(9+bytes.len());payload.extend(timestamp_100ns.to_le_bytes());payload.push(u8::from(keyframe));payload.extend(bytes);
        let reply=self.call(FRAME,payload,Duration::from_millis(250))?;
        match reply.as_slice(){[0,0,0,0]=>Ok(true),[1,0,0,0]=>Ok(false),_=>{self.shared.close();Err(failure())}}
    }
    pub fn take_packet(&mut self)->Result<Option<VideoPacket>>{
        if !self.shared.authority_live(){self.shared.close();return Err(failure());}
        let mut packets=self.shared.packets.lock().map_err(|_|failure())?;
        if packets.queue.front().is_some_and(|packet|Instant::now()>=packet.deadline){drop(packets);self.shared.close();return Err(failure());}
        let packet=packets.queue.pop_front();
        if let Some(value)=&packet{packets.bytes-=value.packet.bytes.len()+value.packet.sequence_header.len();}
        drop(packets);
        if !self.shared.authority_live()||packet.as_ref().is_some_and(|value|Instant::now()>=value.deadline){self.shared.close();return Err(failure());}
        Ok(packet.map(|value|value.packet))
    }
    /// Returns an intent fence, not a decoder-refresh receipt. Only a matching
    /// actual keyframe packet at/after the fence confirms the new keyframe.
    pub fn request_keyframe(&mut self)->Result<u64>{
        if !self.owned_capture()||self.profile.is_none(){return Err(failure());}
        let reply=self.call(KEYFRAME,Vec::new(),Duration::from_millis(250))?;
        if reply.len()!=12||reply[..4]!=[0;4]{self.shared.close();return Err(failure());}
        let fence=u64::from_le_bytes(reply[4..].try_into().map_err(|_|failure())?);
        if !(1..=10_000_000).contains(&fence){self.shared.close();return Err(failure());}Ok(fence)
    }
    pub fn stop_confirmed(&mut self)->Result<()>{
        if self.shared.revoking.load(Ordering::Acquire){return Err(failure());}
        *self.shared.stop_deadline.lock().map_err(|_|failure())?=Some(Instant::now()+Duration::from_millis(1000));
        self.shared.revoking.store(true,Ordering::Release);
        if let Ok(mut packets)=self.shared.packets.lock(){packets.queue.clear();packets.bytes=0;}
        let receipt=self.call(STOP,Vec::new(),Duration::from_millis(750));
        let retired=self.close();
        if receipt?.as_slice()!=[0,0,0,0]{return Err(failure());}retired
    }
    pub fn close(&mut self)->Result<()>{
        self.shared.close();self.sender.take();let result=self.shared.process.stop(1000);
        let mut retained=Vec::new();for thread in self.threads.drain(..){if thread.is_finished(){let _=thread.join();}else{retained.push(thread);}}self.threads=retained;result
    }
}
impl Drop for VideoWorker{fn drop(&mut self){let _=self.close();}}

fn exchange(input:&mut File,output:&mut File,shared:&Shared,id:&mut u64,kind:u16,payload:&[u8],timeout:Duration)->Result<(u16,Vec<u8>)>{
    if !shared.transport_live()||*id>=10_000_000||payload.len()>3_110_409{return Err(failure());}
    *id+=1;*shared.pending.lock().map_err(|_|failure())?=Some(Instant::now()+timeout);
    let version:u16=if shared.window.is_some(){2}else{1};
    let mut header=Vec::with_capacity(20);header.extend(b"VLKV");header.extend(version.to_le_bytes());header.extend(kind.to_le_bytes());header.extend(id.to_le_bytes());header.extend((payload.len()as u32).to_le_bytes());
    input.write_all(&header).and_then(|_|input.write_all(payload)).map_err(|_|failure())?;
    let mut reply=[0u8;20];output.read_exact(&mut reply).map_err(|_|failure())?;
    let reply_kind=u16::from_le_bytes([reply[6],reply[7]]);
    let length=u32::from_le_bytes(reply[16..20].try_into().map_err(|_|failure())?)as usize;
    let status_length=if kind==CONFIGURE{if version==2{74}else{66}}else if kind==KEYFRAME&&version==2{12}else{4};
    if &reply[..4]!=b"VLKV"||reply[4..6]!=version.to_le_bytes()||u64::from_le_bytes(reply[8..16].try_into().map_err(|_|failure())?)!=*id||
        !matches!(reply_kind,0x8000|0x8001)||length>2*1024*1024+65536+29||
        (reply_kind==0x8001&&kind!=POLL)||(reply_kind==0x8000&&length!=4&&length!=status_length){return Err(failure());}
    let mut bytes=vec![0u8;length];output.read_exact(&mut bytes).map_err(|_|failure())?;
    let pending=shared.pending.lock().map_err(|_|failure())?.take().ok_or_else(failure)?;
    // During explicit Stop, finish reading one already-issued Poll solely to
    // preserve framing. The I/O loop discards that result before publication.
    if Instant::now()>=pending||!shared.transport_live(){return Err(failure());}
    if reply_kind==0x8000&&(bytes.len()<4||bytes[3]&0x80!=0){return Err(failure());}
    Ok((reply_kind,bytes))
}
fn io_loop(mut input:File,mut output:File,shared:Arc<Shared>,receiver:mpsc::Receiver<Call>){
    let run=(||->Result<()>{
        let mut id=0;let mut configured=false;let mut last_poll=Instant::now();let mut first_input=true;
        let mut accepted:VecDeque<(i64,bool,Instant)>=VecDeque::new();
        let mut native_sequence=0u64;let mut native_pts:Option<i64>=None;let mut keyframe_fences:VecDeque<u64>=VecDeque::new();
        while shared.transport_live(){
            if !shared.revoking.load(Ordering::Acquire)&&accepted.front().is_some_and(|(_,_,submitted)|submitted.elapsed()>=Duration::from_millis(250)){return Err(failure());}
            if configured&&!shared.revoking.load(Ordering::Acquire)&&last_poll.elapsed()>=Duration::from_millis(8){
                let requested_at=Instant::now();
                let(kind,bytes)=exchange(&mut input,&mut output,&shared,&mut id,POLL,&[],Duration::from_millis(250))?;
                last_poll=Instant::now();
                if shared.revoking.load(Ordering::Acquire){
                    // Drain an in-flight Poll without accepting any media.
                }else if kind==0x8001{
                    let owned=shared.window.is_some();let prefix=if owned{29}else{17};
                    if bytes.len()<prefix||bytes[8]>1{return Err(failure());}
                    let pts=i64::from_le_bytes(bytes[..8].try_into().map_err(|_|failure())?);
                    let size=u32::from_le_bytes(bytes[9..13].try_into().map_err(|_|failure())?)as usize;
                    let header=u32::from_le_bytes(bytes[13..17].try_into().map_err(|_|failure())?)as usize;
                    if pts<0||size==0||size>2*1024*1024||header>65536||bytes.len()!=prefix+size+header{return Err(failure());}
                    let received=Instant::now();
                    let(source_sequence,deadline)=if owned{
                        let sequence=u64::from_le_bytes(bytes[17..25].try_into().map_err(|_|failure())?);
                        let age=u32::from_le_bytes(bytes[25..29].try_into().map_err(|_|failure())?);
                        if sequence!=native_sequence+1||sequence>10_000_000||(native_sequence==0&&bytes[8]!=1)||native_pts.is_some_and(|previous|pts<=previous){return Err(failure());}
                        while keyframe_fences.front().is_some_and(|fence|*fence<=sequence){
                            if keyframe_fences.pop_front()!=Some(sequence)||bytes[8]!=1{return Err(failure());}
                        }
                        // Add the FULL local RTT to source-reported age rather
                        // than starting a fresh budget on receipt.
                        let age=Duration::from_millis(u64::from(age)).checked_add(received.duration_since(requested_at)).ok_or_else(failure)?;
                        let remaining=Duration::from_millis(250).checked_sub(age).filter(|value|!value.is_zero()).ok_or_else(failure)?;
                        native_sequence=sequence;native_pts=Some(pts);(Some(sequence),received+remaining)
                    }else{
                        let(expected,force,submitted)=accepted.pop_front().ok_or_else(failure)?;
                        if pts!=expected||force&&bytes[8]!=1||submitted.elapsed()>=Duration::from_millis(250){return Err(failure());}
                        (None,submitted+Duration::from_millis(250))
                    };
                    let deadline=deadline.min(received+Duration::from_millis(100));
                    let packet=VideoPacket{timestamp_100ns:pts,keyframe:bytes[8]!=0,source_sequence,bytes:bytes[prefix..prefix+size].to_vec(),sequence_header:bytes[prefix+size..].to_vec()};
                    let mut packets=shared.packets.lock().map_err(|_|failure())?;
                    if !shared.authority_live()||Instant::now()>=deadline||packets.queue.len()>=4||packets.bytes+size+header>4*1024*1024{return Err(failure());}
                    packets.bytes+=size+header;packets.queue.push_back(Queued{packet,deadline});
                }else if bytes.as_slice()!=[1,0,0,0]{return Err(failure());}
            }
            let call=match receiver.recv_timeout(Duration::from_millis(8)){Ok(value)=>value,Err(mpsc::RecvTimeoutError::Timeout)=>continue,Err(_)=>break};
            if call.kind!=STOP&&shared.revoking.load(Ordering::Acquire){return Err(failure());}
            if (!configured&&call.kind!=CONFIGURE&&call.kind!=STOP)||(configured&&call.kind==CONFIGURE){return Err(failure());}
            if shared.window.is_some()&&call.kind==FRAME||shared.window.is_none()&&call.kind==KEYFRAME{return Err(failure());}
            let submitted=Instant::now();
            let(kind,reply)=exchange(&mut input,&mut output,&shared,&mut id,call.kind,&call.payload,call.timeout)?;
            if kind!=0x8000{return Err(failure());}
            if call.kind==CONFIGURE{
                let size=if shared.window.is_some(){69}else{61};let end=4+size;
                if call.payload.len()!=size||reply.len()!=end+1||reply[..4]!=[0;4]||reply[4..end]!=call.payload[..]||reply[end]>1||call.payload[60]!=0&&reply[end]!=1{return Err(failure());}
                configured=true;
            }
            if call.kind==KEYFRAME{
                if reply.len()!=12||reply[..4]!=[0;4]{return Err(failure());}
                let fence=u64::from_le_bytes(reply[4..].try_into().map_err(|_|failure())?);
                if fence<=native_sequence||fence>10_000_000||keyframe_fences.back().is_some_and(|previous|fence<*previous){return Err(failure());}
                if keyframe_fences.back()!=Some(&fence){if keyframe_fences.len()>=8{return Err(failure());}keyframe_fences.push_back(fence);}
            }
            if call.kind==FRAME&&reply.as_slice()==[0,0,0,0]{
                if accepted.len()>=6||call.payload.len()<9{return Err(failure());}
                let pts=i64::from_le_bytes(call.payload[..8].try_into().map_err(|_|failure())?);
                accepted.push_back((pts,call.payload[8]!=0||first_input,submitted));first_input=false;
            }
            call.reply.try_send(Ok(reply)).map_err(|_|failure())?;
            if call.kind==STOP{break;}
        }Ok(())
    })();let _=run;shared.close();
}
