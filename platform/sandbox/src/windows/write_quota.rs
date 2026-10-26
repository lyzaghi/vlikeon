//! Enforces physical write quotas on Windows for core scratch/save subtrees,
//! presentation helper temporary storage, shader/driver caches, and profile-wide
//! native session storage.

use omniplay_contracts::{ErrorCode, PlatformError, Result};
use std::{
    fs,
    os::windows::fs::MetadataExt,
    path::Path,
};

pub const CLUSTER_ROUND_BYTES: u64 = 4096;
pub const MAX_CORE_SCRATCH_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_CORE_SCRATCH_FILES: u32 = 256;
pub const MAX_CORE_SAVE_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_CORE_SAVE_FILES: u32 = 256;
pub const MAX_HELPER_TEMP_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_HELPER_TEMP_FILES: u32 = 64;
pub const MAX_CACHE_WRITE_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_CACHE_WRITE_FILES: u32 = 256;
pub const MAX_PROFILE_NATIVE_WORK_BYTES: u64 = 32 * 1024 * 1024 * 1024;
pub const MAX_PROFILE_NATIVE_WORK_FILES: u32 = 4096;

const REPARSE_ATTRIBUTE: u32 = 0x400;
const MAX_SUBTREE_DEPTH: u8 = 16;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubtreeUsage {
    pub logical_bytes: u64,
    pub physical_bytes: u64,
    pub file_count: u32,
    pub directory_count: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WindowsWorkWriteUsage {
    pub core_scratch: SubtreeUsage,
    pub core_save: SubtreeUsage,
    pub helper_temp: SubtreeUsage,
    pub cache: SubtreeUsage,
}

impl WindowsWorkWriteUsage {
    pub fn total_physical_bytes(&self) -> u64 {
        self.core_scratch
            .physical_bytes
            .saturating_add(self.core_save.physical_bytes)
            .saturating_add(self.helper_temp.physical_bytes)
            .saturating_add(self.cache.physical_bytes)
    }
}

fn quota_exceeded(area: &str) -> PlatformError {
    PlatformError::new(
        ErrorCode::ResourceLimit,
        format!("Windows native write quota exceeded in {area}"),
    )
    .recover("Close the game session or clear temporary session data before retrying")
}

fn invalid_entry(area: &str) -> PlatformError {
    PlatformError::new(
        ErrorCode::StorageFailure,
        format!("Windows native storage subtree {area} contains a non-ordinary or linked entry"),
    )
}

fn round_up_cluster(bytes: u64) -> Option<u64> {
    if bytes == 0 {
        return Some(0);
    }
    let rem = bytes % CLUSTER_ROUND_BYTES;
    if rem == 0 {
        Some(bytes)
    } else {
        bytes.checked_add(CLUSTER_ROUND_BYTES - rem)
    }
}

pub fn measure_subtree(
    root: &Path,
    area: &str,
    max_bytes: u64,
    max_files: u32,
) -> Result<SubtreeUsage> {
    let meta = match fs::symlink_metadata(root) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(SubtreeUsage::default()),
        Err(_) => return Err(invalid_entry(area)),
    };
    if !meta.is_dir() || meta.file_type().is_symlink() || (meta.file_attributes() & REPARSE_ATTRIBUTE) != 0 {
        return Err(invalid_entry(area));
    }

    let mut usage = SubtreeUsage::default();
    let mut stack = vec![(root.to_path_buf(), 0u8)];
    while let Some((dir, depth)) = stack.pop() {
        for entry in fs::read_dir(&dir).map_err(|_| invalid_entry(area))? {
            let entry = entry.map_err(|_| invalid_entry(area))?;
            let path = entry.path();
            let entry_meta = fs::symlink_metadata(&path).map_err(|_| invalid_entry(area))?;
            if entry_meta.file_type().is_symlink() || (entry_meta.file_attributes() & REPARSE_ATTRIBUTE) != 0 {
                return Err(invalid_entry(area));
            }
            if entry_meta.is_dir() {
                if depth >= MAX_SUBTREE_DEPTH {
                    return Err(quota_exceeded(area));
                }
                usage.directory_count = usage.directory_count.checked_add(1).ok_or_else(|| quota_exceeded(area))?;
                if usage.directory_count > max_files {
                    return Err(quota_exceeded(area));
                }
                stack.push((path, depth + 1));
            } else if entry_meta.is_file() {
                usage.file_count = usage.file_count.checked_add(1).ok_or_else(|| quota_exceeded(area))?;
                if usage.file_count > max_files {
                    return Err(quota_exceeded(area));
                }
                let size = entry_meta.file_size();
                let physical = round_up_cluster(size).ok_or_else(|| quota_exceeded(area))?;
                usage.logical_bytes = usage.logical_bytes.checked_add(size).ok_or_else(|| quota_exceeded(area))?;
                usage.physical_bytes = usage.physical_bytes.checked_add(physical).ok_or_else(|| quota_exceeded(area))?;
                if usage.physical_bytes > max_bytes {
                    return Err(quota_exceeded(area));
                }
            } else {
                return Err(invalid_entry(area));
            }
        }
    }
    Ok(usage)
}

pub fn enforce_work_write_quotas(
    work_dir: &Path,
    allow_legacy_os_writes: bool,
) -> Result<WindowsWorkWriteUsage> {
    let (scratch_cap_bytes, scratch_cap_files, save_cap_bytes, save_cap_files) = if allow_legacy_os_writes {
        (
            MAX_CORE_SCRATCH_BYTES,
            MAX_CORE_SCRATCH_FILES,
            MAX_CORE_SAVE_BYTES,
            MAX_CORE_SAVE_FILES,
        )
    } else {
        (0u64, 0u32, 0u64, 0u32)
    };

    let core_scratch = measure_subtree(
        &work_dir.join("scratch"),
        "core_scratch",
        scratch_cap_bytes,
        scratch_cap_files,
    )?;
    let core_save = measure_subtree(
        &work_dir.join("save"),
        "core_save",
        save_cap_bytes,
        save_cap_files,
    )?;
    let helper_temp = measure_subtree(
        &work_dir.join("presentation-tmp"),
        "presentation_helper_temp",
        MAX_HELPER_TEMP_BYTES,
        MAX_HELPER_TEMP_FILES,
    )?;
    let cache = measure_subtree(
        &work_dir.join("cache"),
        "runtime_cache",
        MAX_CACHE_WRITE_BYTES,
        MAX_CACHE_WRITE_FILES,
    )?;

    Ok(WindowsWorkWriteUsage {
        core_scratch,
        core_save,
        helper_temp,
        cache,
    })
}

pub fn enforce_profile_write_quota(profile_root: &Path) -> Result<SubtreeUsage> {
    let native_work = measure_subtree(
        &profile_root.join("native-work"),
        "profile_native_work",
        MAX_PROFILE_NATIVE_WORK_BYTES,
        MAX_PROFILE_NATIVE_WORK_FILES,
    )?;
    let retirement = measure_subtree(
        &profile_root.join("native-retirement"),
        "profile_native_retirement",
        16 * 1024 * 1024,
        512,
    )?;
    let total_physical = native_work
        .physical_bytes
        .checked_add(retirement.physical_bytes)
        .ok_or_else(|| quota_exceeded("profile_native_work"))?;
    if total_physical > MAX_PROFILE_NATIVE_WORK_BYTES {
        return Err(quota_exceeded("profile_native_work"));
    }
    Ok(SubtreeUsage {
        logical_bytes: native_work.logical_bytes.saturating_add(retirement.logical_bytes),
        physical_bytes: total_physical,
        file_count: native_work.file_count.saturating_add(retirement.file_count),
        directory_count: native_work.directory_count.saturating_add(retirement.directory_count),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn enforces_readonly_vs_legacy_core_and_helper_write_quotas() {
        let root = std::env::temp_dir().join(format!("omniplay-win-quota-{}", Uuid::new_v4()));
        let work = root.join("native-work").join(Uuid::new_v4().to_string());
        fs::create_dir_all(work.join("scratch")).unwrap();
        fs::create_dir_all(work.join("save")).unwrap();
        fs::create_dir_all(work.join("presentation-tmp")).unwrap();
        fs::create_dir_all(work.join("cache")).unwrap();

        // Empty directories pass both readonly and legacy modes
        let empty = enforce_work_write_quotas(&work, false).unwrap();
        assert_eq!(empty.total_physical_bytes(), 0);

        // Writing to scratch when allow_legacy_os_writes == false must fail with ResourceLimit
        fs::write(work.join("scratch").join("illegal.tmp"), b"actor-write").unwrap();
        let err = enforce_work_write_quotas(&work, false).unwrap_err();
        assert_eq!(err.code, ErrorCode::ResourceLimit);

        // Under legacy OS writes, bounded scratch/helper/cache writes succeed and round to 4 KiB clusters
        fs::write(work.join("presentation-tmp").join("frame.tmp"), vec![0u8; 5000]).unwrap();
        fs::write(work.join("cache").join("shader.bin"), vec![1u8; 100]).unwrap();
        let usage = enforce_work_write_quotas(&work, true).unwrap();
        assert_eq!(usage.core_scratch.physical_bytes, 4096);
        assert_eq!(usage.helper_temp.physical_bytes, 8192);
        assert_eq!(usage.cache.physical_bytes, 4096);

        let profile_usage = enforce_profile_write_quota(&root).unwrap();
        assert_eq!(profile_usage.physical_bytes, 4096 + 8192 + 4096);

        let _ = fs::remove_dir_all(&root);
    }
}
