use crate::{Capability, ErrorCode, PlatformError, Result, RuntimeKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState { Created, Preparing, Ready, Running, Paused, Stopping, Stopped, Failed }

impl SessionState {
    pub fn is_terminal(self) -> bool { matches!(self, Self::Stopped | Self::Failed) }

    pub fn transition(self, next: Self) -> Result<Self> {
        let allowed = matches!((self, next),
            (Self::Created, Self::Preparing | Self::Stopping | Self::Failed) |
            (Self::Preparing, Self::Ready | Self::Stopping | Self::Failed) |
            (Self::Ready, Self::Running | Self::Stopping | Self::Failed) |
            (Self::Running, Self::Paused | Self::Stopping | Self::Failed) |
            (Self::Paused, Self::Running | Self::Stopping | Self::Failed) |
            (Self::Stopping, Self::Stopped | Self::Failed));
        if self == next || allowed { return Ok(next); }
        Err(PlatformError::new(ErrorCode::InvalidState, format!("Cannot transition from {self:?} to {next:?}")))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionInfo {
    pub id: Uuid,
    pub game_id: Uuid,
    pub runtime_id: String,
    pub runtime_kind: RuntimeKind,
    pub state: SessionState,
    pub capabilities: BTreeSet<Capability>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub error: Option<PlatformError>,
    #[serde(default)]pub close_requested:bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PerformancePreset { Smooth, Battery, Accurate }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionConfig {
    pub preset: PerformancePreset,
    pub fullscreen: bool,
    pub audio_volume: f32,
    pub latency_frames: u8,
    pub diagnostics_enabled: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self { preset: PerformancePreset::Accurate, fullscreen: false, audio_volume: 0.8, latency_frames: 2, diagnostics_enabled: false }
    }
}

impl SessionConfig {
    pub fn validate(&self) -> Result<()> {
        if !self.audio_volume.is_finite() || !(0.0..=1.0).contains(&self.audio_volume) || self.latency_frames > 8 {
            return Err(PlatformError::new(ErrorCode::InvalidRequest, "Invalid audio volume or frame queue limit"));
        }
        Ok(())
    }
}
