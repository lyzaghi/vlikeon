//! One-step native pairing cleanup orchestration. No startup/product caller or
//! default owner implementation exists yet. Never dispatch from renderer IPC.
use crate::{Database,stream_credentials::{self as journal,Record,RecoveryStep,CleanupObservation}};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_streaming::native_credentials::{CredentialAddress,PairingSecretStore,delete_material};
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

/// Executes at most one current plan step, with no retry loop and no detached
/// worker. Caller provides trusted native time, an admitted scoped vault and
/// the real exclusive owner. Side effects are not rolled back on failed/unknown
/// SQL commit; retain the durable fence and reload/re-observe on the next call.
pub fn advance(database:&Database,profile:Uuid,address:CredentialAddress,expected_revision:u64,
    store:&dyn PairingSecretStore,owner:&mut dyn NativeRecoveryOwner,now:u64,deadline:Instant)->Result<StepOutcome>{
    let started=Instant::now();check_deadline(deadline)?;
    if deadline.saturating_duration_since(started)>Duration::from_secs(30){
        return Err(PlatformError::new(ErrorCode::InvalidRequest,"Native pairing recovery requires a bounded operation deadline"));
    }
    address.validate()?;
    if address.profile()!=profile||address.device()!=database.device_id{return Err(conflict());}
    owner.check_exclusive(profile,database.device_id)?;check_deadline(deadline)?;
    let snapshot=journal::load(database,profile)?;check_deadline(deadline)?;
    if snapshot.revision()!=expected_revision{return Err(conflict());}
    let record=snapshot.records().iter().find(|record|record.address()==address).ok_or_else(conflict)?;
    let actions=snapshot.recovery_plan(checked_now(now,started)?)?;
    let Some(action)=actions.iter().find(|action|action.address()==address)else{return Ok(StepOutcome::AlreadyRetired);};
    owner.check_exclusive(profile,database.device_id)?;check_deadline(deadline)?;
    let observation=match action.step(){
        RecoveryStep::FenceGeneration=>{
            let updated=journal::revoke(database,profile,expected_revision,address)?;
            return Ok(StepOutcome::Committed{revision:updated.revision()});
        },
        RecoveryStep::RevalidateBeforeUse=>return Ok(StepOutcome::RevalidationRequired),
        RecoveryStep::WaitForPairCleanup=>return Ok(StepOutcome::Pending),
        RecoveryStep::FinalizeRetirement=>{
            let updated=journal::finalize_retirement(database,profile,expected_revision,address)?;
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
    let updated=journal::observe_cleanup(database,profile,expected_revision,address,observation)?;
    Ok(StepOutcome::Committed{revision:updated.revision()})
}
