#![forbid(unsafe_code)]

pub mod channel;
pub mod package;
pub mod store;
pub mod transport;
pub mod repair;
pub mod cleanup;

use omniplay_contracts::{ErrorCode,PlatformError,Result};
use std::path::PathBuf;

pub fn rejected(message:&str)->PlatformError {PlatformError::new(ErrorCode::UpdateRejected,message)}

pub fn install_root()->Result<PathBuf> {
    let base=if cfg!(windows){std::env::var_os("LOCALAPPDATA").map(PathBuf::from)}
        else if cfg!(target_os="macos"){std::env::var_os("HOME").map(|p|PathBuf::from(p).join("Library/Application Support"))}
        else{std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(||std::env::var_os("HOME").map(|p|PathBuf::from(p).join(".local/share")))};
    let base=base.filter(|path|path.is_absolute()).ok_or_else(||rejected("The user installation directory must be absolute"))?;
    Ok(base.join("OmniPlay").join("install"))
}

pub fn acknowledge_broker_health()->Result<()> {
    let Some(id)=std::env::var_os("OMNIPLAY_ACTIVE_RELEASE")else{return Ok(());};
    let id=id.to_str().and_then(|v|uuid::Uuid::parse_str(v).ok()).ok_or_else(||rejected("Active release context is invalid"))?;
    let store=store::Store::open(&install_root()?,channel::Channel::embedded()?)?;
    let release=store.active()?;
    if release.id!=id{return Err(rejected("The broker belongs to an inactive release"));}
    let expected=package::verify_entry(&store.version_directory(id),&release.manifest,&release.manifest.broker_entry)?;
    if std::env::current_exe().and_then(|p|p.canonicalize()).map_err(omniplay_core::util::storage_error)?!=expected{return Err(rejected("The running broker is not the installed entry point"));}
    store.acknowledge_health(id)
}
pub fn note_opened_profile_schema(schema:u32)->Result<()> {store::Store::open(&install_root()?,channel::Channel::embedded()?)?.note_opened_schema(schema)}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::{PackageFile, PackageManifest};
    use omniplay_contracts::ContentHash;
    use sha2::{Digest, Sha256};
    use std::io::Write;
    use std::sync::atomic::AtomicBool;
    use uuid::Uuid;

    fn hash_bytes(bytes: &[u8]) -> ContentHash {
        ContentHash(format!("{:x}", Sha256::digest(bytes)))
    }

    fn write_oppk_package(path: & std::path::Path, manifest: &PackageManifest, payloads: &[(&str, &[u8])]) {
        let mut f = std::fs::File::create(path).expect("create package file");
        f.write_all(b"OPPK0001").unwrap();
        let inv = serde_json::to_vec(manifest).unwrap();
        f.write_all(&(inv.len() as u32).to_le_bytes()).unwrap();
        f.write_all(&inv).unwrap();
        for (_, data) in payloads {
            f.write_all(data).unwrap();
        }
        f.sync_all().unwrap();
    }

    #[test]
    fn selective_repair_staging_sweep_and_release_pruning() {
        let root = std::env::temp_dir().join(format!("omniplay-updater-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let channel = channel::Channel::embedded().expect("embedded channel");
        let mut store = store::Store::open(&root, channel).expect("open store");

        let browser_bytes = b"browser_binary_v1";
        let broker_bytes = b"broker_binary_v1";
        let notice_bytes = b"third_party_notices";
        let offer_bytes = b"{\"source\":\"offer\"}";

        let payloads: Vec<(&str, &[u8])> = vec![
            ("bin/omniplay-browser.exe", browser_bytes),
            ("bin/omniplay-broker.exe", broker_bytes),
            ("licenses/third_party_notices.txt", notice_bytes),
            ("licenses/source_offer.json", offer_bytes),
        ];

        let manifest = PackageManifest {
            schema_version: 1,
            channel: "stable".to_string(),
            platform: "windows-x86_64".to_string(),
            version: "1.0.0".parse().unwrap(),
            chromium_revision: "130.0.6723.70".to_string(),
            minimum_data_schema: 1,
            maximum_data_schema: 20,
            browser_entry: "bin/omniplay-browser.exe".to_string(),
            broker_entry: "bin/omniplay-broker.exe".to_string(),
            files: payloads
                .iter()
                .map(|(p, b)| PackageFile {
                    path: (*p).to_string(),
                    bytes: b.len() as u64,
                    sha256: hash_bytes(b),
                    executable: p.ends_with(".exe"),
                })
                .collect(),
        };

        let rel_active = Uuid::new_v4();
        let ver_dir = store.version_directory(rel_active);
        for (rel_path, data) in &payloads {
            let full = ver_dir.join(rel_path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(&full, data).unwrap();
        }

        // Insert active release + 2 older releases to test pruning and repair
        let rel_prev = Uuid::new_v4();
        let rel_old = Uuid::new_v4();
        for (idx, (id, status, ver)) in [
            (rel_old, "previous", "0.8.0"),
            (rel_prev, "previous", "0.9.0"),
            (rel_active, "active", "1.0.0"),
        ]
        .into_iter()
        .enumerate()
        {
            let mut m = manifest.clone();
            m.version = ver.parse().unwrap();
            if id != rel_active {
                let d = store.version_directory(id);
                std::fs::create_dir_all(&d).unwrap();
                std::fs::write(d.join("dummy.bin"), b"old").unwrap();
            }
            let conn = rusqlite::Connection::open(store.root.join("install.sqlite3")).unwrap();
            conn.execute(
                "INSERT INTO releases(id,version,metadata_version,epoch,status,manifest,envelope,created_at_ms) VALUES(?1,?2,?3,1,?4,?5,'{}',1729850000000)",
                rusqlite::params![
                    id.to_string(),
                    ver,
                    (idx + 1) as u64,
                    status,
                    serde_json::to_string(&m).unwrap()
                ],
            )
            .unwrap();
        }

        // 1. Corrupt broker binary and run repair_release
        let broker_path = ver_dir.join("bin/omniplay-broker.exe");
        std::fs::write(&broker_path, b"corrupted!").unwrap();
        let pkg_path = root.join("repair.oppk");
        write_oppk_package(&pkg_path, &manifest, &payloads);

        let report = repair::repair_release(&mut store, rel_active, &pkg_path, &AtomicBool::new(false))
            .expect("repair_release");
        assert_eq!(report.status, "repaired");
        assert_eq!(report.corrupted_files, vec!["bin/omniplay-broker.exe".to_string()]);
        assert_eq!(std::fs::read(&broker_path).unwrap(), broker_bytes);

        // 2. Staging sweep (max_age_seconds = 0 sweeps all staging folders)
        let stale_staging = store.root.join("staging").join("abandoned-1");
        std::fs::create_dir_all(&stale_staging).unwrap();
        std::fs::write(stale_staging.join("part.bin"), vec![0u8; 512]).unwrap();
        let sweep_res = cleanup::sweep_staging(&store, 0).expect("sweep_staging");
        assert_eq!(sweep_res.swept_directories, 1);
        assert_eq!(sweep_res.reclaimed_bytes, 512);

        // 3. Prune old releases keeping top 2 (active 1.0.0 + previous 0.9.0), pruning 0.8.0
        let prune_res = cleanup::prune_old_releases(&mut store, 2).expect("prune_old_releases");
        assert_eq!(prune_res.pruned_releases, vec![rel_old.to_string()]);
        assert_eq!(prune_res.remaining_releases, 2);
        assert!(!store.version_directory(rel_old).exists());
        assert!(store.version_directory(rel_active).exists());

        let _ = std::fs::remove_dir_all(&root);
    }
}
