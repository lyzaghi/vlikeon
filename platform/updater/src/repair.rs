use crate::{package, rejected, store::Store};
use omniplay_contracts::{ErrorCode, PlatformError, Result};
use omniplay_core::util::{hash_file, storage_error};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RepairReport {
    pub release_id: Uuid,
    pub inspected_files: usize,
    pub corrupted_files: Vec<String>,
    pub repaired_files: Vec<String>,
    pub status: String,
}

pub fn repair_release(
    store: &mut Store,
    id: Uuid,
    package_path: &Path,
    cancel: &AtomicBool,
) -> Result<RepairReport> {
    let _lock = store.lock()?;
    if cancel.load(Ordering::Relaxed) {
        return Err(PlatformError::new(ErrorCode::Cancelled, "Repair was cancelled"));
    }
    let release = store.releases()?.into_iter().find(|r| r.id == id).ok_or_else(|| {
        PlatformError::new(ErrorCode::NotFound, "Release to repair was not found")
    })?;

    let version_dir = store.version_directory(id);
    let mut corrupted = Vec::new();
    let mut inspected = 0;

    for file in &release.manifest.files {
        inspected += 1;
        let file_path = version_dir.join(&file.path);
        if !file_path.exists() {
            corrupted.push(file.path.clone());
            continue;
        }
        if let Ok(meta) = file_path.metadata() {
            if meta.len() != file.bytes {
                corrupted.push(file.path.clone());
                continue;
            }
        } else {
            corrupted.push(file.path.clone());
            continue;
        }
        if let Ok(mut opened) = File::open(&file_path) {
            if let Ok((h, bytes)) = hash_file(&mut opened, file.bytes) {
                if h != file.sha256 || bytes != file.bytes {
                    corrupted.push(file.path.clone());
                }
            } else {
                corrupted.push(file.path.clone());
            }
        } else {
            corrupted.push(file.path.clone());
        }
    }

    if corrupted.is_empty() {
        store.verify_installed(&release, true)?;
        return Ok(RepairReport {
            release_id: id,
            inspected_files: inspected,
            corrupted_files: vec![],
            repaired_files: vec![],
            status: "already_healthy".into(),
        });
    }

    if cancel.load(Ordering::Relaxed) {
        return Err(PlatformError::new(ErrorCode::Cancelled, "Repair was cancelled"));
    }

    let mut pkg_file = File::open(package_path).map_err(storage_error)?;
    let staging_dir = store.root.join("staging").join(format!("repair-{}", Uuid::new_v4()));
    if let Some(parent) = staging_dir.parent() {
        std::fs::create_dir_all(parent).map_err(storage_error)?;
    }
    std::fs::create_dir(&staging_dir).map_err(storage_error)?;

    let unpack_res = package::unpack_matching_manifest(&mut pkg_file, &staging_dir, &release.manifest, cancel);
    if let Err(err) = unpack_res {
        let _ = std::fs::remove_dir_all(&staging_dir);
        return Err(err);
    }

    let mut repaired = Vec::new();
    for rel_path in &corrupted {
        if cancel.load(Ordering::Relaxed) {
            let _ = std::fs::remove_dir_all(&staging_dir);
            return Err(PlatformError::new(ErrorCode::Cancelled, "Repair was cancelled"));
        }
        let src = staging_dir.join(rel_path);
        let dst = version_dir.join(rel_path);
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).map_err(storage_error)?;
        }
        std::fs::copy(&src, &dst).map_err(storage_error)?;
        repaired.push(rel_path.clone());
    }

    let _ = std::fs::remove_dir_all(&staging_dir);
    store.verify_installed(&release, true)?;

    Ok(RepairReport {
        release_id: id,
        inspected_files: inspected,
        corrupted_files: corrupted,
        repaired_files: repaired,
        status: "repaired".into(),
    })
}
