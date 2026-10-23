//! Salvage bytes independently of library/save metadata. Nothing in this module
//! publishes a game, save, head, lineage or authenticated origin.
use crate::{asset_storage::AssetLease, save_payloads, util::{digest, storage_error}, Database};
use omniplay_contracts::{ContentHash, ErrorCode, PlatformError, Result, MAX_ROM_BYTES};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs::{self, File}, io::{Read, Seek, SeekFrom, Write}, path::{Path, PathBuf}, time::{Duration, Instant}};

const MAGIC: &[u8; 8] = b"OPRC0001";
const DOMAIN: &[u8] = b"Vlikeon unverified recovery v1\0";
const MANIFEST_MAX: usize = 4096;
const PAGE: usize = 64;
const INVENTORY_MAX: usize = 20000;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub schema_version: u32,
    pub trust: String,
    pub source_kind: String,
    pub display_name: String,
    pub payload_bytes: u64,
    pub payload_hash: ContentHash,
    pub prefix_hex: String,
}

impl Record {
    fn validate(&self) -> Result<()> {
        ContentHash::parse(self.payload_hash.0.clone())?;
        if self.schema_version != 1 || self.trust != "unverified"
            || !matches!(self.source_kind.as_str(), "assets" | "legacy_saves" | "repair_files" | "web_assets" | "web_saves")
            || self.display_name.len() > 240 || self.display_name.chars().any(char::is_control)
            || self.payload_bytes > MAX_ROM_BYTES
            || self.prefix_hex.len() != self.payload_bytes.min(32) as usize * 2
            || !self.prefix_hex.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)) {
            return Err(corrupt());
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct Candidate {
    pub candidate_id: ContentHash,
    pub source_kind: String,
    pub display_name: String,
    pub bytes: u64,
    pub inspectable: bool,
}
#[derive(Serialize)]
pub struct Page {
    pub candidates: Vec<Candidate>,
    pub next_after: Option<ContentHash>,
    pub retained_skipped: usize,
}
#[derive(Debug, Serialize)]
pub struct Review { pub candidate_id: ContentHash, pub revision: ContentHash, pub record: Record }
#[derive(Debug, Serialize)]
pub struct Exported { pub exported: bool, pub delivery: &'static str, pub record: Record }
struct Leaf { candidate: Candidate, path: PathBuf }

fn corrupt() -> PlatformError { PlatformError::new(ErrorCode::CorruptData, "The recovery copy is invalid or changed; inspect it again") }
fn bounded() -> PlatformError { PlatformError::new(ErrorCode::ResourceLimit, "Manual recovery exceeds its file, inventory or time limit") }
fn budget(until: Instant) -> Result<()> { if Instant::now() >= until { Err(bounded()) } else { Ok(()) } }
fn plain(value: &fs::Metadata, directory: bool) -> bool {
    #[cfg(windows)] {
        use std::os::windows::fs::MetadataExt;
        if value.file_attributes() & 0x400 != 0 { return false; }
    }
    !value.file_type().is_symlink() && if directory { value.is_dir() } else { value.is_file() }
}
fn label(path: &Path) -> String {
    let mut result = String::new();
    for ch in path.file_name().unwrap_or_default().to_string_lossy().chars() {
        let ch = if ch.is_control() { '\u{fffd}' } else { ch };
        if result.len() + ch.len_utf8() > 240 { break; }
        result.push(ch);
    }
    result
}

// Enumerate only the three data namespaces and one level of legacy game
// directories. IDs are opaque hashes, never renderer-provided filesystem paths.
fn inventory(database: &Database) -> Result<(Vec<Leaf>, usize)> {
    save_payloads::directory(&database.root)?;
    let until = Instant::now() + Duration::from_secs(15);
    let mut directories = vec![(database.root.join("assets"), "assets", false),
        (database.root.join("repaired-saves-v1"), "repair_files", false),
        (database.root.join("saves"), "legacy_saves", true)];
    let mut leaves = Vec::new();
    let mut visited = 0;
    let mut skipped = 0;
    while let Some((directory, kind, descend)) = directories.pop() {
        budget(until)?;
        match fs::symlink_metadata(&directory) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(storage_error(error)),
            Ok(metadata) if !plain(&metadata, true) => { skipped += 1; continue; },
            Ok(_) => (),
        }
        for entry in fs::read_dir(&directory).map_err(storage_error)? {
            budget(until)?;
            visited += 1;
            if visited > INVENTORY_MAX { return Err(bounded()); }
            let entry = entry.map_err(storage_error)?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).map_err(storage_error)?;
            if descend && plain(&metadata, true) { directories.push((path, kind, false)); continue; }
            if !plain(&metadata, false) { skipped += 1; continue; }
            let relative = path.strip_prefix(&database.root).map_err(storage_error)?;
            let id = digest(relative.as_os_str().as_encoded_bytes());
            leaves.push(Leaf { candidate: Candidate { candidate_id: id, source_kind: kind.into(),
                display_name: label(&path), bytes: metadata.len(), inspectable: metadata.len() <= MAX_ROM_BYTES }, path });
        }
    }
    leaves.sort_by(|a, b| a.candidate.candidate_id.0.cmp(&b.candidate.candidate_id.0));
    Ok((leaves, skipped))
}

pub fn list(database: &Database, after: Option<&ContentHash>) -> Result<Page> {
    if let Some(after) = after { ContentHash::parse(after.0.clone())?; }
    let _assets = AssetLease::acquire(database)?;
    let _saves = save_payloads::lease(&database.root)?;
    let (leaves, retained_skipped) = inventory(database)?;
    let mut candidates: Vec<_> = leaves.into_iter().map(|leaf| leaf.candidate)
        .filter(|item| after.is_none_or(|after| item.candidate_id.0 > after.0)).take(PAGE + 1).collect();
    let more = candidates.len() > PAGE;
    candidates.truncate(PAGE);
    let next_after = if more { candidates.last().map(|item| item.candidate_id.clone()) } else { None };
    Ok(Page { candidates, next_after, retained_skipped })
}

fn selected(database: &Database, id: &ContentHash) -> Result<(Candidate, File)> {
    ContentHash::parse(id.0.clone())?;
    let (leaves, _) = inventory(database)?;
    let leaf = leaves.into_iter().find(|leaf| &leaf.candidate.candidate_id == id)
        .ok_or_else(|| PlatformError::new(ErrorCode::NotFound, "Recovery source is no longer available; refresh the inventory"))?;
    if !leaf.candidate.inspectable { return Err(bounded()); }
    let parent = leaf.path.parent().ok_or_else(corrupt)?;
    // Reject a changed directory before opening the leaf (also O_NOFOLLOW / no
    // write/delete sharing at open). Hostile profile-ancestor replacement is not
    // claimed to be contained by these cooperative profile leases.
    save_payloads::directory(parent)?;
    let file = save_payloads::open_file(&leaf.path)?;
    Ok((leaf.candidate, file))
}

fn inspect_file(candidate: &Candidate, file: &mut File, until: Instant) -> Result<Review> {
    file.seek(SeekFrom::Start(0)).map_err(storage_error)?;
    let mut hash = Sha256::new();
    let mut size = 0u64;
    let mut prefix = Vec::new();
    let mut buffer = [0u8; 65536];
    loop {
        budget(until)?;
        let count = file.read(&mut buffer).map_err(storage_error)?;
        if count == 0 { break; }
        size += count as u64;
        if size > MAX_ROM_BYTES { return Err(bounded()); }
        prefix.extend_from_slice(&buffer[..count.min(32 - prefix.len())]);
        hash.update(&buffer[..count]);
    }
    let record = Record { schema_version: 1, trust: "unverified".into(), source_kind: candidate.source_kind.clone(),
        display_name: candidate.display_name.clone(), payload_bytes: size,
        payload_hash: ContentHash(format!("{:x}", hash.finalize())), prefix_hex: prefix.iter().map(|byte| format!("{byte:02x}")).collect() };
    record.validate()?;
    let revision = digest(&serde_json::to_vec(&(&candidate.candidate_id, &record)).map_err(storage_error)?);
    Ok(Review { candidate_id: candidate.candidate_id.clone(), revision, record })
}

pub fn inspect(database: &Database, id: &ContentHash) -> Result<Review> {
    let _assets = AssetLease::acquire(database)?;
    let _saves = save_payloads::lease(&database.root)?;
    let (candidate, mut file) = selected(database, id)?;
    inspect_file(&candidate, &mut file, Instant::now() + Duration::from_secs(90))
}

fn fresh_destination(file: &mut File) -> Result<()> {
    if !file.metadata().map_err(storage_error)?.is_file() || file.metadata().map_err(storage_error)?.len() != 0
        || file.stream_position().map_err(storage_error)? != 0 { return Err(corrupt()); }
    Ok(())
}

pub fn export(database: &Database, id: &ContentHash, revision: &ContentHash, destination: &mut File) -> Result<Exported> {
    fresh_destination(destination)?;
    let _assets = AssetLease::acquire(database)?;
    let _saves = save_payloads::lease(&database.root)?;
    let (candidate, mut file) = selected(database, id)?;
    let until = Instant::now() + Duration::from_secs(90);
    let review = inspect_file(&candidate, &mut file, until)?;
    if &review.revision != revision { return Err(PlatformError::new(ErrorCode::Conflict, "Recovery source changed after inspection; inspect it again")); }
    let record = review.record;
    let manifest = serde_json::to_vec(&record).map_err(storage_error)?;
    let mut header = MAGIC.to_vec();
    header.extend_from_slice(&(manifest.len() as u32).to_le_bytes());
    let mut package_hash = Sha256::new();
    package_hash.update(DOMAIN); package_hash.update(&header); package_hash.update(&manifest);
    // A truncated/interrupted package never begins with the accepted magic.
    destination.write_all(&[0u8; 8]).and_then(|_| destination.write_all(&header[8..]))
        .and_then(|_| destination.write_all(&manifest)).map_err(storage_error)?;
    file.seek(SeekFrom::Start(0)).map_err(storage_error)?;
    let mut payload_hash = Sha256::new();
    let mut remaining = record.payload_bytes;
    let mut buffer = [0u8; 65536];
    while remaining > 0 {
        budget(until)?;
        let wanted = remaining.min(buffer.len() as u64) as usize;
        file.read_exact(&mut buffer[..wanted]).map_err(storage_error)?;
        payload_hash.update(&buffer[..wanted]); package_hash.update(&buffer[..wanted]);
        destination.write_all(&buffer[..wanted]).map_err(storage_error)?;
        remaining -= wanted as u64;
    }
    if file.read(&mut buffer[..1]).map_err(storage_error)? != 0 || format!("{:x}", payload_hash.finalize()) != record.payload_hash.0 { return Err(corrupt()); }
    destination.write_all(&package_hash.finalize()).and_then(|_| destination.sync_all()).map_err(storage_error)?;
    destination.seek(SeekFrom::Start(0)).and_then(|_| destination.write_all(MAGIC)).and_then(|_| destination.sync_all()).map_err(storage_error)?;
    Ok(Exported { exported: true, delivery: "file_handle", record })
}

// Verify the complete package before the first extracted byte is written. The
// second copy pass rehashes the input in case another process changed it.
pub fn extract(source: &mut File, destination: &mut File) -> Result<Exported> {
    fresh_destination(destination)?;
    let until = Instant::now() + Duration::from_secs(90);
    source.seek(SeekFrom::Start(0)).map_err(storage_error)?;
    let mut header = [0u8; 12]; source.read_exact(&mut header).map_err(storage_error)?;
    let size = u32::from_le_bytes(header[8..12].try_into().map_err(|_| corrupt())?) as usize;
    if &header[..8] != MAGIC || size == 0 || size > MANIFEST_MAX { return Err(corrupt()); }
    let mut manifest = vec![0; size]; source.read_exact(&mut manifest).map_err(storage_error)?;
    let record: Record = serde_json::from_slice(&manifest).map_err(|_| corrupt())?;
    record.validate()?;
    if source.metadata().map_err(storage_error)?.len() != 12 + size as u64 + record.payload_bytes + 32 { return Err(corrupt()); }
    let mut package_hash = Sha256::new(); package_hash.update(DOMAIN); package_hash.update(header); package_hash.update(&manifest);
    let mut payload_hash = Sha256::new(); let mut prefix = Vec::new();
    let mut remaining = record.payload_bytes; let mut buffer = [0u8; 65536];
    while remaining > 0 {
        budget(until)?;
        let count = remaining.min(buffer.len() as u64) as usize;
        source.read_exact(&mut buffer[..count]).map_err(storage_error)?;
        prefix.extend_from_slice(&buffer[..count.min(32 - prefix.len())]);
        payload_hash.update(&buffer[..count]); package_hash.update(&buffer[..count]); remaining -= count as u64;
    }
    let mut trailer = [0u8; 32]; source.read_exact(&mut trailer).map_err(storage_error)?;
    if package_hash.finalize().as_slice() != trailer || format!("{:x}", payload_hash.finalize()) != record.payload_hash.0
        || prefix.iter().map(|byte| format!("{byte:02x}")).collect::<String>() != record.prefix_hex { return Err(corrupt()); }
    source.seek(SeekFrom::Start(12 + size as u64)).map_err(storage_error)?;
    let copied = (|| {
        let mut hash = Sha256::new(); let mut remaining = record.payload_bytes;
        while remaining > 0 {
            budget(until)?;
            let count = remaining.min(buffer.len() as u64) as usize;
            source.read_exact(&mut buffer[..count]).map_err(storage_error)?;
            hash.update(&buffer[..count]); destination.write_all(&buffer[..count]).map_err(storage_error)?; remaining -= count as u64;
        }
        if format!("{:x}", hash.finalize()) != record.payload_hash.0 { return Err(corrupt()); }
        destination.sync_all().map_err(storage_error)
    })();
    if copied.is_err() { let _ = destination.set_len(0); }
    copied?;
    Ok(Exported { exported: true, delivery: "file_handle", record })
}
