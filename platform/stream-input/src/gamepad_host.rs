//! Private helper orchestration. No serde, renderer method or installed-provider
//! admission is supplied here. The parent reader must renew its lease separately
//! from potentially blocked provider calls; an external process supervisor must
//! still stop a blocked helper and retain unresolved device-cleanup identities.
use crate::gamepad::{Gamepads,HostObservation,Owner,Provider,State,TargetBinding};
use omniplay_contracts::{ErrorCode,PlatformError,RemotePad,Result};
use std::sync::{Arc,Mutex};
use std::time::{Duration,Instant};
use uuid::Uuid;

const PARENT_GAP:Duration=Duration::from_millis(300);
const MAX_SEQUENCE:u64=10_000_000;
fn denied()->PlatformError{PlatformError::new(ErrorCode::PermissionDenied,
    "The private gamepad helper has no live matching parent authorization")}
fn unavailable()->PlatformError{PlatformError::new(ErrorCode::RuntimeUnavailable,
    "The private gamepad helper requires device cleanup or fresh authorization")}

/// OS observations only. The parent pipe cannot claim foreground/process state.
pub struct LocalObservation{
    pub target:Option<TargetBinding>,pub foreground:bool,pub local_override:bool,
}
pub trait LocalGuard{fn observe(&mut self)->LocalObservation;}
struct LeaseState{sequence:u64,deadline:Instant,closed:bool}
struct SharedLease{owner:Owner,state:Mutex<LeaseState>}

/// Owned by the private parent-pipe reader, not by the provider actor. Drop/EOF
/// permanently closes the lease. Renew only after validating this document's
/// capture, pairing and grant; an arriving remote input frame is not a renewal.
pub struct ParentLease{shared:Arc<SharedLease>}
impl ParentLease{
    pub fn refresh(&self,owner:&Owner,sequence:u64,document_alive:bool,
        capture_alive:bool,pair_authorized:bool)->Result<()>{
        let mut state=self.shared.state.lock().map_err(|_|denied())?;
        let now=Instant::now();
        if state.closed||now>=state.deadline||*owner!=self.shared.owner||
           sequence==0||sequence>MAX_SEQUENCE||sequence<=state.sequence||
           !document_alive||!capture_alive||!pair_authorized{
            state.closed=true;return Err(denied());
        }
        state.sequence=sequence;state.deadline=now+PARENT_GAP;Ok(())
    }
    pub fn close(&self){if let Ok(mut state)=self.shared.state.lock(){state.closed=true;}}
}
impl Drop for ParentLease{fn drop(&mut self){self.close();}}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Phase{AwaitingPrepare,Prepared,Active,CleanupRequired,Revoked}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum StopReason{ParentLeaseLost,HostStopped,TargetLost,LocalOverride,Deadline,InputGap,Rejected,ProviderFailure}

pub struct GamepadHost<P:Provider,G:LocalGuard>{
    pads:Gamepads<P>,guard:G,owner:Owner,lease:Arc<SharedLease>,deadline:Instant,focus_deadline:Instant,
    prepared_once:bool,sealed:bool,reason:Option<StopReason>,
    activation_deadline:Option<Instant>,next_cleanup:Instant,
}
impl<P:Provider,G:LocalGuard> GamepadHost<P,G>{
    /// Construct only after native system-visible controller consent and exact
    /// signed helper/provider admission. This function does not establish either.
    /// The returned lease belongs to the independent parent reader.
    pub fn new(provider:P,guard:G,owner:Owner,remaining:Duration)->Result<(Self,ParentLease)>{
        if !owner.valid()||remaining.is_zero()||remaining>Duration::from_secs(30*60){return Err(denied());}
        let now=Instant::now();
        let lease=Arc::new(SharedLease{owner,state:Mutex::new(LeaseState{sequence:0,
            deadline:now+PARENT_GAP,closed:false})});
        let host=Self{pads:Gamepads::new(provider),guard,owner,lease:lease.clone(),
            deadline:now+remaining,focus_deadline:now+remaining.min(Duration::from_secs(30)),
            prepared_once:false,sealed:false,reason:None,activation_deadline:None,next_cleanup:now};
        Ok((host,ParentLease{shared:lease}))
    }
    pub fn phase(&self)->Phase{
        match self.pads.state(){
            State::CleanupRequired=>Phase::CleanupRequired,
            _ if self.sealed=>Phase::Revoked,
            State::Preparing=>Phase::Prepared,State::Active=>Phase::Active,
            _=>Phase::AwaitingPrepare,
        }
    }
    pub fn stop_reason(&self)->Option<StopReason>{self.reason}
    fn observation(&mut self)->HostObservation{
        let local=self.guard.observe();
        // Construction follows a validated private prepare request and grants
        // the first 300 ms. Subsequent renewals require increasing sequences.
        let live=self.lease.state.lock().is_ok_and(|state|!state.closed&&Instant::now()<state.deadline);
        HostObservation{target:local.target,foreground:local.foreground,
            local_override:local.local_override,document_alive:live,
            capture_alive:live,pair_authorized:live}
    }
    fn seal(&mut self,reason:StopReason)->Result<()>{
        self.sealed=true;if self.reason.is_none(){self.reason=Some(reason);}
        let now=Instant::now();
        if self.pads.state()==State::CleanupRequired&&now<self.next_cleanup{return Err(unavailable());}
        self.next_cleanup=now+Duration::from_millis(100);
        if let Some(owner)=self.pads.owner(){self.pads.revoke(&owner)}else{Ok(())}
    }
    /// Call on an independent actor tick (target <=8 ms) even without input.
    /// This cannot interrupt a blocked provider call; external supervision is
    /// mandatory and helper exit alone must not clear pending cleanup records.
    pub fn poll(&mut self)->Result<()>{
        if self.sealed{return self.seal(self.reason.unwrap_or(StopReason::HostStopped));}
        let host=self.observation();
        let reason=if !host.document_alive{Some(StopReason::ParentLeaseLost)}
            else if Instant::now()>=self.deadline||self.activation_deadline.is_some_and(|deadline|
                self.pads.state()==State::Preparing&&Instant::now()>=deadline){Some(StopReason::Deadline)}
            else if host.local_override{Some(StopReason::LocalOverride)}
            else if host.target!=Some(self.owner.target)||(!host.foreground&&
                (self.prepared_once||Instant::now()>=self.focus_deadline)){Some(StopReason::TargetLost)}
            else{None};
        if let Some(reason)=reason{return self.seal(reason);}
        let previous=self.pads.state();let result=self.pads.tick(&host);
        if matches!(previous,State::Active|State::Preparing)&&
           matches!(self.pads.state(),State::Idle|State::CleanupRequired){
            self.sealed=true;self.reason=Some(StopReason::InputGap);
        }
        result
    }
    pub fn ready_for_prepare(&mut self)->Result<bool>{
        self.poll()?;if self.sealed{return Err(denied());}
        let host=self.observation();Ok(host.foreground&&!host.local_override&&
            host.document_alive&&host.target==Some(self.owner.target))
    }
    pub fn prepare(&mut self,numbers:&[u8],system_visible_consent:bool)->Result<Vec<(u8,Uuid)>>{
        self.poll()?;
        if self.sealed||self.prepared_once{return Err(denied());}
        let host=self.observation();
        if !system_visible_consent{let _=self.seal(StopReason::Rejected);return Err(denied());}
        if !host.foreground{return Err(PlatformError::new(ErrorCode::Busy,
            "Bring the consented game window to the foreground and release local input"));}
        self.prepared_once=true;
        let remaining=self.deadline.saturating_duration_since(Instant::now());
        if self.pads.prepare(self.owner,numbers,remaining,&host,system_visible_consent).is_err(){
            let _=self.seal(StopReason::Rejected);return Err(denied());
        }
        let identities=match self.pads.cleanup_identities(){Ok(ids)=>ids,Err(error)=>{
            let _=self.seal(StopReason::ProviderFailure);return Err(error);
        }};
        self.activation_deadline=Some(Instant::now()+Duration::from_secs(5));
        // Refresh OS/parent observations after allocation, before handing out a
        // prepared response. All returned identities precede lazy OS creation.
        self.poll()?;if self.sealed{return Err(denied());}Ok(identities)
    }
    pub fn activate(&mut self,supervisor_ack:&[(u8,Uuid)])->Result<()>{
        self.poll()?;
        if self.sealed||self.pads.state()!=State::Preparing{return Err(denied());}
        // The supervisor must retain exactly this slot/container set before
        // acknowledging activation. No empty/subset/reordered/foreign receipt.
        if self.pads.cleanup_identities()?.as_slice()!=supervisor_ack{
            let _=self.seal(StopReason::Rejected);return Err(denied());
        }
        let host=self.observation();
        if self.pads.activate(&self.owner,&host).is_err(){
            let _=self.seal(StopReason::ProviderFailure);return Err(unavailable());
        }
        self.poll()?;if self.sealed{return Err(denied());}Ok(())
    }
    pub fn input(&mut self,sequence:u64,frames:&[(u8,RemotePad)])->Result<()>{
        self.poll()?;if self.sealed{return Err(denied());}
        let host=self.observation();
        if self.pads.apply(&self.owner,sequence,frames,&host).is_err(){
            let _=self.seal(StopReason::Rejected);return Err(unavailable());
        }
        self.poll()?;if self.sealed{return Err(denied());}Ok(())
    }
    pub fn release(&mut self,sequence:u64)->Result<()>{self.input(sequence,&[])}
    pub fn revoke(&mut self)->Result<()>{self.seal(StopReason::HostStopped)}
    pub fn cleanup_identities(&self)->Result<Vec<(u8,Uuid)>>{self.pads.cleanup_identities()}
}
impl<P:Provider,G:LocalGuard> Drop for GamepadHost<P,G>{
    fn drop(&mut self){let _=self.seal(StopReason::HostStopped);}
}
