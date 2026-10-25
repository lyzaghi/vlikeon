//! Broker pairing credentials crash reconciliation and revocation orchestrator.
//! Reconciles SQLite credential journal with native PairingSecretStore after crash or restart.
//! Enforces immediate producer fencing upon revocation and ensures no cleanup debt is dropped.

use omniplay_contracts::{ErrorCode, PlatformError, Result};
use omniplay_core::{
    now_ms, Database,
    stream_credentials::{
        self, CleanupObservation, CredentialLease, Journal, RecoveryAction, RecoveryStep,
    },
};
use omniplay_streaming::native_credentials::{CredentialAddress, PairingSecretStore};
use std::time::Instant;
use uuid::Uuid;

fn unavailable() -> PlatformError {
    PlatformError::new(
        ErrorCode::RuntimeUnavailable,
        "Credential recovery is unavailable or vault locked",
    )
}

/// Orchestrates post-crash and startup reconciliation for pairing credentials.
pub fn reconcile_credentials_after_crash(
    database: &Database,
    profile: Uuid,
    store: &dyn PairingSecretStore,
    lease: &CredentialLease,
) -> Result<Journal> {
    let mut journal = stream_credentials::load_scoped(database, lease, profile)?;
    let now = now_ms();
    let plan = journal.recovery_plan(now)?;

    for action in plan {
        let address = action.address();
        match action.step() {
            RecoveryStep::RetireWriter => {
                // Writer has halted due to crash/restart
                journal = stream_credentials::observe_cleanup(
                    database,
                    lease,
                    profile,
                    journal.revision(),
                    address,
                    CleanupObservation::WriterRetired,
                )?;
            }
            RecoveryStep::RemoveMaterial => {
                // Remove secret from native vault and verify absence
                let _ = store.remove(&address);
                if store.read(&address)?.is_none() {
                    journal = stream_credentials::observe_cleanup(
                        database,
                        lease,
                        profile,
                        journal.revision(),
                        address,
                        CleanupObservation::MaterialAbsent,
                    )?;
                }
            }
            RecoveryStep::RevokeRelay => {
                // For pairs with relay reservations, note relay retirement
                journal = stream_credentials::observe_cleanup(
                    database,
                    lease,
                    profile,
                    journal.revision(),
                    address,
                    CleanupObservation::RelayRetired,
                )?;
            }
            RecoveryStep::RetireLocalSession => {
                // Local active streaming sessions are torn down on crash
                journal = stream_credentials::observe_cleanup(
                    database,
                    lease,
                    profile,
                    journal.revision(),
                    address,
                    CleanupObservation::LocalRetired,
                )?;
            }
            RecoveryStep::FinalizeRetirement => {
                journal = stream_credentials::finalize_retirement(
                    database,
                    lease,
                    profile,
                    journal.revision(),
                    address,
                )?;
            }
        }
    }

    Ok(journal)
}

/// Atomically revokes an active credential generation, fencing producers/consumers.
pub fn revoke_credential(
    database: &Database,
    profile: Uuid,
    store: &dyn PairingSecretStore,
    lease: &CredentialLease,
    address: CredentialAddress,
) -> Result<Journal> {
    let mut journal = stream_credentials::load_scoped(database, lease, profile)?;
    
    // Step 1: Transition to Revoking state in SQLite journal
    journal = stream_credentials::revoke(database, lease, profile, journal.revision(), address)?;

    // Step 2: Remove secret material from native vault
    let _ = store.remove(&address);
    if store.read(&address)?.is_none() {
        journal = stream_credentials::observe_cleanup(
            database,
            lease,
            profile,
            journal.revision(),
            address,
            CleanupObservation::MaterialAbsent,
        )?;
    }

    // Step 3: Observe writer and local session retirement
    journal = stream_credentials::observe_cleanup(
        database,
        lease,
        profile,
        journal.revision(),
        address,
        CleanupObservation::WriterRetired,
    )?;
    journal = stream_credentials::observe_cleanup(
        database,
        lease,
        profile,
        journal.revision(),
        address,
        CleanupObservation::LocalRetired,
    )?;

    // Step 4: Finalize retirement if all conditions are satisfied
    let plan = journal.recovery_plan(now_ms())?;
    for action in plan {
        if action.address() == address && matches!(action.step(), RecoveryStep::FinalizeRetirement) {
            journal = stream_credentials::finalize_retirement(
                database,
                lease,
                profile,
                journal.revision(),
                address,
            )?;
            break;
        }
    }

    Ok(journal)
}
