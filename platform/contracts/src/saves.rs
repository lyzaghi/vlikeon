use crate::{ContentHash, ErrorCode, PlatformError, Result};
use semver::Version;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveKind { InGame, EmulatorState }

#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum SaveOriginKind{Captured,AuthenticatedRemote,Imported,LegacyProjection}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveOrigin{
    pub schema_version:u32,pub save_id:Uuid,pub kind:SaveOriginKind,
    pub canonical_metadata:Option<SaveMetadata>,pub canonical_hash:Option<ContentHash>,
    pub imported_from:Option<SaveMetadata>,pub imported_from_hash:Option<ContentHash>,
    pub missing_parent:bool,
}
/// Semantic metadata identity, with explicit domain separation and sorted JSON
/// keys. Typed metadata supplies explicit null/default fields and safe integers.
/// It detects accidental divergence; it does not attest a capture device's honesty.
pub fn save_origin_hash(metadata:&SaveMetadata)->Result<ContentHash>{
    use sha2::{Digest,Sha256};metadata.validate()?;
    let value=serde_json::to_value(metadata).map_err(|_|PlatformError::new(ErrorCode::CorruptData,"Save origin could not be encoded"))?;
    let object=value.as_object().ok_or_else(||PlatformError::new(ErrorCode::CorruptData,"Save origin could not be encoded"))?;
    let ordered:std::collections::BTreeMap<_,_>=object.iter().collect();
    let bytes=serde_json::to_vec(&ordered).map_err(|_|PlatformError::new(ErrorCode::CorruptData,"Save origin could not be encoded"))?;
    if bytes.len()>64*1024{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Save origin metadata exceeds its bound"));}
    let mut hash=Sha256::new();hash.update(b"OmniPlay save origin v1\0");hash.update(bytes);Ok(ContentHash(format!("{:x}",hash.finalize())))
}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct SavePurgePreview {
    pub game_id:Uuid,pub revision:ContentHash,pub save_count:u64,pub payload_bytes:u64,pub current_heads:u64,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct SaveCleanupStatus {pub pending_files:u64,pub pending_bytes:u64}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct SaveRepairResult {pub metadata:SaveMetadata,pub repaired:bool}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum CloudSaveRecoveryState{Queued,Downloading,WaitingForGameClose,Recovered,AlreadyIntact,Failed,Cancelled}
impl CloudSaveRecoveryState{pub fn terminal(self)->bool{matches!(self,Self::Recovered|Self::AlreadyIntact|Self::Failed|Self::Cancelled)}}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloudSaveRecoveryStatus{
    pub save_id:Uuid,pub state:CloudSaveRecoveryState,pub requested_at_ms:u64,pub updated_at_ms:u64,pub error:Option<PlatformError>,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct SavePurgeResult {pub game_id:Uuid,pub removed_count:u64,pub retired_bytes:u64,pub cleanup:SaveCleanupStatus}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct SaveHistoryEntry {pub metadata:SaveMetadata,pub current_head_id:Option<Uuid>}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct SaveHistoryPage {pub entries:Vec<SaveHistoryEntry>,pub next_cursor:Option<String>,pub total:u64}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveMetadata {
    pub schema_version: u32,
    pub id: Uuid,
    pub game_id: Uuid,
    pub game_content_hash: ContentHash,
    pub kind: SaveKind,
    pub slot: u16,
    pub generation: u64,
    pub parent_id: Option<Uuid>,
    pub core_id: String,
    pub core_version: Version,
    pub state_format_version: String,
    pub platform: String,
    pub core_config_hash: ContentHash,
    pub payload_hash: ContentHash,
    pub payload_bytes: u64,
    pub created_at_ms: u64,
    pub device_id: Uuid,
    pub label: Option<String>,
    #[serde(default)]
    pub conflict: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveCompatibility {
    pub game_content_hash: ContentHash,
    pub core_id: String,
    pub core_version: Version,
    pub state_format_version: String,
    pub platform: String,
    pub core_config_hash: ContentHash,
    pub portable_in_game_format: Option<String>,
}

impl SaveMetadata {
    pub fn validate(&self) -> Result<()> {
        if self.parent_id==Some(self.id){return Err(PlatformError::new(ErrorCode::InvalidRequest,"A save cannot be its own parent"));}
        if self.schema_version != 1 || self.payload_bytes == 0 || self.payload_bytes > crate::MAX_SAVE_BYTES || self.generation == 0 || self.generation>9_007_199_254_740_991 || self.created_at_ms>9_007_199_254_740_991 {
            return Err(PlatformError::new(ErrorCode::InvalidRequest, "Invalid save version, length or generation"));
        }
        for hash in [&self.game_content_hash, &self.core_config_hash, &self.payload_hash] { ContentHash::parse(hash.0.clone())?; }
        if !crate::valid_identifier(&self.core_id) || self.platform.is_empty() || self.platform.len()>128 || self.state_format_version.is_empty() || self.state_format_version.len()>512 || self.core_version.to_string().len()>256 {
            return Err(PlatformError::new(ErrorCode::InvalidRequest, "Save compatibility metadata is incomplete"));
        }
        if self.label.as_ref().is_some_and(|v| v.len() > 256) {
            return Err(PlatformError::new(ErrorCode::InvalidRequest, "Save label is too long"));
        }
        Ok(())
    }

    pub fn check_compatible(&self, target: &SaveCompatibility) -> Result<()> {
        self.validate()?;
        if self.game_content_hash != target.game_content_hash {
            return Err(PlatformError::new(ErrorCode::ContentMismatch, "Save belongs to different game content"));
        }
        let format_matches = self.state_format_version == target.state_format_version;
        // A concrete, bounded filesystem decoder retains the old file-only wire
        // shape. This is an explicit reader relation, not a guessed version rule
        // or a permission to load an arbitrary emulator state across cores.
        let legacy_psp_files = self.state_format_version == "ppsspp-files-opsf1" &&
            target.state_format_version == "ppsspp-files-opsf2" &&
            target.portable_in_game_format.as_deref() == Some("ppsspp-files-opsf2");
        let legacy_md_files=target.state_format_version=="ares-md-canonical-files-v1"&&target.portable_in_game_format.as_deref()==Some("ares-md-canonical-files-v1")&&
            ((self.core_id=="ares-md-native"&&self.core_version.to_string()=="0.2.0+ares.9408cb43d494"&&self.state_format_version=="ares-md-opsf2-canonical-files-native64")||
             (self.core_id=="ares-md-web"&&self.core_version.to_string()=="0.2.0+ares.9408cb43d494.omweb1"&&self.state_format_version=="ares-md-omweb1-canonical-ram-eeprom.opwh0001"));
        let compatible = match self.kind {
            SaveKind::InGame => legacy_psp_files || legacy_md_files || (format_matches && (self.core_id == target.core_id || target.portable_in_game_format.as_deref() == Some(self.state_format_version.as_str()))),
            SaveKind::EmulatorState => format_matches && self.core_id == target.core_id && self.core_version == target.core_version && self.platform == target.platform && self.core_config_hash == target.core_config_hash,
        };
        if compatible { return Ok(()); }
        Err(PlatformError::new(ErrorCode::IncompatibleSave, "No verified save conversion exists for the selected runtime").recover("Select a compatible runtime or export the original save"))
    }
}
