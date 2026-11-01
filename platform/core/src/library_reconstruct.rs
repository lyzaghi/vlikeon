//! Reconstruction of lost game import metadata and multi-file disc structures
//! from content-addressed blobs retained in `assets/`.
//!
//! All candidate blobs are verified by SHA-256 before publication. Ambiguous
//! multi-track disc bindings require explicit `track_bindings` rather than
//! guessing track order unless a surviving save's `game_content_hash` proves
//! the exact `GameMedia::identity_hash()`.
use crate::{
    asset_storage::AssetLease,
    library::Library,
    now_ms,
    util::{hash_file, storage_error},
    Database,
};
use omniplay_contracts::{
    valid_media_name, ContentFormat, ContentHash, ErrorCode, GameManifest, GameMedia, GameSystem,
    MediaAsset, PlatformError, Result, MAX_ROM_BYTES,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom},
    path::Path,
    time::{Duration, Instant},
};
use uuid::Uuid;

const MAX_TEXT_SCAN_BYTES: u64 = 512 * 1024;
const INVENTORY_BOUND: usize = 20_000;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UnassignedTrackBlob {
    pub content_hash: ContentHash,
    pub byte_length: u64,
    pub detected_kind: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReconstructionCandidate {
    pub candidate_id: String,
    pub revision: ContentHash,
    pub format: ContentFormat,
    pub suggested_system: Option<GameSystem>,
    pub suggested_title: String,
    pub entry_hash: ContentHash,
    pub total_bytes_known: u64,
    pub disc_count: u16,
    pub required_tracks: Vec<String>,
    pub resolved_bindings: BTreeMap<String, ContentHash>,
    pub needs_track_binding: bool,
    pub matched_save_content_hash: Option<ContentHash>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReconstructionPreview {
    pub schema_version: u32,
    pub candidates: Vec<ReconstructionCandidate>,
    pub available_track_blobs: Vec<UnassignedTrackBlob>,
    pub already_indexed_blobs: usize,
    pub retained_unknown_entries: usize,
}

#[derive(Clone, Debug)]
struct ScannedBlob {
    hash: ContentHash,
    bytes: u64,
    kind: BlobKind,
}

#[derive(Clone, Debug)]
enum BlobKind {
    M3u { disc_names: Vec<String> },
    Cue { track_names: Vec<String>, cue_title: Option<String> },
    SingleGame { format: ContentFormat, system: Option<GameSystem>, internal_title: Option<String> },
    WaveTrack,
    BinaryOrRaw { sector_aligned: bool },
}

fn plain_file(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    !meta.file_type().is_symlink() && meta.is_file()
}

fn open_nofollow(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000).share_mode(1);
    }
    let file = options.open(path).map_err(storage_error)?;
    let meta = file.metadata().map_err(storage_error)?;
    if !plain_file(&meta) {
        return Err(PlatformError::new(ErrorCode::CorruptData, "Asset entry is not a regular file"));
    }
    Ok(file)
}

fn sanitize_ascii_title(bytes: &[u8]) -> Option<String> {
    let mut out = String::new();
    for &b in bytes {
        if b == 0 {
            break;
        }
        if (0x20..=0x7E).contains(&b) {
            out.push(b as char);
        }
    }
    let trimmed = out.trim();
    if trimmed.len() >= 2 {
        Some(trimmed.chars().take(120).collect())
    } else {
        None
    }
}

fn parse_normalized_m3u(text: &str) -> Option<Vec<String>> {
    let mut discs = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let expected_prefix = format!("disc-{:02}.", discs.len() + 1);
        if !line.starts_with(&expected_prefix) || !valid_media_name(line) {
            return None;
        }
        let ext = line.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
        if !matches!(ext, "cue" | "chd" | "iso" | "pbp") {
            return None;
        }
        discs.push(line.to_owned());
        if discs.len() > 8 {
            return None;
        }
    }
    if discs.is_empty() { None } else { Some(discs) }
}

fn parse_normalized_cue(text: &str) -> Option<(Vec<String>, Option<String>)> {
    let mut tracks = Vec::new();
    let mut seen = BTreeSet::new();
    let mut has_track = false;
    let mut has_index = false;
    let mut cue_title = None;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let split = line.find(char::is_whitespace).unwrap_or(line.len());
        let cmd = &line[..split];
        let args = line[split..].trim();
        match cmd {
            "FILE" => {
                let rest = args.strip_prefix('"')?;
                let end = rest.find('"')?;
                let name = &rest[..end];
                let kind = rest[end + 1..].trim();
                if !valid_media_name(name) || !name.starts_with("track-") || !matches!(kind, "BINARY" | "WAVE") {
                    return None;
                }
                if seen.insert(name.to_owned()) {
                    tracks.push(name.to_owned());
                }
            }
            "TRACK" => {
                if tracks.is_empty() {
                    return None;
                }
                has_track = true;
            }
            "INDEX" => {
                if !has_track {
                    return None;
                }
                has_index = true;
            }
            "TITLE" => {
                if cue_title.is_none() && !has_track {
                    let cleaned = args.trim_matches('"').trim();
                    if !cleaned.is_empty() && cleaned.len() <= 120 && !cleaned.chars().any(char::is_control) {
                        cue_title = Some(cleaned.to_owned());
                    }
                }
            }
            "PREGAP" | "POSTGAP" | "FLAGS" | "REM" | "PERFORMER" | "SONGWRITER" | "CATALOG" | "ISRC" => {}
            _ => return None,
        }
    }
    if !tracks.is_empty() && has_track && has_index {
        Some((tracks, cue_title))
    } else {
        None
    }
}

fn classify_blob(file: &mut File, byte_length: u64) -> Result<BlobKind> {
    file.seek(SeekFrom::Start(0)).map_err(storage_error)?;
    let mut header = [0u8; 512];
    let n = file.read(&mut header).map_err(storage_error)?;

    // 1. Check small UTF-8 normalized M3U / CUE metadata blobs
    if byte_length > 0 && byte_length <= MAX_TEXT_SCAN_BYTES {
        file.seek(SeekFrom::Start(0)).map_err(storage_error)?;
        let mut raw = Vec::with_capacity(byte_length as usize);
        file.take(MAX_TEXT_SCAN_BYTES).read_to_end(&mut raw).map_err(storage_error)?;
        if let Ok(text) = std::str::from_utf8(&raw) {
            if let Some(disc_names) = parse_normalized_m3u(text) {
                return Ok(BlobKind::M3u { disc_names });
            }
            if let Some((track_names, cue_title)) = parse_normalized_cue(text) {
                return Ok(BlobKind::Cue { track_names, cue_title });
            }
        }
    }

    // 2. Container / disc magic headers
    if n >= 8 && &header[..8] == b"MComprHD" {
        return Ok(BlobKind::SingleGame {
            format: ContentFormat::Chd,
            system: Some(GameSystem::Ps1),
            internal_title: None,
        });
    }
    if n >= 4 && &header[..4] == b"\0PBP" {
        return Ok(BlobKind::SingleGame {
            format: ContentFormat::Pbp,
            system: Some(GameSystem::Psp),
            internal_title: None,
        });
    }
    if n >= 4 && (&header[..4] == b"CISO" || &header[..4] == b"ZISO") {
        return Ok(BlobKind::SingleGame {
            format: ContentFormat::Cso,
            system: Some(GameSystem::Psp),
            internal_title: None,
        });
    }
    if n >= 12 && &header[..4] == b"RIFF" && &header[8..12] == b"WAVE" {
        return Ok(BlobKind::WaveTrack);
    }
    if byte_length >= 0x8060 {
        file.seek(SeekFrom::Start(0x8000)).map_err(storage_error)?;
        let mut pvd = [0u8; 96];
        if file.read_exact(&mut pvd).is_ok() && &pvd[1..6] == b"CD001" {
            let sys_id = std::str::from_utf8(&pvd[8..40]).unwrap_or("").trim();
            let vol_id = sanitize_ascii_title(&pvd[40..72]);
            let system = if sys_id.contains("PSP GAME") {
                Some(GameSystem::Psp)
            } else if sys_id.contains("PLAYSTATION") {
                Some(GameSystem::Ps1)
            } else {
                Some(GameSystem::Ps1)
            };
            return Ok(BlobKind::SingleGame {
                format: ContentFormat::Iso,
                system,
                internal_title: vol_id,
            });
        }
    }

    // 3. Cartridge / ROM headers
    if n >= 64 && header[..4] == [0x80, 0x37, 0x12, 0x40] {
        return Ok(BlobKind::SingleGame {
            format: ContentFormat::N64Z64,
            system: Some(GameSystem::N64),
            internal_title: sanitize_ascii_title(&header[0x20..0x34]),
        });
    }
    if n >= 4 && header[..4] == [0x37, 0x80, 0x40, 0x12] {
        return Ok(BlobKind::SingleGame {
            format: ContentFormat::N64V64,
            system: Some(GameSystem::N64),
            internal_title: None,
        });
    }
    if n >= 4 && header[..4] == [0x40, 0x12, 0x37, 0x80] {
        return Ok(BlobKind::SingleGame {
            format: ContentFormat::N64N64,
            system: Some(GameSystem::N64),
            internal_title: None,
        });
    }
    if n >= 16 && &header[..4] == b"NES\x1a" {
        return Ok(BlobKind::SingleGame {
            format: ContentFormat::RawRom,
            system: Some(GameSystem::Nes),
            internal_title: None,
        });
    }
    if n >= 0xB4 && header[0x04..0x08] == [0x24, 0xFF, 0xAE, 0x51] && header[0xB2] == 0x96 {
        return Ok(BlobKind::SingleGame {
            format: ContentFormat::RawRom,
            system: Some(GameSystem::Gba),
            internal_title: sanitize_ascii_title(&header[0xA0..0xAC]),
        });
    }
    if n >= 0x100 && header[0xC0..0xC4] == [0x24, 0xFF, 0xAE, 0x51] {
        return Ok(BlobKind::SingleGame {
            format: ContentFormat::RawRom,
            system: Some(GameSystem::Nds),
            internal_title: sanitize_ascii_title(&header[0x00..0x0C]),
        });
    }
    if n >= 0x150 && header[0x104..0x108] == [0xCE, 0xED, 0x66, 0x66] {
        let cgb = header[0x143];
        let system = if cgb == 0x80 || cgb == 0xC0 { GameSystem::Gbc } else { GameSystem::Gb };
        return Ok(BlobKind::SingleGame {
            format: ContentFormat::RawRom,
            system: Some(system),
            internal_title: sanitize_ascii_title(&header[0x134..0x143]),
        });
    }
    if n >= 0x180 && (header[0x100..0x110].starts_with(b"SEGA MEGA DRIVE") || header[0x100..0x110].starts_with(b"SEGA GENESIS")) {
        return Ok(BlobKind::SingleGame {
            format: ContentFormat::RawRom,
            system: Some(GameSystem::MegaDrive),
            internal_title: sanitize_ascii_title(&header[0x150..0x180]),
        });
    }
    for snes_base in [0x7FC0u64, 0xFFC0u64] {
        if byte_length >= snes_base + 0x20 {
            file.seek(SeekFrom::Start(snes_base)).map_err(storage_error)?;
            let mut snes_hdr = [0u8; 0x20];
            if file.read_exact(&mut snes_hdr).is_ok() {
                let comp = u16::from_le_bytes([snes_hdr[0x1C], snes_hdr[0x1D]]);
                let csum = u16::from_le_bytes([snes_hdr[0x1E], snes_hdr[0x1F]]);
                if (comp ^ csum) == 0xFFFF && comp != 0 && csum != 0 {
                    return Ok(BlobKind::SingleGame {
                        format: ContentFormat::RawRom,
                        system: Some(GameSystem::Snes),
                        internal_title: sanitize_ascii_title(&snes_hdr[..21]),
                    });
                }
            }
        }
    }

    let sector_aligned = byte_length > 0 && (byte_length % 2352 == 0 || byte_length % 2336 == 0 || byte_length % 2048 == 0);
    Ok(BlobKind::BinaryOrRaw { sector_aligned })
}

fn known_save_game_hashes(database: &Database) -> Result<BTreeSet<String>> {
    let mut hashes = BTreeSet::new();
    let mut stmt = database
        .connection
        .prepare("SELECT metadata_json FROM saves LIMIT 20000")
        .map_err(storage_error)?;
    let mut rows = stmt.query([]).map_err(storage_error)?;
    while let Some(row) = rows.next().map_err(storage_error)? {
        let raw: String = row.get(0).map_err(storage_error)?;
        if let Ok(meta) = serde_json::from_str::<omniplay_contracts::SaveMetadata>(&raw) {
            hashes.insert(meta.game_content_hash.0);
        }
    }
    drop(rows);
    drop(stmt);

    let mut stmt = database
        .connection
        .prepare("SELECT game_content_hash FROM locally_removed_saves LIMIT 20000")
        .map_err(storage_error)?;
    let mut rows = stmt.query([]).map_err(storage_error)?;
    while let Some(row) = rows.next().map_err(storage_error)? {
        let h: String = row.get(0).map_err(storage_error)?;
        hashes.insert(h);
    }
    Ok(hashes)
}

fn indexed_asset_hashes(database: &Database) -> Result<BTreeSet<String>> {
    let mut indexed = BTreeSet::new();
    let mut stmt = database
        .connection
        .prepare("SELECT content_hash, manifest_json FROM games LIMIT 20000")
        .map_err(storage_error)?;
    let mut rows = stmt.query([]).map_err(storage_error)?;
    while let Some(row) = rows.next().map_err(storage_error)? {
        let hash: String = row.get(0).map_err(storage_error)?;
        let raw: String = row.get(1).map_err(storage_error)?;
        indexed.insert(hash);
        if let Ok(manifest) = serde_json::from_str::<GameManifest>(&raw) {
            if let Some(media) = manifest.media {
                for asset in media.assets {
                    indexed.insert(asset.content_hash.0);
                }
            }
        }
    }
    Ok(indexed)
}

fn compute_candidate_revision(candidate: &ReconstructionCandidate) -> ContentHash {
    let mut hasher = Sha256::new();
    hasher.update(b"Vlikeon library reconstruct candidate v1\0");
    hasher.update(candidate.candidate_id.as_bytes());
    hasher.update(candidate.entry_hash.0.as_bytes());
    hasher.update(candidate.total_bytes_known.to_le_bytes());
    hasher.update([u8::from(candidate.needs_track_binding)]);
    for t in &candidate.required_tracks {
        hasher.update((t.len() as u32).to_le_bytes());
        hasher.update(t.as_bytes());
    }
    for (k, v) in &candidate.resolved_bindings {
        hasher.update((k.len() as u32).to_le_bytes());
        hasher.update(k.as_bytes());
        hasher.update(v.0.as_bytes());
    }
    ContentHash(format!("{:x}", hasher.finalize()))
}

fn scan_unindexed_blobs(
    database: &Database,
    until: Instant,
) -> Result<(BTreeMap<String, ScannedBlob>, usize, usize)> {
    let indexed = indexed_asset_hashes(database)?;
    let assets_dir = database.root.join("assets");
    if !assets_dir.exists() {
        return Ok((BTreeMap::new(), 0, 0));
    }
    let dir_meta = fs::symlink_metadata(&assets_dir).map_err(storage_error)?;
    if dir_meta.file_type().is_symlink() || !dir_meta.is_dir() {
        return Err(PlatformError::new(ErrorCode::StorageFailure, "Assets directory is invalid"));
    }

    let mut blobs = BTreeMap::new();
    let mut already_indexed = 0usize;
    let mut unknown = 0usize;
    for (idx, entry) in fs::read_dir(&assets_dir).map_err(storage_error)?.enumerate() {
        if idx >= INVENTORY_BOUND || Instant::now() >= until {
            return Err(PlatformError::new(
                ErrorCode::ResourceLimit,
                "Asset reconstruction scan exceeded bounded budget",
            ));
        }
        let entry = entry.map_err(storage_error)?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            unknown += 1;
            continue;
        };
        let Ok(expected_hash) = ContentHash::parse(name.clone()) else {
            unknown += 1;
            continue;
        };
        if indexed.contains(&name) {
            already_indexed += 1;
            continue;
        }
        let meta = fs::symlink_metadata(entry.path()).map_err(storage_error)?;
        if !plain_file(&meta) || meta.len() == 0 || meta.len() > MAX_ROM_BYTES {
            unknown += 1;
            continue;
        }
        let mut file = open_nofollow(&entry.path())?;
        let (actual_hash, size) = hash_file(&mut file, MAX_ROM_BYTES)?;
        if actual_hash != expected_hash || size != meta.len() {
            unknown += 1;
            continue;
        }
        let kind = classify_blob(&mut file, size)?;
        blobs.insert(
            name,
            ScannedBlob {
                hash: expected_hash,
                bytes: size,
                kind,
            },
        );
    }
    Ok((blobs, already_indexed, unknown))
}

fn try_build_media_from_bindings(
    format: ContentFormat,
    system: GameSystem,
    entry_name: String,
    entry_blob: &ScannedBlob,
    disc_entries: Vec<String>,
    bindings: &BTreeMap<String, ContentHash>,
    blobs: &BTreeMap<String, ScannedBlob>,
) -> Result<(GameMedia, u64, ContentHash)> {
    let mut assets = Vec::with_capacity(1 + bindings.len());
    let mut total = entry_blob.bytes;
    assets.push(MediaAsset {
        name: entry_name.clone(),
        content_hash: entry_blob.hash.clone(),
        byte_length: entry_blob.bytes,
    });
    for (role_name, hash) in bindings {
        if role_name == &entry_name {
            continue;
        }
        let blob = blobs.get(&hash.0).ok_or_else(|| {
            PlatformError::new(ErrorCode::NotFound, "Referenced track or disc blob is missing from assets")
        })?;
        total = total
            .checked_add(blob.bytes)
            .filter(|v| *v <= MAX_ROM_BYTES)
            .ok_or_else(|| PlatformError::new(ErrorCode::ResourceLimit, "Reconstructed disc set exceeds size bound"))?;
        assets.push(MediaAsset {
            name: role_name.clone(),
            content_hash: blob.hash.clone(),
            byte_length: blob.bytes,
        });
    }
    assets.sort_by(|a, b| a.name.cmp(&b.name));
    let media = GameMedia {
        schema_version: 1,
        format,
        entry_name,
        assets,
        disc_entries,
    };
    media.validate(system, total)?;
    let identity = media.identity_hash()?;
    Ok((media, total, identity))
}

fn build_preview_unlocked(database: &Database) -> Result<ReconstructionPreview> {
    let until = Instant::now() + Duration::from_secs(30);
    let save_hashes = known_save_game_hashes(database)?;
    let (blobs, already_indexed, unknown) = scan_unindexed_blobs(database, until)?;

    let cue_blobs: Vec<&ScannedBlob> = blobs
        .values()
        .filter(|b| matches!(b.kind, BlobKind::Cue { .. }))
        .collect();
    let bin_blobs: Vec<&ScannedBlob> = blobs
        .values()
        .filter(|b| matches!(b.kind, BlobKind::BinaryOrRaw { .. }))
        .collect();
    let wav_blobs: Vec<&ScannedBlob> = blobs
        .values()
        .filter(|b| matches!(b.kind, BlobKind::WaveTrack))
        .collect();
    let single_disc_images: Vec<&ScannedBlob> = blobs
        .values()
        .filter(|b| {
            matches!(
                b.kind,
                BlobKind::SingleGame {
                    format: ContentFormat::Chd | ContentFormat::Iso | ContentFormat::Pbp,
                    ..
                }
            )
        })
        .collect();

    let mut claimed_by_m3u = BTreeSet::new();
    let mut candidates = Vec::new();

    // 1. Reconstruct M3U multi-disc playlists first
    for blob in blobs.values() {
        let BlobKind::M3u { disc_names } = &blob.kind else { continue };
        let mut required_tracks = Vec::new();
        let mut resolved = BTreeMap::new();
        let mut total_known = blob.bytes;
        let mut ambiguous = false;

        let all_cue = disc_names.iter().all(|d| d.ends_with(".cue"));
        if all_cue && cue_blobs.len() == disc_names.len() {
            // Try matching cue permutations (up to 8! bounded at <= 24 permutations for <= 4 discs, or exact save hash)
            let mut all_track_names = BTreeSet::new();
            for cue in &cue_blobs {
                if let BlobKind::Cue { track_names, .. } = &cue.kind {
                    for tn in track_names {
                        all_track_names.insert(tn.clone());
                    }
                }
            }
            for d in disc_names {
                required_tracks.push(d.clone());
            }
            for tn in &all_track_names {
                required_tracks.push(tn.clone());
            }

            if disc_names.len() == 1 && all_track_names.len() == 1 && bin_blobs.len() == 1 && all_track_names.iter().all(|t| t.ends_with(".bin")) {
                resolved.insert(disc_names[0].clone(), cue_blobs[0].hash.clone());
                let track_name = all_track_names.iter().next().unwrap().clone();
                resolved.insert(track_name, bin_blobs[0].hash.clone());
                total_known = total_known.saturating_add(cue_blobs[0].bytes).saturating_add(bin_blobs[0].bytes);
            } else {
                // Attempt exact match against surviving `save_hashes` for small multi-disc sets (<= 4 discs & <= 4 bin tracks)
                let mut matched = false;
                if !save_hashes.is_empty() && cue_blobs.len() <= 4 && bin_blobs.len() == all_track_names.len() && bin_blobs.len() <= 4 && wav_blobs.is_empty() {
                    let track_list: Vec<String> = all_track_names.iter().cloned().collect();
                    let cue_perms = permutations(cue_blobs.len());
                    let bin_perms = permutations(bin_blobs.len());
                    'search: for cp in &cue_perms {
                        for bp in &bin_perms {
                            let mut trial = BTreeMap::new();
                            for (i, dname) in disc_names.iter().enumerate() {
                                trial.insert(dname.clone(), cue_blobs[cp[i]].hash.clone());
                            }
                            for (i, tname) in track_list.iter().enumerate() {
                                trial.insert(tname.clone(), bin_blobs[bp[i]].hash.clone());
                            }
                            if let Ok((_, tot, id_hash)) = try_build_media_from_bindings(
                                ContentFormat::M3u,
                                GameSystem::Ps1,
                                "game.m3u".into(),
                                blob,
                                disc_names.clone(),
                                &trial,
                                &blobs,
                            ) {
                                if save_hashes.contains(&id_hash.0) {
                                    resolved = trial;
                                    total_known = tot;
                                    matched = true;
                                    break 'search;
                                }
                            }
                        }
                    }
                }
                if !matched {
                    ambiguous = true;
                }
            }
        } else if !all_cue && single_disc_images.len() == disc_names.len() && disc_names.len() <= 4 {
            for d in disc_names {
                required_tracks.push(d.clone());
            }
            if disc_names.len() == 1 {
                resolved.insert(disc_names[0].clone(), single_disc_images[0].hash.clone());
                total_known = total_known.saturating_add(single_disc_images[0].bytes);
            } else {
                let mut matched = false;
                if !save_hashes.is_empty() {
                    for perm in permutations(single_disc_images.len()) {
                        let mut trial = BTreeMap::new();
                        for (i, dname) in disc_names.iter().enumerate() {
                            trial.insert(dname.clone(), single_disc_images[perm[i]].hash.clone());
                        }
                        if let Ok((_, tot, id_hash)) = try_build_media_from_bindings(
                            ContentFormat::M3u,
                            GameSystem::Ps1,
                            "game.m3u".into(),
                            blob,
                            disc_names.clone(),
                            &trial,
                            &blobs,
                        ) {
                            if save_hashes.contains(&id_hash.0) {
                                resolved = trial;
                                total_known = tot;
                                matched = true;
                                break;
                            }
                        }
                    }
                }
                if !matched {
                    ambiguous = true;
                }
            }
        } else {
            for d in disc_names {
                required_tracks.push(d.clone());
            }
            ambiguous = true;
        }

        let mut matched_save = None;
        if !ambiguous {
            if let Ok((_, _, id_hash)) = try_build_media_from_bindings(
                ContentFormat::M3u,
                GameSystem::Ps1,
                "game.m3u".into(),
                blob,
                disc_names.clone(),
                &resolved,
                &blobs,
            ) {
                for h in resolved.values() {
                    claimed_by_m3u.insert(h.0.clone());
                }
                if save_hashes.contains(&id_hash.0) {
                    matched_save = Some(id_hash);
                }
            } else {
                ambiguous = true;
            }
        }

        let mut cand = ReconstructionCandidate {
            candidate_id: format!("m3u:{}", blob.hash.0),
            revision: ContentHash("0".repeat(64)),
            format: ContentFormat::M3u,
            suggested_system: Some(GameSystem::Ps1),
            suggested_title: format!("Multi-Disc Game ({} discs)", disc_names.len()),
            entry_hash: blob.hash.clone(),
            total_bytes_known: total_known,
            disc_count: disc_names.len() as u16,
            required_tracks,
            resolved_bindings: resolved,
            needs_track_binding: ambiguous,
            matched_save_content_hash: matched_save,
        };
        cand.revision = compute_candidate_revision(&cand);
        candidates.push(cand);
    }

    // 2. Reconstruct standalone CUE sheets not already claimed by an unambiguous M3U
    let mut claimed_tracks = claimed_by_m3u.clone();
    for blob in &cue_blobs {
        if claimed_by_m3u.contains(&blob.hash.0) {
            continue;
        }
        let BlobKind::Cue { track_names, cue_title } = &blob.kind else { continue };
        let mut resolved = BTreeMap::new();
        let mut total_known = blob.bytes;
        let mut ambiguous = false;

        let needed_bins: Vec<&String> = track_names.iter().filter(|t| t.ends_with(".bin")).collect();
        let needed_wavs: Vec<&String> = track_names.iter().filter(|t| t.ends_with(".wav")).collect();
        let free_bins: Vec<&&ScannedBlob> = bin_blobs.iter().filter(|b| !claimed_tracks.contains(&b.hash.0)).collect();
        let free_wavs: Vec<&&ScannedBlob> = wav_blobs.iter().filter(|b| !claimed_tracks.contains(&b.hash.0)).collect();

        if cue_blobs.len() == 1 && needed_bins.len() <= 1 && needed_wavs.len() <= 1 && free_bins.len() == needed_bins.len() && free_wavs.len() == needed_wavs.len() {
            if let Some(b_name) = needed_bins.first() {
                resolved.insert((*b_name).clone(), free_bins[0].hash.clone());
                total_known = total_known.saturating_add(free_bins[0].bytes);
            }
            if let Some(w_name) = needed_wavs.first() {
                resolved.insert((*w_name).clone(), free_wavs[0].hash.clone());
                total_known = total_known.saturating_add(free_wavs[0].bytes);
            }
        } else if !save_hashes.is_empty() && needed_wavs.is_empty() && free_bins.len() == needed_bins.len() && needed_bins.len() <= 4 {
            let mut matched = false;
            for perm in permutations(free_bins.len()) {
                let mut trial = BTreeMap::new();
                for (i, b_name) in needed_bins.iter().enumerate() {
                    trial.insert((*b_name).clone(), free_bins[perm[i]].hash.clone());
                }
                if let Ok((_, tot, id_hash)) = try_build_media_from_bindings(
                    ContentFormat::Cue,
                    GameSystem::Ps1,
                    "game.cue".into(),
                    blob,
                    vec!["game.cue".into()],
                    &trial,
                    &blobs,
                ) {
                    if save_hashes.contains(&id_hash.0) {
                        resolved = trial;
                        total_known = tot;
                        matched = true;
                        break;
                    }
                }
            }
            if !matched {
                ambiguous = true;
            }
        } else {
            ambiguous = true;
        }

        let mut matched_save = None;
        if !ambiguous {
            if let Ok((_, _, id_hash)) = try_build_media_from_bindings(
                ContentFormat::Cue,
                GameSystem::Ps1,
                "game.cue".into(),
                blob,
                vec!["game.cue".into()],
                &resolved,
                &blobs,
            ) {
                for h in resolved.values() {
                    claimed_tracks.insert(h.0.clone());
                }
                if save_hashes.contains(&id_hash.0) {
                    matched_save = Some(id_hash);
                }
            } else {
                ambiguous = true;
            }
        }

        let title = cue_title
            .clone()
            .unwrap_or_else(|| format!("Recovered Disc ({})", &blob.hash.0[..8]));
        let mut cand = ReconstructionCandidate {
            candidate_id: format!("cue:{}", blob.hash.0),
            revision: ContentHash("0".repeat(64)),
            format: ContentFormat::Cue,
            suggested_system: Some(GameSystem::Ps1),
            suggested_title: title,
            entry_hash: blob.hash.clone(),
            total_bytes_known: total_known,
            disc_count: 1,
            required_tracks: track_names.clone(),
            resolved_bindings: resolved,
            needs_track_binding: ambiguous,
            matched_save_content_hash: matched_save,
        };
        cand.revision = compute_candidate_revision(&cand);
        candidates.push(cand);
    }

    // 3. Reconstruct standalone single-file ROM / CHD / ISO / PBP / CSO images
    for blob in blobs.values() {
        if claimed_tracks.contains(&blob.hash.0) {
            continue;
        }
        if let BlobKind::SingleGame {
            format,
            system,
            internal_title,
        } = &blob.kind
        {
            let matched_save = if save_hashes.contains(&blob.hash.0) {
                Some(blob.hash.clone())
            } else {
                None
            };
            let title = internal_title
                .clone()
                .unwrap_or_else(|| format!("Recovered Game ({})", &blob.hash.0[..8]));
            let mut cand = ReconstructionCandidate {
                candidate_id: format!("rom:{}", blob.hash.0),
                revision: ContentHash("0".repeat(64)),
                format: *format,
                suggested_system: *system,
                suggested_title: title,
                entry_hash: blob.hash.clone(),
                total_bytes_known: blob.bytes,
                disc_count: 1,
                required_tracks: Vec::new(),
                resolved_bindings: BTreeMap::new(),
                needs_track_binding: false,
                matched_save_content_hash: matched_save,
            };
            cand.revision = compute_candidate_revision(&cand);
            candidates.push(cand);
        }
    }

    let mut available_track_blobs = Vec::new();
    for blob in blobs.values() {
        let kind_label = match &blob.kind {
            BlobKind::WaveTrack => Some("wav"),
            BlobKind::BinaryOrRaw { sector_aligned: true } => Some("cd_sector_bin"),
            BlobKind::BinaryOrRaw { sector_aligned: false } => Some("raw_binary"),
            BlobKind::Cue { .. } => Some("cue"),
            BlobKind::SingleGame { format: ContentFormat::Chd, .. } => Some("chd"),
            BlobKind::SingleGame { format: ContentFormat::Iso, .. } => Some("iso"),
            BlobKind::SingleGame { format: ContentFormat::Pbp, .. } => Some("pbp"),
            _ => None,
        };
        if let Some(label) = kind_label {
            available_track_blobs.push(UnassignedTrackBlob {
                content_hash: blob.hash.clone(),
                byte_length: blob.bytes,
                detected_kind: label.into(),
            });
        }
    }

    candidates.sort_by(|a, b| a.candidate_id.cmp(&b.candidate_id));
    Ok(ReconstructionPreview {
        schema_version: 1,
        candidates,
        available_track_blobs,
        already_indexed_blobs: already_indexed,
        retained_unknown_entries: unknown,
    })
}

fn permutations(n: usize) -> Vec<Vec<usize>> {
    if n == 0 {
        return vec![Vec::new()];
    }
    let mut current: Vec<usize> = (0..n).collect();
    let mut out = Vec::new();
    fn generate(k: usize, arr: &mut [usize], out: &mut Vec<Vec<usize>>) {
        if k == 1 {
            out.push(arr.to_vec());
            return;
        }
        generate(k - 1, arr, out);
        for i in 0..k - 1 {
            if k % 2 == 0 {
                arr.swap(i, k - 1);
            } else {
                arr.swap(0, k - 1);
            }
            generate(k - 1, arr, out);
        }
    }
    generate(n, &mut current, &mut out);
    out
}

/// Inspects unreferenced blobs in `assets/` to preview reconstructable single-file games and multi-file disc sets.
pub fn preview(database: &Database) -> Result<ReconstructionPreview> {
    let _lease = AssetLease::acquire(database)?;
    build_preview_unlocked(database)
}

/// Commits a reviewed reconstruction candidate into the library without copying or modifying retained blobs.
pub fn reconstruct(
    database: &Database,
    candidate_id: &str,
    expected_revision: &ContentHash,
    title_override: Option<String>,
    system_override: Option<GameSystem>,
    track_bindings: Option<BTreeMap<String, ContentHash>>,
) -> Result<GameManifest> {
    ContentHash::parse(expected_revision.0.clone())?;
    let _lease = AssetLease::acquire(database)?;
    let until = Instant::now() + Duration::from_secs(30);
    let current_preview = build_preview_unlocked(database)?;
    let cand = current_preview
        .candidates
        .iter()
        .find(|c| c.candidate_id == candidate_id)
        .ok_or_else(|| {
            PlatformError::new(
                ErrorCode::NotFound,
                "Reconstruction candidate is no longer available; refresh the preview",
            )
        })?;
    if &cand.revision != expected_revision {
        return Err(PlatformError::new(
            ErrorCode::Conflict,
            "Retained assets changed since the reconstruction preview; inspect again",
        ));
    }

    let (blobs, _, _) = scan_unindexed_blobs(database, until)?;
    let entry_blob = blobs.get(&cand.entry_hash.0).ok_or_else(|| {
        PlatformError::new(ErrorCode::NotFound, "Entry asset blob is missing from assets")
    })?;

    let system = system_override.or(cand.suggested_system).ok_or_else(|| {
        PlatformError::new(
            ErrorCode::InvalidRequest,
            "Select the target game system to reconstruct this content",
        )
    })?;
    let title = title_override
        .unwrap_or_else(|| cand.suggested_title.clone())
        .trim()
        .to_owned();
    if title.is_empty() || title.len() > 512 {
        return Err(PlatformError::new(
            ErrorCode::InvalidRequest,
            "Game title must be between 1 and 512 UTF-8 bytes",
        ));
    }

    let bindings = match track_bindings {
        Some(custom) => custom,
        None => {
            if cand.needs_track_binding {
                return Err(PlatformError::new(
                    ErrorCode::InvalidRequest,
                    "Explicit track_bindings are required when multiple candidate track blobs exist",
                ));
            }
            cand.resolved_bindings.clone()
        }
    };
    for req in &cand.required_tracks {
        if !bindings.contains_key(req) {
            return Err(PlatformError::new(
                ErrorCode::InvalidRequest,
                "Missing required track or disc binding for multi-file set",
            ));
        }
    }

    let extension = cand.format.extension(system)?;
    let entry_name = format!("game.{extension}");
    let disc_entries = match &entry_blob.kind {
        BlobKind::M3u { disc_names } => {
            // Also verify any nested `.cue` disc's `FILE "track-XXXX.*"` references are present in `bindings`
            for dname in disc_names {
                if dname.ends_with(".cue") {
                    let cue_hash = bindings.get(dname).ok_or_else(|| {
                        PlatformError::new(ErrorCode::InvalidRequest, "Missing cue binding for playlist disc")
                    })?;
                    let cue_blob = blobs.get(&cue_hash.0).ok_or_else(|| {
                        PlatformError::new(ErrorCode::NotFound, "Bound cue blob is not in unindexed assets")
                    })?;
                    let BlobKind::Cue { track_names, .. } = &cue_blob.kind else {
                        return Err(PlatformError::new(
                            ErrorCode::ContentMismatch,
                            "Playlist .cue entry was bound to a non-cue asset",
                        ));
                    };
                    for tname in track_names {
                        if !bindings.contains_key(tname) {
                            return Err(PlatformError::new(
                                ErrorCode::InvalidRequest,
                                "Playlist cue sheet references an unbound track blob",
                            ));
                        }
                    }
                }
            }
            disc_names.clone()
        }
        BlobKind::Cue { .. } => vec![entry_name.clone()],
        _ => Vec::new(),
    };

    let (media, total_bytes, identity_hash) = try_build_media_from_bindings(
        cand.format,
        system,
        entry_name,
        entry_blob,
        disc_entries,
        &bindings,
        &blobs,
    )?;

    let manifest = GameManifest {
        schema_version: 1,
        id: Uuid::new_v4(),
        title,
        system,
        content_hash: identity_hash,
        byte_length: total_bytes,
        region: None,
        nominal_refresh_hz: None,
        created_at_ms: now_ms(),
        media: Some(media),
    };
    manifest.validate()?;
    let inserted = Library::new(database).insert_manifest(manifest)?;
    let _ = crate::profile_backup::reconcile_completed_restore_staging_under_lease(database);
    Ok(inserted)
}

/// Reconstructs all unambiguous single-file and multi-file candidates (used during corrupted DB recovery).
pub(crate) fn reconstruct_unambiguous_all(database: &Database) -> Result<Vec<GameManifest>> {
    let _lease = AssetLease::acquire(database)?;
    let current = build_preview_unlocked(database)?;
    let until = Instant::now() + Duration::from_secs(30);
    let (blobs, _, _) = scan_unindexed_blobs(database, until)?;
    let mut restored = Vec::new();

    for cand in current.candidates {
        if cand.needs_track_binding {
            continue;
        }
        let Some(system) = cand.suggested_system else { continue };
        let Some(entry_blob) = blobs.get(&cand.entry_hash.0) else { continue };
        let Ok(extension) = cand.format.extension(system) else { continue };
        let entry_name = format!("game.{extension}");
        let disc_entries = match &entry_blob.kind {
            BlobKind::M3u { disc_names } => disc_names.clone(),
            BlobKind::Cue { .. } => vec![entry_name.clone()],
            _ => Vec::new(),
        };
        let Ok((media, total_bytes, identity_hash)) = try_build_media_from_bindings(
            cand.format,
            system,
            entry_name,
            entry_blob,
            disc_entries,
            &cand.resolved_bindings,
            &blobs,
        ) else {
            continue;
        };
        let manifest = GameManifest {
            schema_version: 1,
            id: Uuid::new_v4(),
            title: cand.suggested_title,
            system,
            content_hash: identity_hash,
            byte_length: total_bytes,
            region: None,
            nominal_refresh_hz: None,
            created_at_ms: now_ms(),
            media: Some(media),
        };
        if manifest.validate().is_ok() {
            if let Ok(inserted) = Library::new(database).insert_manifest(manifest) {
                restored.push(inserted);
            }
        }
    }
    let _ = crate::profile_backup::reconcile_completed_restore_staging_under_lease(database);
    Ok(restored)
}
