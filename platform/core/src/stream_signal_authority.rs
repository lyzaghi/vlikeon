//! Native lifecycle-backed signal authority. The real host must connect these
//! guards to admitted document/session close and native consent. No renderer
//! message can construct them, and installed streaming policy remains disabled.
use crate::{Database,now_ms,stream_credentials::{self as journal,Phase},stream_credential_lease::CredentialLease,
    stream_signal_operations::{NativeSignalAuthority,SignalObservation}};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_streaming::{validate_audience,native_credentials::CredentialAddress};
use serde::Deserialize;
use std::{collections::BTreeMap,sync::{Arc,Weak,atomic::{AtomicBool,Ordering}},time::{Duration,Instant}};
use uuid::Uuid;

struct DocumentState{profile:Uuid,alive:AtomicBool}
/// Trusted native host creates only after document/profile admission and holds
/// until that exact document closes. This is not itself origin attestation.
pub struct NativeSignalDocument{state:Arc<DocumentState>}
impl NativeSignalDocument{
    pub fn new(profile:Uuid)->Result<Self>{
        if profile.is_nil(){return Err(denied());}Ok(Self{state:Arc::new(DocumentState{profile,alive:AtomicBool::new(true)})})
    }
    pub fn close(&self){self.state.alive.store(false,Ordering::Release);}
}
impl Drop for NativeSignalDocument{fn drop(&mut self){self.close();}}
struct PermitState{alive:AtomicBool}
/// Only register() creates this epoch. Drop/close permanently fences native
/// authority but does NOT pretend to clear durable receiver ownership or stop
/// already-issued OS/HTTP/input work. release() needs the operation lease too.
pub struct NativeSignalPermit{address:CredentialAddress,session:Uuid,epoch:u64,revision:u64,state:Arc<PermitState>}
impl NativeSignalPermit{
    pub fn address(&self)->CredentialAddress{self.address}
    pub fn session(&self)->Uuid{self.session}
    pub fn epoch(&self)->u64{self.epoch}
    pub fn revision(&self)->u64{self.revision}
    pub fn close(&self){self.state.alive.store(false,Ordering::Release);}
}
impl Drop for NativeSignalPermit{fn drop(&mut self){self.close();}}
struct Entry{address:CredentialAddress,epoch:u64,document:Weak<DocumentState>,permit:Weak<PermitState>,deadline:Instant}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]
struct InstalledPolicy{schema_version:u32,enabled:bool,relay_origins:Vec<String>,allow_browser_host:bool,allow_remote_input:bool,default_profile:String}
fn installed_configuration()->Result<InstalledPolicy>{
    let policy:InstalledPolicy=serde_json::from_str(include_str!("../../../security/streaming-channel.json")).map_err(|_|denied())?;
    if policy.schema_version!=1||policy.relay_origins.len()>8||
        !["p720_30","p720_60","p1080_30","p1080_60"].contains(&policy.default_profile.as_str()){return Err(denied());}
    for (index,origin) in policy.relay_origins.iter().enumerate(){validate_audience(origin)?;if policy.relay_origins[..index].contains(origin){return Err(denied());}}
    // Signaling authorization does not grant capture, host or remote input.
    let _=(policy.allow_browser_host,policy.allow_remote_input);Ok(policy)
}
fn installed_policy()->Result<InstalledPolicy>{
    let policy=installed_configuration()?;if !policy.enabled||policy.relay_origins.is_empty(){return Err(denied());}Ok(policy)
}
/// Retirement can finish while new sessions are disabled, but never derives a
/// network allowlist from journal/renderer/peer URLs. Empty configuration refuses.
pub fn installed_retirement_origins()->Result<Vec<String>>{
    let policy=installed_configuration()?;if policy.relay_origins.is_empty(){return Err(denied());}Ok(policy.relay_origins)
}
/// Native product entry gate only; this does not supply local consent or a
/// session grant. Current installed policy is disabled and therefore refuses.
pub fn require_installed_pairing_policy()->Result<()>{installed_policy().map(|_|())}
/// Origins for new pairing/activation, unlike retirement this requires enabled.
pub fn installed_pairing_origins()->Result<Vec<String>>{installed_policy().map(|policy|policy.relay_origins)}
/// Native host approval must respect installed hosting/input policy in addition
/// to actual peer consent. This never authorizes capture or input injection.
pub fn require_installed_host_grant(audience:&str,input_allowed:bool)->Result<()>{
    let policy=installed_policy()?;
    if !policy.allow_browser_host||!policy.relay_origins.iter().any(|origin|origin==audience)||
        (input_allowed&&!policy.allow_remote_input){return Err(denied());}Ok(())
}
/// Real native controller owns all backends for this exact session. Confirmed
/// means canceled/joined signaling work and confirmed ICE/media/input/gamepad
/// retirement, including work retained after document close. No default owner.
/// Implementations must not reacquire the credential lease held by release(),
/// reenter this registry, or equate a closed permit/PID timeout with retirement.
pub trait NativeSessionRetirement{
    fn retire(&mut self,address:CredentialAddress,session:Uuid,epoch:u64,deadline:Instant)
        ->Result<crate::stream_credential_recovery::Retirement>;
}
pub struct NativeSignalRegistry<'a>{database:&'a Database,profile:Uuid,entries:BTreeMap<Uuid,Entry>}
impl<'a> NativeSignalRegistry<'a>{
    pub fn new(database:&'a Database,profile:Uuid)->Result<Self>{
        if profile.is_nil(){return Err(denied());}Ok(Self{database,profile,entries:BTreeMap::new()})
    }
    /// Trusted controller invokes after actual native peer/session consent. One
    /// durable receiver owner per pair; an abandoned owner is recovery debt.
    pub fn register(&mut self,document:&NativeSignalDocument,address:CredentialAddress,session:Uuid,expected:u64)->Result<NativeSignalPermit>{
        let started=Instant::now();let now=now_ms();let policy=installed_policy()?;
        if document.state.profile!=self.profile||!document.state.alive.load(Ordering::Acquire)||session.is_nil()||
            address.profile()!=self.profile||address.device()!=self.database.device_id{return Err(denied());}
        // Closed/expired entries retain native cleanup ownership. Never prune
        // them by time/liveness: the real backend owner must retire them first.
        if self.entries.len()>=16||self.entries.contains_key(&session){return Err(denied());}
        let lease=CredentialLease::acquire(self.database,self.profile)?;
        let snapshot=journal::load_scoped(self.database,&lease,self.profile)?;
        if snapshot.revision()!=expected{return Err(denied());}
        let record=snapshot.records().iter().find(|record|record.address()==address).ok_or_else(denied)?;
        if record.phase()!=Phase::Active||!policy.relay_origins.iter().any(|origin|record.grant().is_some_and(|grant|grant.audience()==origin)){return Err(denied());}
        let remaining=record.expires_at_ms().ok_or_else(denied)?.checked_sub(now).filter(|remaining|*remaining>0).ok_or_else(denied)?.min(30*60*1000);
        let deadline=started.checked_add(Duration::from_millis(remaining)).ok_or_else(denied)?;
        let registered=journal::reserve_signal_session(self.database,&lease,self.profile,expected,address,session,now_ms())?;
        // Reservation may already be durable on a late/closed-document result.
        // Leave it for native release/recovery; never auto-reclaim it as idle.
        if !document.state.alive.load(Ordering::Acquire)||Instant::now()>=deadline{return Err(denied());}
        let epoch=registered.revision();let state=Arc::new(PermitState{alive:AtomicBool::new(true)});
        self.entries.insert(session,Entry{address,epoch,document:Arc::downgrade(&document.state),permit:Arc::downgrade(&state),deadline});
        Ok(NativeSignalPermit{address,session,epoch,revision:registered.revision(),state})
    }
    /// Returns None for pending retirement; permit stays permanently closed and
    /// the entry/journal owner remain. A late error may follow a durable release.
    pub fn release(&mut self,permit:&NativeSignalPermit,expected:u64,
        owner:&mut dyn NativeSessionRetirement,deadline:Instant)->Result<Option<u64>>{
        permit.close();
        let started=Instant::now();
        if deadline<=started||deadline.saturating_duration_since(started)>Duration::from_secs(30){return Err(denied());}
        let entry=self.entries.get(&permit.session).ok_or_else(denied)?;
        let registered=entry.permit.upgrade().ok_or_else(denied)?;
        if entry.address!=permit.address||entry.epoch!=permit.epoch||!Arc::ptr_eq(&registered,&permit.state){return Err(denied());}
        let lease=CredentialLease::acquire(self.database,self.profile)?;
        let snapshot=journal::load_scoped(self.database,&lease,self.profile)?;
        if snapshot.revision()!=expected||!snapshot.records().iter().any(|record|record.address()==permit.address&&
            record.signal_session()==Some(permit.session)&&record.signal_epoch()==permit.epoch)||Instant::now()>=deadline{return Err(denied());}
        let retired=owner.retire(permit.address,permit.session,permit.epoch,deadline)?;
        lease.check(self.database,self.profile)?;
        if Instant::now()>=deadline{return Err(denied());}
        if matches!(retired,crate::stream_credential_recovery::Retirement::Pending){return Ok(None);}
        // The journal mutation re-admits revision/namespace/exact owner after
        // backend work; no owner acknowledgment authorizes overwriting changes.
        let released=journal::release_signal_session(self.database,&lease,self.profile,expected,permit.address,permit.session,permit.epoch)?;
        self.entries.remove(&permit.session);
        if Instant::now()>=deadline{return Err(denied());}Ok(Some(released.revision()))
    }
}
impl NativeSignalAuthority for NativeSignalRegistry<'_>{
    fn observe(&mut self,address:CredentialAddress,session:Uuid)->Result<SignalObservation>{
        let checked=Instant::now();let policy=installed_policy()?;
        let entry=self.entries.get(&session).ok_or_else(denied)?;
        let document=entry.document.upgrade().ok_or_else(denied)?;let permit=entry.permit.upgrade().ok_or_else(denied)?;
        if entry.address!=address||entry.deadline<=checked||document.profile!=self.profile||!document.alive.load(Ordering::Acquire)||!permit.alive.load(Ordering::Acquire){return Err(denied());}
        let snapshot=journal::load(self.database,self.profile)?;
        let record=snapshot.records().iter().find(|record|record.address()==address).ok_or_else(denied)?;
        if record.phase()!=Phase::Active||record.signal_session()!=Some(session)||record.signal_epoch()!=entry.epoch||
            record.expires_at_ms().is_none_or(|expiry|expiry<=now_ms())||
            !policy.relay_origins.iter().any(|origin|record.grant().is_some_and(|grant|grant.audience()==origin))||
            Instant::now()>=entry.deadline||!document.alive.load(Ordering::Acquire)||!permit.alive.load(Ordering::Acquire){return Err(denied());}
        // Preserve the original timestamp across SQL/policy work. Operation
        // admission rejects it if observation took longer than 100 ms.
        Ok(SignalObservation::from_native(address,session,entry.epoch,checked,true))
    }
}
fn denied()->PlatformError{PlatformError::new(ErrorCode::PermissionDenied,"Native signaling document, session ownership or installed policy is unavailable")}
