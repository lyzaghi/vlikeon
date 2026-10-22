use crate::{ErrorCode, PlatformError, Result};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameSystem { Nes, Gb, Gbc, Snes, Gba, MegaDrive, Ps1, N64, Nds, Psp, Web, Pc }

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeKind { WebWasm, NativeEmulator, InstalledGame, Remote }

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct GameSummary {
    pub id:Uuid,pub title:String,pub system:GameSystem,pub content_hash:ContentHash,
    pub byte_length:u64,pub created_at_ms:u64,pub last_played_at_ms:Option<u64>,pub playtime_ms:u64,
    pub content_format:Option<crate::ContentFormat>,pub asset_count:u16,
    #[serde(default)]pub favorite:bool,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct LibraryFavorite {pub game_id:Uuid,pub favorite:bool}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct LibraryPage {pub games:Vec<GameSummary>,pub next_cursor:Option<String>,pub total_matching:u64,pub total_all:u64}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Pause, SaveRam, SaveState, Rewind, Netplay, Haptics,
    TouchControls, KeyboardMouse, Gamepad, DynamicResolution,
    VideoCapture, AudioCapture,
    DiscControl,
    PerformanceMeasurement,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentHash(pub String);

impl ContentHash {
    pub fn parse(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()) {
            return Err(PlatformError::new(ErrorCode::InvalidRequest, "Expected a lowercase SHA-256 content hash"));
        }
        Ok(Self(value))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameManifest {
    pub schema_version: u32,
    pub id: Uuid,
    pub title: String,
    pub system: GameSystem,
    pub content_hash: ContentHash,
    pub byte_length: u64,
    pub region: Option<String>,
    pub nominal_refresh_hz: Option<f64>,
    pub created_at_ms: u64,
    #[serde(default,skip_serializing_if="Option::is_none")]pub media:Option<crate::GameMedia>,
}

impl GameManifest {
    pub fn validate(&self) -> Result<()> {
        ContentHash::parse(self.content_hash.0.clone())?;
        if self.schema_version != 1 || self.title.trim().is_empty() || self.title.len() > 512 {
            return Err(PlatformError::new(ErrorCode::InvalidRequest, "Invalid game manifest"));
        }
        if self.byte_length == 0 || self.byte_length > crate::MAX_ROM_BYTES {
            return Err(PlatformError::new(ErrorCode::ResourceLimit, "Content size is outside the allowed range"));
        }
        if let Some(media)=&self.media{media.validate(self.system,self.byte_length)?;if media.identity_hash()?!=self.content_hash{return Err(PlatformError::new(ErrorCode::ContentMismatch,"Media manifest does not match its content identity"));}}
        if let Some(rate) = self.nominal_refresh_hz {
            if !rate.is_finite() || !(20.0..=240.0).contains(&rate) {
                return Err(PlatformError::new(ErrorCode::InvalidRequest, "Invalid nominal refresh rate"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeManifest {
    pub schema_version: u32,
    pub id: String,
    pub version: Version,
    pub kind: RuntimeKind,
    pub systems: BTreeSet<GameSystem>,
    pub capabilities: BTreeSet<Capability>,
    pub platform: String,
    pub executable_sha256: ContentHash,
    pub state_format_version: Option<String>,
    pub source_revision: String,
    pub license_expression: String,
    pub distribution_approved: bool,
    pub security_revoked: bool,
}

impl RuntimeManifest {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != 1 || !valid_identifier(&self.id) || self.systems.is_empty() {
            return Err(PlatformError::new(ErrorCode::InvalidRequest, "Invalid runtime identity or systems"));
        }
        ContentHash::parse(self.executable_sha256.0.clone())?;
        if self.capabilities.contains(&Capability::SaveState) && self.state_format_version.as_deref().unwrap_or("").is_empty() {
            return Err(PlatformError::new(ErrorCode::InvalidRequest, "Save-state capability requires a format version"));
        }
        if !self.distribution_approved || self.security_revoked {
            return Err(PlatformError::new(ErrorCode::UntrustedArtifact, "Runtime is not approved for execution"));
        }
        Ok(())
    }
}

pub fn valid_identifier(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && value.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}
