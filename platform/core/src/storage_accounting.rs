//! Physical disk allocation and active data-retaining reference accounting.
//!
//! Tracks exact physical bytes (cluster/block-allocated bytes with `(dev, ino)`
//! hard-link deduplication on Unix and 4 KiB cluster rounding on Windows) alongside
//! every active database retainer (`games`, `GameMedia.assets`, `saves`, `save_heads`,
//! `save_repair_files`, `save_legacy_payloads`, `save_payload_gc`, `firmware_assets`,
//! `firmware_selections`, active `sessions`, and `native_work_storage`).
use crate::{Database, util::storage_error};
use omniplay_contracts::{ContentHash, ErrorCode, GameManifest, GameSystem, PlatformError, Result};
use rusqlite::{OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    time::{Duration, Instant},
};

pub const DEFAULT_MAX_PROFILE_PHYSICAL_BYTES: u64 = 64 * 1024 * 1024 * 1024;
pub const MIN_PROFILE_PHYSICAL_QUOTA_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_PROFILE_PHYSICAL_QUOTA_BYTES: u64 = 512 * 1024 * 1024 * 1024;
const QUOTA_PREFERENCE_KEY: &str = "storage_physical_quota_bytes_v1";
const CLUSTER_BYTES: u64 = 4096;
const SCAN_ENTRY_LIMIT: usize = 50_000;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NamespaceAccounting {
    pub namespace: String,
    pub logical_bytes: u64,
    pub physical_bytes: u64,
    pub file_count: u64,
    pub unreferenced_files: u64,
    pub unreferenced_physical_bytes: u64,
    pub retained_unknown_entries: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RetainerCounts {
    pub library_games: u64,
    pub active_sessions: u64,
    pub live_saves: u64,
    pub save_heads: u64,
    pub pending_save_gc: u64,
    pub firmware_assets: u64,
    pub firmware_selections: u64,
    pub native_work_debts: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BlobRetentionSummary {
    pub content_hash: ContentHash,
    pub namespace: String,
    pub logical_bytes: u64,
    pub physical_bytes: u64,
    pub retainers: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StorageAccountingReport {
    pub schema_version: u32,
    pub logical_bytes: u64,
    pub physical_bytes: u64,
    pub unmaterialized_reserved_bytes: u64,
    pub effective_committed_physical_bytes: u64,
    pub maximum_physical_bytes: u64,
    pub available_physical_bytes: u64,
    pub hardlink_deduped_files: u64,
    pub namespaces: Vec<NamespaceAccounting>,
    pub retainer_counts: RetainerCounts,
    pub retained_blobs: Vec<BlobRetentionSummary>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct QuotaAdmission {
    pub current_physical_bytes: u64,
    pub unmaterialized_reserved_bytes: u64,
    pub requested_physical_bytes: u64,
    pub projected_physical_bytes: u64,
    pub maximum_physical_bytes: u64,
}

fn plain_entry(metadata: &fs::Metadata, expect_dir: bool) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    !metadata.file_type().is_symlink() && if expect_dir { metadata.is_dir() } else { metadata.is_file() }
}

pub fn round_cluster_bytes(logical_bytes: u64) -> u64 {
    if logical_bytes == 0 {
        0
    } else {
        let rem = logical_bytes % CLUSTER_BYTES;
        if rem == 0 {
            logical_bytes
        } else {
            logical_bytes.saturating_add(CLUSTER_BYTES - rem)
        }
    }
}

fn file_physical_bytes(
    path: &Path,
    metadata: &fs::Metadata,
    seen_inodes: &mut BTreeSet<(u64, u64)>,
    hardlink_deduped: &mut u64,
) -> u64 {
    #[cfg(unix)]
    {
        let _ = path;
        use std::os::unix::fs::MetadataExt;
        let key = (metadata.dev(), metadata.ino());
        if metadata.nlink() > 1 && !seen_inodes.insert(key) {
            *hardlink_deduped = hardlink_deduped.saturating_add(1);
            return 0;
        }
        seen_inodes.insert(key);
        metadata.blocks().saturating_mul(512)
    }
    #[cfg(windows)]
    {
        if let Ok(win) = omniplay_stream_input::windows_file_physical(path) {
            if win.file_index != 0 {
                let key = (win.volume_serial, win.file_index);
                if win.link_count > 1 && !seen_inodes.insert(key) {
                    *hardlink_deduped = hardlink_deduped.saturating_add(1);
                    return 0;
                }
                seen_inodes.insert(key);
            }
            return win.allocation_bytes;
        }
        round_cluster_bytes(metadata.len())
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (path, seen_inodes, hardlink_deduped);
        round_cluster_bytes(metadata.len())
    }
}

pub fn get_physical_quota_bytes(database: &Database) -> Result<u64> {
    let raw: Option<String> = database
        .connection
        .query_row(
            "SELECT substr(value,1,33) FROM preferences WHERE key=?1",
            [QUOTA_PREFERENCE_KEY],
            |row| row.get(0),
        )
        .optional()
        .map_err(storage_error)?;
    match raw {
        None => Ok(DEFAULT_MAX_PROFILE_PHYSICAL_BYTES),
        Some(value) => {
            let parsed = value.parse::<u64>().map_err(|_| {
                PlatformError::new(ErrorCode::CorruptData, "Physical storage quota setting needs recovery")
            })?;
            if !(MIN_PROFILE_PHYSICAL_QUOTA_BYTES..=MAX_PROFILE_PHYSICAL_QUOTA_BYTES).contains(&parsed) {
                return Err(PlatformError::new(
                    ErrorCode::CorruptData,
                    "Physical storage quota setting is outside policy bounds",
                ));
            }
            Ok(parsed)
        }
    }
}

pub fn set_physical_quota_bytes(database: &Database, max_physical_bytes: u64) -> Result<u64> {
    if !(MIN_PROFILE_PHYSICAL_QUOTA_BYTES..=MAX_PROFILE_PHYSICAL_QUOTA_BYTES).contains(&max_physical_bytes) {
        return Err(PlatformError::new(
            ErrorCode::InvalidRequest,
            "Physical storage quota must be between 256 MiB and 512 GiB",
        ));
    }
    database
        .connection
        .execute(
            "INSERT INTO preferences(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            rusqlite::params![QUOTA_PREFERENCE_KEY, max_physical_bytes.to_string()],
        )
        .map_err(storage_error)?;
    database.record_audit("storage.quota_set", None)?;
    Ok(max_physical_bytes)
}

/// Scans `native-work/` physical bytes for `NativeWorkStorageStatus`.
pub fn native_work_physical_bytes(root: &Path) -> Result<u64> {
    let target = root.join("native-work");
    let mut seen_inodes = BTreeSet::new();
    let mut deduped = 0u64;
    let mut visited = 0usize;
    let until = Instant::now() + Duration::from_secs(10);
    let (_, physical, _, _) = scan_tree_recursive(&target, 4, &mut seen_inodes, &mut deduped, &mut visited, until)?;
    Ok(physical)
}

fn scan_tree_recursive(
    dir: &Path,
    max_depth: u8,
    seen_inodes: &mut BTreeSet<(u64, u64)>,
    hardlink_deduped: &mut u64,
    visited: &mut usize,
    until: Instant,
) -> Result<(u64, u64, u64, u64)> {
    let meta = match fs::symlink_metadata(dir) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok((0, 0, 0, 0)),
        Err(err) => return Err(storage_error(err)),
        Ok(m) => m,
    };
    if !plain_entry(&meta, true) {
        return Err(PlatformError::new(
            ErrorCode::StorageFailure,
            "Profile storage directory contains an unowned link or reparse point",
        ));
    }
    let mut logical = 0u64;
    let mut physical = 0u64;
    let mut files = 0u64;
    let mut unknown = 0u64;
    for entry in fs::read_dir(dir).map_err(storage_error)? {
        if Instant::now() >= until || *visited >= SCAN_ENTRY_LIMIT {
            return Err(PlatformError::new(
                ErrorCode::ResourceLimit,
                "Physical storage accounting exceeded its bounded scan budget",
            ));
        }
        *visited += 1;
        let entry = entry.map_err(storage_error)?;
        let path = entry.path();
        let entry_meta = fs::symlink_metadata(&path).map_err(storage_error)?;
        if entry_meta.file_type().is_symlink() {
            unknown = unknown.saturating_add(1);
            continue;
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if entry_meta.file_attributes() & 0x400 != 0 {
                unknown = unknown.saturating_add(1);
                continue;
            }
        }
        if entry_meta.is_dir() {
            if max_depth == 0 {
                unknown = unknown.saturating_add(1);
                continue;
            }
            let (sub_l, sub_p, sub_f, sub_u) =
                scan_tree_recursive(&path, max_depth - 1, seen_inodes, hardlink_deduped, visited, until)?;
            logical = logical.saturating_add(sub_l);
            physical = physical.saturating_add(sub_p);
            files = files.saturating_add(sub_f);
            unknown = unknown.saturating_add(sub_u);
        } else if entry_meta.is_file() {
            logical = logical.saturating_add(entry_meta.len());
            physical = physical.saturating_add(file_physical_bytes(&path, &entry_meta, seen_inodes, hardlink_deduped));
            files = files.saturating_add(1);
        } else {
            unknown = unknown.saturating_add(1);
        }
    }
    Ok((logical, physical, files, unknown))
}

#[derive(Default)]
struct ReferenceMap {
    /// Content hash -> set of retainer tags (`library:<uuid>`, `session:<uuid>`, `save_game:<uuid>`, `native_work:<uuid>`)
    asset_retainers: BTreeMap<String, BTreeSet<String>>,
    /// Repair file UUID (`<file_id>.bin`) -> retainer tags
    repair_file_retainers: BTreeMap<String, BTreeSet<String>>,
    /// Legacy save relative key (`<game_id>/<save_id>.bin`) -> retainer tags
    legacy_save_retainers: BTreeMap<String, BTreeSet<String>>,
    /// Firmware hash (`<hash>.bin`) -> retainer tags
    firmware_retainers: BTreeMap<String, BTreeSet<String>>,
    counts: RetainerCounts,
    reserved_work_bytes: u64,
}

fn collect_retainers(database: &Database, until: Instant) -> Result<ReferenceMap> {
    let mut map = ReferenceMap::default();
    let mut game_assets: BTreeMap<String, Vec<String>> = BTreeMap::new();

    // 1. Library games & multi-file GameMedia assets
    let mut stmt = database
        .connection
        .prepare("SELECT id, content_hash, manifest_json FROM games ORDER BY id LIMIT 20001")
        .map_err(storage_error)?;
    let mut rows = stmt.query([]).map_err(storage_error)?;
    while let Some(row) = rows.next().map_err(storage_error)? {
        if Instant::now() >= until || map.counts.library_games >= 20_000 {
            return Err(PlatformError::new(ErrorCode::ResourceLimit, "Library reference graph exceeds scan bound"));
        }
        let id: String = row.get(0).map_err(storage_error)?;
        let content_hash: String = row.get(1).map_err(storage_error)?;
        let raw_json: String = row.get(2).map_err(storage_error)?;
        let manifest: GameManifest = serde_json::from_str(&raw_json)
            .map_err(|_| PlatformError::new(ErrorCode::CorruptData, "Library manifest JSON is invalid"))?;
        manifest.validate()?;
        map.counts.library_games += 1;

        let tag = format!("library:{id}");
        map.asset_retainers.entry(content_hash.clone()).or_default().insert(tag.clone());
        let mut hashes = vec![content_hash];
        if !matches!(manifest.system, GameSystem::Pc | GameSystem::Web) {
            if let Some(media) = manifest.media {
                for asset in media.assets {
                    map.asset_retainers.entry(asset.content_hash.0.clone()).or_default().insert(tag.clone());
                    hashes.push(asset.content_hash.0);
                }
            }
        }
        game_assets.insert(id, hashes);
    }
    drop(rows);
    drop(stmt);

    // 2. Active sessions (native & web sessions persisted in `sessions`)
    let mut stmt = database
        .connection
        .prepare("SELECT id, game_id FROM sessions WHERE state NOT IN ('stopped','failed') ORDER BY id LIMIT 1024")
        .map_err(storage_error)?;
    let mut rows = stmt.query([]).map_err(storage_error)?;
    while let Some(row) = rows.next().map_err(storage_error)? {
        let session_id: String = row.get(0).map_err(storage_error)?;
        let game_id: String = row.get(1).map_err(storage_error)?;
        map.counts.active_sessions += 1;
        let tag = format!("active_session:{session_id}");
        if let Some(hashes) = game_assets.get(&game_id) {
            for h in hashes {
                map.asset_retainers.entry(h.clone()).or_default().insert(tag.clone());
            }
        }
    }
    drop(rows);
    drop(stmt);

    // 3. Live saves, heads, repair files, legacy payloads, and GC queues
    let mut stmt = database
        .connection
        .prepare("SELECT id, game_id, metadata_json FROM saves ORDER BY id LIMIT 20001")
        .map_err(storage_error)?;
    let mut rows = stmt.query([]).map_err(storage_error)?;
    while let Some(row) = rows.next().map_err(storage_error)? {
        if Instant::now() >= until || map.counts.live_saves >= 20_000 {
            return Err(PlatformError::new(ErrorCode::ResourceLimit, "Save reference graph exceeds scan bound"));
        }
        let save_id: String = row.get(0).map_err(storage_error)?;
        let _game_id: String = row.get(1).map_err(storage_error)?;
        let raw_meta: String = row.get(2).map_err(storage_error)?;
        if raw_meta.len() <= 65536 {
            if let Ok(meta) = serde_json::from_str::<omniplay_contracts::SaveMetadata>(&raw_meta) {
                if meta.validate().is_ok() {
                    map.asset_retainers
                        .entry(meta.game_content_hash.0)
                        .or_default()
                        .insert(format!("save_game_ref:{save_id}"));
                }
            }
        }
        map.counts.live_saves += 1;
    }
    drop(rows);
    drop(stmt);

    map.counts.save_heads = database
        .connection
        .query_row("SELECT count(*) FROM save_heads", [], |r| r.get(0))
        .map_err(storage_error)?;

    let mut stmt = database
        .connection
        .prepare("SELECT file_id, save_id, state FROM save_repair_files ORDER BY file_id LIMIT 25000")
        .map_err(storage_error)?;
    let mut rows = stmt.query([]).map_err(storage_error)?;
    while let Some(row) = rows.next().map_err(storage_error)? {
        let file_id: String = row.get(0).map_err(storage_error)?;
        let save_id: String = row.get(1).map_err(storage_error)?;
        let state: String = row.get(2).map_err(storage_error)?;
        if state != "live" {
            map.counts.pending_save_gc += 1;
        }
        map.repair_file_retainers
            .entry(format!("{file_id}.bin"))
            .or_default()
            .insert(format!("save_repair_{state}:{save_id}"));
    }
    drop(rows);
    drop(stmt);

    let mut stmt = database
        .connection
        .prepare("SELECT save_id, game_id FROM save_legacy_payloads ORDER BY save_id LIMIT 25000")
        .map_err(storage_error)?;
    let mut rows = stmt.query([]).map_err(storage_error)?;
    while let Some(row) = rows.next().map_err(storage_error)? {
        let save_id: String = row.get(0).map_err(storage_error)?;
        let game_id: String = row.get(1).map_err(storage_error)?;
        map.legacy_save_retainers
            .entry(format!("{game_id}/{save_id}.bin"))
            .or_default()
            .insert(format!("save_legacy:{save_id}"));
    }
    drop(rows);
    drop(stmt);

    let mut stmt = database
        .connection
        .prepare("SELECT save_id, game_id FROM save_payload_gc ORDER BY save_id LIMIT 25000")
        .map_err(storage_error)?;
    let mut rows = stmt.query([]).map_err(storage_error)?;
    while let Some(row) = rows.next().map_err(storage_error)? {
        let save_id: String = row.get(0).map_err(storage_error)?;
        let game_id: String = row.get(1).map_err(storage_error)?;
        map.counts.pending_save_gc += 1;
        map.legacy_save_retainers
            .entry(format!("{game_id}/{save_id}.bin"))
            .or_default()
            .insert(format!("save_gc_pending:{save_id}"));
    }
    drop(rows);
    drop(stmt);

    // 4. Firmware assets & active role selections
    let mut stmt = database
        .connection
        .prepare("SELECT content_hash FROM firmware_assets ORDER BY content_hash LIMIT 256")
        .map_err(storage_error)?;
    let mut rows = stmt.query([]).map_err(storage_error)?;
    while let Some(row) = rows.next().map_err(storage_error)? {
        let hash: String = row.get(0).map_err(storage_error)?;
        map.counts.firmware_assets += 1;
        map.firmware_retainers.entry(format!("{hash}.bin")).or_default().insert("firmware_vault".into());
    }
    drop(rows);
    drop(stmt);

    let mut stmt = database
        .connection
        .prepare("SELECT system, role, content_hash FROM firmware_selections ORDER BY system, role LIMIT 256")
        .map_err(storage_error)?;
    let mut rows = stmt.query([]).map_err(storage_error)?;
    while let Some(row) = rows.next().map_err(storage_error)? {
        let system: String = row.get(0).map_err(storage_error)?;
        let role: String = row.get(1).map_err(storage_error)?;
        let hash: String = row.get(2).map_err(storage_error)?;
        map.counts.firmware_selections += 1;
        map.firmware_retainers
            .entry(format!("{hash}.bin"))
            .or_default()
            .insert(format!("firmware_role:{system}:{role}"));
    }
    drop(rows);
    drop(stmt);

    // 5. Native work staging reservations
    let mut stmt = database
        .connection
        .prepare("SELECT work_id, reserved_bytes, phase, manifest_sha256 FROM native_work_storage ORDER BY work_id LIMIT 128")
        .map_err(storage_error)?;
    let mut rows = stmt.query([]).map_err(storage_error)?;
    while let Some(row) = rows.next().map_err(storage_error)? {
        let work_id: String = row.get(0).map_err(storage_error)?;
        let reserved_bytes: u64 = row.get(1).map_err(storage_error)?;
        let phase: String = row.get(2).map_err(storage_error)?;
        let manifest_sha: Option<String> = row.get(3).map_err(storage_error)?;
        map.counts.native_work_debts += 1;
        map.reserved_work_bytes = map.reserved_work_bytes.saturating_add(reserved_bytes);
        if let Some(sha) = manifest_sha {
            map.asset_retainers.entry(sha).or_default().insert(format!("native_work_{phase}:{work_id}"));
        }
    }

    if let Some(staging_raw) = database
        .connection
        .query_row(
            "SELECT substr(value,1,262145) FROM preferences WHERE key=?1",
            [crate::profile_backup::PROFILE_RESTORE_STAGING_KEY],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(storage_error)?
    {
        if staging_raw.len() <= 262_144 {
            if let Ok(staging) = serde_json::from_str::<crate::profile_backup::ProfileRestoreStagingRecord>(&staging_raw) {
                let tag = format!("profile_restore_staging:{}", staging.archive_hash.0);
                for hash in staging.asset_hashes {
                    map.asset_retainers.entry(hash.0).or_default().insert(tag.clone());
                }
            }
        }
    }
    Ok(map)
}

/// Returns the set of asset hashes currently retained by active sessions, native-work staging,
/// or in-progress profile restore staging, so `asset_storage` never collects a pinned blob.
pub fn transient_asset_retainers(database: &Database) -> Result<BTreeSet<String>> {
    let until = Instant::now() + Duration::from_secs(10);
    let refs = collect_retainers(database, until)?;
    let mut pinned = BTreeSet::new();
    for (hash, tags) in refs.asset_retainers {
        if tags.iter().any(|t| {
            t.starts_with("active_session:")
                || t.starts_with("native_work_")
                || t.starts_with("profile_restore_staging:")
        }) {
            pinned.insert(hash);
        }
    }
    Ok(pinned)
}

/// Generates a full physical allocation and active reference accounting report for the profile.
pub fn report(database: &Database) -> Result<StorageAccountingReport> {
    let until = Instant::now() + Duration::from_secs(20);
    let (refs, quota_bytes) = if database.connection.is_autocommit() {
        let tx = Transaction::new_unchecked(&database.connection, TransactionBehavior::Deferred).map_err(storage_error)?;
        let r = collect_retainers(database, until)?;
        let q = get_physical_quota_bytes(database)?;
        tx.commit().map_err(storage_error)?;
        (r, q)
    } else {
        (collect_retainers(database, until)?, get_physical_quota_bytes(database)?)
    };

    let mut seen_inodes = BTreeSet::new();
    let mut hardlink_deduped = 0u64;
    let mut visited = 0usize;
    let mut namespaces = Vec::new();
    let mut retained_blobs = Vec::new();

    // 1. Database files (`omniplay.sqlite3`, `-wal`, `-shm`)
    {
        let mut ns = NamespaceAccounting { namespace: "database".into(), ..Default::default() };
        for suffix in ["omniplay.sqlite3", "omniplay.sqlite3-wal", "omniplay.sqlite3-shm"] {
            let path = database.root.join(suffix);
            if let Ok(meta) = fs::symlink_metadata(&path) {
                if plain_entry(&meta, false) {
                    ns.logical_bytes = ns.logical_bytes.saturating_add(meta.len());
                    ns.physical_bytes = ns.physical_bytes.saturating_add(file_physical_bytes(
                        &path,
                        &meta,
                        &mut seen_inodes,
                        &mut hardlink_deduped,
                    ));
                    ns.file_count += 1;
                } else {
                    ns.retained_unknown_entries += 1;
                }
            }
        }
        namespaces.push(ns);
    }

    // 2. `assets/` namespace with per-blob active retainer resolution
    {
        let mut ns = NamespaceAccounting { namespace: "assets".into(), ..Default::default() };
        let dir = database.root.join("assets");
        if let Ok(dir_meta) = fs::symlink_metadata(&dir) {
            if !plain_entry(&dir_meta, true) {
                return Err(PlatformError::new(ErrorCode::StorageFailure, "Assets directory is not an ordinary directory"));
            }
            for entry in fs::read_dir(&dir).map_err(storage_error)? {
                if Instant::now() >= until || visited >= SCAN_ENTRY_LIMIT {
                    return Err(PlatformError::new(ErrorCode::ResourceLimit, "Storage accounting exceeded scan limit"));
                }
                visited += 1;
                let entry = entry.map_err(storage_error)?;
                let entry_path = entry.path();
                let meta = fs::symlink_metadata(&entry_path).map_err(storage_error)?;
                if !plain_entry(&meta, false) {
                    ns.retained_unknown_entries += 1;
                    continue;
                }
                let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                    ns.retained_unknown_entries += 1;
                    continue;
                };
                let phys = file_physical_bytes(&entry_path, &meta, &mut seen_inodes, &mut hardlink_deduped);
                ns.logical_bytes = ns.logical_bytes.saturating_add(meta.len());
                ns.physical_bytes = ns.physical_bytes.saturating_add(phys);
                ns.file_count += 1;
                if let Ok(hash) = ContentHash::parse(name.clone()) {
                    if let Some(tags) = refs.asset_retainers.get(&name) {
                        if retained_blobs.len() < 512 {
                            retained_blobs.push(BlobRetentionSummary {
                                content_hash: hash,
                                namespace: "assets".into(),
                                logical_bytes: meta.len(),
                                physical_bytes: phys,
                                retainers: tags.iter().cloned().collect(),
                            });
                        }
                    } else {
                        ns.unreferenced_files += 1;
                        ns.unreferenced_physical_bytes = ns.unreferenced_physical_bytes.saturating_add(phys);
                    }
                } else {
                    ns.retained_unknown_entries += 1;
                }
            }
        }
        namespaces.push(ns);
    }

    // 3. `repaired-saves-v1/` namespace
    {
        let mut ns = NamespaceAccounting { namespace: "repaired_saves".into(), ..Default::default() };
        let dir = database.root.join("repaired-saves-v1");
        if let Ok(dir_meta) = fs::symlink_metadata(&dir) {
            if plain_entry(&dir_meta, true) {
                for entry in fs::read_dir(&dir).map_err(storage_error)? {
                    visited += 1;
                    let entry = entry.map_err(storage_error)?;
                    let entry_path = entry.path();
                    let meta = fs::symlink_metadata(&entry_path).map_err(storage_error)?;
                    if !plain_entry(&meta, false) {
                        ns.retained_unknown_entries += 1;
                        continue;
                    }
                    let phys = file_physical_bytes(&entry_path, &meta, &mut seen_inodes, &mut hardlink_deduped);
                    ns.logical_bytes = ns.logical_bytes.saturating_add(meta.len());
                    ns.physical_bytes = ns.physical_bytes.saturating_add(phys);
                    ns.file_count += 1;
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if !refs.repair_file_retainers.contains_key(&name) {
                        ns.unreferenced_files += 1;
                        ns.unreferenced_physical_bytes = ns.unreferenced_physical_bytes.saturating_add(phys);
                    }
                }
            }
        }
        namespaces.push(ns);
    }

    // 4. `saves/` legacy save namespace (one directory level `<game_id>/<save_id>.bin`)
    {
        let mut ns = NamespaceAccounting { namespace: "legacy_saves".into(), ..Default::default() };
        let dir = database.root.join("saves");
        if let Ok(dir_meta) = fs::symlink_metadata(&dir) {
            if plain_entry(&dir_meta, true) {
                for game_entry in fs::read_dir(&dir).map_err(storage_error)? {
                    visited += 1;
                    let game_entry = game_entry.map_err(storage_error)?;
                    let game_meta = fs::symlink_metadata(game_entry.path()).map_err(storage_error)?;
                    if !plain_entry(&game_meta, true) {
                        ns.retained_unknown_entries += 1;
                        continue;
                    }
                    let game_name = game_entry.file_name().to_string_lossy().into_owned();
                    for leaf in fs::read_dir(game_entry.path()).map_err(storage_error)? {
                        visited += 1;
                        let leaf = leaf.map_err(storage_error)?;
                        let leaf_path = leaf.path();
                        let leaf_meta = fs::symlink_metadata(&leaf_path).map_err(storage_error)?;
                        if !plain_entry(&leaf_meta, false) {
                            ns.retained_unknown_entries += 1;
                            continue;
                        }
                        let phys = file_physical_bytes(&leaf_path, &leaf_meta, &mut seen_inodes, &mut hardlink_deduped);
                        ns.logical_bytes = ns.logical_bytes.saturating_add(leaf_meta.len());
                        ns.physical_bytes = ns.physical_bytes.saturating_add(phys);
                        ns.file_count += 1;
                        let rel = format!("{game_name}/{}", leaf.file_name().to_string_lossy());
                        if !refs.legacy_save_retainers.contains_key(&rel) {
                            ns.unreferenced_files += 1;
                            ns.unreferenced_physical_bytes = ns.unreferenced_physical_bytes.saturating_add(phys);
                        }
                    }
                }
            }
        }
        namespaces.push(ns);
    }

    // 5. `firmware/` namespace
    {
        let mut ns = NamespaceAccounting { namespace: "firmware".into(), ..Default::default() };
        let dir = database.root.join("firmware");
        if let Ok(dir_meta) = fs::symlink_metadata(&dir) {
            if plain_entry(&dir_meta, true) {
                for entry in fs::read_dir(&dir).map_err(storage_error)? {
                    visited += 1;
                    let entry = entry.map_err(storage_error)?;
                    let entry_path = entry.path();
                    let meta = fs::symlink_metadata(&entry_path).map_err(storage_error)?;
                    if !plain_entry(&meta, false) {
                        ns.retained_unknown_entries += 1;
                        continue;
                    }
                    let phys = file_physical_bytes(&entry_path, &meta, &mut seen_inodes, &mut hardlink_deduped);
                    ns.logical_bytes = ns.logical_bytes.saturating_add(meta.len());
                    ns.physical_bytes = ns.physical_bytes.saturating_add(phys);
                    ns.file_count += 1;
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if let Some(tags) = refs.firmware_retainers.get(&name) {
                        if let Some(hex) = name.strip_suffix(".bin") {
                            if let Ok(hash) = ContentHash::parse(hex.to_owned()) {
                                if retained_blobs.len() < 512 {
                                    retained_blobs.push(BlobRetentionSummary {
                                        content_hash: hash,
                                        namespace: "firmware".into(),
                                        logical_bytes: meta.len(),
                                        physical_bytes: phys,
                                        retainers: tags.iter().cloned().collect(),
                                    });
                                }
                            }
                        }
                    } else {
                        ns.unreferenced_files += 1;
                        ns.unreferenced_physical_bytes = ns.unreferenced_physical_bytes.saturating_add(phys);
                    }
                }
            }
        }
        namespaces.push(ns);
    }

    // 6. `native-work/` namespace
    let native_work_phys;
    {
        let (logical, physical, files, unknown) = scan_tree_recursive(
            &database.root.join("native-work"),
            4,
            &mut seen_inodes,
            &mut hardlink_deduped,
            &mut visited,
            until,
        )?;
        native_work_phys = physical;
        namespaces.push(NamespaceAccounting {
            namespace: "native_work".into(),
            logical_bytes: logical,
            physical_bytes: physical,
            file_count: files,
            unreferenced_files: 0,
            unreferenced_physical_bytes: 0,
            retained_unknown_entries: unknown,
        });
    }

    // 7. `corrupt-db-quarantine/` namespace (from Item 47 recovery)
    {
        let (logical, physical, files, unknown) = scan_tree_recursive(
            &database.root.join("corrupt-db-quarantine"),
            2,
            &mut seen_inodes,
            &mut hardlink_deduped,
            &mut visited,
            until,
        )?;
        if files > 0 || unknown > 0 {
            namespaces.push(NamespaceAccounting {
                namespace: "corrupt_db_quarantine".into(),
                logical_bytes: logical,
                physical_bytes: physical,
                file_count: files,
                unreferenced_files: files,
                unreferenced_physical_bytes: physical,
                retained_unknown_entries: unknown,
            });
        }
    }

    let total_logical: u64 = namespaces.iter().fold(0u64, |acc, n| acc.saturating_add(n.logical_bytes));
    let total_physical: u64 = namespaces.iter().fold(0u64, |acc, n| acc.saturating_add(n.physical_bytes));
    let unmaterialized_reserved = refs.reserved_work_bytes.saturating_sub(native_work_phys);
    let effective_committed = total_physical.saturating_add(unmaterialized_reserved);
    let available = quota_bytes.saturating_sub(effective_committed);

    Ok(StorageAccountingReport {
        schema_version: 1,
        logical_bytes: total_logical,
        physical_bytes: total_physical,
        unmaterialized_reserved_bytes: unmaterialized_reserved,
        effective_committed_physical_bytes: effective_committed,
        maximum_physical_bytes: quota_bytes,
        available_physical_bytes: available,
        hardlink_deduped_files: hardlink_deduped,
        namespaces,
        retainer_counts: refs.counts,
        retained_blobs,
    })
}

/// Enforces the profile-wide physical storage quota before writing or reserving new bytes.
pub fn enforce_physical_quota(
    database: &Database,
    additional_logical_bytes: u64,
    additional_files: u32,
) -> Result<QuotaAdmission> {
    let summary = report(database)?;
    let requested_physical = round_cluster_bytes(additional_logical_bytes)
        .saturating_add(u64::from(additional_files).saturating_mul(CLUSTER_BYTES));
    let projected = summary
        .effective_committed_physical_bytes
        .checked_add(requested_physical)
        .ok_or_else(|| PlatformError::new(ErrorCode::ResourceLimit, "Projected physical storage overflows quota"))?;
    if projected > summary.maximum_physical_bytes {
        return Err(PlatformError::new(
            ErrorCode::ResourceLimit,
            "Profile physical storage quota would be exceeded",
        )
        .recover("Remove unused library assets, clear retired saves, or increase the profile physical quota"));
    }
    Ok(QuotaAdmission {
        current_physical_bytes: summary.physical_bytes,
        unmaterialized_reserved_bytes: summary.unmaterialized_reserved_bytes,
        requested_physical_bytes: requested_physical,
        projected_physical_bytes: projected,
        maximum_physical_bytes: summary.maximum_physical_bytes,
    })
}

use rusqlite::Transaction;
