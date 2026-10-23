use crate::{ErrorCode, GameManifest, GameSystem, PlatformError, Result, SaveCompatibility, SaveKind, SaveMetadata, SessionConfig, SessionInfo};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub profile_id: Uuid,
    pub protocol_major: u16,
    pub protocol_minor: u16,
    pub id: Uuid,
    pub deadline_ms: u64,
    pub command: Command,
}

impl Request {
    pub fn validate(&self, now_ms: u64) -> Result<()> {
        if self.protocol_major != crate::PROTOCOL_MAJOR || self.protocol_minor > crate::PROTOCOL_MINOR {
            return Err(PlatformError::new(ErrorCode::UnsupportedProtocol, "Control protocol version is not supported"));
        }
        if self.deadline_ms <= now_ms || self.deadline_ms - now_ms > 120_000 {
            return Err(PlatformError::new(ErrorCode::DeadlineExceeded, "Request deadline expired or exceeds policy"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Hello,
    DeviceSnapshot,
    GameInputGet {game_id:Uuid},
    GameInputSet {game_id:Uuid,input:crate::GameInputProfile},
    GameTouchGet {game_id:Uuid},
    GameTouchSet {game_id:Uuid,touch:crate::GameTouchProfile},
    GameCoreOptionsGet {game_id:Uuid,runtime_id:String},
    GameCoreOptionsList {game_id:Uuid},
    GameCoreOptionsSet {game_id:Uuid,runtime_id:String,options:crate::GameCoreOptions},
    RuntimeOptionsGet {runtime_id:String},
    FileSelectImport,
    FileSelectExport { suggested_name: String },
    LibraryList { query: Option<String>, offset: u32, limit: u16 },
    LibraryPage { query:Option<String>,system:Option<GameSystem>,after:Option<String>,limit:u16,#[serde(default)]favorites_only:bool },
    LibraryGet { game_id: Uuid },
    LibraryFavoriteGet {game_id:Uuid},
    LibraryFavoriteSet {game_id:Uuid,favorite:bool},
    LibraryImport { grant_id: Uuid, title: String, system: GameSystem },
    LibraryImportStart { grant_id:Uuid,title:String,system:GameSystem,format_hint:Option<crate::ContentFormat> },
    DiscSelectFolder,
    LibraryImportBundleStart { grant_id:Uuid,entry_name:String,title:String,system:GameSystem,encoding:crate::CueEncoding },
    LibraryImportStatus { job_id:Uuid },
    LibraryImportCancel { job_id:Uuid },
    LibraryRemove { game_id: Uuid },
    StorageUnusedPreview {after_hash:Option<String>},
    StorageUnusedRemove {after_hash:Option<String>,revision:crate::ContentHash},
    SaveBackupExport {save_ids:Vec<Uuid>,destination_grant_id:Uuid},
    SaveBackupRestore {grant_id:Uuid},
    LibraryRecoveryList {after:Option<String>},
    LibraryRecoveryImport {candidate_id:String,revision:crate::ContentHash,title:String,system:GameSystem},
    SaveOrphanPreview {save_id:Uuid,after:Option<String>},
    SaveOrphanRecover {save_id:Uuid,candidate_id:String,revision:crate::ContentHash},
    ManualRecoveryList {after:Option<crate::ContentHash>},
    ManualRecoveryInspect {candidate_id:crate::ContentHash},
    ManualRecoveryExport {candidate_id:crate::ContentHash,revision:crate::ContentHash,destination_grant_id:Uuid},
    ManualRecoveryExtract {grant_id:Uuid,destination_grant_id:Uuid},
    InstalledGameSelect { title: String },
    InstalledGameLaunch { game_id: Uuid },
    InstalledGameProcesses,
    StreamCapturePrepare {game_id:Uuid,pair_id:Uuid,session_id:Uuid,host_public_key:String,grant:crate::StreamGrantDocument},
    StreamCaptureSelect {capture_id:Uuid,session_id:Uuid,choice_id:Uuid},
    StreamCaptureRelease {capture_id:Uuid,session_id:Uuid},
    StreamInputArm {game_id:Uuid,pair_id:Uuid,session_id:Uuid,host_public_key:String,grant:crate::StreamGrantDocument,capture_id:Uuid,capture_window:String},
    StreamInputFrame {grant_id:Uuid,session_id:Uuid,sequence:u64,frame:crate::RemoteInputFrame},
    StreamInputRelease {grant_id:Uuid,session_id:Uuid},
    StreamInputStatus {grant_id:Uuid,session_id:Uuid},
    StreamInputRevoke {grant_id:Uuid,session_id:Uuid},
    AssetReadStart { game_id: Uuid, asset_name:Option<String> },
    AssetRangesStart {game_id:Uuid},
    AssetRangesStatus {job_id:Uuid},
    AssetRangesCollect {job_id:Uuid},
    AssetRangesCancel {job_id:Uuid},
    AssetRangesKeepAlive {set_id:Uuid},
    AssetRangesClose {set_id:Uuid},
    AssetRangeRead {set_id:Uuid,asset_index:u16,offset:u64,limit:u32},
    SaveReadStart { save_id: Uuid, compatibility: SaveCompatibility },
    TransferRead { transfer_id: Uuid, offset: u64, limit: u32 },
    SaveUploadStart { metadata: SaveMetadata, expected_parent_id: Option<Uuid> },
    SaveUploadChunk { transfer_id: Uuid, offset: u64, payload_base64: String },
    SaveUploadCommit { transfer_id: Uuid },
    TransferAbort { transfer_id: Uuid },
    SessionPrepare { game_id: Uuid, runtime_id: String, config: SessionConfig },
    SessionPrepareStart { game_id:Uuid,runtime_id:String,config:SessionConfig },
    SessionPrepareStatus { job_id:Uuid },
    SessionPrepareCancel { job_id:Uuid },
    SessionStart { session_id: Uuid },
    SessionPause { session_id: Uuid },
    SessionResume { session_id: Uuid },
    SessionStop { session_id: Uuid },
    SessionCancelClose { session_id:Uuid },
    SessionDetach { session_id:Uuid },
    SessionDiscStatus { session_id:Uuid },
    SessionSelectDisc { session_id:Uuid,index:u32 },
    SessionList,
    RuntimeList,
    FirmwareList,
    FirmwareReadStart {game_id:Uuid,role:String,expected_hash:crate::ContentHash},
    FirmwareImportSelected { system:GameSystem, role:String },
    FirmwareSelect { system:GameSystem, role:String, content_hash:crate::ContentHash },
    FirmwareForget { content_hash:crate::ContentHash },
    WebSessionBegin { game_id:Uuid, runtime_id:String },
    WebSessionHeartbeat { session_id:Uuid, state:crate::SessionState },
    MetricsSnapshot,
    PerformanceRunCommit { run:crate::PerformanceRunInput },
    PerformanceNativeStart { session_id:Uuid },
    PerformanceNativeStatus { session_id:Uuid },
    PerformanceNativeCancel { session_id:Uuid },
    PerformanceRunList { game_id:Uuid },
    PerformanceRunCompare { baseline_id:Uuid, candidate_id:Uuid },
    PerformanceRunExport { run_id:Uuid, destination_grant_id:Uuid },
    PerformanceRunDelete { run_id:Uuid },
    PerformanceRunsClear,
    DiagnosticsPreview,
    SaveList { game_id: Uuid },
    SavePage {game_id:Uuid,after:Option<String>,limit:u16},
    SaveHeads { game_id: Uuid },
    SaveOriginGet {save_id:Uuid},
    SavePromote { save_id: Uuid, expected_head_id: Option<Uuid> },
    SavePurgePreview {game_id:Uuid},
    SavePurge {game_id:Uuid,expected_revision:crate::ContentHash},
    SaveCleanup,
    SaveCleanupStatus,
    NativeWorkStatus,
    NativeWorkRecoverStart,
    NativeWorkRecoverStatus {job_id:Uuid},
    NativeWorkRecoverCancel {job_id:Uuid},
    SaveImport { grant_id: Uuid, game_id: Uuid },
    SaveRepair {grant_id:Uuid,save_id:Uuid},
    SaveCreate { session_id: Uuid, kind: SaveKind, slot: u16, expected_parent_id: Option<Uuid> },
    SessionSaveCompatibility { session_id:Uuid, kind:SaveKind },
    SaveLoad { session_id: Uuid, save_id: Uuid },
    SaveExport { save_id: Uuid, destination_grant_id: Uuid },
    SaveLiveExport {session_id:Uuid,kind:SaveKind,destination_grant_id:Uuid},
    SaveExportUploadStart {metadata:crate::SaveMetadata,destination_grant_id:Uuid},
    SaveExportUploadChunk {transfer_id:Uuid,offset:u64,payload_base64:String},
    SaveExportUploadCommit {transfer_id:Uuid,destination_grant_id:Uuid},
    PermissionRevoke { grant_id: Uuid },
    DiagnosticsExport { destination_grant_id: Uuid },
    SettingsGet,
    SettingsSet { settings: UserSettings },
    GameSettingsGet { game_id:Uuid },
    GameSettingsSet { game_id:Uuid, config:SessionConfig },
    RuntimeRank { game_id:Uuid, device:crate::DeviceCapabilities, candidates:Vec<crate::RuntimeCandidate> },
    AssistantAdvise { game_id:Uuid, topic:crate::AdviceTopic },
    AssistantAccept { proposal_id:Uuid },
    AssistantDismiss { proposal_id:Uuid },
    SyncStatus,
    SyncBackgroundConfigure { enabled:bool },
    SyncActionStart{action:crate::SyncAccountAction},
    SyncActionStatus{action_id:Uuid},
    SyncActionCollect{action_id:Uuid},
    SyncActionCancel{action_id:Uuid},
    SyncActionDiscard{action_id:Uuid},
    SyncCreate { audience: String },
    SyncRegister { recovery_key: String, registration_token: String },
    SyncRecover { audience: String, recovery_key: String },
    SyncRenew { recovery_key: String },
    SyncDevices,
    SyncRevokeDevice { recovery_key: String, device_id: Uuid },
    SyncDeleteAccount { recovery_key: String },
    SyncDisconnect,
    SyncRunStart,
    SyncRunStatus { job_id: Uuid },
    SyncRunCancel { job_id: Uuid },
    SyncSaveRecoveryRequest{save_id:Uuid},
    SyncSaveRecoveryList{game_id:Uuid},
    SyncSaveRecoveryCancel{save_id:Uuid},
    UpdateStatus,
    UpdateDownloadStart,
    UpdateJobStatus { job_id: Uuid },
    UpdateJobCancel { job_id: Uuid },
    UpdateActivate { release_id: Uuid },
    UpdateRollback,
    Shutdown,
}

impl Command {
    pub fn contains_secrets(&self)->bool {
        matches!(self,Self::SyncActionStart{..}|Self::SyncActionCollect{..}|Self::SyncCreate{..}|Self::SyncRegister{..}|Self::SyncRecover{..}|Self::SyncRenew{..}|Self::SyncRevokeDevice{..}|Self::SyncDeleteAccount{..}|Self::StreamCapturePrepare{..}|Self::StreamCaptureSelect{..}|Self::StreamCaptureRelease{..}|Self::StreamInputArm{..}|Self::StreamInputFrame{..}|Self::StreamInputRelease{..}|Self::StreamInputStatus{..}|Self::StreamInputRevoke{..})
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserSettings {
    pub schema_version: u32,
    pub locale: String,
    pub reduce_motion: bool,
    pub high_contrast: bool,
    pub text_scale: f32,
    pub telemetry_opt_in: bool,
    pub cloud_sync_opt_in: bool,
    pub ai_enabled: bool,
    pub suspend_background_tabs: bool,
    #[serde(default)]
    pub appearance: AppearanceSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default,deny_unknown_fields)]
pub struct AppearanceSettings {
    pub accent: String,
    pub library_layout: String,
    pub library_density: String,
}
impl Default for AppearanceSettings {
    fn default()->Self {Self{accent:"lime".into(),library_layout:"cards".into(),library_density:"comfortable".into()}}
}
impl AppearanceSettings {
    pub fn validate(&self)->Result<()> {
        if !["lime","blue","violet","amber"].contains(&self.accent.as_str())||!["cards","list"].contains(&self.library_layout.as_str())||!["comfortable","compact"].contains(&self.library_density.as_str()) {
            return Err(PlatformError::new(ErrorCode::InvalidRequest,"Unsupported appearance settings"));
        }
        Ok(())
    }
}

impl Default for UserSettings {
    fn default() -> Self {
        Self { schema_version: 1, locale: "vi".into(), reduce_motion: false, high_contrast: false, text_scale: 1.0, telemetry_opt_in: false, cloud_sync_opt_in: false, ai_enabled: false, suspend_background_tabs: false, appearance:AppearanceSettings::default() }
    }
}

impl UserSettings {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != 1 || !["vi", "en"].contains(&self.locale.as_str()) || !self.text_scale.is_finite() || !(0.8..=2.0).contains(&self.text_scale) {
            return Err(PlatformError::new(ErrorCode::InvalidRequest, "Unsupported settings"));
        }
        self.appearance.validate()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub protocol_major: u16,
    pub protocol_minor: u16,
    pub id: Uuid,
    #[serde(flatten)]
    pub outcome: ResponseOutcome,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ResponseOutcome { Ok { result: Value }, Error { error: PlatformError } }

impl Response {
    pub fn result(id: Uuid, value: Result<Value>) -> Self {
        Self {
            protocol_major: crate::PROTOCOL_MAJOR,
            protocol_minor: crate::PROTOCOL_MINOR,
            id,
            outcome: match value { Ok(result) => ResponseOutcome::Ok { result }, Err(error) => ResponseOutcome::Error { error } },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "data", rename_all = "snake_case")]
pub enum PlatformEvent { GameImported(GameManifest), SessionChanged(SessionInfo), SaveCreated { game_id: Uuid, save_id: Uuid }, PermissionRevoked { grant_id: Uuid } }
