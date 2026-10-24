#![forbid(unsafe_code)]

pub mod database;
pub mod diagnostics;
pub mod library;
pub mod installed;
pub mod permissions;
pub mod policy;
pub mod saves;
pub mod save_origins;
pub mod save_maintenance;
pub mod save_payloads;
pub mod input;
pub mod core_options;
pub mod sessions;
pub mod updates;
pub mod util;
pub mod router;
pub mod assistance;
pub mod web_sessions;
pub mod performance;
pub mod firmware;
pub mod native_work_storage;
pub mod stream_gamepad_journal;
pub mod stream_credentials;
pub mod stream_credential_recovery;
pub mod stream_credential_lease;
pub mod stream_credential_writer;
pub mod stream_signal_operations;
pub mod stream_signal_authority;
pub mod stream_gamepad_arbitration;
pub mod media_import;
pub mod asset_storage;
pub mod save_backup;
pub mod library_recovery;
pub mod save_orphan_recovery;
pub mod manual_recovery;

#[cfg(test)]
mod backup_recovery_tests;

pub use database::Database;

pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis().min(u64::MAX as u128) as u64).unwrap_or(0)
}
