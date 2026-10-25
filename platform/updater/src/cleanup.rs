use crate::{rejected, store::Store};
use omniplay_contracts::{ErrorCode, PlatformError, Result};
use omniplay_core::util::storage_error;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SweepSummary {
    pub swept_directories: usize,
    pub reclaimed_bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PruneSummary {
    pub pruned_releases: Vec<String>,
    pub reclaimed_bytes: u64,
    pub remaining_releases: usize,
}

pub fn sweep_staging(store: &Store, max_age_seconds: u64) -> Result<SweepSummary> {
    let staging_dir = store.root.join("staging");
    if !staging_dir.exists() {
        return Ok(SweepSummary {
            swept_directories: 0,
            reclaimed_bytes: 0,
        });
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let mut swept = 0;
    let mut reclaimed = 0u64;

    if let Ok(entries) = std::fs::read_dir(&staging_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Ok(meta) = path.metadata() {
                    let modified = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    if now.saturating_sub(modified) >= max_age_seconds {
                        if let Ok(size) = dir_size(&path) {
                            reclaimed = reclaimed.saturating_add(size);
                        }
                        if std::fs::remove_dir_all(&path).is_ok() {
                            swept += 1;
                        }
                    }
                }
            }
        }
    }

    Ok(SweepSummary {
        swept_directories: swept,
        reclaimed_bytes: reclaimed,
    })
}

pub fn prune_old_releases(store: &mut Store, keep_count: usize) -> Result<PruneSummary> {
    let _lock = store.lock()?;
    let releases = store.releases()?;
    if releases.len() <= keep_count || keep_count < 2 {
        return Ok(PruneSummary {
            pruned_releases: vec![],
            reclaimed_bytes: 0,
            remaining_releases: releases.len(),
        });
    }

    let active_id = store.active().map(|r| r.id).ok();
    let mut pruned = Vec::new();
    let mut reclaimed = 0u64;

    for release in releases.iter().skip(keep_count) {
        if Some(release.id) == active_id {
            continue;
        }
        let ver_dir = store.version_directory(release.id);
        if ver_dir.exists() {
            if let Ok(size) = dir_size(&ver_dir) {
                reclaimed = reclaimed.saturating_add(size);
            }
            let _ = std::fs::remove_dir_all(&ver_dir);
        }
        store
            .delete_release_record(release.id)?;
        pruned.push(release.id.to_string());
    }

    let remaining = store.releases()?.len();
    Ok(PruneSummary {
        pruned_releases: pruned,
        reclaimed_bytes: reclaimed,
        remaining_releases: remaining,
    })
}

fn dir_size(path: &Path) -> Result<u64> {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                total = total.saturating_add(dir_size(&p)?);
            } else if let Ok(meta) = p.metadata() {
                total = total.saturating_add(meta.len());
            }
        }
    }
    Ok(total)
}
