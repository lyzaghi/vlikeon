use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, VerifyingKey};
use omniplay_contracts::{ContentHash, ErrorCode, PlatformError, Result};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const MAX_SIGNED_PAYLOAD: usize = 256 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedRoot {
    pub version: u64,
    pub expires_at_ms: u64,
    pub threshold: u16,
    pub keys: BTreeMap<String,String>,
    pub release_hosts: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactSignature { pub key_id: String, pub signature_base64: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedEnvelope { pub payload_base64: String, pub signatures: Vec<ArtifactSignature> }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseManifest {
    pub schema_version: u32,
    pub metadata_version: u64,
    pub channel: String,
    pub platform: String,
    pub version: Version,
    pub security_epoch: u64,
    pub root_version: u64,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
    pub artifact_url: String,
    pub artifact_sha256: ContentHash,
    pub artifact_bytes: u64,
    pub chromium_revision: String,
    pub minimum_data_schema: u32,
    pub maximum_data_schema: u32,
}

pub struct ReleaseConstraints<'a> {
    pub channel: &'a str,
    pub platform: &'a str,
    pub current_version: &'a Version,
    pub minimum_security_epoch: u64,
    pub highest_metadata_version: u64,
    pub data_schema: u32,
    pub now_ms: u64,
}

impl TrustedRoot {
    pub fn validate(&self, now: u64) -> Result<()> {
        if self.version == 0 || self.expires_at_ms <= now || self.threshold == 0 || self.threshold as usize > self.keys.len() || self.keys.len() > 32 || self.release_hosts.is_empty() {
            return Err(rejected("Trusted update root is expired or invalid"));
        }
        let mut unique_keys = BTreeSet::new();
        for (id,value) in &self.keys {
            if !omniplay_contracts::valid_identifier(id) { return Err(rejected("Trusted key identifier is invalid")); }
            let bytes = STANDARD.decode(value).map_err(|_|rejected("Trusted public key is invalid"))?;
            let key: [u8;32] = bytes.try_into().map_err(|_|rejected("Trusted public key length is invalid"))?;
            VerifyingKey::from_bytes(&key).map_err(|_|rejected("Trusted public key is invalid"))?;
            if !unique_keys.insert(key) { return Err(rejected("The same public key cannot count as multiple threshold signers")); }
        }
        for host in &self.release_hosts {
            if host.is_empty() || host.len() > 253 || !host.bytes().all(|b|b.is_ascii_alphanumeric() || b==b'.' || b==b'-') { return Err(rejected("Release host allowlist is invalid")); }
        }
        Ok(())
    }

    pub fn verify_payload(&self, envelope: &SignedEnvelope, now: u64) -> Result<Vec<u8>> {
        self.validate(now)?;
        if envelope.payload_base64.len() > MAX_SIGNED_PAYLOAD * 2 || envelope.signatures.len() > 32 { return Err(rejected("Signed metadata exceeds its size limit")); }
        let payload = STANDARD.decode(&envelope.payload_base64).map_err(|_|rejected("Signed metadata encoding is invalid"))?;
        if payload.is_empty() || payload.len() > MAX_SIGNED_PAYLOAD { return Err(rejected("Signed metadata exceeds its size limit")); }
        let mut verified = BTreeSet::new();
        for signature in &envelope.signatures {
            let Some(encoded_key) = self.keys.get(&signature.key_id) else { continue; };
            let key_bytes: [u8;32] = STANDARD.decode(encoded_key).map_err(|_|rejected("Public key encoding is invalid"))?.try_into().map_err(|_|rejected("Public key length is invalid"))?;
            let key = VerifyingKey::from_bytes(&key_bytes).map_err(|_|rejected("Public key is invalid"))?;
            let signature_bytes = STANDARD.decode(&signature.signature_base64).map_err(|_|rejected("Signature encoding is invalid"))?;
            let parsed = Signature::from_slice(&signature_bytes).map_err(|_|rejected("Signature length is invalid"))?;
            if key.verify_strict(&payload,&parsed).is_ok() { verified.insert(signature.key_id.clone()); }
        }
        if verified.len() < self.threshold as usize { return Err(rejected("Update signature threshold was not met")); }
        Ok(payload)
    }

    pub fn verify_release(&self, envelope: &SignedEnvelope, policy: &ReleaseConstraints<'_>) -> Result<ReleaseManifest> {
        let payload = self.verify_payload(envelope,policy.now_ms)?;
        let release: ReleaseManifest = serde_json::from_slice(&payload).map_err(|_|rejected("Release metadata is invalid"))?;
        ContentHash::parse(release.artifact_sha256.0.clone()).map_err(|_|rejected("Artifact digest is invalid"))?;
        if release.schema_version != 1 || release.channel != policy.channel || release.platform != policy.platform || release.version <= *policy.current_version || release.security_epoch < policy.minimum_security_epoch || release.metadata_version <= policy.highest_metadata_version || release.root_version != self.version || release.issued_at_ms > policy.now_ms.saturating_add(300_000) || release.expires_at_ms <= policy.now_ms || release.expires_at_ms <= release.issued_at_ms || release.expires_at_ms - release.issued_at_ms > 7*24*60*60*1000 || release.artifact_bytes == 0 || release.artifact_bytes > 2*1024*1024*1024 || release.minimum_data_schema > release.maximum_data_schema || policy.data_schema < release.minimum_data_schema || policy.data_schema > release.maximum_data_schema || release.chromium_revision.len()!=40 || !release.chromium_revision.bytes().all(|b|b.is_ascii_hexdigit()&&!b.is_ascii_uppercase()) {
            return Err(rejected("Release does not satisfy channel, version, security or compatibility policy"));
        }
        // Validate a strict HTTPS authority before the downloader is allowed to touch the network.
        let rest = release.artifact_url.strip_prefix("https://").ok_or_else(||rejected("Update artifacts require HTTPS"))?;
        let (authority,path) = rest.split_once('/').ok_or_else(||rejected("Artifact URL has no path"))?;
        if authority.contains('@') || authority.contains(':') || !self.release_hosts.contains(authority) || path.is_empty() || release.artifact_url.contains('#') || release.artifact_url.chars().any(|c|c.is_control() || c=='\\') {
            return Err(rejected("Artifact URL is outside the trusted release hosts"));
        }
        Ok(release)
    }

    pub fn rotate(&self, old_signed: &SignedEnvelope, new_signed: &SignedEnvelope, now: u64) -> Result<Self> {
        let payload = self.verify_payload(old_signed,now)?;
        let next: TrustedRoot = serde_json::from_slice(&payload).map_err(|_|rejected("New trust root is invalid"))?;
        if Some(next.version) != self.version.checked_add(1) { return Err(rejected("Trust root versions must increase by exactly one")); }
        let self_signed_payload = next.verify_payload(new_signed,now)?;
        if self_signed_payload != payload { return Err(rejected("Old and new roots did not sign the same metadata")); }
        Ok(next)
    }

    pub fn rotate_historical(&self,old_signed:&SignedEnvelope,new_signed:&SignedEnvelope)->Result<Self> {
        // An expired root may authenticate its immediate successor. The final root
        // must pass the current-time expiry check before it can authenticate a release.
        // This prevents an offline installation becoming impossible to recover merely
        // because an intermediate public root expired while it was switched off.
        self.rotate(old_signed,new_signed,0)
    }
}

fn rejected(message: &str) -> PlatformError { PlatformError::new(ErrorCode::UpdateRejected,message) }
