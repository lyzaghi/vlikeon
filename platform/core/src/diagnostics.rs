use crate::{Database, util::storage_error};
use omniplay_contracts::{Result, SessionInfo};
use serde::Serialize;
use std::{fs::File, io::Write};

#[derive(Serialize)]
pub struct DiagnosticPreview {
    pub schema_version: u32,
    pub application_version: &'static str,
    pub operating_system: &'static str,
    pub architecture: &'static str,
    pub exported_at_ms: u64,
    pub game_count: u64,
    pub save_count: u64,
    pub active_sessions: Vec<DiagnosticSession>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub native_work: Option<NativeWorkDiagnostic>,
    pub includes_roms: bool,
    pub includes_save_payloads: bool,
    pub includes_filesystem_paths: bool,
    pub includes_browsing_history: bool,
}

#[derive(Serialize)]
pub struct DiagnosticSession { pub runtime_id: String, pub runtime_kind: omniplay_contracts::RuntimeKind, pub state: omniplay_contracts::SessionState, pub error_code: Option<omniplay_contracts::ErrorCode> }

/// Aggregate observations only; never include work/profile/job IDs, paths,
/// process identities, raw errors or recovery/deletion capabilities.
#[derive(Serialize)]
pub struct NativeWorkDiagnostic {
    pub observed_at_ms:u64,
    pub storage:Option<omniplay_contracts::NativeWorkStorageStatus>,
    pub storage_error:Option<omniplay_contracts::ErrorCode>,
    pub recovery:Option<NativeRecoveryDiagnostic>,
    pub recovery_status_error:Option<omniplay_contracts::ErrorCode>,
    pub worker_pending:bool,
}
#[derive(Serialize)]
pub struct NativeRecoveryDiagnostic {
    pub state:&'static str,
    pub error_code:Option<omniplay_contracts::ErrorCode>,
    pub summary:omniplay_contracts::NativeWorkRecoverySummary,
    pub started_at_ms:u64,
    pub finished_at_ms:Option<u64>,
}

impl DiagnosticPreview {
    pub fn collect(database: &Database, sessions: &[SessionInfo]) -> Result<Self> {
        Ok(Self {
            schema_version: 1,
            application_version: env!("CARGO_PKG_VERSION"),
            operating_system: std::env::consts::OS,
            architecture: std::env::consts::ARCH,
            exported_at_ms: crate::now_ms(),
            game_count: database.connection.query_row("SELECT count(*) FROM games",[],|r|r.get(0)).map_err(storage_error)?,
            save_count: database.connection.query_row("SELECT count(*) FROM saves",[],|r|r.get(0)).map_err(storage_error)?,
            active_sessions: sessions.iter().map(|s| DiagnosticSession { runtime_id: s.runtime_id.clone(), runtime_kind: s.runtime_kind, state: s.state, error_code: s.error.as_ref().map(|e|e.code) }).collect(),
            native_work:None,
            includes_roms: false, includes_save_payloads: false, includes_filesystem_paths: false, includes_browsing_history: false,
        })
    }

    pub fn write(&self, destination: &mut File) -> Result<()> {
        let json = serde_json::to_vec_pretty(self).map_err(storage_error)?;
        destination.write_all(&json).map_err(storage_error)?;
        destination.sync_all().map_err(storage_error)
    }
}
