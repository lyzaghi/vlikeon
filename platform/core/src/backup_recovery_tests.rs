//! Authored acceptance fixtures; not executed during source authoring.
use crate::{Database,library::Library,saves::SaveStore,util::digest};
use omniplay_contracts::{ContentHash,GameManifest,GameSystem,SaveKind,SaveMetadata};
use std::{fs::{self,File,OpenOptions},io::{Read,Seek,SeekFrom},path::PathBuf};
use uuid::Uuid;
struct Directory(PathBuf);
impl Drop for Directory{fn drop(&mut self){let _=fs::remove_dir_all(&self.0);}}
struct Fixture{database:Database,_directory:Directory,game:GameManifest}
impl Fixture{
    fn new()->Self{let base=std::env::temp_dir().join(format!("vlikeon-backup-fixture-{}",Uuid::new_v4()));fs::create_dir(&base).unwrap();let source=base.join("selected.nes");fs::write(&source,[19;64]).unwrap();let database=Database::open(&base.join("profile")).unwrap();let game=Library::new(&database).import(File::open(source).unwrap(),"Fixture".into(),GameSystem::Nes).unwrap();Self{database,_directory:Directory(base),game}}
    fn save(&mut self,payload:&[u8])->SaveMetadata{self.save_in_slot(payload,0,None)}
    fn save_in_slot(&mut self,payload:&[u8],slot:u16,parent_id:Option<Uuid>)->SaveMetadata{let metadata=SaveMetadata{schema_version:1,id:Uuid::new_v4(),game_id:self.game.id,game_content_hash:self.game.content_hash.clone(),kind:SaveKind::InGame,slot,generation:1,parent_id,core_id:"fixture".into(),core_version:"1.0.0".parse().unwrap(),state_format_version:"1".into(),platform:"fixture".into(),core_config_hash:ContentHash("a".repeat(64)),payload_hash:digest(payload),payload_bytes:payload.len()as u64,created_at_ms:crate::now_ms(),device_id:self.database.device_id,label:None,conflict:false};SaveStore::new(&mut self.database).commit(metadata,payload,parent_id).unwrap()}
    fn archive(&mut self,ids:&[Uuid])->Vec<u8>{let path=self._directory.0.join(format!("{}.opbackup",Uuid::new_v4()));let mut file=OpenOptions::new().read(true).write(true).create_new(true).open(path).unwrap();crate::save_backup::export(&mut self.database,ids,&mut file).unwrap();file.seek(SeekFrom::Start(0)).unwrap();let mut bytes=Vec::new();file.read_to_end(&mut bytes).unwrap();bytes}
}
#[test]
fn archive_roundtrip_preserves_heads_origin_references_and_retry_identity(){
    let mut f=Fixture::new();let first=f.save(b"first");let second=f.save(b"second");let bytes=f.archive(&[first.id,second.id]);
    let result=crate::save_backup::restore(&mut f.database,&mut bytes.as_slice()).unwrap();assert_eq!(result.entries.len(),2);for entry in &result.entries{assert!(entry.error.is_none());assert!(!entry.already_present);let id=entry.restored_id.unwrap();let(local,origin)=crate::save_origins::inspect(&f.database.connection,id).unwrap();assert!(local.conflict);assert_eq!(origin.kind,omniplay_contracts::SaveOriginKind::Imported);assert!(origin.imported_from.is_some());let retained:String=f.database.connection.query_row("SELECT value FROM preferences WHERE key=?1",[format!("backup_reference.{id}")],|row|row.get(0)).unwrap();assert!(retained.contains("canonical_metadata"));}
    let head:String=f.database.connection.query_row("SELECT save_id FROM save_heads WHERE game_id=?1",[f.game.id.to_string()],|row|row.get(0)).unwrap();assert_eq!(head,first.id.to_string());
    let retry=crate::save_backup::restore(&mut f.database,&mut bytes.as_slice()).unwrap();assert!(retry.entries.iter().all(|entry|entry.already_present));assert_eq!(retry.entries[0].restored_id,result.entries[0].restored_id);
}
#[test]
fn damaged_archive_refuses_before_any_generation_is_imported(){
    let mut f=Fixture::new();let save=f.save(b"original");let mut bytes=f.archive(&[save.id]);bytes[14]^=1;
    assert!(crate::save_backup::restore(&mut f.database,&mut bytes.as_slice()).is_err());let count:i64=f.database.connection.query_row("SELECT count(*) FROM saves",[],|row|row.get(0)).unwrap();assert_eq!(count,1);
}
#[test]
fn active_session_returns_explicit_restore_failure_without_changing_head(){
    let mut f=Fixture::new();let save=f.save(b"original");let bytes=f.archive(&[save.id]);f.database.connection.execute("INSERT INTO sessions(id,game_id,runtime_id,state,info_json,created_at_ms,updated_at_ms) VALUES(?1,?2,'fixture','paused','{}',1,1)",rusqlite::params![Uuid::new_v4().to_string(),f.game.id.to_string()]).unwrap();
    let restored=crate::save_backup::restore(&mut f.database,&mut bytes.as_slice()).unwrap();assert_eq!(restored.entries[0].error,Some(omniplay_contracts::ErrorCode::Busy));assert!(restored.entries[0].restored_id.is_none());let count:i64=f.database.connection.query_row("SELECT count(*) FROM saves",[],|row|row.get(0)).unwrap();assert_eq!(count,1);
}
#[test]
fn exact_orphan_payload_repairs_a_copy_without_adopting_unknown_metadata(){
    let mut f=Fixture::new();let save=f.save(b"progress");let legacy=f.database.root.join("saves").join(f.game.id.to_string());fs::create_dir_all(&legacy).unwrap();let orphan=legacy.join(format!("{}.bin",Uuid::new_v4()));fs::write(&orphan,b"progress").unwrap();let live:String=f.database.connection.query_row("SELECT file_id FROM save_payload_locations WHERE save_id=?1",[save.id.to_string()],|row|row.get(0)).unwrap();fs::write(f.database.root.join("repaired-saves-v1").join(format!("{live}.bin")),b"damaged").unwrap();
    let page=crate::save_orphan_recovery::preview(&f.database,save.id,None).unwrap();assert_eq!(page.matches.len(),1);let candidate=&page.matches[0];let result=crate::save_orphan_recovery::recover(&mut f.database,save.id,&candidate.candidate_id,&candidate.revision).unwrap();assert!(result.repaired);assert_eq!(fs::read(orphan).unwrap(),b"progress");assert_eq!(SaveStore::new(&mut f.database).read(save.id,None).unwrap().1,b"progress");
}

#[test]
fn full_profile_backup_roundtrip_restores_single_and_multiple_saves_without_trailer_overread() {
    // 1. Single-save archive (save payload sits at the end of payload section right before 32-byte trailer)
    let mut single_src = Fixture::new();
    let single_save = single_src.save(b"single-save-before-trailer");
    let single_archive_path = single_src._directory.0.join("single.opprofile");
    let mut single_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&single_archive_path)
        .unwrap();
    let single_exported = crate::profile_backup::export(&mut single_src.database, true, &mut single_file).unwrap();
    assert_eq!(single_exported.games, 1);
    assert_eq!(single_exported.saves, 1);
    assert!(single_exported.include_rom_assets);

    let single_inspected = crate::profile_backup::inspect(&mut single_file).unwrap();
    assert_eq!(single_inspected.games, 1);
    assert_eq!(single_inspected.saves, 1);
    assert!(single_inspected.include_rom_assets);

    let single_dest_root = single_src._directory.0.join("restored-single");
    let mut single_dest_db = Database::open(&single_dest_root).unwrap();
    let single_restored = crate::profile_backup::restore(&mut single_dest_db, &mut single_file).unwrap();
    assert_eq!(single_restored.games_restored, 1);
    assert_eq!(single_restored.saves_restored, 1);
    assert_eq!(single_restored.saves_failed, 0);
    let derived_single_id =
        crate::profile_backup::derived_profile_save_id(&single_restored.archive_hash, single_save.id, 0);
    let (_, single_payload) = SaveStore::new(&mut single_dest_db).read(derived_single_id, None).unwrap();
    assert_eq!(single_payload, b"single-save-before-trailer");

    // 2. Multi-save archive (first, middle, and final save payloads + local quota preservation)
    let mut source = Fixture::new();
    let save_a = source.save(b"save-alpha-payload");
    let save_b = source.save(b"save-beta-payload-longer");
    let save_c = source.save(b"save-gamma-final-payload");
    crate::storage_accounting::set_physical_quota_bytes(
        &source.database,
        2 * crate::storage_accounting::MIN_PROFILE_PHYSICAL_QUOTA_BYTES,
    )
    .unwrap();

    let archive_path = source._directory.0.join("profile.opprofile");
    let mut archive_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&archive_path)
        .unwrap();
    let export_summary = crate::profile_backup::export(&mut source.database, true, &mut archive_file).unwrap();
    assert_eq!(export_summary.games, 1);
    assert_eq!(export_summary.saves, 3);
    assert!(export_summary.include_rom_assets);

    let dest_root = source._directory.0.join("restored-profile");
    let mut dest_db = Database::open(&dest_root).unwrap();
    let local_quota = crate::storage_accounting::MIN_PROFILE_PHYSICAL_QUOTA_BYTES;
    crate::storage_accounting::set_physical_quota_bytes(&dest_db, local_quota).unwrap();

    let restored = crate::profile_backup::restore(&mut dest_db, &mut archive_file).unwrap();
    assert_eq!(restored.games_restored, 1);
    assert_eq!(restored.saves_restored, 3);
    assert_eq!(restored.saves_failed, 0);
    // Destination physical quota must NEVER be raised from source archive metadata
    assert_eq!(
        crate::storage_accounting::get_physical_quota_bytes(&dest_db).unwrap(),
        local_quota
    );

    let games = Library::new(&dest_db).list(None, 0, 50).unwrap();
    assert_eq!(games.len(), 1);
    assert_eq!(games[0].content_hash, source.game.content_hash);

    for (orig_id, expected_bytes) in [
        (save_a.id, b"save-alpha-payload".as_slice()),
        (save_b.id, b"save-beta-payload-longer".as_slice()),
        (save_c.id, b"save-gamma-final-payload".as_slice()),
    ] {
        let restored_id = crate::profile_backup::derived_profile_save_id(&restored.archive_hash, orig_id, 0);
        let (_, payload) = SaveStore::new(&mut dest_db).read(restored_id, None).unwrap();
        assert_eq!(payload, expected_bytes);
    }

    let retry = crate::profile_backup::restore(&mut dest_db, &mut archive_file).unwrap();
    assert_eq!(retry.saves_already_present, 3);
    assert_eq!(retry.saves_restored, 0);
}

#[test]
fn corrupt_database_diagnosis_quarantine_and_interrupted_journal_reconciliation() {
    let base = std::env::temp_dir().join(format!("vlikeon-db-recover-{}", Uuid::new_v4()));
    let _guard = Directory(base.clone());
    fs::create_dir_all(&base).unwrap();
    let rom_path = base.join("cart.nes");
    let mut nes_rom = vec![0u8; 16 + 16384];
    nes_rom[..4].copy_from_slice(b"NES\x1a");
    nes_rom[4] = 1;
    fs::write(&rom_path, &nes_rom).unwrap();

    let profile_root = base.join("profile");
    let expected_hash = {
        let mut db = Database::open(&profile_root).unwrap();
        let game = Library::new(&db)
            .import(File::open(&rom_path).unwrap(), "Recoverable NES".into(), GameSystem::Nes)
            .unwrap();
        let metadata = SaveMetadata {
            schema_version: 1,
            id: Uuid::new_v4(),
            game_id: game.id,
            game_content_hash: game.content_hash.clone(),
            kind: SaveKind::InGame,
            slot: 0,
            generation: 1,
            parent_id: None,
            core_id: "fceumm-web".into(),
            core_version: "1.0.0".parse().unwrap(),
            state_format_version: "1".into(),
            platform: "web".into(),
            core_config_hash: ContentHash("b".repeat(64)),
            payload_hash: digest(b"sram-bytes"),
            payload_bytes: 10,
            created_at_ms: crate::now_ms(),
            device_id: db.device_id,
            label: None,
            conflict: false,
        };
        SaveStore::new(&mut db).commit(metadata, b"sram-bytes", None).unwrap();
        game.content_hash
    };

    fs::write(profile_root.join("omniplay.sqlite3"), b"corrupt-sqlite-header").unwrap();
    let health = crate::database_recovery::diagnose(&profile_root).unwrap();
    assert_eq!(
        health.status,
        crate::database_recovery::DatabaseHealthStatus::HeaderCorrupt
    );
    assert!(health.recoverable);

    let outcome = crate::database_recovery::recover(&profile_root).unwrap();
    let qdir = profile_root
        .join("corrupt-db-quarantine")
        .join(&outcome.quarantine_id);
    assert!(qdir.join("quarantine-receipt.json").is_file());
    assert!(qdir.join("salvaged-snapshot.json").is_file());
    assert!(!profile_root
        .join(crate::database_recovery::RECOVERY_JOURNAL_FILE)
        .exists());
    assert!(outcome.salvaged_games + outcome.reconstructed_games >= 1);

    let reopened = Database::open(&profile_root).unwrap();
    let games = Library::new(&reopened).list(None, 0, 50).unwrap();
    assert_eq!(games.len(), 1);
    assert_eq!(games[0].content_hash, expected_hash);
}

#[test]
fn library_reconstruct_rebuilds_normalized_cue_and_m3u_disc_sets() {
    let base = std::env::temp_dir().join(format!("vlikeon-reconstruct-{}", Uuid::new_v4()));
    let _guard = Directory(base.clone());
    for (case, entry, disc_count) in [("cue", "Disc A.cue", 1), ("m3u", "Collection.m3u", 2)] {
        let case_root = base.join(case);
        let selected = case_root.join("selected");
        fs::create_dir_all(&selected).unwrap();
        for (disc, value) in [("A", 0xAAu8), ("B", 0xBBu8)].into_iter().take(disc_count) {
            fs::write(selected.join(format!("Original {disc}.bin")), vec![value; 2352 * 4]).unwrap();
            fs::write(
                selected.join(format!("Disc {disc}.cue")),
                format!("FILE \"Original {disc}.bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n"),
            ).unwrap();
        }
        if disc_count == 2 {
            fs::write(selected.join(entry), b"Disc A.cue\nDisc B.cue\n").unwrap();
        }

        // Use the public importer so recovery consumes actual normalized producer output.
        let imported_db = Database::open(&case_root.join("imported-profile")).unwrap();
        let original = crate::media_import::import_bundle(
            &imported_db,
            crate::media_import::DiscScope::selected(&selected).unwrap(),
            entry,
            "Original PS1 Set".into(),
            GameSystem::Ps1,
            omniplay_contracts::CueEncoding::Utf8,
            &std::sync::atomic::AtomicBool::new(false),
            |_| {},
        ).unwrap();
        let expected_media = original.media.as_ref().unwrap();
        assert_eq!(expected_media.disc_entries.len(), disc_count);
        assert_eq!(expected_media.assets.len(), if disc_count == 1 { 2 } else { 5 });
        assert_eq!(expected_media.entry_name, format!("game.{case}"));
        assert!(expected_media.assets.iter().any(|asset| asset.name == "track-0001.bin"));
        if disc_count == 2 {
            assert_eq!(expected_media.disc_entries, ["disc-01.cue", "disc-02.cue"]);
            assert!(expected_media.assets.iter().any(|asset| asset.name == "track-0002.bin"));
        }

        // Model metadata loss by copying only immutable content-addressed blobs into an empty profile.
        let db = Database::open(&case_root.join("recovered-profile")).unwrap();
        let assets_dir = db.root.join("assets");
        fs::create_dir_all(&assets_dir).unwrap();
        let mut expected_blobs = std::collections::BTreeMap::new();
        let mut bindings = std::collections::BTreeMap::new();
        for asset in &expected_media.assets {
            let bytes = fs::read(imported_db.root.join("assets").join(&asset.content_hash.0)).unwrap();
            fs::write(assets_dir.join(&asset.content_hash.0), &bytes).unwrap();
            expected_blobs.insert(asset.content_hash.0.clone(), bytes);
            if asset.name != expected_media.entry_name {
                bindings.insert(asset.name.clone(), asset.content_hash.clone());
            }
        }
        assert!(Library::new(&db).list(None, 0, 50).unwrap().is_empty());
        let preview = crate::library_reconstruct::preview(&db).unwrap();
        let cand = preview.candidates.iter()
            .find(|candidate| candidate.entry_hash == expected_media.entry().unwrap().content_hash)
            .unwrap_or_else(|| panic!("imported {case} entry should be discovered in assets/"));
        let manifest = crate::library_reconstruct::reconstruct(
            &db,
            &cand.candidate_id,
            &cand.revision,
            Some("Rebuilt PS1 Set".into()),
            Some(GameSystem::Ps1),
            Some(bindings),
        ).unwrap();
        assert_eq!(manifest.title, "Rebuilt PS1 Set");
        assert_eq!(manifest.system, GameSystem::Ps1);
        assert_eq!(manifest.content_hash, original.content_hash, "{case} media identity");
        assert_eq!(manifest.byte_length, original.byte_length);
        assert_eq!(serde_json::to_value(manifest.media.as_ref().unwrap()).unwrap(),
            serde_json::to_value(expected_media).unwrap(), "{case} asset names, hashes, lengths and disc order");
        assert_eq!(Library::new(&db).list(None, 0, 50).unwrap().len(), 1);
        assert_eq!(fs::read_dir(&assets_dir).unwrap().count(), expected_blobs.len());
        for (hash, bytes) in expected_blobs {
            assert_eq!(fs::read(assets_dir.join(hash)).unwrap(), bytes, "{case} payload must remain unchanged");
        }
    }
}

#[test]
fn storage_accounting_tracks_active_retainers_hardlinks_and_enforces_physical_quota() {
    let mut f = Fixture::new();
    let save = f.save(b"retained-save");
    let live_file_id: String = f
        .database
        .connection
        .query_row(
            "SELECT file_id FROM save_payload_locations WHERE save_id=?1",
            [save.id.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    let repaired_dir = f.database.root.join("repaired-saves-v1");
    let hardlink_alias = repaired_dir.join(format!("{}-hardlink.bin", Uuid::new_v4()));
    let _ = fs::hard_link(repaired_dir.join(format!("{live_file_id}.bin")), &hardlink_alias);

    let report = crate::storage_accounting::report(&f.database).unwrap();
    assert!(report.logical_bytes > 0);
    assert!(report.retainer_counts.library_games >= 1);
    assert!(report.retainer_counts.live_saves >= 1);
    if hardlink_alias.exists() {
        assert!(report.hardlink_deduped_files >= 1);
    }

    let min_quota = crate::storage_accounting::MIN_PROFILE_PHYSICAL_QUOTA_BYTES;
    crate::storage_accounting::set_physical_quota_bytes(&f.database, min_quota).unwrap();
    assert_eq!(
        crate::storage_accounting::get_physical_quota_bytes(&f.database).unwrap(),
        min_quota
    );
    assert!(
        crate::storage_accounting::enforce_physical_quota(&f.database, min_quota + 1, 1).is_err()
    );
}

// ---- Round-2 review fixtures (authored, not executed) ----

fn export_profile_archive(f: &mut Fixture, name: &str) -> (PathBuf, crate::profile_backup::ProfileBackupExported) {
    let path = f._directory.0.join(name);
    let mut file = OpenOptions::new().read(true).write(true).create_new(true).open(&path).unwrap();
    let exported = crate::profile_backup::export(&mut f.database, true, &mut file).unwrap();
    (path, exported)
}

#[test]
fn profile_restore_broker_inspect_confirm_restore_cancel_and_archive_swap() {
    use crate::{permissions::{FileAccess, PermissionBroker}, profile_backup::{ProfileRestoreBroker, restore_verified}};
    use omniplay_contracts::ErrorCode;
    let mut src = Fixture::new();
    src.save(b"restore-token-save");
    let (archive, _) = export_profile_archive(&mut src, "token.opprofile");
    let owner = Uuid::new_v4();
    let mut grants = PermissionBroker::default();
    let mut broker = ProfileRestoreBroker::default();

    // Inspect consumes the single-use grant; restore must use the scoped token instead.
    let grant = grants.grant_selected_file(owner, &archive, FileAccess::ProfileBackupRead, 60_000).unwrap();
    // An ImportRead consumer cannot take a ProfileBackupRead grant.
    assert_eq!(grants.take(owner, grant, FileAccess::ImportRead).err().unwrap().code, ErrorCode::PermissionDenied);
    let file = grants.take(owner, grant, FileAccess::ProfileBackupRead).unwrap();
    assert_eq!(grants.take(owner, grant, FileAccess::ProfileBackupRead).err().unwrap().code, ErrorCode::PermissionExpired);
    let summary = broker.inspect_and_stage(owner, file, 60_000).unwrap();
    let token = summary.restore_token.unwrap();

    // Wrong owner is refused; cancel invalidates; a cancelled token cannot restore.
    assert_eq!(broker.take_for_restore(Uuid::new_v4(), token, &summary.archive_hash).err().unwrap().code, ErrorCode::PermissionDenied);
    assert!(broker.cancel(owner, token).unwrap());
    assert_eq!(broker.take_for_restore(owner, token, &summary.archive_hash).err().unwrap().code, ErrorCode::PermissionExpired);

    // Wrong expected hash fails closed before any local write.
    let g2 = grants.grant_selected_file(owner, &archive, FileAccess::ProfileBackupRead, 60_000).unwrap();
    let s2 = broker.inspect_and_stage(owner, grants.take(owner, g2, FileAccess::ProfileBackupRead).unwrap(), 60_000).unwrap();
    let wrong = ContentHash("0".repeat(64));
    assert_eq!(broker.take_for_restore(owner, s2.restore_token.unwrap(), &wrong).err().unwrap().code, ErrorCode::ContentMismatch);

    // Valid restore succeeds exactly once.
    let dest_root = src._directory.0.join("restored-token");
    let mut dest = Database::open(&dest_root).unwrap();
    let g3 = grants.grant_selected_file(owner, &archive, FileAccess::ProfileBackupRead, 60_000).unwrap();
    let s3 = broker.inspect_and_stage(owner, grants.take(owner, g3, FileAccess::ProfileBackupRead).unwrap(), 60_000).unwrap();
    let t3 = s3.restore_token.unwrap();
    let mut staged = broker.take_for_restore(owner, t3, &s3.archive_hash).unwrap();
    let restored = restore_verified(&mut dest, &mut staged, Some(&s3.archive_hash)).unwrap();
    assert_eq!(restored.games_restored, 1);
    assert_eq!(broker.take_for_restore(owner, t3, &s3.archive_hash).err().unwrap().code, ErrorCode::PermissionExpired);

    // Archive mutated (same length) after inspection: full-stream hash check rejects before writes.
    let g4 = grants.grant_selected_file(owner, &archive, FileAccess::ProfileBackupRead, 60_000).unwrap();
    let s4 = broker.inspect_and_stage(owner, grants.take(owner, g4, FileAccess::ProfileBackupRead).unwrap(), 60_000).unwrap();
    {
        let mut w = OpenOptions::new().read(true).write(true).open(&archive).unwrap();
        w.seek(SeekFrom::Start(100)).unwrap();
        let mut b = [0u8; 1];
        w.read_exact(&mut b).unwrap();
        w.seek(SeekFrom::Start(100)).unwrap();
        std::io::Write::write_all(&mut w, &[b[0] ^ 0xFF]).unwrap();
    }
    let other_root = src._directory.0.join("restored-swapped");
    let mut other = Database::open(&other_root).unwrap();
    let mut swapped = broker.take_for_restore(owner, s4.restore_token.unwrap(), &s4.archive_hash).unwrap();
    assert!(restore_verified(&mut other, &mut swapped, Some(&s4.archive_hash)).is_err());
    assert!(Library::new(&other).list(None, 0, 10).unwrap().is_empty());
}

#[test]
fn chunked_restore_staging_over_256kib_survives_reopen_keeps_other_archives_and_fails_closed() {
    use omniplay_contracts::ErrorCode;
    let base = std::env::temp_dir().join(format!("vlikeon-staging-{}", Uuid::new_v4()));
    let _guard = Directory(base.clone());
    let root = base.join("profile");
    let archive_a = ContentHash("a".repeat(64));
    let archive_b = ContentHash("b".repeat(64));
    let hashes: Vec<ContentHash> = (0..4200u32).map(|i| digest(format!("asset-{i}").as_bytes())).collect();
    {
        let db = Database::open(&root).unwrap();
        fs::create_dir_all(root.join("assets")).unwrap();
        for h in &hashes { fs::write(root.join("assets").join(&h.0), b"x").unwrap(); }
        crate::profile_backup::persist_restore_staging_retainers(&db, &archive_a, &hashes).unwrap();
        crate::profile_backup::persist_restore_staging_retainers(&db, &archive_b, &hashes[..3]).unwrap();
    }
    // Simulated restart before manifest publication.
    let db = Database::open(&root).unwrap();
    let pinned = crate::storage_accounting::transient_asset_retainers(&db).unwrap();
    for h in &hashes { assert!(pinned.contains(&h.0)); }
    let preview = crate::asset_storage::preview(&db, None).unwrap();
    assert_eq!(preview.unused_assets, 0);
    // Corrupt one chunk row: accounting and cleanup fail closed rather than dropping pins.
    db.connection.execute(
        "UPDATE preferences SET value='{bad' WHERE key=?1",
        [format!("{}{}:chunk:0000", crate::profile_backup::PROFILE_RESTORE_STAGING_PREFIX, archive_a.0)],
    ).unwrap();
    assert_eq!(crate::storage_accounting::transient_asset_retainers(&db).err().unwrap().code, ErrorCode::CorruptData);
    assert_eq!(crate::asset_storage::preview(&db, None).err().unwrap().code, ErrorCode::CorruptData);
}

#[test]
fn storage_cluster_rounding_uses_volume_cluster_size() {
    assert_eq!(crate::storage_accounting::round_to_cluster_bytes(1, 65_536), 65_536);
    assert_eq!(crate::storage_accounting::round_to_cluster_bytes(65_537, 65_536), 131_072);
    assert_eq!(crate::storage_accounting::round_to_cluster_bytes(4_097, 4_096), 8_192);
}

fn make_corrupt_profile(base: &PathBuf) -> (PathBuf, ContentHash, PathBuf) {
    let rom_path = base.join("cart.nes");
    let mut rom = vec![0u8; 16 + 16384];
    rom[..4].copy_from_slice(b"NES\x1a");
    rom[4] = 1;
    fs::write(&rom_path, &rom).unwrap();
    let root = base.join("profile");
    let (hash, payload_path) = {
        let mut db = Database::open(&root).unwrap();
        let game = Library::new(&db).import(File::open(&rom_path).unwrap(), "Fault NES".into(), GameSystem::Nes).unwrap();
        let metadata = SaveMetadata {
            schema_version: 1, id: Uuid::new_v4(), game_id: game.id, game_content_hash: game.content_hash.clone(),
            kind: SaveKind::InGame, slot: 0, generation: 1, parent_id: None, core_id: "fceumm-web".into(),
            core_version: "1.0.0".parse().unwrap(), state_format_version: "1".into(), platform: "web".into(),
            core_config_hash: ContentHash("b".repeat(64)), payload_hash: digest(b"sram-bytes"), payload_bytes: 10,
            created_at_ms: crate::now_ms(), device_id: db.device_id, label: None, conflict: false,
        };
        let saved = SaveStore::new(&mut db).commit(metadata, b"sram-bytes", None).unwrap();
        let file_id: String = db.connection.query_row(
            "SELECT file_id FROM save_payload_locations WHERE save_id=?1",
            [saved.id.to_string()], |row| row.get(0),
        ).unwrap();
        let payload_path = root.join("repaired-saves-v1").join(format!("{file_id}.bin"));
        assert_eq!(fs::read(&payload_path).unwrap(), b"sram-bytes");
        (game.content_hash, payload_path)
    };
    fs::write(root.join("omniplay.sqlite3"), b"corrupt-sqlite-header").unwrap();
    fs::write(root.join("omniplay.sqlite3-wal"), b"stale-wal").unwrap();
    (root, hash, payload_path)
}

#[test]
fn corrupt_database_recovery_resumes_after_every_injected_interruption() {
    use crate::database_recovery::{RecoveryFaultPoint, with_recovery_fault_once, RECOVERY_JOURNAL_FILE};
    for point in [
        RecoveryFaultPoint::AfterInitialJournalWrite,
        RecoveryFaultPoint::BetweenQuarantineRenames,
        RecoveryFaultPoint::MidStagingRebuild,
        RecoveryFaultPoint::BeforePublishRename,
        RecoveryFaultPoint::AfterPublishRenameBeforeJournalRemove,
    ] {
        let base = std::env::temp_dir().join(format!("vlikeon-fault-{}", Uuid::new_v4()));
        let _guard = Directory(base.clone());
        fs::create_dir_all(&base).unwrap();
        let (root, hash, orphan_payload) = make_corrupt_profile(&base);
        let interrupted = with_recovery_fault_once(point, || crate::database_recovery::recover(&root).map(|_| ()));
        assert!(interrupted.is_err(), "fault {point:?} must interrupt recovery");
        assert_eq!(interrupted.unwrap_err().message, "Simulated interruption at recovery journal fault point");
        assert!(root.join(RECOVERY_JOURNAL_FILE).exists(), "journal must persist after {point:?}");

        // Restart: startup reconciliation resumes and publishes a healthy database.
        // The whole SQLite file is unreadable, so only data recoverable from content-addressed
        // assets can return: the game is reconstructed from `assets/`. Save rows and heads lived
        // only in the destroyed metadata DB and are NOT expected to survive; the damaged files must
        // instead be preserved byte-for-byte in quarantine.
        let reopened = Database::open(&root).unwrap();
        assert!(!root.join(RECOVERY_JOURNAL_FILE).exists());
        let games = Library::new(&reopened).list(None, 0, 50).unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].content_hash, hash);
        let counts: (u64, u64) = reopened.connection.query_row(
            "SELECT (SELECT count(*) FROM saves), (SELECT count(*) FROM save_heads)",
            [], |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap();
        assert_eq!(counts, (0, 0), "lost metadata must not be invented");
        assert_eq!(fs::read(&orphan_payload).unwrap(), b"sram-bytes");
        let qroot = root.join("corrupt-db-quarantine");
        let qdirs: Vec<_> = fs::read_dir(&qroot).unwrap().flatten()
            .filter(|e| e.path().join("quarantine-receipt.json").is_file()).collect();
        assert_eq!(qdirs.len(), 1);
        assert_eq!(fs::read(qdirs[0].path().join("omniplay.sqlite3")).unwrap(), b"corrupt-sqlite-header");
        assert_eq!(fs::read(qdirs[0].path().join("omniplay.sqlite3-wal")).unwrap(), b"stale-wal");
        drop(reopened);
        // Idempotent: a second open changes nothing.
        let again = Database::open(&root).unwrap();
        let again_games = Library::new(&again).list(None, 0, 50).unwrap();
        assert_eq!(again_games.len(), 1);
        assert_eq!(again_games[0].id, games[0].id);
        assert_eq!(again_games[0].content_hash, hash);
        let again_counts: (u64, u64) = again.connection.query_row(
            "SELECT (SELECT count(*) FROM saves), (SELECT count(*) FROM save_heads)",
            [], |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap();
        assert_eq!(again_counts, (0, 0));
        assert_eq!(fs::read(&orphan_payload).unwrap(), b"sram-bytes");
        assert!(!root.join(RECOVERY_JOURNAL_FILE).exists());
        assert_eq!(fs::read_dir(&qroot).unwrap().count(), 1);
        assert_eq!(fs::read(qdirs[0].path().join("omniplay.sqlite3")).unwrap(), b"corrupt-sqlite-header");
        assert_eq!(fs::read(qdirs[0].path().join("omniplay.sqlite3-wal")).unwrap(), b"stale-wal");
    }
}

#[test]
fn salvaged_save_heads_recover_after_every_injected_interruption() {
    use crate::database_recovery::{DatabaseHealthStatus, RecoveryFaultPoint, with_recovery_fault_once, RECOVERY_JOURNAL_FILE};
    use omniplay_contracts::SaveOriginKind;

    for point in [
        RecoveryFaultPoint::AfterInitialJournalWrite,
        RecoveryFaultPoint::BetweenQuarantineRenames,
        RecoveryFaultPoint::MidStagingRebuild,
        RecoveryFaultPoint::BeforePublishRename,
        RecoveryFaultPoint::AfterPublishRenameBeforeJournalRemove,
    ] {
        let mut fixture = Fixture::new();
        let first_payload = b"first-checkpoint".as_slice();
        let second_payload = b"second-checkpoint".as_slice();
        let first = fixture.save_in_slot(first_payload, 3, None);
        let second = fixture.save_in_slot(second_payload, 3, Some(first.id));
        assert_eq!(first.generation, 1);
        assert_eq!(second.generation, 2);
        assert_eq!(second.parent_id, Some(first.id));
        assert!(!first.conflict && !second.conflict);
        // Deliberately select the older generation; latest-generation guessing must fail.
        SaveStore::new(&mut fixture.database).promote(first.id, Some(second.id)).unwrap();

        let root = fixture.database.root.clone();
        let game_hash = fixture.game.content_hash.clone();
        let device_id = fixture.database.device_id;
        // Hold the temporary-directory guard through both reopens, but close SQLite before quarantine.
        let Fixture { database, _directory: _guard, game: _ } = fixture;
        drop(database);
        assert_eq!(crate::database_recovery::diagnose(&root).unwrap().status, DatabaseHealthStatus::Healthy);
        {
            // An isolated foreign-key violation makes diagnosis fail while valid save/head rows
            // remain readable. Production recovery, including salvage and snapshot writing, runs unchanged.
            let damaged = rusqlite::Connection::open(root.join("omniplay.sqlite3")).unwrap();
            damaged.execute_batch("PRAGMA foreign_keys=OFF; PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE;").unwrap();
            damaged.execute(
                "INSERT INTO sessions(id,game_id,runtime_id,state,info_json,created_at_ms,updated_at_ms) VALUES(?1,?2,'fixture','stopped','{}',1,1)",
                rusqlite::params![Uuid::new_v4().to_string(), Uuid::nil().to_string()],
            ).unwrap();
        }
        let health = crate::database_recovery::diagnose(&root).unwrap();
        assert_eq!(health.status, DatabaseHealthStatus::IntegrityFailed);
        assert!(health.findings.iter().any(|finding| finding.contains("foreign_key_check violation in table sessions")));
        let damaged_db_bytes = fs::read(root.join("omniplay.sqlite3")).unwrap();

        let interrupted = with_recovery_fault_once(point, || crate::database_recovery::recover(&root).map(|_| ()));
        assert!(interrupted.is_err(), "fault {point:?} must interrupt salvage recovery");
        assert_eq!(interrupted.unwrap_err().message, "Simulated interruption at recovery journal fault point");
        assert!(root.join(RECOVERY_JOURNAL_FILE).is_file());
        let quarantine_root = root.join("corrupt-db-quarantine");
        let quarantine_dirs: Vec<_> = fs::read_dir(&quarantine_root).unwrap()
            .map(|entry| entry.unwrap().path()).collect();
        assert_eq!(quarantine_dirs.len(), 1);
        let quarantine = &quarantine_dirs[0];
        let quarantine_id = quarantine.file_name().unwrap().to_str().unwrap();
        let snapshot_bytes = fs::read(quarantine.join("salvaged-snapshot.json")).unwrap();
        let snapshot: serde_json::Value = serde_json::from_slice(&snapshot_bytes).unwrap();
        assert_eq!(snapshot["head_save_ids"], serde_json::json!([first.id]));
        let snapshot_saves = snapshot["saves"].as_array().unwrap();
        assert_eq!(snapshot_saves.len(), 2, "fault {point:?} must have real salvaged save metadata");
        for original in [&first, &second] {
            assert!(snapshot_saves.iter().any(|save| save["metadata"] == serde_json::to_value(original).unwrap()));
        }

        let expected = [(first.clone(), first_payload), (second.clone(), second_payload)];
        let verify = |database: &mut Database| {
            assert_eq!(database.device_id, device_id);
            let games = Library::new(database).list(None, 0, 50).unwrap();
            assert_eq!(games.len(), 1);
            assert_eq!(games[0].content_hash, game_hash);
            let counts: (u64, u64, u64) = database.connection.query_row(
                "SELECT (SELECT count(*) FROM saves), (SELECT count(*) FROM save_heads), (SELECT count(*) FROM save_origins)",
                [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            ).unwrap();
            assert_eq!(counts, (2, 1, 2));
            let recovered = SaveStore::new(database).list(games[0].id).unwrap();
            assert_eq!(recovered.len(), 2);
            let heads = SaveStore::new(database).heads(games[0].id).unwrap();
            assert_eq!(heads.len(), 1);
            let mut seen_originals = std::collections::BTreeSet::new();
            let mut state = std::collections::BTreeMap::new();
            for local in recovered {
                let (_, origin) = crate::save_origins::inspect(&database.connection, local.id).unwrap();
                assert_eq!(origin.kind, SaveOriginKind::Imported);
                let imported = origin.imported_from.as_ref().unwrap();
                let (original, payload) = expected.iter().find(|(save, _)| save.id == imported.id).unwrap();
                assert!(seen_originals.insert(original.id), "each source generation is restored once");
                assert_eq!(serde_json::to_value(imported).unwrap(), serde_json::to_value(original).unwrap());
                assert!(!expected.iter().any(|(save, _)| save.id == local.id));
                assert_eq!(local.game_id, games[0].id);
                assert_eq!(local.game_content_hash, game_hash);
                assert_eq!(local.kind, original.kind);
                assert_eq!(local.slot, original.slot);
                assert_eq!(local.generation, original.generation);
                // Local imports are independent branches; original lineage remains in imported_from.
                assert_eq!(local.parent_id, None);
                assert_eq!(local.conflict, original.id != first.id);
                let mut expected_canonical = local.clone();
                expected_canonical.conflict = true;
                assert_eq!(serde_json::to_value(origin.canonical_metadata.as_ref().unwrap()).unwrap(), serde_json::to_value(&expected_canonical).unwrap());
                assert_eq!(origin.imported_from_hash, Some(omniplay_contracts::save_origin_hash(original).unwrap()));
                let (_, actual_payload) = SaveStore::new(database).read(local.id, None).unwrap();
                assert_eq!(actual_payload.as_slice(), *payload);
                assert_eq!(digest(&actual_payload), original.payload_hash);
                if original.id == first.id {
                    assert_eq!(heads[0].id, local.id, "selected older head must survive recovery");
                    assert_eq!(serde_json::to_value(&heads[0]).unwrap(), serde_json::to_value(&local).unwrap());
                }
                let reference: String = database.connection.query_row(
                    "SELECT value FROM preferences WHERE key=?1", [format!("backup_reference.{}", local.id)], |row| row.get(0),
                ).unwrap();
                let reference: serde_json::Value = serde_json::from_str(&reference).unwrap();
                assert_eq!(reference, serde_json::json!({"original_save_id": original.id, "salvaged_from_quarantine": quarantine_id}));
                state.insert(local.id, serde_json::json!({"metadata": local, "origin": origin, "reference": reference}));
            }
            assert_eq!(seen_originals, std::collections::BTreeSet::from([first.id, second.id]));
            serde_json::json!({"game_id": games[0].id, "saves": state, "heads": heads})
        };

        let mut reopened = Database::open(&root).unwrap();
        let recovered_state = verify(&mut reopened);
        assert!(!root.join(RECOVERY_JOURNAL_FILE).exists());
        assert_eq!(crate::database_recovery::diagnose(&root).unwrap().status, DatabaseHealthStatus::Healthy);
        assert!(quarantine.join("quarantine-receipt.json").is_file());
        assert_eq!(fs::read(quarantine.join("omniplay.sqlite3")).unwrap(), damaged_db_bytes);
        assert_eq!(fs::read(quarantine.join("salvaged-snapshot.json")).unwrap(), snapshot_bytes);
        drop(reopened);

        let mut again = Database::open(&root).unwrap();
        assert_eq!(verify(&mut again), recovered_state, "reopen must keep the same identities, origins, payloads and head");
        assert!(!root.join(RECOVERY_JOURNAL_FILE).exists());
        assert_eq!(fs::read_dir(&quarantine_root).unwrap().count(), 1);
        assert_eq!(fs::read(quarantine.join("omniplay.sqlite3")).unwrap(), damaged_db_bytes);
        assert_eq!(fs::read(quarantine.join("salvaged-snapshot.json")).unwrap(), snapshot_bytes);
    }
}

#[test]
fn interrupted_recovery_fails_closed_on_missing_or_tampered_snapshot_and_journal() {
    use crate::database_recovery::{RecoveryFaultPoint, with_recovery_fault_once, RECOVERY_JOURNAL_FILE};
    use omniplay_contracts::ErrorCode;
    let setup = |label: &str| {
        let base = std::env::temp_dir().join(format!("vlikeon-{label}-{}", Uuid::new_v4()));
        fs::create_dir_all(&base).unwrap();
        let (root, _, _) = make_corrupt_profile(&base);
        let _ = with_recovery_fault_once(RecoveryFaultPoint::AfterInitialJournalWrite, || crate::database_recovery::recover(&root).map(|_| ()));
        (base, root)
    };
    // Tampered snapshot checksum.
    let (base, root) = setup("snap");
    let _g = Directory(base.clone());
    let qdir = fs::read_dir(root.join("corrupt-db-quarantine")).unwrap().flatten().next().unwrap().path();
    fs::write(qdir.join("salvaged-snapshot.json"), b"{}").unwrap();
    assert_eq!(Database::open(&root).err().unwrap().code, ErrorCode::CorruptData);
    // Missing snapshot.
    let (base2, root2) = setup("nosnap");
    let _g2 = Directory(base2.clone());
    let q2 = fs::read_dir(root2.join("corrupt-db-quarantine")).unwrap().flatten().next().unwrap().path();
    fs::remove_file(q2.join("salvaged-snapshot.json")).unwrap();
    assert_eq!(Database::open(&root2).err().unwrap().code, ErrorCode::CorruptData);
    // Tampered journal checksum while the live DB is still corrupt: fail closed, no blank DB.
    let (base3, root3) = setup("jrnl");
    let _g3 = Directory(base3.clone());
    let jpath = root3.join(RECOVERY_JOURNAL_FILE);
    let text = fs::read_to_string(&jpath).unwrap().replace("\"journal_sha256\"", "\"journal_sha256\": \"") ;
    fs::write(&jpath, text).unwrap();
    assert!(Database::open(&root3).is_err());
}

#[test]
fn staging_meta_with_inflated_counts_fails_closed_without_allocating() {
    use omniplay_contracts::ErrorCode;
    let base = std::env::temp_dir().join(format!("vlikeon-inflated-{}", Uuid::new_v4()));
    let _guard = Directory(base.clone());
    let db = Database::open(&base.join("profile")).unwrap();
    let archive = ContentHash("c".repeat(64));
    let hashes: Vec<ContentHash> = (0..3u32).map(|i| digest(format!("h-{i}").as_bytes())).collect();
    crate::profile_backup::persist_restore_staging_retainers(&db, &archive, &hashes).unwrap();
    let key = format!("{}{}:meta", crate::profile_backup::PROFILE_RESTORE_STAGING_PREFIX, archive.0);
    let meta = format!(
        "{{\"schema_version\":1,\"archive_hash\":\"{}\",\"started_at_ms\":1,\"chunk_count\":1,\"total_hashes\":4294967295,\"asset_hashes\":[]}}",
        archive.0
    );
    db.connection.execute("UPDATE preferences SET value=?1 WHERE key=?2", rusqlite::params![meta, key]).unwrap();
    assert_eq!(crate::profile_backup::read_restore_staging_retainers(&db).err().unwrap().code, ErrorCode::CorruptData);
}

#[test]
fn subprocess_hard_crash_recovery_worker() {
    let Ok(profile_dir) = std::env::var("OMNIPLAY_TEST_RECOVERY_HARD_CRASH_DIR") else {
        return;
    };
    let _ = crate::database_recovery::recover(std::path::Path::new(&profile_dir));
}

#[test]
fn corrupt_database_recovery_survives_abrupt_subprocess_kill_and_os_storage_lock() {
    use crate::database_recovery::{DatabaseHealthStatus, RecoveryFaultPoint, RECOVERY_JOURNAL_FILE, STAGING_DB_FILE};
    use omniplay_contracts::ErrorCode;

    // 1. Real OS process abrupt exit (`std::process::exit(99)` with zero stack unwinding or Drop cleanup)
    // across all 5 recovery journal fault points, preserving salvaged save generations and selected head.
    let current_exe = std::env::current_exe().unwrap();
    for point in [
        RecoveryFaultPoint::AfterInitialJournalWrite,
        RecoveryFaultPoint::BetweenQuarantineRenames,
        RecoveryFaultPoint::MidStagingRebuild,
        RecoveryFaultPoint::BeforePublishRename,
        RecoveryFaultPoint::AfterPublishRenameBeforeJournalRemove,
    ] {
        let mut fixture = Fixture::new();
        let first = fixture.save_in_slot(b"hard-crash-gen1", 2, None);
        let second = fixture.save_in_slot(b"hard-crash-gen2", 2, Some(first.id));
        SaveStore::new(&mut fixture.database).promote(first.id, Some(second.id)).unwrap();
        let root = fixture.database.root.clone();
        let game_hash = fixture.game.content_hash.clone();
        let Fixture { database, _directory: _guard, game: _ } = fixture;
        drop(database);

        {
            let damaged = rusqlite::Connection::open(root.join("omniplay.sqlite3")).unwrap();
            damaged.execute_batch("PRAGMA foreign_keys=OFF; PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE;").unwrap();
            damaged.execute(
                "INSERT INTO sessions(id,game_id,runtime_id,state,info_json,created_at_ms,updated_at_ms) VALUES(?1,?2,'fixture','stopped','{}',1,1)",
                rusqlite::params![Uuid::new_v4().to_string(), Uuid::nil().to_string()],
            ).unwrap();
        }
        assert_eq!(crate::database_recovery::diagnose(&root).unwrap().status, DatabaseHealthStatus::IntegrityFailed);

        let output = std::process::Command::new(&current_exe)
            .arg("--exact")
            .arg("backup_recovery_tests::subprocess_hard_crash_recovery_worker")
            .arg("--nocapture")
            .env("OMNIPLAY_TEST_RECOVERY_HARD_CRASH_DIR", &root)
            .env("OMNIPLAY_TEST_RECOVERY_HARD_CRASH_POINT", format!("{point:?}"))
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(99),
            "Child process must terminate abruptly with exit code 99 at {point:?}"
        );
        assert!(
            root.join(RECOVERY_JOURNAL_FILE).is_file(),
            "Recovery journal must exist on disk after abrupt process death at {point:?}"
        );

        let mut reopened = Database::open(&root).unwrap();
        assert!(!root.join(RECOVERY_JOURNAL_FILE).exists());
        assert_eq!(crate::database_recovery::diagnose(&root).unwrap().status, DatabaseHealthStatus::Healthy);
        let games = Library::new(&reopened).list(None, 0, 50).unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].content_hash, game_hash);
        let saves = SaveStore::new(&mut reopened).list(games[0].id).unwrap();
        assert_eq!(saves.len(), 2);
        let heads = SaveStore::new(&mut reopened).heads(games[0].id).unwrap();
        assert_eq!(heads.len(), 1);
        let (_, head_payload) = SaveStore::new(&mut reopened).read(heads[0].id, None).unwrap();
        assert_eq!(head_payload, b"hard-crash-gen1");
    }

    // 2. Real OS storage fault (exclusive sharing-violation lock on staging DB during recovery)
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        let base = std::env::temp_dir().join(format!("vlikeon-oslock-{}", Uuid::new_v4()));
        let _guard = Directory(base.clone());
        fs::create_dir_all(&base).unwrap();
        let (root, expected_hash, _) = make_corrupt_profile(&base);
        let locked_staging = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .share_mode(0)
            .open(root.join(STAGING_DB_FILE))
            .unwrap();
        let err = crate::database_recovery::recover(&root).unwrap_err();
        assert_eq!(err.code, ErrorCode::StorageFailure);
        assert!(root.join(RECOVERY_JOURNAL_FILE).is_file());
        drop(locked_staging);

        let reopened = Database::open(&root).unwrap();
        assert!(!root.join(RECOVERY_JOURNAL_FILE).exists());
        let games = Library::new(&reopened).list(None, 0, 50).unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].content_hash, expected_hash);
    }
}

#[test]
fn library_reconstruct_chd_iso_pbp_multi_disc_and_ambiguous_bindings() {
    use omniplay_contracts::{ContentFormat, CueEncoding, ErrorCode};
    use std::{collections::BTreeMap, sync::atomic::AtomicBool};

    let base = std::env::temp_dir().join(format!("vlikeon-reconstruct-formats-{}", Uuid::new_v4()));
    let _guard = Directory(base.clone());
    fs::create_dir_all(&base).unwrap();

    let make_chd = |tag: u8| -> Vec<u8> {
        let mut buf = vec![tag; 4096];
        buf[..8].copy_from_slice(b"MComprHD");
        buf
    };
    let make_pbp = |tag: u8| -> Vec<u8> {
        let mut buf = vec![tag; 4096];
        buf[..4].copy_from_slice(b"\0PBP");
        buf
    };
    let make_iso = |sys_id: &[u8], vol_id: &[u8], tag: u8| -> Vec<u8> {
        let mut buf = vec![tag; 0x8060 + 2048];
        buf[0x8000] = 0x01;
        buf[0x8001..0x8006].copy_from_slice(b"CD001");
        buf[0x8006] = 0x01;
        buf[0x8008..0x8028].fill(b' ');
        buf[0x8008..0x8008 + sys_id.len()].copy_from_slice(sys_id);
        buf[0x8028..0x8048].fill(b' ');
        buf[0x8028..0x8028 + vol_id.len()].copy_from_slice(vol_id);
        buf
    };

    // 1. Standalone CHD (PS1), ISO (PS1 & PSP), and PBP (PSP) imported via public import_bundle
    for (label, filename, system, expected_format, bytes) in [
        ("chd-ps1", "disc.chd", GameSystem::Ps1, ContentFormat::Chd, make_chd(0x11)),
        ("iso-ps1", "disc.iso", GameSystem::Ps1, ContentFormat::Iso, make_iso(b"PLAYSTATION", b"PS1_ISO_VOL", 0x22)),
        ("iso-psp", "umd.iso", GameSystem::Psp, ContentFormat::Iso, make_iso(b"PSP GAME", b"PSP_ISO_VOL", 0x33)),
        ("pbp-psp", "eboot.pbp", GameSystem::Psp, ContentFormat::Pbp, make_pbp(0x44)),
    ] {
        let case_root = base.join(label);
        let selected = case_root.join("selected");
        fs::create_dir_all(&selected).unwrap();
        fs::write(selected.join(filename), &bytes).unwrap();

        let imported_db = Database::open(&case_root.join("imported")).unwrap();
        let original = crate::media_import::import_bundle(
            &imported_db,
            crate::media_import::DiscScope::selected(&selected).unwrap(),
            filename,
            format!("Original {label}"),
            system,
            CueEncoding::Utf8,
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();

        let db = Database::open(&case_root.join("recovered")).unwrap();
        let assets_dir = db.root.join("assets");
        fs::create_dir_all(&assets_dir).unwrap();
        for asset in &original.media.as_ref().unwrap().assets {
            let b = fs::read(imported_db.root.join("assets").join(&asset.content_hash.0)).unwrap();
            fs::write(assets_dir.join(&asset.content_hash.0), b).unwrap();
        }

        let preview = crate::library_reconstruct::preview(&db).unwrap();
        assert_eq!(preview.candidates.len(), 1, "{label} must yield 1 candidate");
        let cand = &preview.candidates[0];
        assert_eq!(cand.format, expected_format);
        assert_eq!(cand.suggested_system, Some(system));
        assert!(!cand.needs_track_binding);

        let rebuilt = crate::library_reconstruct::reconstruct(
            &db,
            &cand.candidate_id,
            &cand.revision,
            Some(format!("Rebuilt {label}")),
            None,
            None,
        )
        .unwrap();
        assert_eq!(rebuilt.content_hash, original.content_hash);
        assert_eq!(rebuilt.system, system);
        assert_eq!(
            serde_json::to_value(&rebuilt.media).unwrap(),
            serde_json::to_value(&original.media).unwrap()
        );
    }

    // 2. Two-disc CHD M3U & two-disc ISO M3U: ambiguous without save hash, explicit bindings, and save-hash auto-resolution
    for (ext, disc1_bytes, disc2_bytes) in [
        ("chd", make_chd(0x51), make_chd(0x52)),
        ("iso", make_iso(b"PLAYSTATION", b"DISC1", 0x61), make_iso(b"PLAYSTATION", b"DISC2", 0x62)),
    ] {
        let case_root = base.join(format!("m3u-{ext}"));
        let selected = case_root.join("selected");
        fs::create_dir_all(&selected).unwrap();
        let d1_name = format!("Alpha.{ext}");
        let d2_name = format!("Beta.{ext}");
        fs::write(selected.join(&d1_name), &disc1_bytes).unwrap();
        fs::write(selected.join(&d2_name), &disc2_bytes).unwrap();
        fs::write(selected.join("Set.m3u"), format!("{d1_name}\n{d2_name}\n")).unwrap();

        let imported_db = Database::open(&case_root.join("imported")).unwrap();
        let original = crate::media_import::import_bundle(
            &imported_db,
            crate::media_import::DiscScope::selected(&selected).unwrap(),
            "Set.m3u",
            format!("Multi {ext}"),
            GameSystem::Ps1,
            CueEncoding::Utf8,
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
        let orig_media = original.media.as_ref().unwrap();

        // 2a. Without save hash: M3U candidate is ambiguous (needs_track_binding == true)
        let db_ambig = Database::open(&case_root.join("recovered-ambig")).unwrap();
        let assets_ambig = db_ambig.root.join("assets");
        fs::create_dir_all(&assets_ambig).unwrap();
        let mut explicit_bindings = BTreeMap::new();
        for asset in &orig_media.assets {
            let b = fs::read(imported_db.root.join("assets").join(&asset.content_hash.0)).unwrap();
            fs::write(assets_ambig.join(&asset.content_hash.0), b).unwrap();
            if asset.name != "game.m3u" {
                explicit_bindings.insert(asset.name.clone(), asset.content_hash.clone());
            }
        }
        let preview_ambig = crate::library_reconstruct::preview(&db_ambig).unwrap();
        let m3u_cand = preview_ambig
            .candidates
            .iter()
            .find(|c| c.format == ContentFormat::M3u)
            .unwrap();
        assert!(m3u_cand.needs_track_binding, "2-disc {ext} M3U without saves must require track binding");
        assert_eq!(
            crate::library_reconstruct::reconstruct(
                &db_ambig,
                &m3u_cand.candidate_id,
                &m3u_cand.revision,
                None,
                None,
                None,
            )
            .unwrap_err()
            .code,
            ErrorCode::InvalidRequest
        );
        // Supplying explicit bindings reconstructs exact identity
        let rebuilt_explicit = crate::library_reconstruct::reconstruct(
            &db_ambig,
            &m3u_cand.candidate_id,
            &m3u_cand.revision,
            Some(format!("Rebuilt {ext} M3U")),
            Some(GameSystem::Ps1),
            Some(explicit_bindings),
        )
        .unwrap();
        assert_eq!(rebuilt_explicit.content_hash, original.content_hash);
        assert_eq!(
            serde_json::to_value(&rebuilt_explicit.media).unwrap(),
            serde_json::to_value(&original.media).unwrap()
        );

        // 2b. With surviving save hash in `locally_removed_saves`: preview auto-disambiguates disc order!
        let db_saved = Database::open(&case_root.join("recovered-saved")).unwrap();
        let assets_saved = db_saved.root.join("assets");
        fs::create_dir_all(&assets_saved).unwrap();
        for asset in &orig_media.assets {
            let b = fs::read(imported_db.root.join("assets").join(&asset.content_hash.0)).unwrap();
            fs::write(assets_saved.join(&asset.content_hash.0), b).unwrap();
        }
        db_saved
            .connection
            .execute(
                "INSERT INTO locally_removed_saves(save_id, game_content_hash, removed_at_ms) VALUES(?1, ?2, 1)",
                rusqlite::params![Uuid::new_v4().to_string(), original.content_hash.0],
            )
            .unwrap();
        let preview_saved = crate::library_reconstruct::preview(&db_saved).unwrap();
        assert_eq!(preview_saved.candidates.len(), 1, "claimed discs must not appear as separate standalone games");
        let resolved_cand = &preview_saved.candidates[0];
        assert!(!resolved_cand.needs_track_binding);
        assert_eq!(resolved_cand.matched_save_content_hash, Some(original.content_hash.clone()));
        let rebuilt_auto = crate::library_reconstruct::reconstruct(
            &db_saved,
            &resolved_cand.candidate_id,
            &resolved_cand.revision,
            Some(format!("Auto {ext} M3U")),
            None,
            None,
        )
        .unwrap();
        assert_eq!(rebuilt_auto.content_hash, original.content_hash);
    }

    // 3. Multiple standalone CUE+BIN disc sets coexisting in `assets/`
    let multi_root = base.join("two-cue-sets");
    let mut imported_manifests = Vec::new();
    let db_multi = Database::open(&multi_root.join("recovered")).unwrap();
    let assets_multi = db_multi.root.join("assets");
    fs::create_dir_all(&assets_multi).unwrap();

    for (idx, tag) in [(1, 0x71u8), (2, 0x72u8)] {
        let sel = multi_root.join(format!("sel-{idx}"));
        fs::create_dir_all(&sel).unwrap();
        fs::write(sel.join(format!("Game{idx}.bin")), vec![tag; 2352 * 4]).unwrap();
        fs::write(
            sel.join(format!("Game{idx}.cue")),
            format!("TITLE \"Cue Set {idx}\"\nFILE \"Game{idx}.bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n"),
        )
        .unwrap();
        let imp_db = Database::open(&multi_root.join(format!("imp-{idx}"))).unwrap();
        let manifest = crate::media_import::import_bundle(
            &imp_db,
            crate::media_import::DiscScope::selected(&sel).unwrap(),
            &format!("Game{idx}.cue"),
            format!("Cue Set {idx}"),
            GameSystem::Ps1,
            CueEncoding::Utf8,
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
        for asset in &manifest.media.as_ref().unwrap().assets {
            let b = fs::read(imp_db.root.join("assets").join(&asset.content_hash.0)).unwrap();
            fs::write(assets_multi.join(&asset.content_hash.0), b).unwrap();
        }
        imported_manifests.push(manifest);
    }

    // Without save hashes, both CUE candidates in `assets/` are ambiguous
    let preview_both = crate::library_reconstruct::preview(&db_multi).unwrap();
    assert_eq!(preview_both.candidates.len(), 2);
    assert!(preview_both.candidates.iter().all(|c| c.needs_track_binding));
    assert!(crate::library_reconstruct::reconstruct_unambiguous_all(&db_multi).unwrap().is_empty());

    // Record a surviving save hash for Set 1: Set 1 auto-resolves, and once reconstructed, Set 2 becomes unambiguous too!
    db_multi
        .connection
        .execute(
            "INSERT INTO locally_removed_saves(save_id, game_content_hash, removed_at_ms) VALUES(?1, ?2, 1)",
            rusqlite::params![Uuid::new_v4().to_string(), imported_manifests[0].content_hash.0],
        )
        .unwrap();
    let preview_with_save = crate::library_reconstruct::preview(&db_multi).unwrap();
    let cand1 = preview_with_save
        .candidates
        .iter()
        .find(|c| c.matched_save_content_hash == Some(imported_manifests[0].content_hash.clone()))
        .unwrap();
    assert!(!cand1.needs_track_binding);
    let rebuilt1 = crate::library_reconstruct::reconstruct(&db_multi, &cand1.candidate_id, &cand1.revision, None, None, None).unwrap();
    assert_eq!(rebuilt1.content_hash, imported_manifests[0].content_hash);

    // Now only Set 2 remains unindexed in `assets/`, so it is unambiguous!
    let preview_remaining = crate::library_reconstruct::preview(&db_multi).unwrap();
    assert_eq!(preview_remaining.candidates.len(), 1);
    assert!(!preview_remaining.candidates[0].needs_track_binding);
    let rebuilt2 = crate::library_reconstruct::reconstruct(
        &db_multi,
        &preview_remaining.candidates[0].candidate_id,
        &preview_remaining.candidates[0].revision,
        None,
        None,
        None,
    )
    .unwrap();
    assert_eq!(rebuilt2.content_hash, imported_manifests[1].content_hash);
}

#[test]
fn storage_accounting_measures_windows_sparse_and_compressed_files_accurately() {
    let f = Fixture::new();
    let assets_dir = f.database.root.join("assets");
    fs::create_dir_all(&assets_dir).unwrap();

    // Create a 1 MiB sparse file and a 256 KiB NTFS-compressed file in assets/
    let sparse_bytes = vec![0u8; 1024 * 1024];
    let sparse_hash = digest(&sparse_bytes);
    let sparse_path = assets_dir.join(&sparse_hash.0);
    fs::write(&sparse_path, &sparse_bytes).unwrap();

    let comp_bytes = vec![b'Z'; 256 * 1024];
    let comp_hash = digest(&comp_bytes);
    let comp_path = assets_dir.join(&comp_hash.0);
    fs::write(&comp_path, &comp_bytes).unwrap();

    #[cfg(windows)]
    {
        let _ = std::process::Command::new("fsutil")
            .args(["sparse", "setflag"])
            .arg(&sparse_path)
            .arg("1")
            .output();
        let _ = std::process::Command::new("fsutil")
            .args(["sparse", "setrange"])
            .arg(&sparse_path)
            .args(["0", "1048576"])
            .output();
        let _ = std::process::Command::new("compact")
            .arg("/c")
            .arg(&comp_path)
            .output();

        let sparse_phys = omniplay_stream_input::windows_file_physical(&sparse_path).unwrap();
        let comp_phys = omniplay_stream_input::windows_file_physical(&comp_path).unwrap();
        assert!(
            sparse_phys.allocation_bytes < sparse_bytes.len() as u64,
            "NTFS sparse file physical allocation ({}) must be less than logical size ({})",
            sparse_phys.allocation_bytes,
            sparse_bytes.len()
        );
        assert!(
            comp_phys.allocation_bytes < comp_bytes.len() as u64,
            "NTFS compressed file physical allocation ({}) must be less than logical size ({})",
            comp_phys.allocation_bytes,
            comp_bytes.len()
        );
    }

    let report = crate::storage_accounting::report(&f.database).unwrap();
    assert_eq!(
        report.measurement_quality,
        crate::storage_accounting::StorageMeasurementQuality::ExactOsAllocation
    );
    assert!(report.volume_cluster_bytes >= 512);
    let assets_ns = report.namespaces.iter().find(|n| n.namespace == "assets").unwrap();
    assert!(assets_ns.logical_bytes >= (1024 * 1024 + 256 * 1024));
    #[cfg(windows)]
    assert!(
        assets_ns.physical_bytes < assets_ns.logical_bytes,
        "assets namespace physical bytes ({}) must reflect sparse/compressed savings vs logical bytes ({})",
        assets_ns.physical_bytes,
        assets_ns.logical_bytes
    );
}
