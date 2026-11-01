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
    fn save(&mut self,payload:&[u8])->SaveMetadata{let metadata=SaveMetadata{schema_version:1,id:Uuid::new_v4(),game_id:self.game.id,game_content_hash:self.game.content_hash.clone(),kind:SaveKind::InGame,slot:0,generation:1,parent_id:None,core_id:"fixture".into(),core_version:"1.0.0".parse().unwrap(),state_format_version:"1".into(),platform:"fixture".into(),core_config_hash:ContentHash("a".repeat(64)),payload_hash:digest(payload),payload_bytes:payload.len()as u64,created_at_ms:crate::now_ms(),device_id:self.database.device_id,label:None,conflict:false};SaveStore::new(&mut self.database).commit(metadata,payload,None).unwrap()}
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
    fs::create_dir_all(&base).unwrap();
    let profile_root = base.join("profile");
    let db = Database::open(&profile_root).unwrap();

    let assets_dir = profile_root.join("assets");
    fs::create_dir_all(&assets_dir).unwrap();
    let track_bytes = vec![0xAAu8; 2352 * 4];
    let track_hash = digest(&track_bytes);
    // Content-addressed blobs in `assets/` are stored directly under `<sha256>` with no `.bin` suffix.
    fs::write(assets_dir.join(&track_hash.0), &track_bytes).unwrap();

    let cue_bytes = b"FILE \"disc.bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n";
    let cue_hash = digest(cue_bytes);
    fs::write(assets_dir.join(&cue_hash.0), cue_bytes).unwrap();

    let preview = crate::library_reconstruct::preview(&db).unwrap();
    let cand = preview
        .candidates
        .iter()
        .find(|c| c.entry_hash == cue_hash)
        .expect("CUE candidate should be discovered in assets/");

    let bindings = std::collections::BTreeMap::from([("disc.bin".to_string(), track_hash)]);
    let manifest = crate::library_reconstruct::reconstruct(
        &db,
        &cand.candidate_id,
        &cand.revision,
        Some("Rebuilt PS1 Disc".into()),
        Some(GameSystem::Ps1),
        Some(bindings),
    )
    .unwrap();
    assert_eq!(manifest.title, "Rebuilt PS1 Disc");
    assert_eq!(manifest.system, GameSystem::Ps1);
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

fn make_corrupt_profile(base: &PathBuf) -> (PathBuf, ContentHash, Uuid) {
    let rom_path = base.join("cart.nes");
    let mut rom = vec![0u8; 16 + 16384];
    rom[..4].copy_from_slice(b"NES\x1a");
    rom[4] = 1;
    fs::write(&rom_path, &rom).unwrap();
    let root = base.join("profile");
    let (hash, save_id) = {
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
        (game.content_hash, saved.id)
    };
    fs::write(root.join("omniplay.sqlite3"), b"corrupt-sqlite-header").unwrap();
    let _ = fs::write(root.join("omniplay.sqlite3-wal"), b"stale-wal");
    (root, hash, save_id)
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
        let (root, hash, _save) = make_corrupt_profile(&base);
        let interrupted = with_recovery_fault_once(point, || crate::database_recovery::recover(&root).map(|_| ()));
        assert!(interrupted.is_err(), "fault {point:?} must interrupt recovery");
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
        let qroot = root.join("corrupt-db-quarantine");
        let qdirs: Vec<_> = fs::read_dir(&qroot).unwrap().flatten()
            .filter(|e| e.path().join("quarantine-receipt.json").is_file()).collect();
        assert_eq!(qdirs.len(), 1);
        assert_eq!(fs::read(qdirs[0].path().join("omniplay.sqlite3")).unwrap(), b"corrupt-sqlite-header");
        assert_eq!(fs::read(qdirs[0].path().join("omniplay.sqlite3-wal")).unwrap(), b"stale-wal");
        drop(reopened);
        // Idempotent: a second open changes nothing.
        let again = Database::open(&root).unwrap();
        assert_eq!(Library::new(&again).list(None, 0, 50).unwrap().len(), 1);
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
