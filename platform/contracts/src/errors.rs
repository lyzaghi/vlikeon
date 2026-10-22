use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidRequest,
    UnsupportedProtocol,
    UnsupportedCapability,
    InvalidState,
    PermissionDenied,
    PermissionExpired,
    NotFound,
    Conflict,
    ContentMismatch,
    CorruptData,
    IncompatibleSave,
    ResourceLimit,
    Busy,
    Cancelled,
    DeadlineExceeded,
    RuntimeCrashed,
    RuntimeUnavailable,
    NetworkUnavailable,
    UntrustedArtifact,
    UpdateRejected,
    StorageFailure,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlatformError {
    pub code: ErrorCode,
    pub message: String,
    pub recoverable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
}

impl PlatformError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), recoverable: false, action: None }
    }

    pub fn recover(mut self, action: impl Into<String>) -> Self {
        self.recoverable = true;
        self.action = Some(action.into());
        self
    }
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{:?}: {}", self.code, self.message) }
}
impl std::error::Error for PlatformError {}
pub type Result<T> = std::result::Result<T, PlatformError>;
