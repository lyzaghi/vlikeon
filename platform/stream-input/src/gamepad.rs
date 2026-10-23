//! Private provider-side gamepad ownership. No serde/renderer entry point.
//! A trusted host must supply fresh capture/process/focus/owner observations
//! on every call and tick independently while no remote packets arrive.
//! No OS provider is enabled by this module; installed-provider admission,
//! helper IPC and global driver arbitration remain separate required work.
use omniplay_contracts::{ErrorCode,PlatformError,RemotePad,Result};
use std::collections::BTreeSet;
use std::time::{Duration,Instant};
use uuid::Uuid;

const INPUT_GAP:Duration=Duration::from_millis(250);
const MAX_LIFETIME:Duration=Duration::from_secs(30*60);

fn denied()->PlatformError{PlatformError::new(ErrorCode::PermissionDenied,"Virtual gamepad permission does not match the current host session")}
fn unavailable()->PlatformError{PlatformError::new(ErrorCode::RuntimeUnavailable,"The approved virtual gamepad provider is unavailable or requires cleanup")}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct TargetBinding{pub process_id:u32,pub creation_time:u64,pub window:u64}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Owner{
    pub grant:Uuid,pub game:Uuid,pub pair:Uuid,pub session:Uuid,pub capture:Uuid,
    pub epoch:u32,pub target:TargetBinding,
}
impl Owner{
    pub(crate) fn valid(&self)->bool{
        [self.grant,self.game,self.pair,self.session,self.capture].iter().all(|id|!id.is_nil())&&
        (1..=1_000_000).contains(&self.epoch)&&self.target.process_id>0&&
        self.target.creation_time>0&&self.target.window>0
    }
}

/// Values observed by the trusted local helper, never attested by the peer.
pub struct HostObservation{
    pub target:Option<TargetBinding>,pub foreground:bool,pub local_override:bool,
    pub document_alive:bool,pub capture_alive:bool,pub pair_authorized:bool,
}
impl HostObservation{
    fn permits(&self,owner:&Owner)->bool{
        self.target==Some(owner.target)&&self.foreground&&!self.local_override&&
        self.document_alive&&self.capture_alive&&self.pair_authorized
    }
}

/// Standard Web Gamepad order: LX, LY, RX, RY; positive Y is down.
/// Trigger magnitudes are unsigned and independent of their digital bits.
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub struct Report{pub sticks:[i16;4],pub triggers:[u16;2],pub buttons:u16}
impl Report{
    pub fn from_remote(pad:&RemotePad)->Result<Self>{
        if pad.axes.iter().any(|axis|!axis.is_finite()||!(-1.0..=1.0).contains(axis))||
           pad.axes[4..].iter().any(|axis|*axis<0.0){
            return Err(PlatformError::new(ErrorCode::InvalidRequest,"Virtual gamepad axes or triggers are invalid"));
        }
        let mut report=Self{buttons:pad.buttons,..Self::default()};
        for(index,axis)in report.sticks.iter_mut().enumerate(){*axis=(pad.axes[index]*32767.0).round()as i16;}
        for(index,trigger)in report.triggers.iter_mut().enumerate(){*trigger=(pad.axes[index+4]*65535.0).round()as u16;}
        Ok(report)
    }
}

/// Implement only in a provider admitted by the signed installed product.
/// Device ownership must be private and exclusive across provider clients.
/// `detach` returns Ok ONLY after real device removal/disconnection is known.
/// No default keyboard fallback or arbitrary driver IOCTL is supplied here.
pub trait Provider{
    type Device;
    fn create(&mut self,slot:u8,owner:&Owner)->Result<Self::Device>;
    fn write(&mut self,device:&mut Self::Device,report:&Report)->Result<()>;
    /// Retiring an allocated but uninitialized device must not create it just
    /// to publish zero. Lazy providers override this with an existing-only path.
    fn neutralize(&mut self,device:&mut Self::Device)->Result<()>{self.write(device,&Report::default())}
    fn detach(&mut self,device:&mut Self::Device)->Result<()>;
    /// Stable provider identity for an external cleanup observer. Windows lazy
    /// reservation supplies this before OS creation; not all providers can.
    fn cleanup_identity(&self,_device:&Self::Device)->Option<Uuid>{None}
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum State{Idle,Preparing,Active,CleanupRequired}
struct Slot<D>{number:u8,device:D}

/// One host-authorized group, up to four exclusive player slots. The service
/// must arbitrate all groups through one OS provider; this object is not a
/// cross-process lock. Failed removal retains the actual device and owner.
pub struct Gamepads<P:Provider>{
    provider:P,slots:Vec<Slot<P::Device>>,owner:Option<Owner>,state:State,
    deadline:Option<Instant>,last_frame:Option<Instant>,sequence:u64,last_epoch:u32,
}
impl<P:Provider> Gamepads<P>{
    pub fn new(provider:P)->Self{Self{provider,slots:Vec::new(),owner:None,state:State::Idle,deadline:None,last_frame:None,sequence:0,last_epoch:0}}
    pub fn state(&self)->State{self.state}
    pub fn owner(&self)->Option<Owner>{self.owner}
    pub fn reserved_slots(&self)->Vec<u8>{self.slots.iter().map(|slot|slot.number).collect()}
    /// No identity is invented for a backend lacking external observation.
    pub fn cleanup_identities(&self)->Result<Vec<(u8,Uuid)>>{
        let identities:Vec<_>=self.slots.iter().map(|slot|self.provider.cleanup_identity(&slot.device)
            .filter(|id|!id.is_nil()).map(|id|(slot.number,id)).ok_or_else(unavailable)).collect::<Result<_>>()?;
        if identities.iter().map(|(_,id)|*id).collect::<BTreeSet<_>>().len()!=identities.len(){return Err(unavailable());}
        Ok(identities)
    }

    /// Called only after native host consent naming the peer, game, exact
    /// slots and system-visible nature of virtual controllers. A pairing's
    /// generic input bit alone does not supply this consent.
    pub fn authorize(&mut self,owner:Owner,numbers:&[u8],remaining:Duration,
        host:&HostObservation,system_visible_gamepad_consent:bool)->Result<()>{
        self.prepare(owner,numbers,remaining,host,system_visible_gamepad_consent)?;
        self.activate(&owner,host)
    }

    /// Two-phase helpers publish the provider identities to their supervisor
    /// after prepare and require its acknowledgment before activate. OS creation
    /// is deferred only by lazy providers; Linux uinput create is already live.
    pub fn prepare(&mut self,owner:Owner,numbers:&[u8],remaining:Duration,
        host:&HostObservation,system_visible_gamepad_consent:bool)->Result<()>{
        if self.state!=State::Idle||!self.slots.is_empty(){return Err(unavailable());}
        let unique:BTreeSet<_>=numbers.iter().copied().collect();
        if !owner.valid()||owner.epoch<=self.last_epoch||!host.permits(&owner)||
           !system_visible_gamepad_consent||remaining.is_zero()||remaining>MAX_LIFETIME||
           numbers.is_empty()||numbers.len()>4||unique.len()!=numbers.len()||numbers.iter().any(|slot|*slot>3){return Err(denied());}
        self.owner=Some(owner);self.last_epoch=owner.epoch;self.sequence=0;
        self.deadline=Some(Instant::now()+remaining);self.last_frame=None;
        // Consume this authorization epoch even when provider creation fails;
        // retry requires fresh host consent, not replay of a partial attempt.
        self.state=State::CleanupRequired;
        for number in numbers{
            match self.provider.create(*number,&owner){
                Ok(device)=>self.slots.push(Slot{number:*number,device}),
                Err(_)=>{let _=self.retire();return Err(unavailable());}
            }
            // Eager providers can create a real device here. A slow first
            // creation must not authorize creation of the remaining slots.
            if self.deadline.is_none_or(|deadline|Instant::now()>=deadline){let _=self.retire();return Err(denied());}
        }
        if self.deadline.is_none_or(|deadline|Instant::now()>=deadline){let _=self.retire();return Err(denied());}
        self.state=State::Preparing;Ok(())
    }

    pub fn activate(&mut self,owner:&Owner,host:&HostObservation)->Result<()>{
        self.matches(owner)?;self.tick(host)?;
        if self.state!=State::Preparing{return Err(denied());}
        for index in 0..self.slots.len(){
            if self.provider.write(&mut self.slots[index].device,&Report::default()).is_err(){let _=self.retire();return Err(unavailable());}
            // Provider readiness may block. Do not initialize another slot or
            // publish Active after the original grant has expired.
            self.tick(host)?;
            if self.state!=State::Preparing{return Err(denied());}
        }
        if self.deadline.is_none_or(|deadline|Instant::now()>=deadline){let _=self.retire();return Err(denied());}
        // Initialization/readiness can take longer than one input period.
        // The remote input-gap clock starts only when the group becomes active;
        // the overall authorization deadline still includes all setup time.
        self.last_frame=Some(Instant::now());self.state=State::Active;Ok(())
    }

    fn matches(&self,owner:&Owner)->Result<()>{if self.owner.as_ref()!=Some(owner){return Err(denied());}Ok(())}
    /// Called by an independent helper watchdog even without remote frames.
    /// Loss of focus/owner/capture/authorization or an input gap retires rather
    /// than silently resuming. New authority must have a strictly newer epoch.
    pub fn tick(&mut self,host:&HostObservation)->Result<()>{
        if self.state==State::CleanupRequired{return self.retire();}
        if !matches!(self.state,State::Preparing|State::Active){return Ok(());}
        let now=Instant::now();
        if self.owner.as_ref().is_none_or(|owner|!host.permits(owner))||
           self.deadline.is_none_or(|deadline|now>=deadline)||
           (self.state==State::Active&&self.last_frame.is_none_or(|last|now.duration_since(last)>INPUT_GAP)){return self.retire();}
        Ok(())
    }

    /// Full snapshot with explicit stable slot numbers; omission neutralizes
    /// an authorized slot. List compaction never reassigns another controller.
    pub fn apply(&mut self,owner:&Owner,sequence:u64,frames:&[(u8,RemotePad)],host:&HostObservation)->Result<()>{
        self.matches(owner)?;self.tick(host)?;
        if self.state!=State::Active{return Err(denied());}
        if sequence==0||sequence>10_000_000||sequence<=self.sequence{return Err(denied());}
        if frames.len()>4{let _=self.retire();return Err(denied());}
        let mut seen=BTreeSet::new();let mut reports=Vec::with_capacity(frames.len());
        for(number,pad)in frames{
            if !seen.insert(*number)||!self.slots.iter().any(|slot|slot.number==*number){let _=self.retire();return Err(denied());}
            match Report::from_remote(pad){Ok(report)=>reports.push((*number,report)),Err(error)=>{let _=self.retire();return Err(error);}}
        }
        // All validation precedes the first OS write. Partial multi-device
        // publication is still possible; provider failure retires every slot.
        self.sequence=sequence;
        for index in 0..self.slots.len(){
            let slot=&mut self.slots[index];
            let report=reports.iter().find(|(number,_)|*number==slot.number).map(|(_,report)|*report).unwrap_or_default();
            if self.provider.write(&mut slot.device,&report).is_err(){let _=self.retire();return Err(unavailable());}
            // Check against the PREVIOUS accepted frame. Resetting the clock
            // first would revive input after a slow OS write crossed its gap
            // or the overall grant deadline. Retirement may empty the slots;
            // return before indexing another slot in that case.
            self.tick(host)?;
            if self.state!=State::Active{return Err(denied());}
        }
        self.last_frame=Some(Instant::now());Ok(())
    }

    /// Reliable release fence shares the frame sequence space, so delayed
    /// unordered input cannot revive held buttons after a successful release.
    pub fn release(&mut self,owner:&Owner,sequence:u64,host:&HostObservation)->Result<()>{
        self.apply(owner,sequence,&[],host)
    }
    pub fn revoke(&mut self,owner:&Owner)->Result<()>{self.matches(owner)?;self.retire()}
    fn retire(&mut self)->Result<()>{
        self.state=State::CleanupRequired;self.last_frame=None;self.deadline=None;
        let mut retained=Vec::new();
        for mut slot in self.slots.drain(..){
            // Even if neutral fails, acknowledged detach can make the device
            // absent. A failed detach keeps the handle reserved for retry.
            let _=self.provider.neutralize(&mut slot.device);
            if self.provider.detach(&mut slot.device).is_err(){retained.push(slot);}
        }
        self.slots=retained;
        if self.slots.is_empty(){self.owner=None;self.state=State::Idle;Ok(())}else{Err(unavailable())}
    }
}
impl<P:Provider> Drop for Gamepads<P>{fn drop(&mut self){let _=self.retire();}}
