//! One-step native pairing cleanup orchestration. No startup/product caller or
//! default owner implementation exists yet. Never dispatch from renderer IPC.
use crate::{Database,stream_credentials::{self as journal,Journal,Record,Phase,RecoveryStep,CleanupObservation}};
use crate::stream_credential_lease::CredentialLease;
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_streaming::{RelayOperation,verify_grant_for_retirement,native_relay::NativePairRelay,
    native_credentials::{CredentialAddress,SecretKind,PairingSecretStore,delete_material,load_identity}};
use serde::Deserialize;
use std::sync::atomic::{AtomicBool,Ordering};
use std::time::{Duration,Instant};
use uuid::Uuid;

/// An explicit observation by the native owner, not a wire/renderer assertion.
pub enum Retirement{Confirmed,Pending}

/// The real implementation must hold cross-process profile/device exclusion
/// against ALL credential writers and recovery workers for the entire step,
/// including vault I/O and the final journal commit. A Rust mutable borrow alone
/// does not establish that exclusion. Every operation must honor the supplied
/// cancellation deadline where its OS/network backend supports it. There is no
/// default "everything retired" implementation.
pub trait NativeRecoveryOwner{
    fn check_exclusive(&mut self,profile:Uuid,device:Uuid)->Result<()>;
    /// Confirm only after the actual generation's last writer is contained and
    /// cannot later publish. Missing PID, timeout or old journal age is unknown.
    fn retire_writer(&mut self,record:&Record,deadline:Instant)->Result<Retirement>;
    /// Fence/stop every local signing/session/input/capture producer that can
    /// use this generation. A disconnected channel alone is not confirmation.
    fn retire_local(&mut self,record:&Record,deadline:Instant)->Result<Retirement>;
    /// Use the journal's exact signed grant/audience and pinned native identity
    /// for authenticated revocation. No renderer URL or alternate key fallback.
    /// Unknown replies stay pending. Expiry requires the relay protocol's actual
    /// verified retirement rule; this executor does not infer it from now.
    fn revoke_relay(&mut self,record:&Record,deadline:Instant)->Result<Retirement>;
}

pub enum StepOutcome{
    /// Durable metadata changed. Caller reloads before selecting another step.
    Committed{revision:u64},
    Pending,
    /// Active records still need separate live operation-authority admission.
    RevalidationRequired,
    AlreadyRetired,
}
fn conflict()->PlatformError{PlatformError::new(ErrorCode::Conflict,"Reload native pairing recovery state before continuing")}
fn check_deadline(deadline:Instant)->Result<()>{
    if Instant::now()>=deadline{return Err(PlatformError::new(ErrorCode::DeadlineExceeded,"Native pairing recovery did not finish within its operation deadline"));}Ok(())
}
fn checked_now(now:u64,started:Instant)->Result<u64>{
    now.checked_add(u64::try_from(started.elapsed().as_millis()).map_err(|_|conflict())?).ok_or_else(conflict)
}

/// Only metadata transitions, with the real profile lease retained. This does
/// not supply/default any writer, session, vault or relay retirement receipt.
/// Native recovery controller explicitly selects an exact generation/revision;
/// never run a returned plan automatically while a consent flow is pending.
pub fn advance_metadata(database:&Database,lease:&CredentialLease,profile:Uuid,address:CredentialAddress,
    expected:u64,now:u64,deadline:Instant)->Result<StepOutcome>{
    let started=Instant::now();check_deadline(deadline)?;lease.check(database,profile)?;
    if deadline.saturating_duration_since(started)>Duration::from_secs(30){return Err(conflict());}
    address.validate()?;
    if address.profile()!=profile||address.device()!=database.device_id{return Err(conflict());}
    let snapshot=journal::load_scoped(database,lease,profile)?;check_deadline(deadline)?;
    if snapshot.revision()!=expected||!snapshot.records().iter().any(|record|record.address()==address){return Err(conflict());}
    let actions=snapshot.recovery_plan(checked_now(now,started)?)?;
    let Some(action)=actions.iter().find(|action|action.address()==address)else{return Ok(StepOutcome::AlreadyRetired);};
    let updated=match action.step(){
        RecoveryStep::FenceGeneration=>journal::revoke(database,lease,profile,expected,address)?,
        RecoveryStep::FinalizeRetirement=>journal::finalize_retirement(database,lease,profile,expected,address)?,
        RecoveryStep::RevalidateBeforeUse=>return Ok(StepOutcome::RevalidationRequired),
        // Keep every unobserved external producer/deletion action pending.
        _=>return Ok(StepOutcome::Pending),
    };
    check_deadline(deadline)?;lease.check(database,profile)?;
    Ok(StepOutcome::Committed{revision:updated.revision()})
}

#[derive(Deserialize)]#[serde(deny_unknown_fields)]
struct RelayRetirement{revoked:bool,retirement_protocol:u32,pair_id:Uuid,grant_hash:String,reason:String}

/// Explicit native user revocation, separate from the advisory recovery plan.
/// Active is not an abandoned candidate. Fence first and retain every producer/
/// session/relay/material debt for the subsequent actual cleanup owners.
pub fn revoke_active(database:&Database,lease:&CredentialLease,profile:Uuid,address:CredentialAddress,
    expected:u64,deadline:Instant)->Result<Journal>{
    let started=Instant::now();check_deadline(deadline)?;
    if deadline.saturating_duration_since(started)>Duration::from_secs(30){return Err(conflict());}
    let snapshot=journal::load_scoped(database,lease,profile)?;check_deadline(deadline)?;
    if snapshot.revision()!=expected||!snapshot.records().iter().any(|record|record.address()==address&&record.phase()==Phase::Active){return Err(conflict());}
    let updated=journal::revoke(database,lease,profile,expected,address)?;
    check_deadline(deadline)?;lease.check(database,profile)?;Ok(updated)
}

/// Actual vault removal only after the journal admits RemoveMaterial. Existing
/// writer/local/relay receipts and identity-child ordering are prerequisites;
/// this function does not manufacture any of them. Hold the real lease through
/// synchronous delete, missing-only read-back and final journal publication.
pub fn remove_retired_material(database:&Database,lease:&CredentialLease,profile:Uuid,address:CredentialAddress,
    expected:u64,store:&dyn PairingSecretStore,now:u64,deadline:Instant)->Result<Journal>{
    let started=Instant::now();check_deadline(deadline)?;lease.check(database,profile)?;
    if deadline.saturating_duration_since(started)>Duration::from_secs(30){return Err(conflict());}
    address.validate()?;
    if address.profile()!=profile||address.device()!=database.device_id{return Err(conflict());}
    let snapshot=journal::load_scoped(database,lease,profile)?;check_deadline(deadline)?;
    if snapshot.revision()!=expected{return Err(conflict());}
    let plan=snapshot.recovery_plan(checked_now(now,started)?)?;
    if !plan.iter().any(|action|action.address()==address&&matches!(action.step(),RecoveryStep::RemoveMaterial)){return Err(conflict());}
    lease.check(database,profile)?;check_deadline(deadline)?;
    delete_material(store,&address)?;
    // A locked/error/late read-back cannot become positive absence evidence.
    check_deadline(deadline)?;lease.check(database,profile)?;
    let updated=journal::observe_cleanup(database,lease,profile,expected,address,CleanupObservation::MaterialAbsent)?;
    check_deadline(deadline)?;Ok(updated)
}

/// Concrete paired-relay cleanup branch for the native recovery controller.
/// Local generation fencing, writer containment and local retirement must
/// already be durably confirmed. It is NOT a renderer revoke endpoint or proof
/// that those local owners have stopped merely because a network reply exists.
pub fn retire_pair_remote(database:&Database,lease:&CredentialLease,profile:Uuid,address:CredentialAddress,
    expected:u64,store:&dyn PairingSecretStore,relay:&NativePairRelay,now:u64,deadline:Instant,cancel:&AtomicBool)->Result<Journal>{
    let started=Instant::now();check_deadline(deadline)?;lease.check(database,profile)?;
    if deadline.saturating_duration_since(started)>Duration::from_secs(30)||address.kind()!=SecretKind::PairEncryption||cancel.load(Ordering::Acquire){return Err(conflict());}
    let snapshot=journal::load_scoped(database,lease,profile)?;check_deadline(deadline)?;
    if snapshot.revision()!=expected{return Err(conflict());}
    let record=snapshot.records().iter().find(|record|record.address()==address).ok_or_else(conflict)?;
    if record.phase()!=Phase::Revoking||!record.writer_retired()||!record.local_retired()||record.relay_retired(){return Err(conflict());}
    let metadata=record.grant().ok_or_else(conflict)?;
    let grant=verify_grant_for_retirement(metadata.document(),metadata.audience(),checked_now(now,started)?)?;
    if grant.id!=address.id(){return Err(conflict());}
    // Revoking identity material is retained specifically for authenticating
    // pair retirement. It is not reopened for normal send/receive/ICE operations.
    let identity_record=snapshot.records().iter().find(|identity|identity.address().kind()==SecretKind::DeviceSigning&&
        matches!(identity.phase(),Phase::Active|Phase::Revoking)&&identity.writer_retired()&&!identity.material_absent()&&identity.public_key()==record.public_key()).ok_or_else(conflict)?;
    let identity=load_identity(store,&identity_record.address(),identity_record.public_key())?;
    check_deadline(deadline)?;lease.check(database,profile)?;
    let response=relay.request(&identity,metadata.audience(),RelayOperation::RevokePair{grant:metadata.document().clone()},checked_now(now,started)?,deadline,cancel)?;
    let receipt:RelayRetirement=serde_json::from_value(response).map_err(|_|conflict())?;
    if !receipt.revoked||receipt.retirement_protocol!=1||receipt.pair_id!=address.id()||
        record.grant_hash()!=Some(receipt.grant_hash.as_str())||!matches!(receipt.reason.as_str(),"revoked"|"expired"){return Err(conflict());}
    check_deadline(deadline)?;lease.check(database,profile)?;
    if cancel.load(Ordering::Acquire){return Err(PlatformError::new(ErrorCode::Cancelled,"Native relay retirement was cancelled; reload and reconcile the same grant"));}
    // Never turn HTTP status, a legacy unbound reply, NotFound, local time or a
    // lost response into a durable relay-retired observation.
    journal::observe_cleanup(database,lease,profile,expected,address,CleanupObservation::RelayRetired)
}

/// Executes at most one current plan step, with no retry loop and no detached
/// worker. Caller provides trusted native time, an admitted scoped vault and
/// the real exclusive owner. Side effects are not rolled back on failed/unknown
/// SQL commit; retain the durable fence and reload/re-observe on the next call.
pub fn advance(database:&Database,lease:&CredentialLease,profile:Uuid,address:CredentialAddress,expected_revision:u64,
    store:&dyn PairingSecretStore,owner:&mut dyn NativeRecoveryOwner,now:u64,deadline:Instant)->Result<StepOutcome>{
    let started=Instant::now();check_deadline(deadline)?;
    lease.check(database,profile)?;
    if deadline.saturating_duration_since(started)>Duration::from_secs(30){
        return Err(PlatformError::new(ErrorCode::InvalidRequest,"Native pairing recovery requires a bounded operation deadline"));
    }
    address.validate()?;
    if address.profile()!=profile||address.device()!=database.device_id{return Err(conflict());}
    owner.check_exclusive(profile,database.device_id)?;check_deadline(deadline)?;
    let snapshot=journal::load_scoped(database,lease,profile)?;check_deadline(deadline)?;
    if snapshot.revision()!=expected_revision{return Err(conflict());}
    let record=snapshot.records().iter().find(|record|record.address()==address).ok_or_else(conflict)?;
    let actions=snapshot.recovery_plan(checked_now(now,started)?)?;
    let Some(action)=actions.iter().find(|action|action.address()==address)else{return Ok(StepOutcome::AlreadyRetired);};
    owner.check_exclusive(profile,database.device_id)?;check_deadline(deadline)?;
    let observation=match action.step(){
        RecoveryStep::FenceGeneration=>{
            let updated=journal::revoke(database,lease,profile,expected_revision,address)?;
            return Ok(StepOutcome::Committed{revision:updated.revision()});
        },
        RecoveryStep::RevalidateBeforeUse=>return Ok(StepOutcome::RevalidationRequired),
        RecoveryStep::WaitForPairCleanup=>return Ok(StepOutcome::Pending),
        RecoveryStep::FinalizeRetirement=>{
            let updated=journal::finalize_retirement(database,lease,profile,expected_revision,address)?;
            return Ok(StepOutcome::Committed{revision:updated.revision()});
        },
        RecoveryStep::RetireWriter=>match owner.retire_writer(record,deadline)?{
            Retirement::Confirmed=>CleanupObservation::WriterRetired,Retirement::Pending=>return Ok(StepOutcome::Pending),
        },
        RecoveryStep::RetireLocalSessions=>match owner.retire_local(record,deadline)?{
            Retirement::Confirmed=>CleanupObservation::LocalRetired,Retirement::Pending=>return Ok(StepOutcome::Pending),
        },
        RecoveryStep::RevokeRelay=>{
            if record.grant().is_none(){return Err(conflict());}
            match owner.revoke_relay(record,deadline)?{
                Retirement::Confirmed=>CleanupObservation::RelayRetired,Retirement::Pending=>return Ok(StepOutcome::Pending),
            }
        },
        RecoveryStep::RemoveMaterial=>{
            // The current plan requires writer/local/relay retirement first.
            // The original exclusive owner must remain held through remove,
            // read-back and journal observation: no new writer may resurrect it.
            delete_material(store,&address)?;CleanupObservation::MaterialAbsent
        },
    };
    // A slow/failed/uncertain backend never receives a positive journal receipt.
    // Its idempotent retirement or deletion can be re-observed after reload.
    check_deadline(deadline)?;owner.check_exclusive(profile,database.device_id)?;check_deadline(deadline)?;
    lease.check(database,profile)?;
    let updated=journal::observe_cleanup(database,lease,profile,expected_revision,address,observation)?;
    Ok(StepOutcome::Committed{revision:updated.revision()})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream_credential_writer::NativeCredentialWriter;
    use omniplay_streaming::native_credentials::{validate_secret_envelope, SECRET_RECORD_BYTES};
    use std::collections::HashMap;
    use std::sync::Mutex;
    use zeroize::Zeroizing;

    #[derive(Default)]
    struct MemorySecretStore {
        records: Mutex<HashMap<String, Vec<u8>>>,
    }

    impl PairingSecretStore for MemorySecretStore {
        fn read(&self, address: &CredentialAddress) -> Result<Option<Zeroizing<Vec<u8>>>> {
            let key = address.account()?;
            let map = self.records.lock().unwrap();
            if let Some(bytes) = map.get(&key) {
                validate_secret_envelope(address, bytes)?;
                Ok(Some(Zeroizing::new(bytes.clone())))
            } else {
                Ok(None)
            }
        }
        fn write(&self, address: &CredentialAddress, record: &[u8]) -> Result<()> {
            validate_secret_envelope(address, record)?;
            assert_eq!(record.len(), SECRET_RECORD_BYTES);
            let key = address.account()?;
            let mut map = self.records.lock().unwrap();
            if map.contains_key(&key) {
                return Err(PlatformError::new(ErrorCode::Conflict, "Duplicate generation"));
            }
            map.insert(key, record.to_vec());
            Ok(())
        }
        fn remove(&self, address: &CredentialAddress) -> Result<()> {
            let key = address.account()?;
            self.records.lock().unwrap().remove(&key);
            Ok(())
        }
    }

    struct TestRecoveryOwner {
        profile: Uuid,
        device: Uuid,
    }

    impl NativeRecoveryOwner for TestRecoveryOwner {
        fn check_exclusive(&mut self, profile: Uuid, device: Uuid) -> Result<()> {
            if self.profile == profile && self.device == device {
                Ok(())
            } else {
                Err(conflict())
            }
        }
        fn retire_writer(&mut self, _record: &Record, _deadline: Instant) -> Result<Retirement> {
            Ok(Retirement::Confirmed)
        }
        fn retire_local(&mut self, _record: &Record, _deadline: Instant) -> Result<Retirement> {
            Ok(Retirement::Confirmed)
        }
        fn revoke_relay(&mut self, _record: &Record, _deadline: Instant) -> Result<Retirement> {
            Ok(Retirement::Confirmed)
        }
    }

    #[test]
    fn credential_lease_exclusion_writer_lifecycle_and_stepwise_recovery() {
        let base = std::env::temp_dir().join(format!("vlikeon-cred-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let db = Database::open(&base.join("profile")).unwrap();
        let profile_id = Uuid::new_v4();
        let store = MemorySecretStore::default();

        // 1. Verify Windows/POSIX CredentialLease nonblocking exclusion returns Busy on contention
        {
            let _lease1 = CredentialLease::acquire(&db, profile_id).unwrap();
            let Err(err) = CredentialLease::acquire(&db, profile_id) else {
                panic!("Expected Busy on concurrent CredentialLease::acquire");
            };
            assert_eq!(err.code, ErrorCode::Busy);
        }

        // 2. Prepare identity (Reserved -> Writing -> MaterialReady) and activate after local consent
        let (addr, rev_active) = {
            let mut writer = NativeCredentialWriter::open(&db, profile_id, &store).unwrap();
            let dl = Instant::now() + Duration::from_secs(15);
            let cand = writer.prepare_identity(0, dl).unwrap();
            assert_eq!(cand.revision(), 3);
            let rev = writer
                .activate_identity_after_local_consent(cand.address(), cand.revision(), crate::now_ms(), dl)
                .unwrap();
            assert_eq!(rev, 4);
            (cand.address(), rev)
        };

        // 3. Revoke active identity and run stepwise recovery (RetireLocalSessions -> RemoveMaterial -> FinalizeRetirement)
        let lease = CredentialLease::acquire(&db, profile_id).unwrap();
        let dl = Instant::now() + Duration::from_secs(15);
        let mut owner = TestRecoveryOwner {
            profile: profile_id,
            device: db.device_id,
        };

        // Active record returns RevalidationRequired before explicit revocation
        assert!(matches!(
            advance(&db, &lease, profile_id, addr, rev_active, &store, &mut owner, crate::now_ms(), dl).unwrap(),
            StepOutcome::RevalidationRequired
        ));

        let revoked = revoke_active(&db, &lease, profile_id, addr, rev_active, dl).unwrap();
        let mut rev = revoked.revision();

        // Step 1: RetireLocalSessions
        match advance(&db, &lease, profile_id, addr, rev, &store, &mut owner, crate::now_ms(), dl).unwrap() {
            StepOutcome::Committed { revision } => rev = revision,
            _ => panic!("Expected RetireLocalSessions commit"),
        }
        // Step 2: RemoveMaterial (deletes from vault, verifies absence, and transitions to Retired when no pair debt remains)
        match advance(&db, &lease, profile_id, addr, rev, &store, &mut owner, crate::now_ms(), dl).unwrap() {
            StepOutcome::Committed { revision } => rev = revision,
            _ => panic!("Expected RemoveMaterial commit"),
        }
        assert!(store.read(&addr).unwrap().is_none());
        // Step 3: AlreadyRetired
        assert!(matches!(
            advance(&db, &lease, profile_id, addr, rev, &store, &mut owner, crate::now_ms(), dl).unwrap(),
            StepOutcome::AlreadyRetired
        ));

        drop(lease);
        drop(db);
        let _ = std::fs::remove_dir_all(&base);
    }
}
