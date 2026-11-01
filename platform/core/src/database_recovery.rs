//! Diagnosis, non-destructive quarantine, row-level salvage, and reconstruction
//! of corrupted SQLite profile databases (`omniplay.sqlite3`).
//!
//! Damaged database files and `-wal`/`-shm` sidecars are never deleted or modified
//! in place: they are moved atomically into `corrupt-db-quarantine/<id>/` with an
//! audited SHA-256 receipt before initializing a clean schema-15 database.
use crate::{
    database::CURRENT_SCHEMA_VERSION,
    library_reconstruct,
    now_ms,
    saves::SaveStore,
    util::{bounded_read, digest, hash_file, storage_error, sync_directory},
    Database,
};
use omniplay_contracts::{
    ContentHash, ErrorCode, GameCoreOptions, GameInputProfile, GameManifest, GameSystem,
    GameTouchProfile, PlatformError, Result, SaveMetadata, SessionConfig, UserSettings,
    MAX_CORE_OPTION_PROFILE_BYTES, MAX_SAVE_BYTES,
};
use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;

const SQLITE_HEADER_MAGIC: &[u8; 16] = b"SQLite format 3\0";
pub const RECOVERY_JOURNAL_FILE: &str = "database-recovery-journal.json";
const STAGING_DB_FILE: &str = "omniplay.recovering.sqlite3";
const SALVAGED_SNAPSHOT_FILE: &str = "salvaged-snapshot.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryJournalPhase {
    Quarantining,
    Rebuilding,
    Publishing,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct RecoveryJournalPayload {
    schema_version: u32,
    quarantine_id: String,
    started_at_ms: u64,
    previous_status: DatabaseHealthStatus,
    phase: RecoveryJournalPhase,
    salvaged_sha256: ContentHash,
    quarantined_files: Vec<QuarantinedFileReceipt>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct RecoveryJournalEnvelope {
    payload: RecoveryJournalPayload,
    journal_sha256: ContentHash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseHealthStatus {
    Healthy,
    Missing,
    HeaderCorrupt,
    Unopenable,
    NewerSchema,
    IntegrityFailed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DatabaseHealthReport {
    pub schema_version: u32,
    pub status: DatabaseHealthStatus,
    pub user_version: Option<u32>,
    pub db_bytes: u64,
    pub wal_present: bool,
    pub shm_present: bool,
    pub recoverable: bool,
    pub findings: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct QuarantinedFileReceipt {
    pub name: String,
    pub byte_length: u64,
    pub sha256: ContentHash,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DatabaseRecoveryOutcome {
    pub schema_version: u32,
    pub previous_status: DatabaseHealthStatus,
    pub quarantine_id: String,
    pub quarantined_files: Vec<QuarantinedFileReceipt>,
    pub preserved_device_id: bool,
    pub salvaged_settings: bool,
    pub salvaged_games: usize,
    pub reconstructed_games: usize,
    pub salvaged_firmware: usize,
    pub salvaged_saves: usize,
    pub salvaged_suppression_markers: usize,
}

fn plain_entry(meta: &fs::Metadata, expect_dir: bool) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    !meta.file_type().is_symlink() && if expect_dir { meta.is_dir() } else { meta.is_file() }
}

fn acquire_recovery_lease(root: &Path) -> Result<File> {
    fs::create_dir_all(root).map_err(storage_error)?;
    let root_meta = fs::symlink_metadata(root).map_err(storage_error)?;
    if !plain_entry(&root_meta, true) {
        return Err(PlatformError::new(
            ErrorCode::StorageFailure,
            "Profile directory is not an ordinary directory",
        ));
    }
    let lock_path = root.join("database-recovery.lock");
    let mut opts = OpenOptions::new();
    opts.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600).custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        opts.custom_flags(0x00200000).share_mode(3);
    }
    let file = opts.open(lock_path).map_err(storage_error)?;
    let meta = file.metadata().map_err(storage_error)?;
    if !plain_entry(&meta, false) || meta.len() != 0 {
        return Err(PlatformError::new(ErrorCode::CorruptData, "Database recovery lock file is invalid"));
    }
    file.try_lock()
        .map_err(|_| PlatformError::new(ErrorCode::Busy, "Database recovery is already in progress"))?;
    Ok(file)
}

/// Diagnoses `omniplay.sqlite3` without mutating or creating database files.
pub fn diagnose(root: &Path) -> Result<DatabaseHealthReport> {
    let db_path = root.join("omniplay.sqlite3");
    let wal_present = fs::symlink_metadata(root.join("omniplay.sqlite3-wal")).is_ok();
    let shm_present = fs::symlink_metadata(root.join("omniplay.sqlite3-shm")).is_ok();

    let meta = match fs::symlink_metadata(&db_path) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            let journal_present = fs::symlink_metadata(root.join(RECOVERY_JOURNAL_FILE)).is_ok();
            let mut findings = vec!["omniplay.sqlite3 is absent".into()];
            if journal_present {
                findings.push("incomplete database-recovery-journal.json detected; recovery will resume cleanly".into());
            }
            return Ok(DatabaseHealthReport {
                schema_version: 1,
                status: DatabaseHealthStatus::Missing,
                user_version: None,
                db_bytes: 0,
                wal_present,
                shm_present,
                recoverable: journal_present
                    || wal_present
                    || root.join("assets").exists()
                    || root.join("saves").exists()
                    || root.join("repaired-saves-v1").exists(),
                findings,
            });
        }
        Err(err) => return Err(storage_error(err)),
        Ok(m) => m,
    };

    if !plain_entry(&meta, false) {
        return Err(PlatformError::new(
            ErrorCode::StorageFailure,
            "Database path is a symbolic link, directory, or reparse point",
        ));
    }

    let db_bytes = meta.len();
    let mut findings = Vec::new();
    if db_bytes > 0 {
        if db_bytes < 100 {
            findings.push("SQLite file is truncated below 100-byte header minimum".into());
            return Ok(DatabaseHealthReport {
                schema_version: 1,
                status: DatabaseHealthStatus::HeaderCorrupt,
                user_version: None,
                db_bytes,
                wal_present,
                shm_present,
                recoverable: true,
                findings,
            });
        }
        let mut f = File::open(&db_path).map_err(storage_error)?;
        let mut hdr = [0u8; 16];
        f.read_exact(&mut hdr).map_err(storage_error)?;
        if &hdr != SQLITE_HEADER_MAGIC {
            findings.push("SQLite file header magic does not match 'SQLite format 3\\0'".into());
            return Ok(DatabaseHealthReport {
                schema_version: 1,
                status: DatabaseHealthStatus::HeaderCorrupt,
                user_version: None,
                db_bytes,
                wal_present,
                shm_present,
                recoverable: true,
                findings,
            });
        }
    }

    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = match Connection::open_with_flags(&db_path, flags) {
        Ok(c) => c,
        Err(_) => {
            findings.push("SQLite engine could not open database in read-only mode".into());
            return Ok(DatabaseHealthReport {
                schema_version: 1,
                status: DatabaseHealthStatus::Unopenable,
                user_version: None,
                db_bytes,
                wal_present,
                shm_present,
                recoverable: true,
                findings,
            });
        }
    };
    let _ = conn.busy_timeout(std::time::Duration::from_secs(2));
    let _ = conn.execute_batch("PRAGMA trusted_schema=OFF;");

    let user_version: u32 = match conn.query_row("PRAGMA user_version", [], |r| r.get(0)) {
        Ok(v) => v,
        Err(_) => {
            findings.push("Failed reading PRAGMA user_version (page 1 or WAL corrupt)".into());
            return Ok(DatabaseHealthReport {
                schema_version: 1,
                status: DatabaseHealthStatus::IntegrityFailed,
                user_version: None,
                db_bytes,
                wal_present,
                shm_present,
                recoverable: true,
                findings,
            });
        }
    };

    if user_version > CURRENT_SCHEMA_VERSION {
        findings.push(format!(
            "Database schema version {user_version} exceeds supported version {CURRENT_SCHEMA_VERSION}"
        ));
        return Ok(DatabaseHealthReport {
            schema_version: 1,
            status: DatabaseHealthStatus::NewerSchema,
            user_version: Some(user_version),
            db_bytes,
            wal_present,
            shm_present,
            recoverable: false,
            findings,
        });
    }

    // Run bounded SQLite B-tree quick_check
    match conn.prepare("PRAGMA quick_check(16)") {
        Ok(mut stmt) => match stmt.query_map([], |r| r.get::<_, String>(0)) {
            Ok(rows) => {
                for row in rows {
                    match row {
                        Ok(msg) if msg == "ok" => {}
                        Ok(msg) => findings.push(format!("quick_check: {msg}")),
                        Err(_) => findings.push("quick_check row read failed".into()),
                    }
                }
            }
            Err(_) => findings.push("PRAGMA quick_check execution failed".into()),
        },
        Err(_) => findings.push("PRAGMA quick_check preparation failed".into()),
    }

    // Run foreign_key_check
    if user_version >= 1 {
        match conn.prepare("PRAGMA foreign_key_check") {
            Ok(mut stmt) => match stmt.query_map([], |r| r.get::<_, String>(0)) {
                Ok(mut rows) => {
                    if let Some(first) = rows.next() {
                        let tbl = first.unwrap_or_else(|_| "unknown".into());
                        findings.push(format!("foreign_key_check violation in table {tbl}"));
                    }
                }
                Err(_) => findings.push("PRAGMA foreign_key_check execution failed".into()),
            },
            Err(_) => findings.push("PRAGMA foreign_key_check preparation failed".into()),
        }

        // Validate essential JSON & identity records in core tables
        match conn.query_row("SELECT value FROM preferences WHERE key='device_id'", [], |r| r.get::<_, String>(0)) {
            Ok(raw_id) => {
                if Uuid::parse_str(&raw_id).is_err() {
                    findings.push("preferences.device_id is not a valid UUID".into());
                }
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {}
            Err(_) => findings.push("preferences table could not be queried".into()),
        }

        if let Ok(raw_settings) = conn.query_row(
            "SELECT value FROM preferences WHERE key='settings'",
            [],
            |r| r.get::<_, String>(0),
        ) {
            match serde_json::from_str::<UserSettings>(&raw_settings) {
                Ok(s) => {
                    if s.validate().is_err() {
                        findings.push("preferences.settings failed policy validation".into());
                    }
                }
                Err(_) => findings.push("preferences.settings contains malformed JSON".into()),
            }
        }

        match conn.prepare("SELECT id, content_hash, manifest_json FROM games LIMIT 5000") {
            Ok(mut stmt) => match stmt.query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
            }) {
                Ok(rows) => {
                    for row in rows {
                        match row {
                            Ok((id, chash, mjson)) => {
                                match serde_json::from_str::<GameManifest>(&mjson) {
                                    Ok(m) => {
                                        if m.validate().is_err() || m.id.to_string() != id || m.content_hash.0 != chash {
                                            findings.push(format!("games row {id} failed manifest validation"));
                                            break;
                                        }
                                    }
                                    Err(_) => {
                                        findings.push(format!("games row {id} has corrupt manifest_json"));
                                        break;
                                    }
                                }
                            }
                            Err(_) => {
                                findings.push("games table B-tree read error".into());
                                break;
                            }
                        }
                    }
                }
                Err(_) => findings.push("games table scan failed".into()),
            },
            Err(_) => findings.push("games table is missing or unreadable".into()),
        }
    }

    let status = if findings.is_empty() {
        DatabaseHealthStatus::Healthy
    } else {
        DatabaseHealthStatus::IntegrityFailed
    };

    Ok(DatabaseHealthReport {
        schema_version: 1,
        status,
        user_version: Some(user_version),
        db_bytes,
        wal_present,
        shm_present,
        recoverable: status != DatabaseHealthStatus::Healthy,
        findings,
    })
}

#[derive(Default, Clone, Serialize, Deserialize)]
struct SalvagedData {
    device_id: Option<Uuid>,
    settings: Option<UserSettings>,
    quota_bytes: Option<u64>,
    removed_saves: Vec<(Uuid, ContentHash, u64)>,
    firmware: Vec<(ContentHash, u64, u64)>,
    firmware_selections: Vec<(GameSystem, String, ContentHash)>,
    games: Vec<SalvagedGame>,
    saves: Vec<SalvagedSave>,
    head_save_ids: BTreeSet<Uuid>,
}

#[derive(Clone, Serialize, Deserialize)]
struct SalvagedGame {
    manifest: GameManifest,
    favorite: bool,
    playtime_ms: u64,
    last_played_at_ms: Option<u64>,
    session_config: Option<SessionConfig>,
    input_profile: Option<GameInputProfile>,
    touch_profile: Option<GameTouchProfile>,
    core_options: BTreeMap<String, GameCoreOptions>,
}

#[derive(Clone, Serialize, Deserialize)]
struct SalvagedSave {
    metadata: SaveMetadata,
}

fn assets_exist_for_manifest(root: &Path, manifest: &GameManifest) -> bool {
    let assets_dir = root.join("assets");
    if let Some(media) = &manifest.media {
        media.assets.iter().all(|a| {
            fs::symlink_metadata(assets_dir.join(&a.content_hash.0))
                .is_ok_and(|m| plain_entry(&m, false) && m.len() == a.byte_length)
        })
    } else {
        fs::symlink_metadata(assets_dir.join(&manifest.content_hash.0))
            .is_ok_and(|m| plain_entry(&m, false) && m.len() == manifest.byte_length)
    }
}

fn index_disk_save_payloads(root: &Path) -> BTreeMap<(String, u64), PathBuf> {
    let mut index = BTreeMap::new();
    let repaired = root.join("repaired-saves-v1");
    if let Ok(entries) = fs::read_dir(&repaired) {
        for entry in entries.take(10_000).flatten() {
            let path = entry.path();
            if let Ok(meta) = fs::symlink_metadata(&path) {
                if plain_entry(&meta, false) && meta.len() > 0 && meta.len() <= MAX_SAVE_BYTES {
                    if let Ok(mut file) = File::open(&path) {
                        if let Ok((h, sz)) = hash_file(&mut file, MAX_SAVE_BYTES) {
                            index.insert((h.0, sz), path);
                        }
                    }
                }
            }
        }
    }
    let legacy = root.join("saves");
    if let Ok(game_dirs) = fs::read_dir(&legacy) {
        for gdir in game_dirs.take(2_000).flatten() {
            if fs::symlink_metadata(gdir.path()).is_ok_and(|m| plain_entry(&m, true)) {
                if let Ok(files) = fs::read_dir(gdir.path()) {
                    for entry in files.take(2_000).flatten() {
                        let path = entry.path();
                        if let Ok(meta) = fs::symlink_metadata(&path) {
                            if plain_entry(&meta, false) && meta.len() > 0 && meta.len() <= MAX_SAVE_BYTES {
                                if let Ok(mut file) = File::open(&path) {
                                    if let Ok((h, sz)) = hash_file(&mut file, MAX_SAVE_BYTES) {
                                        index.entry((h.0, sz)).or_insert(path);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    index
}

fn salvage_from_damaged(root: &Path) -> SalvagedData {
    let mut out = SalvagedData::default();
    let db_path = root.join("omniplay.sqlite3");
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let Ok(conn) = Connection::open_with_flags(&db_path, flags) else {
        return out;
    };
    let _ = conn.busy_timeout(std::time::Duration::from_secs(2));
    let _ = conn.execute_batch("PRAGMA trusted_schema=OFF;");

    // 1. Salvage preferences
    let mut pref_map = BTreeMap::new();
    if let Ok(mut stmt) = conn.prepare("SELECT key, value FROM preferences LIMIT 20000") {
        if let Ok(rows) = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))) {
            for (k, v) in rows.flatten() {
                if k.len() <= 256 && v.len() <= 262_144 {
                    pref_map.insert(k, v);
                }
            }
        }
    }
    if let Some(raw_id) = pref_map.get("device_id") {
        if let Ok(id) = Uuid::parse_str(raw_id) {
            if !id.is_nil() {
                out.device_id = Some(id);
            }
        }
    }
    if let Some(raw_settings) = pref_map.get("settings") {
        if let Ok(mut s) = serde_json::from_str::<UserSettings>(raw_settings) {
            if s.validate().is_ok() {
                // Keep cloud_sync_opt_in conservative until sync credentials are verified
                s.cloud_sync_opt_in = false;
                out.settings = Some(s);
            }
        }
    }
    if let Some(raw_quota) = pref_map.get("storage_physical_quota_bytes_v1") {
        if let Ok(q) = raw_quota.parse::<u64>() {
            if (crate::storage_accounting::MIN_PROFILE_PHYSICAL_QUOTA_BYTES
                ..=crate::storage_accounting::MAX_PROFILE_PHYSICAL_QUOTA_BYTES)
                .contains(&q)
            {
                out.quota_bytes = Some(q);
            }
        }
    }

    // 2. Salvage local save suppression markers
    if let Ok(mut stmt) = conn.prepare("SELECT save_id, game_content_hash, removed_at_ms FROM locally_removed_saves LIMIT 20000") {
        if let Ok(rows) = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, u64>(2)?))
        }) {
            for (sid, ghash, ts) in rows.flatten() {
                if let (Ok(id), Ok(ch)) = (Uuid::parse_str(&sid), ContentHash::parse(ghash)) {
                    out.removed_saves.push((id, ch, ts));
                }
            }
        }
    }

    // 3. Salvage firmware assets & selections verified on disk
    if let Ok(mut stmt) = conn.prepare("SELECT content_hash, byte_length, created_at_ms FROM firmware_assets LIMIT 128") {
        if let Ok(rows) = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, u64>(1)?, r.get::<_, u64>(2)?))
        }) {
            for (raw_hash, byte_len, created) in rows.flatten() {
                let Ok(hash) = ContentHash::parse(raw_hash.clone()) else { continue };
                let path = root.join("firmware").join(format!("{raw_hash}.bin"));
                if let Ok(mut f) = File::open(&path) {
                    if let Ok(bytes) = bounded_read(&mut f, byte_len) {
                        if bytes.len() as u64 == byte_len && digest(&bytes) == hash {
                            out.firmware.push((hash, byte_len, created));
                        }
                    }
                }
            }
        }
    }
    if let Ok(mut stmt) = conn.prepare("SELECT system, role, content_hash FROM firmware_selections LIMIT 128") {
        if let Ok(rows) = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
        }) {
            for (sys_str, role, raw_hash) in rows.flatten() {
                let Ok(sys) = serde_json::from_value::<GameSystem>(serde_json::Value::String(sys_str)) else {
                    continue;
                };
                let Ok(hash) = ContentHash::parse(raw_hash) else { continue };
                if crate::firmware::role_sizes(sys, &role).is_ok()
                    && out.firmware.iter().any(|(h, _, _)| h == &hash)
                {
                    out.firmware_selections.push((sys, role, hash));
                }
            }
        }
    }

    // 4. Salvage games & per-game settings
    if let Ok(mut stmt) = conn.prepare(
        "SELECT manifest_json, COALESCE(favorite,0), COALESCE(playtime_ms,0), last_played_at_ms FROM games LIMIT 5000",
    ) {
        if let Ok(rows) = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, bool>(1)?,
                r.get::<_, u64>(2)?,
                r.get::<_, Option<u64>>(3)?,
            ))
        }) {
            for (mjson, fav, playtime, last_played) in rows.flatten() {
                let Ok(manifest) = serde_json::from_str::<GameManifest>(&mjson) else { continue };
                if manifest.validate().is_err() || !assets_exist_for_manifest(root, &manifest) {
                    continue;
                }
                let gid = manifest.id;
                let session_config = pref_map
                    .get(&format!("session.{gid}"))
                    .and_then(|s| serde_json::from_str::<SessionConfig>(s).ok())
                    .filter(|c| c.validate().is_ok());
                let input_profile = pref_map
                    .get(&format!("input.{gid}"))
                    .and_then(|s| serde_json::from_str::<GameInputProfile>(s).ok())
                    .filter(|p| p.validate().is_ok());
                let touch_profile = pref_map
                    .get(&format!("touch.{gid}"))
                    .and_then(|s| serde_json::from_str::<GameTouchProfile>(s).ok())
                    .filter(|p| p.validate().is_ok());

                let mut core_options = BTreeMap::new();
                let prefix = format!("core_options.{gid}.");
                for (k, v) in &pref_map {
                    if let Some(rt) = k.strip_prefix(&prefix) {
                        if omniplay_contracts::core_option_identifier(rt) && v.len() <= MAX_CORE_OPTION_PROFILE_BYTES {
                            if let Ok(opts) = serde_json::from_str::<GameCoreOptions>(v) {
                                if opts.validate().is_ok() {
                                    core_options.insert(rt.to_owned(), opts);
                                }
                            }
                        }
                    }
                }

                out.games.push(SalvagedGame {
                    manifest,
                    favorite: fav,
                    playtime_ms: playtime,
                    last_played_at_ms: last_played,
                    session_config,
                    input_profile,
                    touch_profile,
                    core_options,
                });
            }
        }
    }

    // 5. Salvage save heads & verified save generations
    if let Ok(mut stmt) = conn.prepare("SELECT save_id FROM save_heads LIMIT 5000") {
        if let Ok(rows) = stmt.query_map([], |r| r.get::<_, String>(0)) {
            for raw in rows.flatten() {
                if let Ok(id) = Uuid::parse_str(&raw) {
                    out.head_save_ids.insert(id);
                }
            }
        }
    }

    let payload_index = index_disk_save_payloads(root);
    if let Ok(mut stmt) = conn.prepare("SELECT metadata_json FROM saves ORDER BY generation ASC, created_at_ms ASC LIMIT 5000") {
        if let Ok(rows) = stmt.query_map([], |r| r.get::<_, String>(0)) {
            for raw in rows.flatten() {
                let Ok(meta) = serde_json::from_str::<SaveMetadata>(&raw) else { continue };
                if meta.validate().is_err() {
                    continue;
                }
                let Some(path) = payload_index.get(&(meta.payload_hash.0.clone(), meta.payload_bytes)) else {
                    continue;
                };
                let Ok(mut f) = File::open(path) else { continue };
                let Ok(payload) = bounded_read(&mut f, meta.payload_bytes) else { continue };
                if payload.len() as u64 == meta.payload_bytes && digest(&payload) == meta.payload_hash {
                    out.saves.push(SalvagedSave { metadata: meta });
                }
            }
        }
    }
    out
}

fn write_atomic_json(directory: &Path, filename: &str, bytes: &[u8]) -> Result<()> {
    fs::create_dir_all(directory).map_err(storage_error)?;
    let tmp_path = directory.join(format!(".recovery-tmp-{}", Uuid::new_v4()));
    {
        let mut opts = OpenOptions::new();
        opts.write(true).read(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut file = opts.open(&tmp_path).map_err(storage_error)?;
        file.write_all(bytes).map_err(storage_error)?;
        file.sync_all().map_err(storage_error)?;
    }
    if let Err(err) = fs::rename(&tmp_path, directory.join(filename)) {
        let _ = fs::remove_file(&tmp_path);
        return Err(storage_error(err));
    }
    sync_directory(directory)
}

fn write_recovery_journal(root: &Path, payload: &RecoveryJournalPayload) -> Result<()> {
    let payload_bytes = serde_json::to_vec(payload).map_err(storage_error)?;
    let envelope = RecoveryJournalEnvelope {
        payload: payload.clone(),
        journal_sha256: digest(&payload_bytes),
    };
    let raw = serde_json::to_vec_pretty(&envelope).map_err(storage_error)?;
    write_atomic_json(root, RECOVERY_JOURNAL_FILE, &raw)
}

fn read_recovery_journal(root: &Path) -> Result<Option<RecoveryJournalPayload>> {
    let path = root.join(RECOVERY_JOURNAL_FILE);
    let mut file = match File::open(&path) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(storage_error(err)),
        Ok(f) => f,
    };
    let bytes = bounded_read(&mut file, 1024 * 1024)?;
    let envelope: RecoveryJournalEnvelope = serde_json::from_slice(&bytes).map_err(|_| {
        PlatformError::new(
            ErrorCode::CorruptData,
            "Database recovery journal is unreadable or truncated",
        )
    })?;
    if envelope.payload.schema_version != 1 {
        return Err(PlatformError::new(
            ErrorCode::CorruptData,
            "Database recovery journal uses an unsupported schema version",
        ));
    }
    let canonical = serde_json::to_vec(&envelope.payload).map_err(storage_error)?;
    if digest(&canonical) != envelope.journal_sha256 {
        return Err(PlatformError::new(
            ErrorCode::CorruptData,
            "Database recovery journal failed its SHA-256 integrity check",
        ));
    }
    Ok(Some(envelope.payload))
}

fn quarantine_database_files(
    root: &Path,
    quarantine_dir: &Path,
    already_recorded: &[QuarantinedFileReceipt],
) -> Result<Vec<QuarantinedFileReceipt>> {
    fs::create_dir_all(quarantine_dir).map_err(storage_error)?;
    let mut receipts: BTreeMap<String, QuarantinedFileReceipt> = already_recorded
        .iter()
        .map(|r| (r.name.clone(), r.clone()))
        .collect();

    for name in ["omniplay.sqlite3", "omniplay.sqlite3-wal", "omniplay.sqlite3-shm"] {
        let src = root.join(name);
        let dst = quarantine_dir.join(name);
        match fs::symlink_metadata(&src) {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                // If an interrupted run already moved this file into quarantine_dir, retain its receipt.
                if !receipts.contains_key(name) {
                    if let Ok(dst_meta) = fs::symlink_metadata(&dst) {
                        if plain_entry(&dst_meta, false) {
                            let mut f = File::open(&dst).map_err(storage_error)?;
                            let (sha, len) = hash_file(&mut f, u64::MAX)?;
                            receipts.insert(
                                name.into(),
                                QuarantinedFileReceipt {
                                    name: name.into(),
                                    byte_length: len,
                                    sha256: sha,
                                },
                            );
                        }
                    }
                }
            }
            Err(err) => return Err(storage_error(err)),
            Ok(meta) => {
                if !plain_entry(&meta, false) {
                    return Err(PlatformError::new(
                        ErrorCode::StorageFailure,
                        "Refusing to quarantine non-regular database file",
                    ));
                }
                let mut f = File::open(&src).map_err(storage_error)?;
                let (sha, len) = hash_file(&mut f, u64::MAX)?;
                drop(f);
                if dst.exists() {
                    let mut df = File::open(&dst).map_err(storage_error)?;
                    let (dsha, dlen) = hash_file(&mut df, u64::MAX)?;
                    drop(df);
                    if dsha == sha && dlen == len {
                        fs::remove_file(&src).map_err(storage_error)?;
                    } else {
                        return Err(PlatformError::new(
                            ErrorCode::Conflict,
                            "Quarantine destination already contains a conflicting database artifact",
                        ));
                    }
                } else {
                    fs::rename(&src, &dst).map_err(storage_error)?;
                }
                receipts.insert(
                    name.into(),
                    QuarantinedFileReceipt {
                        name: name.into(),
                        byte_length: len,
                        sha256: sha,
                    },
                );
            }
        }
    }

    let ordered: Vec<QuarantinedFileReceipt> = ["omniplay.sqlite3", "omniplay.sqlite3-wal", "omniplay.sqlite3-shm"]
        .into_iter()
        .filter_map(|name| receipts.remove(name))
        .collect();
    Ok(ordered)
}

fn cleanup_staging_db_files(root: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let _ = fs::remove_file(root.join(format!("{STAGING_DB_FILE}{suffix}")));
    }
}

fn execute_recovery_plan(
    root: &Path,
    mut journal: RecoveryJournalPayload,
    salvaged: SalvagedData,
    diagnosis_snapshot: Option<&DatabaseHealthReport>,
) -> Result<DatabaseRecoveryOutcome> {
    let quarantine_root = root.join("corrupt-db-quarantine");
    let quarantine_dir = quarantine_root.join(&journal.quarantine_id);
    fs::create_dir_all(&quarantine_dir).map_err(storage_error)?;

    // If journal is already in Publishing phase and omniplay.sqlite3 has already been renamed into place
    // and passes diagnosis, just clean up staging artifacts + journal and return the outcome.
    if journal.phase == RecoveryJournalPhase::Publishing
        && !root.join(STAGING_DB_FILE).exists()
        && matches!(diagnose(root), Ok(ref rep) if rep.status == DatabaseHealthStatus::Healthy)
    {
        cleanup_staging_db_files(root);
        let _ = fs::remove_file(root.join(RECOVERY_JOURNAL_FILE));
        sync_directory(root)?;
        return Ok(DatabaseRecoveryOutcome {
            schema_version: 1,
            previous_status: journal.previous_status,
            quarantine_id: journal.quarantine_id,
            quarantined_files: journal.quarantined_files,
            preserved_device_id: salvaged.device_id.is_some(),
            salvaged_settings: salvaged.settings.is_some(),
            salvaged_games: salvaged.games.len(),
            reconstructed_games: 0,
            salvaged_firmware: salvaged.firmware.len(),
            salvaged_saves: salvaged.saves.len(),
            salvaged_suppression_markers: salvaged.removed_saves.len(),
        });
    }

    // 1. Finish moving damaged database files into quarantine_dir and write quarantine-receipt.json
    let quarantined_files =
        quarantine_database_files(root, &quarantine_dir, &journal.quarantined_files)?;
    let receipt_path = quarantine_dir.join("quarantine-receipt.json");
    if !receipt_path.exists() {
        let receipt_json = serde_json::to_vec_pretty(&serde_json::json!({
            "schema_version": 1,
            "quarantine_id": journal.quarantine_id,
            "quarantined_at_ms": now_ms(),
            "previous_status": journal.previous_status,
            "diagnosis": diagnosis_snapshot,
            "files": quarantined_files,
        }))
        .map_err(storage_error)?;
        write_atomic_json(&quarantine_dir, "quarantine-receipt.json", &receipt_json)?;
    }
    sync_directory(&quarantine_dir)?;
    sync_directory(&quarantine_root)?;

    journal.quarantined_files = quarantined_files.clone();
    journal.phase = RecoveryJournalPhase::Rebuilding;
    write_recovery_journal(root, &journal)?;

    // 2. Build replacement schema-15 database in `omniplay.recovering.sqlite3` so `omniplay.sqlite3`
    // never exists in a half-populated state if a crash occurs mid-rebuild.
    cleanup_staging_db_files(root);
    let mut database = Database::open_staging(root, STAGING_DB_FILE)?;

    let mut preserved_device_id = false;
    if let Some(dev_id) = salvaged.device_id {
        database
            .connection
            .execute(
                "INSERT INTO preferences(key,value) VALUES('device_id',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                [dev_id.to_string()],
            )
            .map_err(storage_error)?;
        database.device_id = dev_id;
        preserved_device_id = true;
    }

    let mut salvaged_settings = false;
    if let Some(settings) = &salvaged.settings {
        if database.set_settings(settings).is_ok() {
            salvaged_settings = true;
        }
    }
    if let Some(quota) = salvaged.quota_bytes {
        let _ = crate::storage_accounting::set_physical_quota_bytes(&database, quota);
    }

    let mut salvaged_suppression_markers = 0usize;
    for (sid, ghash, removed_at) in &salvaged.removed_saves {
        if database
            .connection
            .execute(
                "INSERT OR IGNORE INTO locally_removed_saves(save_id, game_content_hash, removed_at_ms) VALUES(?1,?2,?3)",
                params![sid.to_string(), ghash.0, removed_at],
            )
            .is_ok()
        {
            salvaged_suppression_markers += 1;
        }
    }

    let mut salvaged_firmware = 0usize;
    for (hash, byte_len, created) in &salvaged.firmware {
        if database
            .connection
            .execute(
                "INSERT OR IGNORE INTO firmware_assets(content_hash, byte_length, created_at_ms) VALUES(?1,?2,?3)",
                params![hash.0, byte_len, created],
            )
            .is_ok()
        {
            salvaged_firmware += 1;
        }
    }
    for (sys, role, hash) in &salvaged.firmware_selections {
        let _ = crate::firmware::FirmwareVault::new(&mut database).select(*sys, role, hash);
    }

    let mut salvaged_games = 0usize;
    for sg in salvaged.games {
        if let Ok(inserted) = crate::library::Library::new(&database).insert_manifest(sg.manifest) {
            salvaged_games += 1;
            let _ = database.connection.execute(
                "UPDATE games SET favorite=?1, playtime_ms=?2, last_played_at_ms=?3 WHERE id=?4",
                params![sg.favorite, sg.playtime_ms, sg.last_played_at_ms, inserted.id.to_string()],
            );
            if let Some(cfg) = &sg.session_config {
                let _ = crate::assistance::set_session_preferences(&database, inserted.id, cfg);
            }
            if let Some(inp) = sg.input_profile {
                let _ = crate::input::store(&database, inserted.id, inp);
            }
            if let Some(tch) = sg.touch_profile {
                let _ = crate::input::store_touch(&database, inserted.id, tch);
            }
            for (rt, opts) in sg.core_options {
                let _ = crate::core_options::store(&database, inserted.id, &rt, opts);
            }
        }
    }

    // 3. Reconstruct any remaining unambiguous ROMs and multi-file disc sets from `assets/`
    let reconstructed = library_reconstruct::reconstruct_unambiguous_all(&database).unwrap_or_default();
    let reconstructed_games = reconstructed.len();

    // 4. Restore salvaged save generations against salvaged or reconstructed games
    let payload_index = index_disk_save_payloads(root);
    let mut salvaged_saves = 0usize;
    let mut restored_for_promotion = Vec::new();
    for ssave in salvaged.saves {
        let Some(game) = crate::library::Library::new(&database)
            .by_hash(&ssave.metadata.game_content_hash)
            .ok()
            .flatten()
        else {
            continue;
        };
        if salvaged
            .removed_saves
            .iter()
            .any(|(rid, _, _)| rid == &ssave.metadata.id)
        {
            continue;
        }
        let Some(payload_path) =
            payload_index.get(&(ssave.metadata.payload_hash.0.clone(), ssave.metadata.payload_bytes))
        else {
            continue;
        };
        let Ok(mut pf) = File::open(payload_path) else {
            continue;
        };
        let Ok(payload) = bounded_read(&mut pf, ssave.metadata.payload_bytes) else {
            continue;
        };
        if payload.len() as u64 != ssave.metadata.payload_bytes
            || digest(&payload) != ssave.metadata.payload_hash
        {
            continue;
        }
        let was_head = salvaged.head_save_ids.contains(&ssave.metadata.id);
        let new_id = Uuid::new_v4();
        let reference = serde_json::json!({
            "salvaged_from_quarantine": journal.quarantine_id,
            "original_save_id": ssave.metadata.id,
        })
        .to_string();
        if let Ok(committed) = SaveStore::new(&mut database).restore_backup(
            ssave.metadata,
            &payload,
            game.id,
            new_id,
            &reference,
        ) {
            salvaged_saves += 1;
            if was_head {
                restored_for_promotion.push((committed.id, game.id, committed.kind, committed.slot));
            }
        }
    }

    for (save_id, game_id, kind, slot) in restored_for_promotion {
        let kind_str = crate::saves::kind_name(kind);
        let existing_head: Option<String> = database
            .connection
            .query_row(
                "SELECT save_id FROM save_heads WHERE game_id=?1 AND kind=?2 AND slot=?3",
                params![game_id.to_string(), kind_str, slot],
                |r| r.get(0),
            )
            .ok();
        let existing_uuid = existing_head.and_then(|s| Uuid::parse_str(&s).ok());
        let _ = SaveStore::new(&mut database).promote(save_id, existing_uuid);
    }

    let _ = database.record_audit("database.recovered_corrupt", None);
    // Checkpoint WAL and switch staging DB out of WAL before closing so `omniplay.recovering.sqlite3`
    // is a single self-contained, flushed SQLite file ready for atomic rename.
    database
        .connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE;")
        .map_err(storage_error)?;
    drop(database);

    // 5. Publish phase: atomically promote `omniplay.recovering.sqlite3` -> `omniplay.sqlite3`
    journal.phase = RecoveryJournalPhase::Publishing;
    write_recovery_journal(root, &journal)?;

    fs::rename(root.join(STAGING_DB_FILE), root.join("omniplay.sqlite3")).map_err(storage_error)?;
    cleanup_staging_db_files(root);
    let _ = fs::remove_file(root.join(RECOVERY_JOURNAL_FILE));
    sync_directory(root)?;

    Ok(DatabaseRecoveryOutcome {
        schema_version: 1,
        previous_status: journal.previous_status,
        quarantine_id: journal.quarantine_id,
        quarantined_files,
        preserved_device_id,
        salvaged_settings,
        salvaged_games,
        reconstructed_games,
        salvaged_firmware,
        salvaged_saves,
        salvaged_suppression_markers,
    })
}

fn reconcile_incomplete_recovery_locked(root: &Path) -> Result<Option<DatabaseRecoveryOutcome>> {
    let journal = match read_recovery_journal(root) {
        Ok(Some(j)) => j,
        Ok(None) => return Ok(None),
        Err(err) => {
            // If `omniplay.sqlite3` is still healthy on disk, quarantine had not started yet;
            // discard the unreadable journal. Otherwise fail closed so we never overwrite a
            // quarantined profile with a blank database.
            if matches!(diagnose(root), Ok(rep) if rep.status == DatabaseHealthStatus::Healthy) {
                let _ = fs::remove_file(root.join(RECOVERY_JOURNAL_FILE));
                return Ok(None);
            }
            return Err(err);
        }
    };

    let snapshot_path = root
        .join("corrupt-db-quarantine")
        .join(&journal.quarantine_id)
        .join(SALVAGED_SNAPSHOT_FILE);
    let mut snap_file = File::open(&snapshot_path).map_err(|_| {
        PlatformError::new(
            ErrorCode::CorruptData,
            "Incomplete database recovery is missing its salvaged snapshot",
        )
    })?;
    let snap_bytes = bounded_read(&mut snap_file, 64 * 1024 * 1024)?;
    if digest(&snap_bytes) != journal.salvaged_sha256 {
        return Err(PlatformError::new(
            ErrorCode::CorruptData,
            "Incomplete database recovery salvaged snapshot failed checksum verification",
        ));
    }
    let salvaged: SalvagedData = serde_json::from_slice(&snap_bytes).map_err(|_| {
        PlatformError::new(
            ErrorCode::CorruptData,
            "Incomplete database recovery salvaged snapshot is invalid",
        )
    })?;

    let outcome = execute_recovery_plan(root, journal, salvaged, None)?;
    Ok(Some(outcome))
}

/// Reconciles any interrupted `database-recovery-journal.json` on startup before `Database::open`
/// touches `omniplay.sqlite3`, preventing blank-DB creation after a mid-recovery crash.
pub fn reconcile_incomplete_recovery(root: &Path) -> Result<Option<DatabaseRecoveryOutcome>> {
    if !root.join(RECOVERY_JOURNAL_FILE).exists() {
        return Ok(None);
    }
    let _lease = acquire_recovery_lease(root)?;
    reconcile_incomplete_recovery_locked(root)
}

/// Quarantines a corrupted or missing SQLite database into `corrupt-db-quarantine/<id>/`,
/// journals salvaged state with SHA-256 verification, builds a clean schema-15 database in
/// `omniplay.recovering.sqlite3`, and atomically promotes it to `omniplay.sqlite3`.
pub fn recover(root: &Path) -> Result<DatabaseRecoveryOutcome> {
    let _lease = acquire_recovery_lease(root)?;
    if let Some(resumed) = reconcile_incomplete_recovery_locked(root)? {
        return Ok(resumed);
    }

    let health = diagnose(root)?;
    match health.status {
        DatabaseHealthStatus::Healthy => {
            return Err(PlatformError::new(
                ErrorCode::InvalidState,
                "Profile database is healthy and does not require corruption recovery",
            ));
        }
        DatabaseHealthStatus::NewerSchema => {
            return Err(PlatformError::new(
                ErrorCode::InvalidState,
                "Database belongs to a newer application schema and will not be quarantined",
            ));
        }
        DatabaseHealthStatus::Missing
        | DatabaseHealthStatus::HeaderCorrupt
        | DatabaseHealthStatus::Unopenable
        | DatabaseHealthStatus::IntegrityFailed => {}
    }

    // 1. Best-effort read-only salvage BEFORE moving any database files
    let salvaged = salvage_from_damaged(root);
    let salvaged_bytes = serde_json::to_vec_pretty(&salvaged).map_err(storage_error)?;
    let salvaged_sha256 = digest(&salvaged_bytes);

    let quarantine_id = format!("{}-{}", now_ms(), Uuid::new_v4());
    let quarantine_dir = root.join("corrupt-db-quarantine").join(&quarantine_id);
    write_atomic_json(&quarantine_dir, SALVAGED_SNAPSHOT_FILE, &salvaged_bytes)?;

    // 2. Persist checksummed recovery journal BEFORE moving `omniplay.sqlite3`
    let journal = RecoveryJournalPayload {
        schema_version: 1,
        quarantine_id,
        started_at_ms: now_ms(),
        previous_status: health.status,
        phase: RecoveryJournalPhase::Quarantining,
        salvaged_sha256,
        quarantined_files: Vec::new(),
    };
    write_recovery_journal(root, &journal)?;

    // 3. Execute quarantine -> staging rebuild -> atomic publish -> journal clearance
    execute_recovery_plan(root, journal, salvaged, Some(&health))
}
