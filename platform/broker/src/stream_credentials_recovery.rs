//! Broker pairing credentials crash reconciliation and revocation orchestrator.
//! Reconciles SQLite credential journal with native PairingSecretStore after crash or restart.
//! Delegates step-by-step transitions through `omniplay_core::stream_credential_recovery::advance`
//! with a concrete `NativeRecoveryOwner`, ensuring no cleanup debt is dropped.

use omniplay_contracts::Result;
use omniplay_core::{
    now_ms, Database,
    stream_credential_lease::CredentialLease,
    stream_credential_recovery::{
        self, NativeRecoveryOwner, Retirement, StepOutcome,
    },
    stream_credentials::{
        self, Journal, Record, RecoveryStep,
    },
};
use omniplay_streaming::native_credentials::{CredentialAddress, PairingSecretStore};
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Broker-owned recovery witness that confirms local writer/session containment.
pub struct BrokerCredentialRecoveryOwner {
    profile: Uuid,
    device: Uuid,
    writer_contained: bool,
    local_sessions_contained: bool,
    relay_confirmed: bool,
}

impl BrokerCredentialRecoveryOwner {
    pub fn new(
        profile: Uuid,
        device: Uuid,
        writer_contained: bool,
        local_sessions_contained: bool,
        relay_confirmed: bool,
    ) -> Self {
        Self {
            profile,
            device,
            writer_contained,
            local_sessions_contained,
            relay_confirmed,
        }
    }
}

impl NativeRecoveryOwner for BrokerCredentialRecoveryOwner {
    fn check_exclusive(&mut self, profile: Uuid, device: Uuid) -> Result<()> {
        if self.profile != profile || self.device != device {
            return Err(omniplay_contracts::PlatformError::new(
                omniplay_contracts::ErrorCode::PermissionDenied,
                "Credential recovery owner does not match the active profile/device",
            ));
        }
        Ok(())
    }

    fn retire_writer(&mut self, _record: &Record, _deadline: Instant) -> Result<Retirement> {
        Ok(if self.writer_contained {
            Retirement::Confirmed
        } else {
            Retirement::Pending
        })
    }

    fn retire_local(&mut self, _record: &Record, _deadline: Instant) -> Result<Retirement> {
        Ok(if self.local_sessions_contained {
            Retirement::Confirmed
        } else {
            Retirement::Pending
        })
    }

    fn revoke_relay(&mut self, _record: &Record, _deadline: Instant) -> Result<Retirement> {
        Ok(if self.relay_confirmed {
            Retirement::Confirmed
        } else {
            Retirement::Pending
        })
    }
}

/// Orchestrates post-crash and startup reconciliation for pairing credentials
/// by executing bounded recovery steps via `stream_credential_recovery::advance`.
pub fn reconcile_credentials_after_crash(
    database: &Database,
    profile: Uuid,
    store: &dyn PairingSecretStore,
    lease: &CredentialLease,
    owner: &mut dyn NativeRecoveryOwner,
    deadline: Instant,
) -> Result<Journal> {
    // At most 32 bounded state transitions per reconciliation pass
    for _ in 0..32 {
        let journal = stream_credentials::load_scoped(database, lease, profile)?;
        let now = now_ms();
        let plan = journal.recovery_plan(now)?;
        let Some(action) = plan.into_iter().find(|a| {
            !matches!(
                a.step(),
                RecoveryStep::RevalidateBeforeUse | RecoveryStep::WaitForPairCleanup
            )
        }) else {
            return Ok(journal);
        };

        let outcome = stream_credential_recovery::advance(
            database,
            lease,
            profile,
            action.address(),
            journal.revision(),
            store,
            owner,
            now,
            deadline,
        )?;
        match outcome {
            StepOutcome::Committed { .. } => continue,
            StepOutcome::Pending
            | StepOutcome::RevalidationRequired
            | StepOutcome::AlreadyRetired => {
                return stream_credentials::load_scoped(database, lease, profile);
            }
        }
    }

    stream_credentials::load_scoped(database, lease, profile)
}

/// Atomically fences an active credential generation and advances local cleanup steps.
pub fn revoke_credential(
    database: &Database,
    profile: Uuid,
    store: &dyn PairingSecretStore,
    lease: &CredentialLease,
    address: CredentialAddress,
    owner: &mut dyn NativeRecoveryOwner,
) -> Result<Journal> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let journal = stream_credentials::load_scoped(database, lease, profile)?;
    let _ = stream_credential_recovery::revoke_active(
        database,
        lease,
        profile,
        address,
        journal.revision(),
        deadline,
    )?;
    reconcile_credentials_after_crash(database, profile, store, lease, owner, deadline)
}
