//! Candidate parent-side orchestration; deliberately not exposed by service RPC.
//! A real supervised private channel must supply the child identity and checked
//! responses. Native consent, signed artifacts/driver and global slot arbitration
//! are prerequisites, not assertions accepted from a renderer or remote peer.
use omniplay_contracts::{ContentHash,ErrorCode,PlatformError,RemotePad,Result};
use omniplay_core::{Database,stream_gamepad_journal::{self as journal,DeviceIdentity,
    ProcessIdentity,Snapshot,CleanupState,RecoveryObserver,Reconciliation}};
use omniplay_core::stream_gamepad_arbitration::Arbitration;
use omniplay_stream_input::{gamepad::{Owner,Report},gamepad_protocol::{self as wire,Action,Binding,Identity,Pad,Reply}};
use std::{collections::BTreeSet,time::{Duration,Instant}};
use uuid::Uuid;

fn invalid()->PlatformError{PlatformError::new(ErrorCode::PermissionDenied,
    "The native gamepad helper response does not match its reserved authorization")}
fn stopped()->PlatformError{PlatformError::new(ErrorCode::RuntimeUnavailable,
    "Native gamepad authorization stopped; cleanup must be reconciled before reuse")}
fn journal_owner(value:Owner)->journal::Owner{journal::Owner{grant:value.grant,game:value.game,pair:value.pair,
    session:value.session,capture:value.capture,epoch:value.epoch,
    process:ProcessIdentity{process_id:value.target.process_id,creation_time:value.target.creation_time},window:value.target.window}}
fn ids(value:&[Identity])->Vec<DeviceIdentity>{value.iter().map(|id|DeviceIdentity{slot:id.slot,container:id.container}).collect()}
fn valid_ids(value:&[Identity],empty:bool)->bool{
    (empty||!value.is_empty())&&value.len()<=4&&value.iter().all(|id|id.slot<=3&&!id.container.is_nil())&&
        value.iter().map(|id|id.slot).collect::<BTreeSet<_>>().len()==value.len()&&
        value.iter().map(|id|id.container).collect::<BTreeSet<_>>().len()==value.len()
}
/// One exact child, admitted executable and private pipe. Implement exchange
/// with bounded command/reply waits and a separate parent heartbeat writer;
/// provider stalls must be handled by an external process watchdog. `stop`
/// cancels future writes and attempts bounded actual child termination, not journal
/// deletion. Implementations must never clear device ownership on child exit.
pub trait ParentChannel{
    fn helper(&self)->ProcessIdentity;
    fn helper_sha256(&self)->&ContentHash;
    /// The prepared reply has already completed the initial wait/readiness flow.
    /// Assign the request id only in the serialized pipe writer, alongside
    /// heartbeats. Return that id with the correlated completed response.
    fn exchange(&mut self,binding:Binding,action:Action,timeout:Duration)->Result<(u64,Reply)>;
    /// Heartbeats have no reply. Request IDs share the command sequence space;
    /// serialize all pipe writes, including these, through one private writer.
    fn heartbeat(&mut self,binding:Binding)->Result<u64>;
    fn stop(&mut self)->Result<()>;
    /// Same stop/containment operation within an original caller deadline.
    /// Never implement by invoking stop() with a fresh internal wait budget.
    fn stop_until(&mut self,deadline:Instant)->Result<()>;
}

/// A native authority observation freshly validated by the broker, independent
/// of arriving remote packets. `checked_at` is monotonic, never peer supplied.
pub struct Authority{
    pub owner:Owner,pub checked_at:Instant,pub document_alive:bool,
    pub capture_alive:bool,pub pair_authorized:bool,
}
impl Authority{
    fn permits(&self,expected:Owner)->bool{
        self.owner==expected&&self.document_alive&&self.capture_alive&&self.pair_authorized&&
            Instant::now().checked_duration_since(self.checked_at).is_some_and(|age|age<=Duration::from_millis(100))
    }
}

pub struct GamepadSupervisor<T:ParentChannel>{
    channel:T,arbitration:Arbitration,owner:Owner,profile:Uuid,client:Uuid,helper:ProcessIdentity,
    identities:Vec<Identity>,journal:Option<Snapshot>,reservation:Option<Uuid>,request_id:u64,input_sequence:u64,
    deadline:Instant,sealed:bool,active:bool,
}
impl<T:ParentChannel> GamepadSupervisor<T>{
    /// Take ownership only from the final prepared receipt, after native consent
    /// for exactly these slots. The caller must have admitted the exact channel,
    /// artifact, child and driver. A successful return means the complete device
    /// identity set is durably reserved, not that any controller is active.
    pub fn prepared(database:&mut Database,profile:Uuid,client:Uuid,owner:Owner,
        selected_slots:&[u8],remaining:Duration,mut channel:T,reply:Reply)->Result<Self>{
        let arbitration=match coordination_directory(&database.root,profile).and_then(|directory|Arbitration::open(&directory)){
            Ok(value)=>value,Err(error)=>{let _=channel.stop();return Err(error);}
        };
        let now=Instant::now();let helper=channel.helper();
        let mut supervisor=Self{channel,arbitration,owner,profile,client,helper,identities:Vec::new(),
            journal:None,reservation:None,request_id:reply.id,input_sequence:0,deadline:now+remaining.min(Duration::from_secs(30*60)),
            sealed:false,active:false};
        let result=(||{
            let identities=reply.identities.as_ref().ok_or_else(invalid)?;
            if profile.is_nil()||client.is_nil()||remaining.is_zero()||remaining>Duration::from_secs(30*60)||
               reply.protocol!=wire::VERSION||reply.id==0||reply.id>=wire::MAX_ID||reply.binding.owner()!=owner||
               reply.status!="ok"||!reply.complete||reply.phase!="prepared"||reply.reason.is_some()||
               !valid_ids(identities,false)||identities.iter().map(|id|id.slot).ne(selected_slots.iter().copied())||
               helper.process_id==0||helper.creation_time==0{return Err(invalid());}
            supervisor.identities=identities.clone();
            supervisor.journal=Some(journal::reserve(database,journal::Reservation{profile,client,
                owner:journal_owner(supervisor.owner),helper,helper_sha256:supervisor.channel.helper_sha256().clone(),
                identities:ids(identities)})?);
            supervisor.reservation=supervisor.journal.as_ref().map(|value|value.record().reservation());
            supervisor.arbitration.reserve(database,supervisor.journal.as_ref().ok_or_else(stopped)?)?;
            Ok(())
        })();
        if let Err(error)=result{supervisor.sealed=true;let _=supervisor.channel.stop();return Err(error);}
        Ok(supervisor)
    }
    fn exchange(&mut self,action:Action,timeout:Duration)->Result<(u64,Reply)>{
        let(id,reply)=self.channel.exchange(self.owner.into(),action,timeout)?;
        if id<=self.request_id||id>wire::MAX_ID{return Err(invalid());}
        self.request_id=id;Ok((id,reply))
    }
    fn permitted(&self,authority:&Authority)->bool{
        !self.sealed&&Instant::now()<self.deadline&&authority.permits(self.owner)&&
            self.channel.helper()==self.helper
    }
    fn validate(&self,reply:&Reply,id:u64)->Result<()>{
        if reply.protocol!=wire::VERSION||reply.id!=id||reply.binding.owner()!=self.owner||!reply.complete||
           !matches!(reply.status.as_str(),"ok"|"error")||
           !matches!(reply.phase.as_str(),"prepared"|"active"|"cleanup_required"|"revoked")||
           reply.reason.as_ref().is_some_and(|reason|reason.is_empty()||reason.len()>64||
               !reason.bytes().all(|b|b.is_ascii_lowercase()||b==b'_'))||
           reply.identities.as_ref().is_some_and(|identities|!valid_ids(identities,true)||
               identities.iter().any(|identity|!self.identities.contains(identity))){return Err(invalid());}
        Ok(())
    }
    fn retain_cleanup(&mut self,database:&mut Database,reply:Option<&Reply>)->Result<()>{
        self.retain_cleanup_inner(database,reply,None)
    }
    fn retain_cleanup_inner(&mut self,database:&mut Database,reply:Option<&Reply>,deadline:Option<Instant>)->Result<()>{
        self.sealed=true;self.active=false;
        let stop=match deadline{Some(end)=>self.channel.stop_until(end),None=>self.channel.stop()};
        if deadline.is_some_and(|end|Instant::now()>=end){return Err(stopped());}
        let Some(snapshot)=self.journal.as_ref()else{return Err(stopped());};
        let(state,pending)=match reply{
            Some(reply)if reply.phase=="revoked"=>(CleanupState::Revoked,reply.identities.as_deref()),
            Some(reply)if reply.phase=="cleanup_required"=>(CleanupState::CleanupRequired,reply.identities.as_deref()),
            _=>(CleanupState::CleanupRequired,None),
        };
        let pending=pending.map(ids);
        // Stop before SQLite can block on another writer or disk fault. The
        // existing durable row still blocks reassignment throughout shutdown.
        let updated=journal::record_cleanup(database,snapshot,journal_owner(self.owner),self.helper,state,pending.as_deref());
        // An uncertain commit remains authoritative
        // on disk; subsequent recovery must reload instead of trusting memory.
        self.journal=Some(updated?);
        if deadline.is_some_and(|end|Instant::now()>=end){return Err(stopped());}
        self.arbitration.mirror(database,self.journal.as_ref().ok_or_else(stopped)?)?;
        if deadline.is_some_and(|end|Instant::now()>=end||stop.is_err()){return Err(stopped());}Ok(())
    }
    pub fn activate(&mut self,database:&mut Database,authority:&Authority)->Result<()>{
        if !self.permitted(authority)||self.active{let _=self.retain_cleanup(database,None);return Err(stopped());}
        let snapshot=self.journal.as_ref().ok_or_else(stopped)?;
        match journal::begin_activation(database,snapshot){
            Ok(next)=>self.journal=Some(next),
            Err(error)=>{self.sealed=true;let _=self.channel.stop();return Err(error);}
        }
        // The commit above is the only path to emitting activation. A pipe
        // failure after this point leaves activation_attempted durable.
        let result=(||{
            self.arbitration.mirror(database,self.journal.as_ref().ok_or_else(stopped)?)?;
            // Committing FULL WAL may wait behind another writer. Do not emit
            // Activate using an authority observation that aged during commit.
            if !self.permitted(authority){return Err(stopped());}
            let(id,reply)=self.exchange(Action::Activate{identities:self.identities.clone()},Duration::from_secs(5))?;self.validate(&reply,id)?;
            if reply.status!="ok"||reply.phase!="active"||reply.reason.is_some()||
               reply.identities.as_deref()!=Some(self.identities.as_slice())||Instant::now()>=self.deadline{return Err(stopped());}
            let next=journal::record_active(database,self.journal.as_ref().ok_or_else(stopped)?,
                journal_owner(self.owner),self.helper,&ids(&self.identities))?;
            self.journal=Some(next);
            self.arbitration.mirror(database,self.journal.as_ref().ok_or_else(stopped)?)?;
            self.active=true;Ok(())
        })();
        if result.is_err(){let _=self.retain_cleanup(database,None);}result
    }
    /// Call <=100 ms on a broker authority lane independent of remote input.
    /// Caller must supervise the child separately if any exchange blocks this
    /// object; this orchestration alone is not a process watchdog.
    pub fn heartbeat(&mut self,database:&mut Database,authority:&Authority)->Result<()>{
        if !self.permitted(authority){
            let _=self.retain_cleanup(database,None);return Err(stopped());
        }
        let result=self.channel.heartbeat(self.owner.into()).and_then(|id|{
            if id<=self.request_id||id>wire::MAX_ID{return Err(invalid());}
            self.request_id=id;Ok(())
        });
        if result.is_err(){let _=self.retain_cleanup(database,None);}result
    }
    pub fn input(&mut self,database:&mut Database,authority:&Authority,sequence:u64,frames:Vec<(u8,RemotePad)>)->Result<()>{
        if !self.active||!self.permitted(authority)||frames.len()>4{
            let _=self.retain_cleanup(database,None);return Err(stopped());
        }
        if sequence==0||sequence>wire::MAX_ID||sequence<=self.input_sequence||
           frames.iter().map(|(slot,_)|*slot).collect::<BTreeSet<_>>().len()!=frames.len()||
           frames.iter().any(|(slot,pad)|!self.identities.iter().any(|id|id.slot==*slot)||Report::from_remote(pad).is_err()){
            let _=self.retain_cleanup(database,None);return Err(invalid());
        }
        self.input_sequence=sequence;
        let action=Action::Input{sequence,pads:frames.into_iter().map(|(slot,state)|Pad{slot,state}).collect()};
        self.active_call(database,action)
    }
    pub fn release(&mut self,database:&mut Database,authority:&Authority,sequence:u64)->Result<()>{
        if !self.active||!self.permitted(authority){let _=self.retain_cleanup(database,None);return Err(stopped());}
        if sequence==0||sequence>wire::MAX_ID||sequence<=self.input_sequence{
            let _=self.retain_cleanup(database,None);return Err(invalid());
        }
        self.input_sequence=sequence;
        self.active_call(database,Action::Release{sequence})
    }
    fn active_call(&mut self,database:&mut Database,action:Action)->Result<()>{
        let result=(||{
            let(id,reply)=self.exchange(action,Duration::from_millis(750))?;self.validate(&reply,id)?;
            if matches!(reply.phase.as_str(),"cleanup_required"|"revoked"){
                self.retain_cleanup(database,Some(&reply))?;return Err(stopped());
            }
            if reply.status!="ok"||reply.phase!="active"||reply.reason.is_some()||
               reply.identities.as_deref()!=Some(self.identities.as_slice()){return Err(invalid());}
            Ok(())
        })();
        if result.is_err()&&!self.sealed{let _=self.retain_cleanup(database,None);}result
    }
    pub fn revoke(&mut self,database:&mut Database)->Result<()>{
        self.revoke_until(database,Instant::now()+Duration::from_millis(1750))
    }
    /// Records cleanup debt, not aggregate device/session retirement. Storage
    /// can complete late: reload/reconcile; no late success or owner deletion.
    pub fn revoke_until(&mut self,database:&mut Database,deadline:Instant)->Result<()>{
        if self.sealed{let contained=self.channel.stop_until(deadline);return if self.journal.is_none()&&Instant::now()<deadline{contained}else{Err(stopped())};}
        let result=(||{
            let remaining=deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero(){return Err(stopped());}
            let(id,reply)=self.exchange(Action::Stop,remaining.min(Duration::from_millis(750)))?;self.validate(&reply,id)?;
            if !matches!(reply.phase.as_str(),"cleanup_required"|"revoked"){return Err(invalid());}
            self.retain_cleanup_inner(database,Some(&reply),Some(deadline))
        })();
        if result.is_err()&&!self.sealed{let _=self.retain_cleanup_inner(database,None,Some(deadline));}result
    }
    /// Uses the authoritative database row after every uncertain commit/pipe
    /// failure. Recovery may retain debt indefinitely; no force-clear or TTL.
    pub fn reconcile(&mut self,database:&mut Database,observer:&mut impl RecoveryObserver)->Result<bool>{
        if !self.sealed{return Err(invalid());}
        let Some(current)=journal::load(database,self.profile)?else{
            if self.arbitration.pending()?.is_some_and(|record|Some(record.reservation())==self.reservation){return Err(stopped());}
            self.journal=None;return Ok(true);
        };
        if current.record().owner()!=journal_owner(self.owner)||current.record().client()!=self.client||
           current.record().helper()!=self.helper||Some(current.record().reservation())!=self.reservation{return Err(invalid());}
        let(current,ready)=journal::reconcile_retained(database,&current,observer)?;
        self.journal=Some(current);
        let current=self.journal.as_ref().ok_or_else(stopped)?;
        if self.arbitration.pending()?.is_some_and(|record|record.reservation()==current.record().reservation()){
            self.arbitration.mirror(database,current)?;
        }
        if !ready{return Ok(false);}
        self.arbitration.release(database,current,observer)?;
        match journal::reconcile(database,current,observer)?{
            Reconciliation::Removed=>{self.journal=None;Ok(true)},
            Reconciliation::Retained(value)=>{self.journal=Some(value);Ok(false)},
        }
    }
}
impl<T:ParentChannel> Drop for GamepadSupervisor<T>{
    fn drop(&mut self){self.sealed=true;let _=self.channel.stop();}
}

/// Concrete normal-user Windows transport adapter. Its client retains the supplied
/// verified executable file guard through all worker-thread lifetimes. It does not itself
/// approve that file, its hash, the signed driver or native consent; construction
/// remains private to a future admitted service path.
#[cfg(windows)]
pub struct WindowsParentChannel{
    client:omniplay_stream_input::gamepad_client::GamepadClient,
    helper:ProcessIdentity,
}
#[cfg(windows)]
impl WindowsParentChannel{
    pub fn from_prepared(client:omniplay_stream_input::gamepad_client::GamepadClient)->Result<Self>{
        let identity=client.helper_witness()?.identity();
        Ok(Self{client,helper:ProcessIdentity{process_id:identity.process_id,creation_time:identity.creation_time}})
    }
    pub fn witness(&self)->Result<omniplay_stream_input::gamepad_process::ProcessWitness>{self.client.helper_witness()}
}
#[cfg(windows)]
impl ParentChannel for WindowsParentChannel{
    fn helper(&self)->ProcessIdentity{self.helper}
    fn helper_sha256(&self)->&ContentHash{self.client.helper_sha256()}
    fn exchange(&mut self,binding:Binding,action:Action,timeout:Duration)->Result<(u64,Reply)>{
        if binding.owner()!=self.client.owner(){let _=self.client.stop();return Err(invalid());}
        self.client.exchange(action,timeout)
    }
    fn heartbeat(&mut self,binding:Binding)->Result<u64>{
        if binding.owner()!=self.client.owner(){let _=self.client.stop();return Err(invalid());}
        self.client.heartbeat()
    }
    fn stop(&mut self)->Result<()>{self.client.stop()}
    fn stop_until(&mut self,deadline:Instant)->Result<()>{self.client.stop_until(deadline)}
}

#[cfg(all(target_os = "linux", any(target_arch = "x86_64", target_arch = "aarch64")))]
pub struct LinuxParentChannel {
    client: omniplay_stream_input::gamepad_linux_host::LinuxGamepadHostClient,
    helper: ProcessIdentity,
}

#[cfg(all(target_os = "linux", any(target_arch = "x86_64", target_arch = "aarch64")))]
impl LinuxParentChannel {
    pub fn new(client: omniplay_stream_input::gamepad_linux_host::LinuxGamepadHostClient) -> Self {
        let identity = client.helper_witness().map(|w| w.identity()).unwrap_or(omniplay_stream_input::gamepad_linux_host::LinuxProcessIdentity { process_id: 0, creation_time: 0 });
        Self {
            client,
            helper: ProcessIdentity { process_id: identity.process_id, creation_time: identity.creation_time },
        }
    }
}

#[cfg(all(target_os = "linux", any(target_arch = "x86_64", target_arch = "aarch64")))]
impl ParentChannel for LinuxParentChannel {
    fn helper(&self) -> ProcessIdentity { self.helper }
    fn helper_sha256(&self) -> &ContentHash { self.client.helper_sha256() }
    fn exchange(&mut self, binding: Binding, action: Action, timeout: Duration) -> Result<(u64, Reply)> {
        if binding.owner() != self.client.owner() { let _ = self.client.stop(); return Err(invalid()); }
        self.client.exchange(action, timeout)
    }
    fn heartbeat(&mut self, binding: Binding) -> Result<u64> {
        if binding.owner() != self.client.owner() { let _ = self.client.stop(); return Err(invalid()); }
        self.client.heartbeat()
    }
    fn stop(&mut self) -> Result<()> { self.client.stop() }
    fn stop_until(&mut self, deadline: Instant) -> Result<()> { self.client.stop_until(deadline) }
}

#[cfg(all(target_os = "linux", any(target_arch = "x86_64", target_arch = "aarch64")))]
pub struct LinuxRecoveryObserver {
    process: omniplay_stream_input::gamepad_linux_host::LinuxProcessWitness,
}

#[cfg(all(target_os = "linux", any(target_arch = "x86_64", target_arch = "aarch64")))]
impl LinuxRecoveryObserver {
    pub fn new(pid: u32, creation_time: u64) -> Self {
        Self { process: omniplay_stream_input::gamepad_linux_host::LinuxProcessWitness::new(pid, creation_time) }
    }
}

#[cfg(all(target_os = "linux", any(target_arch = "x86_64", target_arch = "aarch64")))]
impl RecoveryObserver for LinuxRecoveryObserver {
    fn helper_exited(&mut self, identity: ProcessIdentity) -> Result<bool> {
        self.process.exited(omniplay_stream_input::gamepad_linux_host::LinuxProcessIdentity {
            process_id: identity.process_id,
            creation_time: identity.creation_time,
        })
    }
    fn presence(&mut self, _container: Uuid) -> journal::Presence {
        // Linux uinput virtual gamepads are owned by the open /dev/uinput descriptor
        // and destroyed by the kernel only once the owning process has exited.
        match self.process.exited(self.process.identity()) {
            Ok(true) => journal::Presence::Absent,
            Ok(false) => journal::Presence::Present,
            Err(_) => journal::Presence::Unknown,
        }
    }
}

#[cfg(all(target_os = "linux", any(target_arch = "x86_64", target_arch = "aarch64")))]
pub fn recover_linux_gamepad(database: &mut Database, profile: Uuid, cancel: &std::sync::atomic::AtomicBool) -> Result<bool> {
    use std::sync::atomic::Ordering;
    if cancel.load(Ordering::Acquire) { return Err(stopped()); }
    let directory = coordination_directory(&database.root, profile)?;
    let mut arbitration = Arbitration::open(&directory)?;
    let Some(saved) = journal::load(database, profile)? else { return Ok(arbitration.pending()?.is_none()); };
    let identity = saved.record().helper();
    let mut observer = LinuxRecoveryObserver::new(identity.process_id, identity.creation_time);
    let (saved, ready) = journal::reconcile_retained(database, &saved, &mut observer)?;
    if arbitration.pending()?.is_some_and(|record| record.reservation() == saved.record().reservation()) {
        arbitration.mirror(database, &saved)?;
    }
    if !ready { return Ok(false); }
    arbitration.release(database, &saved, &mut observer)?;
    match journal::reconcile(database, &saved, &mut observer)? {
        Reconciliation::Removed => Ok(true),
        Reconciliation::Retained(_) => Ok(false),
    }
}

/// Live-parent recovery from the actual launch handle plus native provider
/// inventory. Startup after broker loss uses the separate read-only recovery
/// witness below; this live observer never substitutes a newly opened PID.
#[cfg(all(feature="windows-gamepad",target_os="windows",target_env="msvc",any(target_arch="x86_64",target_arch="aarch64")))]
pub struct WindowsRecoveryObserver{process:omniplay_stream_input::gamepad_process::ProcessWitness}
#[cfg(all(feature="windows-gamepad",target_os="windows",target_env="msvc",any(target_arch="x86_64",target_arch="aarch64")))]
impl WindowsRecoveryObserver{
    pub fn from_child(child:std::os::windows::io::BorrowedHandle<'_>)->Result<Self>{
        omniplay_stream_input::gamepad_process::ProcessWitness::from_child(child).map(|process|Self{process})
    }
    pub fn from_witness(process:omniplay_stream_input::gamepad_process::ProcessWitness)->Self{Self{process}}
}
#[cfg(all(feature="windows-gamepad",target_os="windows",target_env="msvc",any(target_arch="x86_64",target_arch="aarch64")))]
impl RecoveryObserver for WindowsRecoveryObserver{
    fn helper_exited(&mut self,identity:ProcessIdentity)->Result<bool>{
        self.process.exited(omniplay_stream_input::gamepad_process::ProcessIdentity{
            process_id:identity.process_id,creation_time:identity.creation_time})
    }
    fn presence(&mut self,container:Uuid)->journal::Presence{
        use omniplay_stream_input::gamepad_winuhid::{DevicePresence,WinUHidProvider};
        match WinUHidProvider::presence(container){
            DevicePresence::Present=>journal::Presence::Present,
            DevicePresence::Absent=>journal::Presence::Absent,
            DevicePresence::Unknown=>journal::Presence::Unknown,
        }
    }
}

/// Read-only OS observation plus journal reconciliation for a saved helper.
/// Explicit-feature startup invokes it only on the dedicated recovery lane;
/// default builds do not run it. Real OS qualification remains deferred.
#[cfg(all(feature="windows-gamepad",target_os="windows",target_env="msvc",any(target_arch="x86_64",target_arch="aarch64")))]
pub fn recover_windows_gamepad(database:&mut Database,profile:Uuid,cancel:&std::sync::atomic::AtomicBool)->Result<bool>{
    use std::sync::atomic::Ordering;
    if cancel.load(Ordering::Acquire){return Err(stopped());}
    let directory=coordination_directory(&database.root,profile)?;
    let mut arbitration=Arbitration::open(&directory)?;
    let Some(saved)=journal::load(database,profile)?else{return Ok(arbitration.pending()?.is_none())};
    let identity=saved.record().helper();
    let process=omniplay_stream_input::gamepad_process::RecoveryProcessWitness::from_saved(
        omniplay_stream_input::gamepad_process::ProcessIdentity{process_id:identity.process_id,creation_time:identity.creation_time})?;
    struct Observer<'a>{process:omniplay_stream_input::gamepad_process::RecoveryProcessWitness,cancel:&'a std::sync::atomic::AtomicBool}
    impl RecoveryObserver for Observer<'_>{
        fn helper_exited(&mut self,identity:ProcessIdentity)->Result<bool>{
            if self.cancel.load(Ordering::Acquire){return Err(stopped());}
            self.process.exited(omniplay_stream_input::gamepad_process::ProcessIdentity{
                process_id:identity.process_id,creation_time:identity.creation_time})
        }
        fn presence(&mut self,container:Uuid)->journal::Presence{
            if self.cancel.load(Ordering::Acquire){return journal::Presence::Unknown;}
            use omniplay_stream_input::gamepad_winuhid::{DevicePresence,WinUHidProvider};
            match WinUHidProvider::presence(container){
                DevicePresence::Present=>journal::Presence::Present,
                DevicePresence::Absent=>journal::Presence::Absent,
                DevicePresence::Unknown=>journal::Presence::Unknown,
            }
        }
    }
    let mut observer=Observer{process,cancel};
    let(saved,ready)=journal::reconcile_retained(database,&saved,&mut observer)?;
    if arbitration.pending()?.is_some_and(|record|record.reservation()==saved.record().reservation()){
        arbitration.mirror(database,&saved)?;
    }
    if !ready{return Ok(false);}
    arbitration.release(database,&saved,&mut observer)?;
    match journal::reconcile(database,&saved,&mut observer)?{
        Reconciliation::Removed=>Ok(true),Reconciliation::Retained(_)=>Ok(false),
    }
}

/// Exact existing native profile layout; never follow a renderer-selected
/// coordination path or derive a shared store from an arbitrary parent folder.
pub fn coordination_directory(root:&std::path::Path,profile:Uuid)->Result<std::path::PathBuf>{
    if !root.is_absolute()||profile.is_nil()||root.file_name().and_then(|name|name.to_str())!=Some(profile.to_string().as_str()){
        return Err(invalid());
    }
    let profiles=root.parent().filter(|path|path.file_name().is_some_and(|name|name=="profiles")).ok_or_else(invalid)?;
    profiles.parent().map(|path|path.to_owned()).ok_or_else(invalid)
}
