//! Native-only durable gamepad cleanup ownership. No renderer command or device
//! authority is provided here. One bounded record per profile database; a live
//! service still needs cross-profile/OS slot arbitration before activation.
use crate::{Database,util::storage_error};
use omniplay_contracts::{ContentHash,ErrorCode,PlatformError,Result};
use rusqlite::{Connection,OptionalExtension,TransactionBehavior,params};
use serde::{Deserialize,Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

const KEY:&str="native_gamepad_ownership_v1";
const MAX_BYTES:usize=8192;
const MAX_REVISION:u64=10_000_000;
fn invalid()->PlatformError{PlatformError::new(ErrorCode::CorruptData,
    "Native gamepad ownership needs confirmed recovery before reuse")}
fn busy()->PlatformError{PlatformError::new(ErrorCode::Busy,
    "A native gamepad group still owns this profile's device reservation")}
fn conflict()->PlatformError{PlatformError::new(ErrorCode::Conflict,
    "Native gamepad ownership changed; reload its current cleanup record")}

#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct ProcessIdentity{pub process_id:u32,pub creation_time:u64}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct Owner{
    pub grant:Uuid,pub game:Uuid,pub pair:Uuid,pub session:Uuid,pub capture:Uuid,
    pub epoch:u32,pub process:ProcessIdentity,pub window:u64,
}
impl Owner{
    fn valid(&self)->bool{
        [self.grant,self.game,self.pair,self.session,self.capture].iter().all(|id|!id.is_nil())&&
            (1..=1_000_000).contains(&self.epoch)&&self.process.process_id>0&&
            self.process.creation_time>0&&self.window>0
    }
}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct DeviceIdentity{pub slot:u8,pub container:Uuid}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct DeviceRecord{identity:DeviceIdentity,observed_present:bool,removed:bool}
impl DeviceRecord{
    pub fn identity(&self)->&DeviceIdentity{&self.identity}
    pub fn observed_present(&self)->bool{self.observed_present}
    pub fn removed(&self)->bool{self.removed}
}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]#[serde(rename_all="snake_case")]
pub enum Phase{Reserved,ActivationAttempted,Active,CleanupRequired}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct Record{
    schema_version:u32,revision:u64,reservation:Uuid,profile:Uuid,device:Uuid,client:Uuid,owner:Owner,
    helper:ProcessIdentity,helper_sha256:ContentHash,provider:String,
    phase:Phase,activation_attempted:bool,devices:Vec<DeviceRecord>,
}
impl Record{
    pub fn owner(&self)->Owner{self.owner}
    pub fn reservation(&self)->Uuid{self.reservation}
    pub fn revision(&self)->u64{self.revision}
    pub fn profile(&self)->Uuid{self.profile}
    pub fn device(&self)->Uuid{self.device}
    pub fn client(&self)->Uuid{self.client}
    pub fn helper(&self)->ProcessIdentity{self.helper}
    pub fn helper_sha256(&self)->&ContentHash{&self.helper_sha256}
    pub fn phase(&self)->Phase{self.phase}
    pub fn activation_attempted(&self)->bool{self.activation_attempted}
    pub fn devices(&self)->&[DeviceRecord]{&self.devices}
    pub(crate) fn validate(&self)->Result<()>{
        let hash=&self.helper_sha256.0;
        if self.schema_version!=1||self.revision==0||self.revision>MAX_REVISION||
           [self.reservation,self.profile,self.device,self.client].iter().any(Uuid::is_nil)||!self.owner.valid()||
           self.helper.process_id==0||self.helper.creation_time==0||
           self.provider!="windows_winuhid"||hash.len()!=64||
           !hash.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b)){
            return Err(invalid());
        }
        validate_identities(&self.devices.iter().map(|device|device.identity.clone()).collect::<Vec<_>>(),false)?;
        if (!self.activation_attempted&&self.devices.iter().any(|device|device.observed_present))||
           (self.phase==Phase::Reserved&&(self.activation_attempted||self.devices.iter().any(|device|device.removed)))||
           (self.phase==Phase::ActivationAttempted&&(!self.activation_attempted||self.devices.iter().any(|device|device.removed||device.observed_present)))||
           (self.phase==Phase::Active&&(!self.activation_attempted||self.devices.iter().any(|device|device.removed||!device.observed_present))){
            return Err(invalid());
        }
        Ok(())
    }
}
/// Raw bytes are kept privately for compare-and-swap. A stale caller cannot
/// delete/update a newer authorization even if it reuses a public grant UUID.
pub struct Snapshot{record:Record,raw:String}
impl Snapshot{
    pub fn record(&self)->&Record{&self.record}
    pub(crate) fn raw(&self)->&str{&self.raw}
}
pub struct Reservation{
    pub profile:Uuid,pub client:Uuid,pub owner:Owner,pub helper:ProcessIdentity,
    pub helper_sha256:ContentHash,pub identities:Vec<DeviceIdentity>,
}
fn validate_identities(ids:&[DeviceIdentity],allow_empty:bool)->Result<()>{
    if (!allow_empty&&ids.is_empty())||ids.len()>4||ids.iter().any(|id|id.slot>3||id.container.is_nil())||
       ids.iter().map(|id|id.slot).collect::<BTreeSet<_>>().len()!=ids.len()||
       ids.iter().map(|id|id.container).collect::<BTreeSet<_>>().len()!=ids.len(){return Err(invalid());}Ok(())
}
fn encode(record:&Record)->Result<String>{
    record.validate()?;let raw=serde_json::to_string(record).map_err(storage_error)?;
    if raw.len()>MAX_BYTES{return Err(invalid());}Ok(raw)
}
fn read(connection:&Connection,profile:Uuid,device:Uuid)->Result<Option<Snapshot>>{
    if profile.is_nil()||device.is_nil(){return Err(invalid());}
    let raw:Option<(i64,String)>=connection.query_row(
        "SELECT length(CAST(value AS BLOB)),substr(value,1,8193) FROM preferences WHERE key=?1",
        [KEY],|row|Ok((row.get(0)?,row.get(1)?))).optional().map_err(storage_error)?;
    let Some((bytes,raw))=raw else{return Ok(None)};
    if bytes<1||bytes>MAX_BYTES as i64||raw.len()!=bytes as usize{return Err(invalid());}
    let record:Record=serde_json::from_str(&raw).map_err(|_|invalid())?;
    if record.profile!=profile||record.device!=device||encode(&record)?!=raw{return Err(invalid());}
    Ok(Some(Snapshot{record,raw}))
}
pub fn load(database:&Database,profile:Uuid)->Result<Option<Snapshot>>{
    read(&database.connection,profile,database.device_id)
}
/// Dedicated connection for the native recovery lane. Never create a profile,
/// migrate schema or invoke unrelated save/native-work startup maintenance.
pub fn open_recovery(root:&std::path::Path,expected_device:Uuid)->Result<Database>{
    if !root.is_absolute()||expected_device.is_nil(){return Err(invalid());}
    fn ordinary(path:&std::path::Path,directory:bool)->Result<()>{
        let shape=std::fs::symlink_metadata(path).map_err(storage_error)?;
        let mut linked=shape.file_type().is_symlink();
        #[cfg(windows)]{use std::os::windows::fs::MetadataExt;linked|=shape.file_attributes()&0x400!=0;}
        if linked||(directory&&!shape.is_dir())||(!directory&&!shape.is_file()){return Err(invalid());}Ok(())
    }
    ordinary(root,true)?;let path=root.join("omniplay.sqlite3");ordinary(&path,false)?;
    let connection=Connection::open_with_flags(&path,rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE|
        rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX).map_err(storage_error)?;
    connection.busy_timeout(std::time::Duration::from_millis(100)).map_err(storage_error)?;
    connection.execute_batch("PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL; PRAGMA trusted_schema=OFF;").map_err(storage_error)?;
    let version:u32=connection.query_row("PRAGMA user_version",[],|row|row.get(0)).map_err(storage_error)?;
    let(bytes,raw):(i64,String)=connection.query_row("SELECT length(CAST(value AS BLOB)),substr(value,1,38) FROM preferences WHERE key='device_id'",[],|row|Ok((row.get(0)?,row.get(1)?))).map_err(storage_error)?;
    if version!=crate::database::CURRENT_SCHEMA_VERSION||bytes!=36||raw!=expected_device.to_string(){return Err(invalid());}
    let database=Database{connection,root:root.to_owned(),device_id:expected_device};
    require_durable_connection(&database)?;Ok(database)
}
fn require_durable_connection(database:&Database)->Result<()>{
    // Never return an activation ticket from inside an outer uncommitted
    // transaction or a connection whose synchronization was weakened.
    let synchronous:u32=database.connection.query_row("PRAGMA synchronous",[],|row|row.get(0)).map_err(storage_error)?;
    let mode:String=database.connection.query_row("PRAGMA journal_mode",[],|row|row.get(0)).map_err(storage_error)?;
    if !database.connection.is_autocommit()||!matches!(synchronous,2|3)||mode!="wal"{return Err(invalid());}Ok(())
}
pub fn reserve(database:&mut Database,value:Reservation)->Result<Snapshot>{
    require_durable_connection(database)?;
    let record=Record{schema_version:1,revision:1,reservation:Uuid::new_v4(),profile:value.profile,device:database.device_id,
        client:value.client,owner:value.owner,helper:value.helper,helper_sha256:value.helper_sha256,
        provider:"windows_winuhid".into(),phase:Phase::Reserved,activation_attempted:false,
        devices:value.identities.into_iter().map(|identity|DeviceRecord{identity,observed_present:false,removed:false}).collect()};
    let raw=encode(&record)?;
    let tx=database.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage_error)?;
    if tx.execute("INSERT INTO preferences(key,value) VALUES(?1,?2) ON CONFLICT(key) DO NOTHING",params![KEY,&raw]).map_err(storage_error)?!=1{return Err(busy());}
    tx.commit().map_err(storage_error)?;Ok(Snapshot{record,raw})
}
fn replace(database:&mut Database,previous:&Snapshot,mut record:Record)->Result<Snapshot>{
    require_durable_connection(database)?;
    if previous.record.device!=database.device_id||encode(&previous.record)?!=previous.raw{return Err(invalid());}
    record.revision=previous.record.revision.checked_add(1).filter(|revision|*revision<=MAX_REVISION).ok_or_else(invalid)?;
    let raw=encode(&record)?;
    let tx=database.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage_error)?;
    if tx.execute("UPDATE preferences SET value=?1 WHERE key=?2 AND value=?3",params![&raw,KEY,&previous.raw]).map_err(storage_error)?!=1{return Err(conflict());}
    tx.commit().map_err(storage_error)?;Ok(Snapshot{record,raw})
}
fn remove(database:&mut Database,previous:&Snapshot)->Result<()>{
    require_durable_connection(database)?;
    if previous.record.device!=database.device_id||encode(&previous.record)?!=previous.raw{return Err(invalid());}
    let tx=database.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage_error)?;
    if tx.execute("DELETE FROM preferences WHERE key=?1 AND value=?2",params![KEY,&previous.raw]).map_err(storage_error)?!=1{return Err(conflict());}
    tx.commit().map_err(storage_error)?;Ok(())
}
pub(crate) fn require_current(database:&Database,previous:&Snapshot)->Result<()>{
    if previous.record.device!=database.device_id||encode(&previous.record)?!=previous.raw{return Err(invalid());}
    let current:bool=database.connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM preferences WHERE key=?1 AND value=?2)",
        params![KEY,&previous.raw],|row|row.get(0)).map_err(storage_error)?;
    if !current{return Err(conflict());}Ok(())
}
/// Commit this before putting Activate on the private pipe. On any uncertain
/// commit/write result, do not activate; reload and retain the authoritative row.
pub fn begin_activation(database:&mut Database,previous:&Snapshot)->Result<Snapshot>{
    if previous.record.phase!=Phase::Reserved||previous.record.activation_attempted{return Err(conflict());}
    let mut record=previous.record.clone();record.phase=Phase::ActivationAttempted;record.activation_attempted=true;
    replace(database,previous,record)
}
/// Only a validated active response from the exact admitted child can call this.
/// This function is not permission to trust a peer's assertion of device presence.
pub fn record_active(database:&mut Database,previous:&Snapshot,owner:Owner,helper:ProcessIdentity,
    identities:&[DeviceIdentity])->Result<Snapshot>{
    if previous.record.phase!=Phase::ActivationAttempted||previous.record.owner!=owner||previous.record.helper!=helper||
       previous.record.devices.iter().map(|device|&device.identity).ne(identities.iter()){return Err(conflict());}
    let mut record=previous.record.clone();record.phase=Phase::Active;
    for device in &mut record.devices{device.observed_present=true;}
    replace(database,previous,record)
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum CleanupState{CleanupRequired,Revoked}
/// Apply only after private-pipe reply authentication, ID/completion checks and
/// exact child identity validation. None remains unknown, never an empty set.
/// Even a complete receipt stays journaled until helper exit is proven below.
pub fn record_cleanup(database:&mut Database,previous:&Snapshot,owner:Owner,helper:ProcessIdentity,
    state:CleanupState,pending:Option<&[DeviceIdentity]>)->Result<Snapshot>{
    require_current(database,previous)?;
    if previous.record.owner!=owner||previous.record.helper!=helper{return Err(conflict());}
    let mut record=previous.record.clone();record.phase=Phase::CleanupRequired;
    if let Some(pending)=pending{
        validate_identities(pending,true)?;
        if (state==CleanupState::Revoked&&!pending.is_empty())||
           (state==CleanupState::CleanupRequired&&pending.is_empty())||
           pending.iter().any(|id|!record.devices.iter().any(|device|!device.removed&&device.identity==*id)){
            return Err(invalid());
        }
        for device in &mut record.devices{if !pending.contains(&device.identity){device.removed=true;}}
    }
    if record==previous.record{return Ok(Snapshot{record,raw:previous.raw.clone()});}
    replace(database,previous,record)
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Presence{Present,Absent,Unknown}
/// Implement in the trusted native supervisor, using an owned process handle
/// or positive proof that the saved process instance is gone, and the actual
/// exact-container OS query. Unknown/access refusal alone is not exit.
pub trait RecoveryObserver{
    fn helper_exited(&mut self,identity:ProcessIdentity)->Result<bool>;
    fn presence(&mut self,container:Uuid)->Presence;
}
pub enum Reconciliation{Retained(Snapshot),Removed}
/// No TTL/clock/empty-inventory cleanup. A created device needs a prior positive
/// presence observation or an authenticated removal receipt before absence can
/// clear it. An unattempted reservation is safe only after exact helper exit.
pub fn reconcile(database:&mut Database,previous:&Snapshot,observer:&mut impl RecoveryObserver)->Result<Reconciliation>{
    let(current,ready)=reconcile_retained(database,previous,observer)?;
    if ready{remove(database,&current)?;Ok(Reconciliation::Removed)}else{Ok(Reconciliation::Retained(current))}
}
/// Preserve the complete profile retirement proof while a shared per-user
/// reservation is released. The caller must release shared ownership BEFORE
/// final profile deletion; missing profile metadata cannot prove shared cleanup.
pub fn reconcile_retained(database:&mut Database,previous:&Snapshot,observer:&mut impl RecoveryObserver)->Result<(Snapshot,bool)>{
    require_current(database,previous)?;
    if !observer.helper_exited(previous.record.helper)?{
        return Ok((Snapshot{record:previous.record.clone(),raw:previous.raw.clone()},false));
    }
    let mut record=previous.record.clone();record.phase=Phase::CleanupRequired;
    for device in &mut record.devices{
        if !record.activation_attempted{device.removed=true;continue;}
        if device.removed{continue;}
        match observer.presence(device.identity.container){
            Presence::Present=>device.observed_present=true,
            Presence::Absent if device.observed_present=>device.removed=true,
            _=>{},
        }
    }
    // Recheck the exact process before publishing the final release. The native
    // observer must verify the saved creation identity, rather than read exit
    // status from an unrelated process that reused the old numeric PID.
    let ready=record.devices.iter().all(|device|device.removed)&&observer.helper_exited(record.helper)?;
    let current=if record==previous.record{Snapshot{record,raw:previous.raw.clone()}}else{replace(database,previous,record)?};
    Ok((current,ready))
}

// Authored regression cases only. Execution is deferred to the owner's frozen
// Antigravity batch; none of these fake observations qualifies an OS device.
#[cfg(test)]
mod tests{
    use super::*;
    struct TemporaryDirectory(std::path::PathBuf);
    impl Drop for TemporaryDirectory{fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);}}
    // Fields drop in declaration order: close SQLite before the owned directory.
    struct Fixture{database:Database,path:TemporaryDirectory,profile:Uuid,client:Uuid,owner:Owner,helper:ProcessIdentity,ids:Vec<DeviceIdentity>}
    impl Fixture{
        fn new()->Self{
            let path=std::env::temp_dir().join(format!("vlikeon-gamepad-journal-{}",Uuid::new_v4()));
            let database=Database::open(&path).unwrap();
            Self{database,path:TemporaryDirectory(path),profile:Uuid::new_v4(),client:Uuid::new_v4(),
                owner:Owner{grant:Uuid::new_v4(),game:Uuid::new_v4(),pair:Uuid::new_v4(),
                    session:Uuid::new_v4(),capture:Uuid::new_v4(),epoch:1,
                    process:ProcessIdentity{process_id:42,creation_time:500},window:800},
                helper:ProcessIdentity{process_id:43,creation_time:501},
                ids:vec![DeviceIdentity{slot:0,container:Uuid::new_v4()},DeviceIdentity{slot:2,container:Uuid::new_v4()}]}
        }
        fn reserve(&mut self)->Snapshot{reserve(&mut self.database,Reservation{profile:self.profile,
            client:self.client,owner:self.owner,helper:self.helper,helper_sha256:ContentHash("a".repeat(64)),identities:self.ids.clone()}).unwrap()}
        fn load(&self)->Snapshot{load(&self.database,self.profile).unwrap().unwrap()}
    }
    struct Observer{exited:bool,presence:Presence}
    impl RecoveryObserver for Observer{
        fn helper_exited(&mut self,_identity:ProcessIdentity)->Result<bool>{Ok(self.exited)}
        fn presence(&mut self,_container:Uuid)->Presence{self.presence}
    }
    #[test]fn stale_reservation_cannot_clear_identical_replacement(){
        let mut fixture=Fixture::new();let old=fixture.reserve();
        let mut observer=Observer{exited:true,presence:Presence::Absent};
        assert!(matches!(reconcile(&mut fixture.database,&old,&mut observer).unwrap(),Reconciliation::Removed));
        let replacement=fixture.reserve();
        assert_ne!(replacement.record.reservation,old.record.reservation);
        assert!(reconcile(&mut fixture.database,&old,&mut observer).is_err());
        assert_eq!(fixture.load().raw,replacement.raw);
    }
    #[test]fn absent_never_observed_attempt_is_retained_across_reopen(){
        let mut fixture=Fixture::new();let reserved=fixture.reserve();
        let attempted=begin_activation(&mut fixture.database,&reserved).unwrap();
        // A second connection reconstructs only durable state, as after losing
        // the old owner's in-memory snapshot. No active receipt was delivered.
        let mut reopened=Database::open(&fixture.path.0).unwrap();
        let persisted=load(&reopened,fixture.profile).unwrap().unwrap();assert_eq!(persisted.raw,attempted.raw);
        for presence in [Presence::Unknown,Presence::Absent]{
            let current=load(&reopened,fixture.profile).unwrap().unwrap();
            let mut observer=Observer{exited:true,presence};
            assert!(matches!(reconcile(&mut reopened,&current,&mut observer).unwrap(),Reconciliation::Retained(_)));
        }
        let current=load(&reopened,fixture.profile).unwrap().unwrap();
        let mut observer=Observer{exited:true,presence:Presence::Present};
        let present=match reconcile(&mut reopened,&current,&mut observer).unwrap(){Reconciliation::Retained(value)=>value,_=>panic!("presence must retain")};
        assert!(present.record.devices.iter().all(|device|device.observed_present&&!device.removed));
        observer.presence=Presence::Absent;
        assert!(matches!(reconcile(&mut reopened,&present,&mut observer).unwrap(),Reconciliation::Removed));
    }
    #[test]fn complete_cleanup_receipt_requires_exact_child_retirement(){
        let mut fixture=Fixture::new();let reserved=fixture.reserve();
        let attempted=begin_activation(&mut fixture.database,&reserved).unwrap();
        let active=record_active(&mut fixture.database,&attempted,fixture.owner,fixture.helper,&fixture.ids).unwrap();
        let closed=record_cleanup(&mut fixture.database,&active,fixture.owner,fixture.helper,CleanupState::Revoked,Some(&[])).unwrap();
        let mut observer=Observer{exited:false,presence:Presence::Unknown};
        assert!(matches!(reconcile(&mut fixture.database,&closed,&mut observer).unwrap(),Reconciliation::Retained(_)));
        observer.exited=true;
        assert!(matches!(reconcile(&mut fixture.database,&closed,&mut observer).unwrap(),Reconciliation::Removed));
    }
    #[test]fn failed_activation_commit_does_not_publish_attempted_state(){
        let mut fixture=Fixture::new();let reserved=fixture.reserve();
        fixture.database.connection.execute_batch(
            "CREATE TEMP TRIGGER reject_gamepad_activation BEFORE UPDATE ON preferences WHEN NEW.key='native_gamepad_ownership_v1' BEGIN SELECT RAISE(ABORT,'synthetic write fault'); END;").unwrap();
        assert!(begin_activation(&mut fixture.database,&reserved).is_err());
        assert_eq!(fixture.load().raw,reserved.raw);
        assert_eq!(fixture.load().record.phase,Phase::Reserved);
    }
    #[test]fn outer_transaction_cannot_issue_activation_ticket(){
        let mut fixture=Fixture::new();let reserved=fixture.reserve();
        fixture.database.connection.execute_batch("BEGIN IMMEDIATE").unwrap();
        assert!(begin_activation(&mut fixture.database,&reserved).is_err());
        fixture.database.connection.execute_batch("ROLLBACK").unwrap();
        assert_eq!(fixture.load().raw,reserved.raw);
    }
    #[test]fn unknown_or_foreign_pending_set_does_not_erase_devices(){
        let mut fixture=Fixture::new();let reserved=fixture.reserve();
        let attempted=begin_activation(&mut fixture.database,&reserved).unwrap();
        let unknown=record_cleanup(&mut fixture.database,&attempted,fixture.owner,fixture.helper,CleanupState::CleanupRequired,None).unwrap();
        assert!(unknown.record.devices.iter().all(|device|!device.removed));
        let foreign=[DeviceIdentity{slot:1,container:Uuid::new_v4()}];
        assert!(record_cleanup(&mut fixture.database,&unknown,fixture.owner,fixture.helper,CleanupState::CleanupRequired,Some(&foreign)).is_err());
        assert_eq!(fixture.load().raw,unknown.raw);
        assert!(record_cleanup(&mut fixture.database,&unknown,fixture.owner,fixture.helper,CleanupState::CleanupRequired,Some(&[])).is_err());
    }
    #[test]fn recovery_connection_opens_only_the_existing_matching_device(){
        let mut fixture=Fixture::new();let reserved=fixture.reserve();
        assert!(open_recovery(&fixture.path.0,Uuid::new_v4()).is_err());
        let recovered=open_recovery(&fixture.path.0,fixture.database.device_id).unwrap();
        assert_eq!(load(&recovered,fixture.profile).unwrap().unwrap().raw,reserved.raw);
        let timeout:u32=recovered.connection.query_row("PRAGMA busy_timeout",[],|row|row.get(0)).unwrap();
        assert_eq!(timeout,100);
        let missing=fixture.path.0.join("not-created-by-recovery");
        assert!(open_recovery(&missing,fixture.database.device_id).is_err());assert!(!missing.exists());
    }
    #[test]fn recovery_connection_refuses_schema_changes_without_migrating(){
        let fixture=Fixture::new();
        fixture.database.connection.execute_batch("PRAGMA user_version=999999").unwrap();
        assert!(open_recovery(&fixture.path.0,fixture.database.device_id).is_err());
        let version:u32=fixture.database.connection.query_row("PRAGMA user_version",[],|row|row.get(0)).unwrap();
        assert_eq!(version,999999);
    }
    #[test]fn shared_owner_survives_connection_loss_and_blocks_another_profile(){
        use crate::stream_gamepad_arbitration::Arbitration;
        let mut first=Fixture::new();let mut second=Fixture::new();
        let a=first.reserve();let b=second.reserve();
        let mut shared=Arbitration::open(&first.path.0).unwrap();
        let mut competing=Arbitration::open(&first.path.0).unwrap();
        shared.reserve(&first.database,&a).unwrap();
        assert!(competing.reserve(&second.database,&b).is_err());drop(shared);
        let reopened=Arbitration::open(&first.path.0).unwrap();
        assert_eq!(reopened.pending().unwrap().unwrap().reservation(),a.record.reservation());
        assert!(competing.reserve(&second.database,&b).is_err());
    }
    #[test]fn shared_mirror_refuses_a_stale_profile_revision(){
        use crate::stream_gamepad_arbitration::Arbitration;
        let mut fixture=Fixture::new();let reserved=fixture.reserve();
        let mut shared=Arbitration::open(&fixture.path.0).unwrap();shared.reserve(&fixture.database,&reserved).unwrap();
        let attempted=begin_activation(&mut fixture.database,&reserved).unwrap();
        shared.mirror(&fixture.database,&attempted).unwrap();
        assert!(shared.mirror(&fixture.database,&reserved).is_err());
        assert_eq!(shared.pending().unwrap().unwrap().phase(),Phase::ActivationAttempted);
    }
    #[test]fn shared_release_keeps_profile_proof_and_cannot_delete_replacement(){
        use crate::stream_gamepad_arbitration::Arbitration;
        let mut first=Fixture::new();let mut second=Fixture::new();let a=first.reserve();let b=second.reserve();
        let mut shared=Arbitration::open(&first.path.0).unwrap();shared.reserve(&first.database,&a).unwrap();
        let mut observer=Observer{exited:false,presence:Presence::Absent};
        assert!(shared.release(&first.database,&a,&mut observer).is_err());
        observer.exited=true;
        let(proof,ready)=reconcile_retained(&mut first.database,&a,&mut observer).unwrap();assert!(ready);
        shared.mirror(&first.database,&proof).unwrap();shared.release(&first.database,&proof,&mut observer).unwrap();
        assert_eq!(first.load().raw,proof.raw);assert!(shared.pending().unwrap().is_none());
        shared.reserve(&second.database,&b).unwrap();
        shared.release(&first.database,&proof,&mut observer).unwrap();
        assert_eq!(shared.pending().unwrap().unwrap().reservation(),b.record.reservation());
    }
    #[test]fn deleted_profile_proof_cannot_force_clear_shared_ownership(){
        use crate::stream_gamepad_arbitration::Arbitration;
        let mut fixture=Fixture::new();let reserved=fixture.reserve();
        let mut shared=Arbitration::open(&fixture.path.0).unwrap();shared.reserve(&fixture.database,&reserved).unwrap();
        let mut observer=Observer{exited:true,presence:Presence::Absent};
        let(proof,ready)=reconcile_retained(&mut fixture.database,&reserved,&mut observer).unwrap();assert!(ready);
        assert!(matches!(reconcile(&mut fixture.database,&proof,&mut observer).unwrap(),Reconciliation::Removed));
        assert!(shared.release(&fixture.database,&proof,&mut observer).is_err());
        assert!(shared.pending().unwrap().is_some());
    }
}
