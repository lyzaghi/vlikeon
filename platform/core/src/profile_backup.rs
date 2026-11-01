//! Streamed full-profile backup and verified restore (`OPPF0001`).
//!
//! Extends beyond `OPBK0001` group save archives to capture global settings,
//! library game manifests (including multi-file disc media, favorites, playtime,
//! and per-game session/input/touch/core-option profiles), firmware vault blobs
//! and role selections, save histories/origins/heads across the entire library,
//! local save suppression markers, and optional content-addressed ROM/disc blobs.
use crate::{
    asset_storage::AssetLease,
    library::Library,
    now_ms,
    save_backup::{ArchivedReference, Entry as SaveArchiveEntry},
    save_origins,
    saves::SaveStore,
    storage_accounting,
    util::{digest, hash_file, storage_error, PendingFile},
    Database,
};
use omniplay_contracts::{
    ContentHash, ErrorCode, GameCoreOptions, GameInputProfile, GameManifest, GameSystem,
    GameTouchProfile, PlatformError, Result, SaveMetadata, SessionConfig, UserSettings,
    MAX_ROM_BYTES, MAX_SAVE_BYTES,
};
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use uuid::Uuid;

const MAGIC: &[u8; 8] = b"OPPF0001";
const DOMAIN: &[u8] = b"Vlikeon full profile backup v1\0";
const MAX_MANIFEST_JSON_BYTES: usize = 32 * 1024 * 1024;
const MAX_PROFILE_GAMES: usize = 10_000;
const MAX_PROFILE_SAVES: usize = 20_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileGameEntry {
    pub manifest: GameManifest,
    pub favorite: bool,
    pub playtime_ms: u64,
    pub last_played_at_ms: Option<u64>,
    pub session_config: Option<SessionConfig>,
    pub input_profile: Option<GameInputProfile>,
    pub touch_profile: Option<GameTouchProfile>,
    #[serde(default)]
    pub core_options: BTreeMap<String, GameCoreOptions>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProfileFirmwareEntry {
    pub content_hash: ContentHash,
    pub byte_length: u64,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProfileFirmwareSelection {
    pub system: GameSystem,
    pub role: String,
    pub content_hash: ContentHash,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RemovedSaveMarker {
    pub save_id: Uuid,
    pub game_content_hash: ContentHash,
    pub removed_at_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PayloadBlobDescriptor {
    pub content_hash: ContentHash,
    pub byte_length: u64,
    pub relative_offset: u64,
    pub kind: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileManifest {
    schema_version: u32,
    exported_at_ms: u64,
    source_device_id: Uuid,
    include_rom_assets: bool,
    settings: UserSettings,
    physical_quota_bytes: Option<u64>,
    games: Vec<ProfileGameEntry>,
    firmware_assets: Vec<ProfileFirmwareEntry>,
    firmware_selections: Vec<ProfileFirmwareSelection>,
    saves: Vec<SaveArchiveEntry>,
    locally_removed_saves: Vec<RemovedSaveMarker>,
    blobs: Vec<PayloadBlobDescriptor>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProfileBackupExported {
    pub exported: bool,
    pub delivery: String,
    pub archive_hash: ContentHash,
    pub total_bytes: u64,
    pub games: usize,
    pub saves: usize,
    pub firmware_assets: usize,
    pub blobs: usize,
    pub include_rom_assets: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProfileBackupSummary {
    pub schema_version: u32,
    pub archive_hash: ContentHash,
    pub exported_at_ms: u64,
    pub source_device_id: Uuid,
    pub include_rom_assets: bool,
    pub games: usize,
    pub saves: usize,
    pub firmware_assets: usize,
    pub locally_removed_saves: usize,
    pub blobs: usize,
    pub total_payload_bytes: u64,
}

pub const PROFILE_RESTORE_STAGING_KEY: &str = "profile_restore_staging_v1";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProfileRestoreStagingRecord {
    pub schema_version: u32,
    pub archive_hash: ContentHash,
    pub started_at_ms: u64,
    pub asset_hashes: Vec<ContentHash>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProfileBackupRestored {
    pub archive_hash: ContentHash,
    pub settings_restored: bool,
    pub games_restored: usize,
    pub games_skipped_missing_assets: usize,
    pub firmware_restored: usize,
    pub saves_restored: usize,
    pub saves_already_present: usize,
    pub saves_skipped: usize,
    pub saves_failed: usize,
    pub suppression_markers_merged: usize,
}

enum BlobSource {
    Save(SaveMetadata),
    Path(PathBuf),
}

fn corrupt(msg: &'static str) -> PlatformError {
    PlatformError::new(ErrorCode::CorruptData, msg)
}

fn check_deadline(until: Instant) -> Result<()> {
    if Instant::now() >= until {
        Err(PlatformError::new(
            ErrorCode::DeadlineExceeded,
            "Full-profile backup operation exceeded its cooperative time budget",
        ))
    } else {
        Ok(())
    }
}

fn require_no_active_sessions(database: &Database) -> Result<()> {
    let active: bool = database
        .connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sessions WHERE state NOT IN ('stopped','failed'))",
            [],
            |r| r.get(0),
        )
        .map_err(storage_error)?;
    if active {
        return Err(PlatformError::new(
            ErrorCode::Busy,
            "Stop active games before exporting or restoring a full profile backup",
        ));
    }
    Ok(())
}

fn capture_profile_state(
    database: &Database,
    include_rom_assets: bool,
    until: Instant,
) -> Result<(ProfileManifest, BTreeMap<String, BlobSource>, ContentHash)> {
    let tx = database.connection.unchecked_transaction().map_err(storage_error)?;
    require_no_active_sessions(database)?;

    let settings = database.settings()?;
    let quota_raw: Option<String> = tx
        .query_row(
            "SELECT value FROM preferences WHERE key='storage_physical_quota_bytes_v1'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(storage_error)?;
    let physical_quota_bytes = match quota_raw {
        Some(s) => Some(storage_accounting::get_physical_quota_bytes(database)?),
        None => None,
    };

    let mut blob_map: BTreeMap<String, (u64, &'static str, BlobSource)> = BTreeMap::new();

    // 1. Games & per-game preferences
    let mut games = Vec::new();
    {
        let mut stmt = tx
            .prepare(
                "SELECT manifest_json, favorite, playtime_ms, last_played_at_ms FROM games ORDER BY id LIMIT ?1",
            )
            .map_err(storage_error)?;
        let rows = stmt
            .query_map([MAX_PROFILE_GAMES + 1], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, bool>(1)?,
                    r.get::<_, u64>(2)?,
                    r.get::<_, Option<u64>>(3)?,
                ))
            })
            .map_err(storage_error)?;
        for row in rows {
            check_deadline(until)?;
            if games.len() >= MAX_PROFILE_GAMES {
                return Err(PlatformError::new(
                    ErrorCode::ResourceLimit,
                    "Profile exceeds full-backup game count bound",
                ));
            }
            let (raw_manifest, favorite, playtime_ms, last_played_at_ms) = row.map_err(storage_error)?;
            let manifest: GameManifest =
                serde_json::from_str(&raw_manifest).map_err(|_| corrupt("Stored game manifest is invalid"))?;
            manifest.validate()?;
            let gid = manifest.id;

            let session_config = {
                let raw: Option<String> = tx
                    .query_row(
                        "SELECT value FROM preferences WHERE key=?1",
                        [format!("session.{gid}")],
                        |r| r.get(0),
                    )
                    .optional()
                    .map_err(storage_error)?;
                match raw {
                    Some(v) => {
                        let cfg: SessionConfig =
                            serde_json::from_str(&v).map_err(|_| corrupt("Stored game session config is invalid"))?;
                        cfg.validate()?;
                        Some(cfg)
                    }
                    None => None,
                }
            };
            let input_profile = {
                let raw: Option<String> = tx
                    .query_row(
                        "SELECT value FROM preferences WHERE key=?1",
                        [format!("input.{gid}")],
                        |r| r.get(0),
                    )
                    .optional()
                    .map_err(storage_error)?;
                match raw {
                    Some(v) => {
                        let inp: GameInputProfile =
                            serde_json::from_str(&v).map_err(|_| corrupt("Stored game input profile is invalid"))?;
                        inp.validate()?;
                        Some(inp)
                    }
                    None => None,
                }
            };
            let touch_profile = {
                let raw: Option<String> = tx
                    .query_row(
                        "SELECT value FROM preferences WHERE key=?1",
                        [format!("touch.{gid}")],
                        |r| r.get(0),
                    )
                    .optional()
                    .map_err(storage_error)?;
                match raw {
                    Some(v) => {
                        let tch: GameTouchProfile =
                            serde_json::from_str(&v).map_err(|_| corrupt("Stored game touch profile is invalid"))?;
                        tch.validate()?;
                        Some(tch)
                    }
                    None => None,
                }
            };
            let mut core_options = BTreeMap::new();
            for rt in crate::core_options::list(database, gid)? {
                let opts = crate::core_options::load(database, gid, &rt)?;
                if !opts.values.is_empty() {
                    core_options.insert(rt, opts);
                }
            }

            if include_rom_assets && !matches!(manifest.system, GameSystem::Pc | GameSystem::Web) {
                if let Some(media) = &manifest.media {
                    for asset in &media.assets {
                        let path = database.root.join("assets").join(&asset.content_hash.0);
                        blob_map.entry(asset.content_hash.0.clone()).or_insert((
                            asset.byte_length,
                            "asset",
                            BlobSource::Path(path),
                        ));
                    }
                } else {
                    let path = database.root.join("assets").join(&manifest.content_hash.0);
                    blob_map.entry(manifest.content_hash.0.clone()).or_insert((
                        manifest.byte_length,
                        "asset",
                        BlobSource::Path(path),
                    ));
                }
            }

            games.push(ProfileGameEntry {
                manifest,
                favorite,
                playtime_ms,
                last_played_at_ms,
                session_config,
                input_profile,
                touch_profile,
                core_options,
            });
        }
    }

    // 2. Firmware assets & role selections
    let mut firmware_assets = Vec::new();
    {
        let mut stmt = tx
            .prepare("SELECT content_hash, byte_length, created_at_ms FROM firmware_assets ORDER BY content_hash LIMIT 129")
            .map_err(storage_error)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, u64>(1)?, r.get::<_, u64>(2)?))
            })
            .map_err(storage_error)?;
        for row in rows {
            let (raw_hash, byte_length, created_at_ms) = row.map_err(storage_error)?;
            let content_hash = ContentHash::parse(raw_hash.clone())?;
            let path = database.root.join("firmware").join(format!("{raw_hash}.bin"));
            blob_map
                .entry(raw_hash)
                .or_insert((byte_length, "firmware", BlobSource::Path(path)));
            firmware_assets.push(ProfileFirmwareEntry {
                content_hash,
                byte_length,
                created_at_ms,
            });
        }
    }

    let mut firmware_selections = Vec::new();
    {
        let mut stmt = tx
            .prepare("SELECT system, role, content_hash FROM firmware_selections ORDER BY system, role LIMIT 129")
            .map_err(storage_error)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
            })
            .map_err(storage_error)?;
        for row in rows {
            let (sys_str, role, raw_hash) = row.map_err(storage_error)?;
            let system: GameSystem = serde_json::from_value(serde_json::Value::String(sys_str))
                .map_err(|_| corrupt("Stored firmware selection system is invalid"))?;
            crate::firmware::role_sizes(system, &role)?;
            firmware_selections.push(ProfileFirmwareSelection {
                system,
                role,
                content_hash: ContentHash::parse(raw_hash)?,
            });
        }
    }

    // 3. Saves, origins, heads, and prior archive chains
    let mut saves = Vec::new();
    {
        let mut stmt = tx
            .prepare("SELECT id FROM saves ORDER BY game_id, generation ASC, created_at_ms ASC, id ASC LIMIT ?1")
            .map_err(storage_error)?;
        let ids: Vec<String> = stmt
            .query_map([MAX_PROFILE_SAVES + 1], |r| r.get(0))
            .map_err(storage_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(storage_error)?;
        if ids.len() > MAX_PROFILE_SAVES {
            return Err(PlatformError::new(
                ErrorCode::ResourceLimit,
                "Profile exceeds full-backup save count bound",
            ));
        }
        for raw_id in ids {
            check_deadline(until)?;
            let id = Uuid::parse_str(&raw_id).map_err(|_| corrupt("Save UUID is invalid"))?;
            let (metadata, origin) = save_origins::inspect(&tx, id)?;
            let current: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM save_heads WHERE save_id=?1)",
                    [id.to_string()],
                    |r| r.get(0),
                )
                .map_err(storage_error)?;
            let reference: Option<String> = tx
                .query_row(
                    "SELECT value FROM preferences WHERE key=?1",
                    [format!("backup_reference.{id}")],
                    |r| r.get(0),
                )
                .optional()
                .map_err(storage_error)?;
            let mut prior = Vec::new();
            if let Some(ref_json) = reference {
                if let Ok(prev) = serde_json::from_str::<SaveArchiveEntry>(&ref_json) {
                    if crate::save_backup::validate(&prev).is_ok() {
                        prior.push(ArchivedReference {
                            metadata: prev.metadata,
                            origin: prev.origin,
                            current: prev.current,
                        });
                        for p in prev.prior.into_iter().take(15) {
                            prior.push(p);
                        }
                    }
                }
            }
            let entry = SaveArchiveEntry {
                metadata: metadata.clone(),
                origin,
                current,
                prior,
            };
            crate::save_backup::validate(&entry)?;
            blob_map.entry(metadata.payload_hash.0.clone()).or_insert((
                metadata.payload_bytes,
                "save",
                BlobSource::Save(metadata),
            ));
            saves.push(entry);
        }
    }

    // 4. Local suppression markers
    let mut locally_removed_saves = Vec::new();
    {
        let mut stmt = tx
            .prepare(
                "SELECT save_id, game_content_hash, removed_at_ms FROM locally_removed_saves ORDER BY save_id LIMIT 20001",
            )
            .map_err(storage_error)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, u64>(2)?))
            })
            .map_err(storage_error)?;
        for row in rows {
            let (sid, ghash, removed_at_ms) = row.map_err(storage_error)?;
            locally_removed_saves.push(RemovedSaveMarker {
                save_id: Uuid::parse_str(&sid).map_err(|_| corrupt("Suppression marker UUID is invalid"))?,
                game_content_hash: ContentHash::parse(ghash)?,
                removed_at_ms,
            });
        }
    }

    let mut blobs = Vec::with_capacity(blob_map.len());
    let mut sources = BTreeMap::new();
    let mut offset = 0u64;
    for (hash_str, (byte_length, kind, src)) in blob_map {
        blobs.push(PayloadBlobDescriptor {
            content_hash: ContentHash(hash_str.clone()),
            byte_length,
            relative_offset: offset,
            kind: kind.into(),
        });
        offset = offset
            .checked_add(byte_length)
            .ok_or_else(|| PlatformError::new(ErrorCode::ResourceLimit, "Profile backup payload size overflow"))?;
        sources.insert(hash_str, src);
    }

    // Compute a deterministic state fingerprint to detect concurrent profile changes during streaming
    let fingerprint = digest(
        &serde_json::to_vec(&(
            &settings,
            &physical_quota_bytes,
            &games,
            &firmware_assets,
            &firmware_selections,
            &saves,
            &locally_removed_saves,
        ))
        .map_err(storage_error)?,
    );

    tx.commit().map_err(storage_error)?;

    let manifest = ProfileManifest {
        schema_version: 1,
        exported_at_ms: now_ms(),
        source_device_id: database.device_id,
        include_rom_assets,
        settings,
        physical_quota_bytes,
        games,
        firmware_assets,
        firmware_selections,
        saves,
        locally_removed_saves,
        blobs,
    };
    Ok((manifest, sources, fingerprint))
}

fn validate_manifest(manifest: &ProfileManifest) -> Result<u64> {
    if manifest.schema_version != 1
        || manifest.source_device_id.is_nil()
        || manifest.games.len() > MAX_PROFILE_GAMES
        || manifest.saves.len() > MAX_PROFILE_SAVES
        || manifest.firmware_assets.len() > 128
        || manifest.firmware_selections.len() > 128
        || manifest.locally_removed_saves.len() > 20_000
    {
        return Err(corrupt("Full-profile backup manifest violates structural bounds"));
    }
    manifest.settings.validate()?;
    if let Some(q) = manifest.physical_quota_bytes {
        if !(storage_accounting::MIN_PROFILE_PHYSICAL_QUOTA_BYTES
            ..=storage_accounting::MAX_PROFILE_PHYSICAL_QUOTA_BYTES)
            .contains(&q)
        {
            return Err(corrupt("Archived physical quota is outside policy bounds"));
        }
    }

    let mut blob_lengths = BTreeMap::new();
    let mut expected_offset = 0u64;
    for desc in &manifest.blobs {
        ContentHash::parse(desc.content_hash.0.clone())?;
        if desc.byte_length == 0
            || desc.byte_length > MAX_ROM_BYTES
            || desc.relative_offset != expected_offset
            || !matches!(desc.kind.as_str(), "save" | "firmware" | "asset")
        {
            return Err(corrupt("Archived blob descriptor table is invalid"));
        }
        if blob_lengths.insert(desc.content_hash.0.clone(), desc.byte_length).is_some() {
            return Err(corrupt("Archived blob descriptor table contains duplicate hashes"));
        }
        expected_offset = expected_offset
            .checked_add(desc.byte_length)
            .ok_or_else(|| corrupt("Archived payload offsets overflow"))?;
    }

    let mut seen_games = BTreeSet::new();
    for g in &manifest.games {
        g.manifest.validate()?;
        if !seen_games.insert(g.manifest.id) {
            return Err(corrupt("Archived games contain duplicate UUIDs"));
        }
        if let Some(cfg) = &g.session_config {
            cfg.validate()?;
        }
        if let Some(inp) = &g.input_profile {
            inp.validate()?;
        }
        if let Some(tch) = &g.touch_profile {
            tch.validate()?;
        }
        if g.core_options.len() > 32 {
            return Err(corrupt("Archived game core options exceed per-game bound"));
        }
        for (rt, opts) in &g.core_options {
            if !omniplay_contracts::core_option_identifier(rt) {
                return Err(corrupt("Archived core option runtime identifier is invalid"));
            }
            opts.validate()?;
        }
        if manifest.include_rom_assets && !matches!(g.manifest.system, GameSystem::Pc | GameSystem::Web) {
            if let Some(media) = &g.manifest.media {
                for asset in &media.assets {
                    if blob_lengths.get(&asset.content_hash.0) != Some(&asset.byte_length) {
                        return Err(corrupt("Archived ROM asset blob is missing from payload table"));
                    }
                }
            } else if blob_lengths.get(&g.manifest.content_hash.0) != Some(&g.manifest.byte_length) {
                return Err(corrupt("Archived single-ROM asset blob is missing from payload table"));
            }
        }
    }

    let mut fw_hashes = BTreeSet::new();
    for fw in &manifest.firmware_assets {
        ContentHash::parse(fw.content_hash.0.clone())?;
        if fw.byte_length == 0 || fw.byte_length > 32 * 1024 * 1024 {
            return Err(corrupt("Archived firmware size is invalid"));
        }
        if blob_lengths.get(&fw.content_hash.0) != Some(&fw.byte_length) {
            return Err(corrupt("Archived firmware blob is missing from payload table"));
        }
        fw_hashes.insert(fw.content_hash.0.clone());
    }
    for sel in &manifest.firmware_selections {
        let sizes = crate::firmware::role_sizes(sel.system, &sel.role)?;
        let Some(&len) = blob_lengths.get(&sel.content_hash.0) else {
            return Err(corrupt("Archived firmware selection references missing blob"));
        };
        if !fw_hashes.contains(&sel.content_hash.0) || !sizes.contains(&len) {
            return Err(corrupt("Archived firmware selection does not match role size"));
        }
    }

    let mut seen_saves = BTreeSet::new();
    for entry in &manifest.saves {
        crate::save_backup::validate(entry)?;
        if !seen_saves.insert(entry.metadata.id) {
            return Err(corrupt("Archived saves contain duplicate UUIDs"));
        }
        if entry.metadata.payload_bytes > MAX_SAVE_BYTES
            || blob_lengths.get(&entry.metadata.payload_hash.0) != Some(&entry.metadata.payload_bytes)
        {
            return Err(corrupt("Archived save payload is missing or mismatched in payload table"));
        }
    }

    for marker in &manifest.locally_removed_saves {
        ContentHash::parse(marker.game_content_hash.0.clone())?;
    }

    Ok(expected_offset)
}

/// Streams a complete full-profile backup (`OPPF0001`) to `destination` using two-phase magic commit.
pub fn export(
    database: &mut Database,
    include_rom_assets: bool,
    destination: &mut File,
) -> Result<ProfileBackupExported> {
    let until = Instant::now() + Duration::from_secs(90);
    let _asset_lease = AssetLease::acquire(database)?;
    let (manifest, sources, initial_fingerprint) =
        capture_profile_state(database, include_rom_assets, until)?;
    let payload_total = validate_manifest(&manifest)?;
    let manifest_bytes = serde_json::to_vec(&manifest).map_err(storage_error)?;
    if manifest_bytes.len() > MAX_MANIFEST_JSON_BYTES {
        return Err(PlatformError::new(
            ErrorCode::ResourceLimit,
            "Full-profile manifest exceeds its 32 MiB JSON limit",
        ));
    }

    destination.seek(SeekFrom::Start(0)).map_err(storage_error)?;
    destination.set_len(0).map_err(storage_error)?;

    let len_le = (manifest_bytes.len() as u32).to_le_bytes();
    let mut package_hasher = Sha256::new();
    package_hasher.update(DOMAIN);
    package_hasher.update(MAGIC);
    package_hasher.update(len_le);
    package_hasher.update(&manifest_bytes);

    // Write invalid zero magic until payload streaming, verification, and fsync succeed
    destination.write_all(&[0u8; 8]).map_err(storage_error)?;
    destination.write_all(&len_le).map_err(storage_error)?;
    destination.write_all(&manifest_bytes).map_err(storage_error)?;

    let mut buffer = [0u8; 65536];
    for desc in &manifest.blobs {
        check_deadline(until)?;
        let src = sources
            .get(&desc.content_hash.0)
            .ok_or_else(|| corrupt("Internal blob source missing"))?;
        let mut reader: File = match src {
            BlobSource::Save(meta) => crate::save_payloads::open(database, meta)?,
            BlobSource::Path(path) => crate::save_payloads::open_file(path)?,
        };
        let mut blob_hasher = Sha256::new();
        let mut copied = 0u64;
        loop {
            check_deadline(until)?;
            let n = reader.read(&mut buffer).map_err(storage_error)?;
            if n == 0 {
                break;
            }
            copied = copied
                .checked_add(n as u64)
                .ok_or_else(|| corrupt("Blob read overflow"))?;
            if copied > desc.byte_length {
                return Err(corrupt("Source blob grew during profile backup"));
            }
            blob_hasher.update(&buffer[..n]);
            package_hasher.update(&buffer[..n]);
            destination.write_all(&buffer[..n]).map_err(storage_error)?;
        }
        let actual_hash = ContentHash(format!("{:x}", blob_hasher.finalize()));
        if copied != desc.byte_length || actual_hash != desc.content_hash {
            return Err(corrupt("Source blob failed integrity check during profile backup"));
        }
    }

    // Re-verify profile snapshot consistency before publishing trailer & magic
    let (_, _, after_fingerprint) = capture_profile_state(database, include_rom_assets, until)?;
    if after_fingerprint != initial_fingerprint {
        let _ = destination.set_len(0);
        return Err(PlatformError::new(
            ErrorCode::Conflict,
            "Profile state changed while backup was streaming; retry the backup",
        ));
    }

    let trailer = package_hasher.finalize();
    let archive_hash = ContentHash(format!("{:x}", trailer));
    destination.write_all(&trailer).map_err(storage_error)?;
    destination.sync_all().map_err(storage_error)?;

    destination.seek(SeekFrom::Start(0)).map_err(storage_error)?;
    destination.write_all(MAGIC).map_err(storage_error)?;
    destination.sync_all().map_err(storage_error)?;

    let total_bytes = 12u64
        .saturating_add(manifest_bytes.len() as u64)
        .saturating_add(payload_total)
        .saturating_add(32);
    let _ = database.record_audit("profile.backup_export", None);

    Ok(ProfileBackupExported {
        exported: true,
        delivery: "file_handle".into(),
        archive_hash,
        total_bytes,
        games: manifest.games.len(),
        saves: manifest.saves.len(),
        firmware_assets: manifest.firmware_assets.len(),
        blobs: manifest.blobs.len(),
        include_rom_assets,
    })
}

fn verify_archive_stream<R: Read + Seek>(
    source: &mut R,
    until: Instant,
) -> Result<(ProfileManifest, u64, ContentHash)> {
    source.seek(SeekFrom::Start(0)).map_err(storage_error)?;
    let mut header = [0u8; 12];
    source
        .read_exact(&mut header)
        .map_err(|_| corrupt("Full-profile backup header is truncated"))?;
    if &header[..8] != MAGIC {
        return Err(corrupt("Unknown or incomplete full-profile backup magic"));
    }
    let json_len = u32::from_le_bytes(header[8..12].try_into().map_err(|_| corrupt("Invalid header"))?) as usize;
    if json_len == 0 || json_len > MAX_MANIFEST_JSON_BYTES {
        return Err(corrupt("Full-profile backup manifest size is invalid"));
    }

    let mut manifest_bytes = vec![0u8; json_len];
    source
        .read_exact(&mut manifest_bytes)
        .map_err(|_| corrupt("Full-profile backup manifest is truncated"))?;
    let manifest: ProfileManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|_| corrupt("Full-profile backup manifest JSON is invalid"))?;
    let payload_total = validate_manifest(&manifest)?;

    let payload_base = 12u64 + json_len as u64;
    let mut package_hasher = Sha256::new();
    package_hasher.update(DOMAIN);
    package_hasher.update(header);
    package_hasher.update(&manifest_bytes);

    let mut buffer = [0u8; 65536];
    for desc in &manifest.blobs {
        check_deadline(until)?;
        let mut blob_hasher = Sha256::new();
        let mut remaining = desc.byte_length;
        while remaining > 0 {
            check_deadline(until)?;
            let want = remaining.min(buffer.len() as u64) as usize;
            source
                .read_exact(&mut buffer[..want])
                .map_err(|_| corrupt("Full-profile backup payload section is truncated"))?;
            blob_hasher.update(&buffer[..want]);
            package_hasher.update(&buffer[..want]);
            remaining -= want as u64;
        }
        if format!("{:x}", blob_hasher.finalize()) != desc.content_hash.0 {
            return Err(corrupt("Full-profile backup blob checksum mismatch"));
        }
    }

    let mut trailer = [0u8; 32];
    source
        .read_exact(&mut trailer)
        .map_err(|_| corrupt("Full-profile backup trailer is missing"))?;
    let mut extra = [0u8; 1];
    if source.read(&mut extra).map_err(storage_error)? != 0 {
        return Err(corrupt("Full-profile backup contains unexpected trailing bytes"));
    }
    let expected_trailer = package_hasher.finalize();
    if expected_trailer.as_slice() != trailer {
        return Err(corrupt("Full-profile backup trailer signature mismatch"));
    }

    let _ = payload_total;
    Ok((manifest, payload_base, ContentHash(format!("{:x}", expected_trailer))))
}

/// Inspects and verifies every byte of a full-profile backup archive without mutating local state.
pub fn inspect<R: Read + Seek>(source: &mut R) -> Result<ProfileBackupSummary> {
    let until = Instant::now() + Duration::from_secs(90);
    let (manifest, _, archive_hash) = verify_archive_stream(source, until)?;
    let total_payload_bytes = manifest
        .blobs
        .iter()
        .fold(0u64, |acc, b| acc.saturating_add(b.byte_length));
    Ok(ProfileBackupSummary {
        schema_version: 1,
        archive_hash,
        exported_at_ms: manifest.exported_at_ms,
        source_device_id: manifest.source_device_id,
        include_rom_assets: manifest.include_rom_assets,
        games: manifest.games.len(),
        saves: manifest.saves.len(),
        firmware_assets: manifest.firmware_assets.len(),
        locally_removed_saves: manifest.locally_removed_saves.len(),
        blobs: manifest.blobs.len(),
        total_payload_bytes,
    })
}

pub(crate) fn derived_profile_save_id(archive_hash: &ContentHash, source_id: Uuid, attempt: u32) -> Uuid {
    let mut hasher = Sha256::new();
    hasher.update(b"Vlikeon profile backup branch v1\0");
    hasher.update(archive_hash.0.as_bytes());
    hasher.update(source_id.as_bytes());
    hasher.update(attempt.to_le_bytes());
    let digest = hasher.finalize();
    let mut raw = [0u8; 16];
    raw.copy_from_slice(&digest[..16]);
    raw[6] = (raw[6] & 0x0f) | 0x40;
    raw[8] = (raw[8] & 0x3f) | 0x80;
    Uuid::from_bytes(raw)
}

fn choose_restored_save_id(database: &Database, archive_hash: &ContentHash, source_id: Uuid) -> Result<Uuid> {
    for attempt in 0..128 {
        let candidate = derived_profile_save_id(archive_hash, source_id, attempt);
        let removed: bool = database
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM locally_removed_saves WHERE save_id=?1)",
                [candidate.to_string()],
                |r| r.get(0),
            )
            .map_err(storage_error)?;
        if !removed {
            return Ok(candidate);
        }
    }
    Err(PlatformError::new(
        ErrorCode::ResourceLimit,
        "Archive save branch reached its explicit reimport history limit",
    ))
}

fn read_exact_archive_blob<R: Read + Seek>(
    source: &mut R,
    payload_base: u64,
    desc: &PayloadBlobDescriptor,
    expected_bytes: u64,
    expected_hash: &ContentHash,
    until: Instant,
) -> Result<Vec<u8>> {
    if expected_bytes > MAX_SAVE_BYTES {
        return Err(PlatformError::new(
            ErrorCode::ResourceLimit,
            "Archive save payload exceeds maximum allowed size",
        ));
    }
    if desc.byte_length != expected_bytes || &desc.content_hash != expected_hash {
        return Err(corrupt("Archive save descriptor does not match canonical save metadata"));
    }
    let start_offset = payload_base
        .checked_add(desc.relative_offset)
        .ok_or_else(|| corrupt("Archive save payload offset overflow"))?;
    source
        .seek(SeekFrom::Start(start_offset))
        .map_err(storage_error)?;

    let total_len = usize::try_from(expected_bytes)
        .map_err(|_| PlatformError::new(ErrorCode::ResourceLimit, "Archive save payload exceeds addressable memory"))?;
    let mut payload = Vec::with_capacity(total_len.min(1024 * 1024));
    let mut remaining = total_len;
    let mut buffer = [0u8; 65536];
    while remaining > 0 {
        check_deadline(until)?;
        let want = remaining.min(buffer.len());
        source.read_exact(&mut buffer[..want]).map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                corrupt("Archive save payload truncated before declared byte_length")
            } else {
                storage_error(e)
            }
        })?;
        payload.extend_from_slice(&buffer[..want]);
        remaining -= want;
    }
    if payload.len() != total_len || digest(&payload) != *expected_hash {
        return Err(corrupt("Archive save payload changed during restore"));
    }
    Ok(payload)
}

fn copy_verified_blob_from_archive<R: Read + Seek>(
    source: &mut R,
    payload_base: u64,
    desc: &PayloadBlobDescriptor,
    target_dir: &Path,
    target_path: &Path,
    until: Instant,
) -> Result<bool> {
    if target_path.exists() {
        let mut f = File::open(target_path).map_err(storage_error)?;
        let (existing_hash, existing_len) = hash_file(&mut f, MAX_ROM_BYTES)?;
        if existing_hash == desc.content_hash && existing_len == desc.byte_length {
            return Ok(false);
        }
        return Err(corrupt("Existing local content blob failed integrity verification"));
    }

    let start_offset = payload_base
        .checked_add(desc.relative_offset)
        .ok_or_else(|| corrupt("Archive blob offset overflow"))?;
    source
        .seek(SeekFrom::Start(start_offset))
        .map_err(storage_error)?;
    let mut pending = PendingFile::create(target_dir)?;
    let mut hasher = Sha256::new();
    let mut remaining = desc.byte_length;
    let mut buffer = [0u8; 65536];
    while remaining > 0 {
        check_deadline(until)?;
        let want = remaining.min(buffer.len() as u64) as usize;
        source.read_exact(&mut buffer[..want]).map_err(storage_error)?;
        hasher.update(&buffer[..want]);
        pending.write_all(&buffer[..want])?;
        remaining -= want as u64;
    }
    if format!("{:x}", hasher.finalize()) != desc.content_hash.0 {
        return Err(corrupt("Archive source changed during blob extraction"));
    }
    pending.commit_new(target_path)?;
    Ok(true)
}

fn cleanup_failed_restore_staging(database: &Database, staged_paths: &[PathBuf]) {
    for path in staged_paths {
        let _ = fs::remove_file(path);
    }
    let _ = database
        .connection
        .execute("DELETE FROM preferences WHERE key=?1", [PROFILE_RESTORE_STAGING_KEY]);
}

/// Verifies an entire `OPPF0001` archive before mutating state, checks physical quota admission
/// strictly against the local profile quota (never raising local quota before restore succeeds),
/// holds `AssetLease` across blob staging through manifest publication, commits metadata atomically,
/// and restores save branches from exact bounded payload slices.
pub fn restore<R: Read + Seek>(database: &mut Database, source: &mut R) -> Result<ProfileBackupRestored> {
    let until = Instant::now() + Duration::from_secs(90);
    require_no_active_sessions(database)?;

    // Hold AssetLease across the entire restore lifecycle so asset_storage cleanup cannot
    // reclaim newly staged ROM/disc blobs before Library manifests are durably committed.
    let _asset_lease = AssetLease::acquire(database)?;

    // 1. Full structural + payload + trailer verification BEFORE any local write
    let (manifest, payload_base, archive_hash) = verify_archive_stream(source, until)?;

    let blob_lookup: BTreeMap<String, &PayloadBlobDescriptor> = manifest
        .blobs
        .iter()
        .map(|d| (d.content_hash.0.clone(), d))
        .collect();

    // Calculate net new bytes needed for physical quota admission against current local quota.
    // Archive metadata must never raise the destination device's physical_quota_bytes.
    let mut missing_bytes = 0u64;
    let mut missing_files = 0u32;
    let mut staged_asset_hashes = Vec::new();
    for desc in &manifest.blobs {
        let already_on_disk = match desc.kind.as_str() {
            "asset" => {
                staged_asset_hashes.push(desc.content_hash.clone());
                database.root.join("assets").join(&desc.content_hash.0).exists()
            }
            "firmware" => database
                .root
                .join("firmware")
                .join(format!("{}.bin", desc.content_hash.0))
                .exists(),
            _ => false,
        };
        if !already_on_disk {
            missing_bytes = missing_bytes.saturating_add(desc.byte_length);
            missing_files = missing_files.saturating_add(1);
        }
    }
    storage_accounting::enforce_physical_quota(database, missing_bytes, missing_files)?;

    // Persist durable staging retainer in preferences so crash/restart before manifest commit
    // still retains staged blobs in storage_accounting until reconciled or cleaned up.
    if !staged_asset_hashes.is_empty() {
        let staging_record = ProfileRestoreStagingRecord {
            archive_hash: archive_hash.clone(),
            started_at_ms: now_ms(),
            asset_hashes: staged_asset_hashes,
        };
        let staging_json = serde_json::to_string(&staging_record).map_err(storage_error)?;
        database
            .connection
            .execute(
                "INSERT INTO preferences(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                params![PROFILE_RESTORE_STAGING_KEY, staging_json],
            )
            .map_err(storage_error)?;
    }

    let mut newly_staged_paths: Vec<PathBuf> = Vec::new();
    let mut wrote_firmware_blobs: BTreeSet<String> = BTreeSet::new();

    // 2. Stage ROM/disc (`assets/<hash>`) and firmware (`firmware/<hash>.bin`) blobs
    let stage_result: Result<()> = (|| {
        let assets_dir = database.root.join("assets");
        for desc in manifest.blobs.iter().filter(|d| d.kind == "asset") {
            let target = assets_dir.join(&desc.content_hash.0);
            if copy_verified_blob_from_archive(source, payload_base, desc, &assets_dir, &target, until)? {
                newly_staged_paths.push(target);
            }
        }
        let fw_dir = database.root.join("firmware");
        for fw in &manifest.firmware_assets {
            let desc = blob_lookup
                .get(&fw.content_hash.0)
                .ok_or_else(|| corrupt("Firmware descriptor missing"))?;
            let target = fw_dir.join(format!("{}.bin", fw.content_hash.0));
            if copy_verified_blob_from_archive(source, payload_base, desc, &fw_dir, &target, until)? {
                newly_staged_paths.push(target);
                wrote_firmware_blobs.insert(fw.content_hash.0.clone());
            }
        }
        Ok(())
    })();

    if let Err(err) = stage_result {
        cleanup_failed_restore_staging(database, &newly_staged_paths);
        return Err(err);
    }

    // 3–6. Commit firmware rows/selections, conservative settings, suppression markers,
    // library manifests, per-game preferences, and staging-marker clearance in ONE transaction.
    let current_settings = match database.settings() {
        Ok(s) => s,
        Err(err) => {
            cleanup_failed_restore_staging(database, &newly_staged_paths);
            return Err(err);
        }
    };
    let mut merged_settings = manifest.settings.clone();
    if !current_settings.cloud_sync_opt_in {
        merged_settings.cloud_sync_opt_in = false;
    }
    if let Err(err) = merged_settings.validate() {
        cleanup_failed_restore_staging(database, &newly_staged_paths);
        return Err(err);
    }
    let merged_settings_json = match serde_json::to_string(&merged_settings).map_err(storage_error) {
        Ok(j) => j,
        Err(err) => {
            cleanup_failed_restore_staging(database, &newly_staged_paths);
            return Err(err);
        }
    };

    let mut firmware_restored = 0usize;
    let mut suppression_markers_merged = 0usize;
    let mut games_restored = 0usize;
    let mut games_skipped_missing_assets = 0usize;

    let metadata_commit: Result<()> = (|| {
        let tx = Transaction::new_unchecked(&database.connection, TransactionBehavior::Immediate)
            .map_err(storage_error)?;

        for fw in &manifest.firmware_assets {
            let changed = tx
                .execute(
                    "INSERT OR IGNORE INTO firmware_assets(content_hash, byte_length, created_at_ms) VALUES(?1,?2,?3)",
                    params![fw.content_hash.0, fw.byte_length, fw.created_at_ms],
                )
                .map_err(storage_error)?;
            if wrote_firmware_blobs.contains(&fw.content_hash.0) || changed > 0 {
                firmware_restored += 1;
            }
        }

        for sel in &manifest.firmware_selections {
            let Ok(allowed_sizes) = crate::firmware::role_sizes(sel.system, &sel.role) else {
                continue;
            };
            let Some(sys_key) = serde_json::to_value(sel.system)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
            else {
                continue;
            };
            let exists: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM firmware_selections WHERE system=?1 AND role=?2)",
                    params![sys_key, sel.role],
                    |r| r.get(0),
                )
                .map_err(storage_error)?;
            if !exists {
                let bytes: Option<u64> = tx
                    .query_row(
                        "SELECT byte_length FROM firmware_assets WHERE content_hash=?1",
                        [&sel.content_hash.0],
                        |r| r.get(0),
                    )
                    .optional()
                    .map_err(storage_error)?;
                if bytes.is_some_and(|v| allowed_sizes.contains(&v)) {
                    tx.execute(
                        "INSERT INTO firmware_selections(system,role,content_hash) VALUES(?1,?2,?3) ON CONFLICT(system,role) DO UPDATE SET content_hash=excluded.content_hash",
                        params![sys_key, sel.role, sel.content_hash.0],
                    )
                    .map_err(storage_error)?;
                }
            }
        }

        tx.execute(
            "INSERT INTO preferences(key,value) VALUES('settings',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [&merged_settings_json],
        )
        .map_err(storage_error)?;

        for marker in &manifest.locally_removed_saves {
            let changed = tx
                .execute(
                    "INSERT OR IGNORE INTO locally_removed_saves(save_id, game_content_hash, removed_at_ms) VALUES(?1,?2,?3)",
                    params![
                        marker.save_id.to_string(),
                        marker.game_content_hash.0,
                        marker.removed_at_ms
                    ],
                )
                .map_err(storage_error)?;
            if changed > 0 {
                suppression_markers_merged += 1;
            }
        }

        for entry in &manifest.games {
            check_deadline(until)?;
            if !matches!(entry.manifest.system, GameSystem::Pc | GameSystem::Web) {
                let assets_present = if let Some(media) = &entry.manifest.media {
                    media.assets.iter().all(|a| {
                        database
                            .root
                            .join("assets")
                            .join(&a.content_hash.0)
                            .metadata()
                            .is_ok_and(|m| m.is_file() && m.len() == a.byte_length)
                    })
                } else {
                    database
                        .root
                        .join("assets")
                        .join(&entry.manifest.content_hash.0)
                        .metadata()
                        .is_ok_and(|m| m.is_file() && m.len() == entry.manifest.byte_length)
                };
                if !assets_present {
                    games_skipped_missing_assets += 1;
                    continue;
                }
            } else {
                games_skipped_missing_assets += 1;
                continue;
            }

            entry.manifest.validate()?;
            let existing_json: Option<String> = tx
                .query_row(
                    "SELECT manifest_json FROM games WHERE content_hash=?1",
                    [&entry.manifest.content_hash.0],
                    |r| r.get(0),
                )
                .optional()
                .map_err(storage_error)?;

            let local_game = if let Some(raw_existing) = existing_json {
                let mut existing: GameManifest = serde_json::from_str(&raw_existing)
                    .map_err(|_| corrupt("Existing library manifest is corrupt"))?;
                if existing.system != entry.manifest.system {
                    return Err(PlatformError::new(
                        ErrorCode::Conflict,
                        "The same content is already registered for another system",
                    ));
                }
                if existing.media.is_none() && entry.manifest.media.is_some() {
                    existing.media = entry.manifest.media.clone();
                    existing.validate()?;
                    tx.execute(
                        "UPDATE games SET manifest_json=?1 WHERE id=?2",
                        params![
                            serde_json::to_string(&existing).map_err(storage_error)?,
                            existing.id.to_string()
                        ],
                    )
                    .map_err(storage_error)?;
                }
                existing
            } else {
                let inserted = entry.manifest.clone();
                tx.execute(
                    "INSERT INTO games(id,content_hash,title,search_title,system,manifest_json,created_at_ms) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                    params![
                        inserted.id.to_string(),
                        inserted.content_hash.0,
                        inserted.title,
                        inserted.title.to_lowercase(),
                        serde_json::to_string(&inserted.system).map_err(storage_error)?,
                        serde_json::to_string(&inserted).map_err(storage_error)?,
                        inserted.created_at_ms
                    ],
                )
                .map_err(storage_error)?;
                tx.execute(
                    "INSERT INTO audit(event,entity_id,created_at_ms) VALUES('library.import',?1,?2)",
                    params![inserted.id.to_string(), now_ms()],
                )
                .map_err(storage_error)?;
                games_restored += 1;
                inserted
            };

            tx.execute(
                "UPDATE games SET favorite = CASE WHEN favorite=1 THEN 1 ELSE ?1 END, playtime_ms = MAX(playtime_ms, ?2), last_played_at_ms = CASE WHEN last_played_at_ms IS NULL THEN ?3 WHEN ?3 IS NULL THEN last_played_at_ms ELSE MAX(last_played_at_ms, ?3) END WHERE id=?4",
                params![
                    entry.favorite,
                    entry.playtime_ms,
                    entry.last_played_at_ms,
                    local_game.id.to_string()
                ],
            )
            .map_err(storage_error)?;

            if let Some(cfg) = &entry.session_config {
                cfg.validate()?;
                let key = format!("session.{}", local_game.id);
                let has_local: bool = tx
                    .query_row("SELECT EXISTS(SELECT 1 FROM preferences WHERE key=?1)", [&key], |r| r.get(0))
                    .map_err(storage_error)?;
                if !has_local {
                    let raw = serde_json::to_string(cfg).map_err(storage_error)?;
                    tx.execute("INSERT INTO preferences(key,value) VALUES(?1,?2)", params![key, raw])
                        .map_err(storage_error)?;
                }
            }
            if let Some(inp) = &entry.input_profile {
                inp.validate()?;
                let raw = serde_json::to_string(inp).map_err(storage_error)?;
                if raw.len() <= 16384 {
                    let key = format!("input.{}", local_game.id);
                    let has_local: bool = tx
                        .query_row("SELECT EXISTS(SELECT 1 FROM preferences WHERE key=?1)", [&key], |r| r.get(0))
                        .map_err(storage_error)?;
                    if !has_local {
                        tx.execute("INSERT INTO preferences(key,value) VALUES(?1,?2)", params![key, raw])
                            .map_err(storage_error)?;
                    }
                }
            }
            if let Some(tch) = &entry.touch_profile {
                tch.validate()?;
                let raw = serde_json::to_string(tch).map_err(storage_error)?;
                if raw.len() <= 8192 {
                    let key = format!("touch.{}", local_game.id);
                    let has_local: bool = tx
                        .query_row("SELECT EXISTS(SELECT 1 FROM preferences WHERE key=?1)", [&key], |r| r.get(0))
                        .map_err(storage_error)?;
                    if !has_local {
                        tx.execute("INSERT INTO preferences(key,value) VALUES(?1,?2)", params![key, raw])
                            .map_err(storage_error)?;
                    }
                }
            }
            for (rt, opts) in &entry.core_options {
                if !omniplay_contracts::core_option_identifier(rt) || opts.values.is_empty() {
                    continue;
                }
                opts.validate()?;
                let raw = serde_json::to_string(opts).map_err(storage_error)?;
                if raw.len() <= omniplay_contracts::MAX_CORE_OPTION_PROFILE_BYTES {
                    let key = format!("core_options.{}.{rt}", local_game.id);
                    let has_local: bool = tx
                        .query_row("SELECT EXISTS(SELECT 1 FROM preferences WHERE key=?1)", [&key], |r| r.get(0))
                        .map_err(storage_error)?;
                    if !has_local {
                        let per_game: u64 = tx
                            .query_row(
                                "SELECT count(*) FROM preferences WHERE key GLOB ?1",
                                [format!("core_options.{}.*", local_game.id)],
                                |r| r.get(0),
                            )
                            .map_err(storage_error)?;
                        if per_game < 32 {
                            tx.execute("INSERT INTO preferences(key,value) VALUES(?1,?2)", params![key, raw])
                                .map_err(storage_error)?;
                        }
                    }
                }
            }
        }

        // Manifests are now registered in `games`; clear the temporary staging retainer atomically.
        tx.execute("DELETE FROM preferences WHERE key=?1", [PROFILE_RESTORE_STAGING_KEY])
            .map_err(storage_error)?;
        tx.commit().map_err(storage_error)
    })();

    if let Err(err) = metadata_commit {
        cleanup_failed_restore_staging(database, &newly_staged_paths);
        return Err(err);
    }

    // 7. Restore save generations from exact bounded payload slices & promote archived heads when no local head exists yet
    let mut saves_restored = 0usize;
    let mut saves_already_present = 0usize;
    let mut saves_skipped = 0usize;
    let mut saves_failed = 0usize;

    for entry in &manifest.saves {
        check_deadline(until)?;
        let original = entry.origin.canonical_metadata.as_ref().unwrap_or(&entry.metadata);
        // Respect local suppression if the source UUID itself was purged locally
        if crate::save_maintenance::locally_removed(database, original.id)? {
            saves_skipped += 1;
            continue;
        }
        let Some(local_game) = Library::new(database).by_hash(&original.game_content_hash)? else {
            saves_skipped += 1;
            continue;
        };
        let Ok(restored_id) = choose_restored_save_id(database, &archive_hash, entry.metadata.id) else {
            saves_skipped += 1;
            continue;
        };
        let reference = serde_json::to_string(entry).map_err(storage_error)?;
        let existing_ref: Option<String> = database
            .connection
            .query_row(
                "SELECT value FROM preferences WHERE key=?1",
                [format!("backup_reference.{restored_id}")],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage_error)?;
        if existing_ref.is_some() {
            saves_already_present += 1;
            continue;
        }

        let Some(desc) = blob_lookup.get(&original.payload_hash.0) else {
            saves_skipped += 1;
            continue;
        };
        let payload = read_exact_archive_blob(
            source,
            payload_base,
            desc,
            original.payload_bytes,
            &original.payload_hash,
            until,
        )?;

        match SaveStore::new(database).restore_backup(
            original.clone(),
            &payload,
            local_game.id,
            restored_id,
            &reference,
        ) {
            Ok(committed) => {
                saves_restored += 1;
                if entry.current {
                    let kind_str = crate::saves::kind_name(committed.kind);
                    let has_head: bool = database
                        .connection
                        .query_row(
                            "SELECT EXISTS(SELECT 1 FROM save_heads WHERE game_id=?1 AND kind=?2 AND slot=?3)",
                            params![local_game.id.to_string(), kind_str, committed.slot],
                            |r| r.get(0),
                        )
                        .map_err(storage_error)?;
                    if !has_head {
                        let _ = SaveStore::new(database).promote(committed.id, None);
                    }
                }
            }
            Err(_) => {
                saves_failed += 1;
            }
        }
    }

    let audit_event = if saves_failed > 0 {
        "profile.backup_restore_partial"
    } else {
        "profile.backup_restore"
    };
    let _ = database.record_audit(audit_event, None);

    Ok(ProfileBackupRestored {
        archive_hash,
        settings_restored: true,
        games_restored,
        games_skipped_missing_assets,
        firmware_restored,
        saves_restored,
        saves_already_present,
        saves_skipped,
        saves_failed,
        suppression_markers_merged,
    })
}
