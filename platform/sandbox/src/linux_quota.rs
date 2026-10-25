//! Enforces physical block disk usage quotas on Linux for session work trees,
//! presentation temporary storage, and profile-wide native session storage,
//! accounting for allocated 512-byte filesystem blocks (`st_blocks`) and
//! deduplicating hard links by `(st_dev, st_ino)`.

use omniplay_contracts::{ErrorCode, PlatformError, Result};
use std::{
    collections::BTreeSet,
    fs,
    os::unix::fs::MetadataExt,
    path::Path,
};

pub const BLOCK_BYTES: u64 = 512;
pub const MAX_SESSION_ROOT_BYTES: u64 = 1024 * 1024 * 1024;
pub const MAX_SESSION_ROOT_FILES: u32 = 256;
pub const MAX_PRESENTATION_TEMP_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_PRESENTATION_TEMP_FILES: u32 = 64;
pub const MAX_SESSION_WORK_BYTES: u64 = 8 * 1024 * 1024 * 1024;
pub const MAX_SESSION_WORK_FILES: u32 = 512;
pub const MAX_PROFILE_NATIVE_WORK_BYTES: u64 = 32 * 1024 * 1024 * 1024;
pub const MAX_PROFILE_NATIVE_WORK_FILES: u32 = 4096;
pub const MAX_PROFILE_RETIREMENT_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_PROFILE_RETIREMENT_FILES: u32 = 512;

const MAX_SUBTREE_DEPTH: u8 = 16;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LinuxPhysicalUsage {
    pub logical_bytes: u64,
    pub physical_bytes: u64,
    pub file_count: u32,
    pub directory_count: u32,
    pub deduplicated_hardlinks: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LinuxSessionWriteUsage {
    pub work_total: LinuxPhysicalUsage,
    pub root_stage: LinuxPhysicalUsage,
    pub presentation_temp: LinuxPhysicalUsage,
}

fn quota_exceeded(area: &str) -> PlatformError {
    PlatformError::new(
        ErrorCode::ResourceLimit,
        format!("Linux native physical disk quota exceeded in {area}"),
    )
    .recover("Close the game session or clear temporary session data before retrying")
}

fn invalid_entry(area: &str) -> PlatformError {
    PlatformError::new(
        ErrorCode::StorageFailure,
        format!("Linux native storage subtree {area} contains a non-ordinary, foreign-owned, or linked entry"),
    )
}

fn allocated_file_bytes(meta: &fs::Metadata) -> Option<u64> {
    let block_bytes = meta.blocks().checked_mul(BLOCK_BYTES)?;
    let logical_rounded = if meta.len() == 0 {
        0
    } else {
        let rem = meta.len() % 4096;
        if rem == 0 {
            meta.len()
        } else {
            meta.len().checked_add(4096 - rem)?
        }
    };
    Some(block_bytes.max(logical_rounded))
}

pub fn measure_subtree_dedup(
    root: &Path,
    area: &str,
    max_bytes: u64,
    max_files: u32,
    seen_inodes: &mut BTreeSet<(u64, u64)>,
) -> Result<LinuxPhysicalUsage> {
    let meta = match fs::symlink_metadata(root) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(LinuxPhysicalUsage::default()),
        Err(_) => return Err(invalid_entry(area)),
    };
    let euid = unsafe { libc::geteuid() };
    if !meta.is_dir() || meta.file_type().is_symlink() || meta.uid() != euid || (meta.mode() & 0o022) != 0 {
        return Err(invalid_entry(area));
    }

    let mut usage = LinuxPhysicalUsage::default();
    let mut stack = vec![(root.to_path_buf(), 0u8)];
    while let Some((dir, depth)) = stack.pop() {
        for entry in fs::read_dir(&dir).map_err(|_| invalid_entry(area))? {
            let entry = entry.map_err(|_| invalid_entry(area))?;
            let path = entry.path();
            let entry_meta = fs::symlink_metadata(&path).map_err(|_| invalid_entry(area))?;
            if entry_meta.file_type().is_symlink() || entry_meta.uid() != euid || (entry_meta.mode() & 0o022) != 0 {
                return Err(invalid_entry(area));
            }
            if entry_meta.is_dir() {
                if depth >= MAX_SUBTREE_DEPTH {
                    return Err(quota_exceeded(area));
                }
                usage.directory_count = usage
                    .directory_count
                    .checked_add(1)
                    .ok_or_else(|| quota_exceeded(area))?;
                if usage.directory_count > max_files {
                    return Err(quota_exceeded(area));
                }
                stack.push((path, depth + 1));
            } else if entry_meta.is_file() {
                usage.file_count = usage
                    .file_count
                    .checked_add(1)
                    .ok_or_else(|| quota_exceeded(area))?;
                if usage.file_count > max_files {
                    return Err(quota_exceeded(area));
                }
                let inode_key = (entry_meta.dev(), entry_meta.ino());
                if seen_inodes.insert(inode_key) {
                    let logical = entry_meta.len();
                    let physical = allocated_file_bytes(&entry_meta).ok_or_else(|| quota_exceeded(area))?;
                    usage.logical_bytes = usage
                        .logical_bytes
                        .checked_add(logical)
                        .ok_or_else(|| quota_exceeded(area))?;
                    usage.physical_bytes = usage
                        .physical_bytes
                        .checked_add(physical)
                        .ok_or_else(|| quota_exceeded(area))?;
                    if usage.physical_bytes > max_bytes {
                        return Err(quota_exceeded(area));
                    }
                } else {
                    usage.deduplicated_hardlinks = usage.deduplicated_hardlinks.saturating_add(1);
                }
            } else {
                return Err(invalid_entry(area));
            }
        }
    }
    Ok(usage)
}

pub fn enforce_work_physical_quotas(work_dir: &Path) -> Result<LinuxSessionWriteUsage> {
    let mut root_inodes = BTreeSet::new();
    let root_stage = measure_subtree_dedup(
        &work_dir.join("root"),
        "linux_session_root_stage",
        MAX_SESSION_ROOT_BYTES,
        MAX_SESSION_ROOT_FILES,
        &mut root_inodes,
    )?;

    let mut temp_inodes = BTreeSet::new();
    let presentation_temp = measure_subtree_dedup(
        &work_dir.join("presentation-tmp"),
        "linux_presentation_temp",
        MAX_PRESENTATION_TEMP_BYTES,
        MAX_PRESENTATION_TEMP_FILES,
        &mut temp_inodes,
    )?;

    let mut work_inodes = BTreeSet::new();
    let work_total = measure_subtree_dedup(
        work_dir,
        "linux_session_work",
        MAX_SESSION_WORK_BYTES,
        MAX_SESSION_WORK_FILES,
        &mut work_inodes,
    )?;

    Ok(LinuxSessionWriteUsage {
        work_total,
        root_stage,
        presentation_temp,
    })
}

pub fn enforce_profile_physical_quota(profile_root: &Path) -> Result<LinuxPhysicalUsage> {
    let mut seen_inodes = BTreeSet::new();
    let native_work = measure_subtree_dedup(
        &profile_root.join("native-work"),
        "linux_profile_native_work",
        MAX_PROFILE_NATIVE_WORK_BYTES,
        MAX_PROFILE_NATIVE_WORK_FILES,
        &mut seen_inodes,
    )?;
    let retirements = measure_subtree_dedup(
        &profile_root.join("native-retirements"),
        "linux_profile_retirements",
        MAX_PROFILE_RETIREMENT_BYTES,
        MAX_PROFILE_RETIREMENT_FILES,
        &mut seen_inodes,
    )?;
    let image_cache = measure_subtree_dedup(
        &profile_root.join("linux-image-cache"),
        "linux_profile_image_cache",
        MAX_PROFILE_NATIVE_WORK_BYTES,
        MAX_PROFILE_NATIVE_WORK_FILES,
        &mut seen_inodes,
    )?;

    let total_physical = native_work
        .physical_bytes
        .checked_add(retirements.physical_bytes)
        .and_then(|sum| sum.checked_add(image_cache.physical_bytes))
        .ok_or_else(|| quota_exceeded("linux_profile_native_work"))?;
    if total_physical > MAX_PROFILE_NATIVE_WORK_BYTES {
        return Err(quota_exceeded("linux_profile_native_work"));
    }

    Ok(LinuxPhysicalUsage {
        logical_bytes: native_work
            .logical_bytes
            .saturating_add(retirements.logical_bytes)
            .saturating_add(image_cache.logical_bytes),
        physical_bytes: total_physical,
        file_count: native_work
            .file_count
            .saturating_add(retirements.file_count)
            .saturating_add(image_cache.file_count),
        directory_count: native_work
            .directory_count
            .saturating_add(retirements.directory_count)
            .saturating_add(image_cache.directory_count),
        deduplicated_hardlinks: native_work
            .deduplicated_hardlinks
            .saturating_add(retirements.deduplicated_hardlinks)
            .saturating_add(image_cache.deduplicated_hardlinks),
    })
}
