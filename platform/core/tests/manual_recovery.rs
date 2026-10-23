use omniplay_contracts::{ContentHash, ErrorCode};
use omniplay_core::{Database, manual_recovery as recovery, util::digest};
use sha2::{Digest, Sha256};
use std::{fs::{self, File, OpenOptions}, io::{Read, Seek, SeekFrom, Write}, path::PathBuf};
use uuid::Uuid;

struct Directory(PathBuf);
impl std::ops::Deref for Directory { type Target=std::path::Path; fn deref(&self)->&Self::Target {&self.0} }
struct Fixture { database: Database, directory: Directory }
impl Fixture {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!("vlikeon-manual-recovery-{}", Uuid::new_v4()));
        fs::create_dir(&directory).unwrap();
        let database = Database::open(&directory.join("profile")).unwrap();
        fs::create_dir_all(database.root.join("assets")).unwrap();
        Self { database, directory:Directory(directory) }
    }
    fn source(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.database.root.join("assets").join(name);
        fs::write(&path, bytes).unwrap(); path
    }
    fn output(&self) -> File {
        OpenOptions::new().read(true).write(true).create_new(true)
            .open(self.directory.join(format!("{}.bin", Uuid::new_v4()))).unwrap()
    }
    fn counts(&self) -> (i64, i64, i64, i64) {
        self.database.connection.query_row("SELECT (SELECT count(*) FROM games),(SELECT count(*) FROM saves),(SELECT count(*) FROM save_heads),(SELECT count(*) FROM save_origins)", [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))).unwrap()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        // Only our generated temporary fixture; no profile or selected user data.
        let base = std::env::temp_dir().canonicalize().unwrap();
        if self.0.file_name().unwrap().to_string_lossy().starts_with("vlikeon-manual-recovery-")
            && self.0.canonicalize().is_ok_and(|path| path.parent() == Some(base.as_path())) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}
fn bytes(file: &mut File) -> Vec<u8> { file.seek(SeekFrom::Start(0)).unwrap(); let mut result=Vec::new(); file.read_to_end(&mut result).unwrap(); result }

#[test]
fn manual_recovery_unknown_names_roundtrip_without_game_metadata_or_publication() {
    let f = Fixture::new(); let payload=b"unknown progress\0\xff\x10";
    let source=f.source("unknown progress-α.bin", payload);
    let before=f.counts(); let page=recovery::list(&f.database,None).unwrap();
    assert_eq!(page.candidates.len(),1);
    let review=recovery::inspect(&f.database,&page.candidates[0].candidate_id).unwrap();
    assert_eq!(review.record.trust,"unverified"); assert_eq!(review.record.payload_hash,digest(payload));
    let mut package=f.output(); recovery::export(&f.database,&review.candidate_id,&review.revision,&mut package).unwrap();
    let mut extracted=f.output(); let result=recovery::extract(&mut package,&mut extracted).unwrap();
    assert_eq!(result.record,review.record); assert_eq!(bytes(&mut extracted),payload);
    assert_eq!(fs::read(source).unwrap(),payload); assert_eq!(f.counts(),before);
}

#[test]
fn manual_recovery_changed_same_size_bytes_refuse_stale_review_before_writes() {
    let f=Fixture::new(); let source=f.source("unknown",b"one");
    let page=recovery::list(&f.database,None).unwrap(); let id=&page.candidates[0].candidate_id;
    let review=recovery::inspect(&f.database,id).unwrap(); fs::write(&source,b"two").unwrap();
    let mut destination=f.output();
    assert_eq!(recovery::export(&f.database,id,&review.revision,&mut destination).unwrap_err().code,ErrorCode::Conflict);
    assert!(bytes(&mut destination).is_empty()); assert_eq!(fs::read(source).unwrap(),b"two");
}

#[test]
fn manual_recovery_corrupt_truncated_and_trailing_packages_write_nothing() {
    let f=Fixture::new(); f.source("unknown",b"progress");
    let page=recovery::list(&f.database,None).unwrap(); let review=recovery::inspect(&f.database,&page.candidates[0].candidate_id).unwrap();
    let mut package=f.output(); recovery::export(&f.database,&review.candidate_id,&review.revision,&mut package).unwrap();
    let original=bytes(&mut package);
    let mut wrong_magic=original.clone(); wrong_magic[0]^=1;
    let mut wrong_manifest=original.clone(); wrong_manifest[16]^=1;
    for damaged in [original[..original.len()-1].to_vec(),[original.clone(),vec![0]].concat(),wrong_magic,wrong_manifest] {
        let mut input=f.output(); input.write_all(&damaged).unwrap(); let mut output=f.output();
        assert!(recovery::extract(&mut input,&mut output).is_err()); assert!(bytes(&mut output).is_empty());
    }
    let mut damaged=original.clone(); let end=damaged.len()-1; damaged[end]^=1;
    let mut input=f.output(); input.write_all(&damaged).unwrap(); let mut output=f.output();
    assert!(recovery::extract(&mut input,&mut output).is_err()); assert!(bytes(&mut output).is_empty());
}

#[test]
fn manual_recovery_unverified_manifest_cannot_claim_provenance_even_with_valid_trailer() {
    let f=Fixture::new();
    for manifest in [serde_json::json!({"schema_version":1,"trust":"authenticated","source_kind":"assets","display_name":"unknown","payload_bytes":1,"payload_hash":digest(b"x"),"prefix_hex":"78"}),
        serde_json::json!({"schema_version":1,"trust":"unverified","source_kind":"assets","display_name":"unknown","payload_bytes":1,"payload_hash":digest(b"x"),"prefix_hex":"78","game_id":Uuid::new_v4()})] {
        let manifest=serde_json::to_vec(&manifest).unwrap(); let mut package=b"OPRC0001".to_vec(); package.extend_from_slice(&(manifest.len() as u32).to_le_bytes()); package.extend_from_slice(&manifest); package.push(b'x');
        let mut hash=Sha256::new(); hash.update(b"Vlikeon unverified recovery v1\0"); hash.update(&package); package.extend_from_slice(&hash.finalize());
        let mut input=f.output(); input.write_all(&package).unwrap(); let mut output=f.output();
        assert!(recovery::extract(&mut input,&mut output).is_err()); assert!(bytes(&mut output).is_empty());
    }
    assert_eq!(f.counts(),(0,0,0,0));
}

#[test]
fn manual_recovery_pages_all_unknown_names_and_preserves_zero_length_files() {
    let f=Fixture::new(); for i in 0..66 {f.source(&format!("unknown-{i}"),&[]);}
    let first=recovery::list(&f.database,None).unwrap(); assert_eq!(first.candidates.len(),64);
    let second=recovery::list(&f.database,first.next_after.as_ref()).unwrap(); assert_eq!(second.candidates.len(),2); assert!(second.next_after.is_none());
    assert!(second.candidates.iter().all(|item| item.candidate_id.0>first.next_after.as_ref().unwrap().0));
    let review=recovery::inspect(&f.database,&second.candidates[0].candidate_id).unwrap(); assert_eq!(review.record.payload_bytes,0);
    let mut package=f.output(); recovery::export(&f.database,&review.candidate_id,&review.revision,&mut package).unwrap(); let mut output=f.output();
    recovery::extract(&mut package,&mut output).unwrap(); assert!(bytes(&mut output).is_empty());
}

#[test]
fn manual_recovery_invalid_identity_and_nonempty_destination_refuse() {
    let f=Fixture::new(); f.source("unknown",b"x");
    assert!(recovery::list(&f.database,Some(&ContentHash("../../escape".into()))).is_err());
    assert!(recovery::inspect(&f.database,&digest(b"not-present")).is_err());
    let page=recovery::list(&f.database,None).unwrap();
    let ordinary=&page.candidates[0]; let review=recovery::inspect(&f.database,&ordinary.candidate_id).unwrap();
    let mut output=f.output(); output.write_all(b"preserve").unwrap();
    assert!(recovery::export(&f.database,&review.candidate_id,&review.revision,&mut output).is_err()); assert_eq!(bytes(&mut output),b"preserve");
}

#[test]
fn manual_recovery_inventory_covers_legacy_and_repair_without_trusting_directory_names() {
    let f=Fixture::new(); let legacy=f.database.root.join("saves").join("lost-game-metadata"); fs::create_dir_all(&legacy).unwrap();
    fs::write(legacy.join("lost-save-id"),b"legacy").unwrap(); let repair=f.database.root.join("repaired-saves-v1"); fs::create_dir(&repair).unwrap(); fs::write(repair.join("unknown"),b"repair").unwrap();
    fs::create_dir(legacy.join("do-not-descend")).unwrap(); fs::write(legacy.join("do-not-descend").join("hidden"),b"hidden").unwrap();
    let page=recovery::list(&f.database,None).unwrap(); assert_eq!(page.candidates.len(),2); assert_eq!(page.retained_skipped,1);
    assert!(page.candidates.iter().any(|candidate|candidate.source_kind=="legacy_saves")); assert!(page.candidates.iter().any(|candidate|candidate.source_kind=="repair_files"));
}

#[test]
fn manual_recovery_respects_the_save_writer_lease() {
    let f=Fixture::new(); let source=f.source("unknown",b"preserve");
    let lease=OpenOptions::new().read(true).write(true).create(true).truncate(false)
        .open(f.database.root.join("save-payload-repair-v1.lock")).unwrap();
    lease.try_lock().unwrap();
    assert_eq!(recovery::list(&f.database,None).err().unwrap().code,ErrorCode::Busy);
    assert_eq!(fs::read(&source).unwrap(),b"preserve");
    drop(lease);assert_eq!(recovery::list(&f.database,None).unwrap().candidates.len(),1);
}

#[cfg(windows)]
#[test]
fn manual_recovery_never_descends_into_windows_junctions() {
    let f=Fixture::new(); f.source("ordinary",b"visible");
    let target=f.directory.join("not-profile-content"); fs::create_dir(&target).unwrap();
    fs::write(target.join("private"),b"do not inventory").unwrap();
    let link=f.database.root.join("repaired-saves-v1");
    let status=std::process::Command::new("C:\\Windows\\System32\\cmd.exe")
        .args(["/d","/c","mklink","/J"]).arg(&link).arg(&target).output().unwrap();
    assert!(status.status.success(),"Junction fixture creation failed");
    let page=recovery::list(&f.database,None).unwrap();
    assert_eq!(page.candidates.len(),1);assert_eq!(page.retained_skipped,1);
    assert_eq!(fs::read(target.join("private")).unwrap(),b"do not inventory");
    // Remove only the verified junction itself before recursive fixture cleanup.
    use std::os::windows::fs::MetadataExt;
    assert!(fs::symlink_metadata(&link).unwrap().file_attributes()&0x400!=0);
    assert_eq!(fs::canonicalize(&link).unwrap(),fs::canonicalize(&target).unwrap());
    fs::remove_dir(&link).unwrap();
    assert_eq!(fs::read(target.join("private")).unwrap(),b"do not inventory");
}
