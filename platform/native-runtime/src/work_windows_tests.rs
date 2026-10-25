//! Authored for the final consolidated pass; not executed during source work.
use super::*;
use omniplay_core::{Database,native_work_storage as storage};
use std::sync::atomic::AtomicBool;

struct Case {root:PathBuf,profile:Uuid}
impl Case {
    fn new()->Self{
        let root=std::env::temp_dir().join(format!("omniplay-native-work-test-{}",Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        Self{root:root.canonicalize().unwrap(),profile:Uuid::new_v4()}
    }
    fn database(&self)->Database{Database::open(&self.root).unwrap()}
    fn work(&self,id:Uuid)->PathBuf{self.root.join("native-work").join(id.to_string())}
    fn reserve(&self,id:Uuid,legacy:bool){
        let _creator=creation_guard(&self.root).unwrap();let mut database=self.database();
        if legacy{storage::reserve(&mut database,self.profile,id,1024,1).unwrap();}
        else{storage::reserve_windows(&mut database,self.profile,id,1024,1).unwrap();}
    }
    fn partial(&self,id:Uuid)->PathBuf{
        let path=self.work(id);fs::create_dir_all(&path).unwrap();fs::write(path.join("game.bin"),b"test-game").unwrap();path
    }
    fn works(&self)->u32{storage::status(&self.database(),self.profile).unwrap().works}
}
impl Drop for Case {fn drop(&mut self){let _=fs::remove_dir_all(&self.root);}}

#[test]
fn live_creator_blocks_releasing_a_directory_not_yet_created(){
    let case=Case::new();let id=Uuid::new_v4();let creator=creation_guard(&case.root).unwrap();
    storage::reserve_windows(&mut case.database(),case.profile,id,1024,1).unwrap();
    assert!(collect_one(&case.root,case.profile,&case.work(id)).is_err());assert_eq!(case.works(),1);
    drop(creator);
    collect_one(&case.root,case.profile,&case.work(id)).unwrap();assert_eq!(case.works(),0);
}

#[test]
fn recovery_inventory_reaches_debt_without_any_work_directory(){
    let case=Case::new();let id=Uuid::new_v4();case.reserve(id,false);
    assert!(!case.root.join("native-work").exists());
    let summary=recover_windows_work(&case.root,case.profile,&AtomicBool::new(false),|_|{}).unwrap();
    assert_eq!((summary.attempted,summary.recovered,summary.retained,summary.unknown_entries),(1,1,0,0));
    assert_eq!(case.works(),0);
}

#[test]
fn protocol_one_partial_creation_without_owner_lease_is_recoverable(){
    let case=Case::new();let id=Uuid::new_v4();case.reserve(id,false);let work=case.partial(id);
    assert!(!work.join("owner.lease").exists());
    collect_one(&case.root,case.profile,&work).unwrap();
    assert!(!work.exists());assert_eq!(case.works(),0);
    assert!(!case.root.join("native-retirement").join(format!("{id}.json")).exists());
}

#[test]
fn live_owner_blocks_partial_creation_recovery_after_creation_handoff(){
    use std::os::windows::fs::OpenOptionsExt;
    let case=Case::new();let id=Uuid::new_v4();case.reserve(id,false);let work=case.partial(id);
    let owner=OpenOptions::new().read(true).write(true).create_new(true).share_mode(1).open(work.join("owner.lease")).unwrap();
    assert!(collect_one(&case.root,case.profile,&work).is_err());assert_eq!(case.works(),1);
    assert_eq!(fs::read(work.join("game.bin")).unwrap(),b"test-game");
    drop(owner);collect_one(&case.root,case.profile,&work).unwrap();assert_eq!(case.works(),0);
}

#[test]
fn legacy_missing_and_unleased_partial_debts_are_never_upgraded(){
    let case=Case::new();let missing=Uuid::new_v4();let partial=Uuid::new_v4();
    case.reserve(missing,true);case.reserve(partial,true);let work=case.partial(partial);
    for id in[missing,partial]{assert!(collect_one(&case.root,case.profile,&case.work(id)).is_err());}
    assert_eq!(case.works(),2);assert_eq!(fs::read(work.join("game.bin")).unwrap(),b"test-game");
}

#[test]
fn staged_debt_without_work_or_owner_lease_stays_retained(){
    let case=Case::new();let missing=Uuid::new_v4();let partial=Uuid::new_v4();
    case.reserve(missing,false);case.reserve(partial,false);let work=case.partial(partial);
    for id in[missing,partial]{
        storage::staged(&case.database(),case.profile,id,&omniplay_core::util::digest(b"staging-receipt")).unwrap();
        assert!(collect_one(&case.root,case.profile,&case.work(id)).is_err());
    }
    assert_eq!(case.works(),2);assert_eq!(fs::read(work.join("game.bin")).unwrap(),b"test-game");
}

#[test]
fn conflicting_journal_or_foreign_profile_cannot_use_prelaunch_authority(){
    let case=Case::new();let id=Uuid::new_v4();case.reserve(id,false);let work=case.partial(id);
    assert!(collect_one(&case.root,Uuid::new_v4(),&work).is_err());
    creation_intent(&work,case.profile,id).unwrap();
    assert!(collect_one(&case.root,case.profile,&work).is_err());
    assert_eq!(case.works(),1);assert_eq!(fs::read(work.join("game.bin")).unwrap(),b"test-game");
}

#[test]
fn creation_protocol_is_immutable_and_old_rows_default_to_zero(){
    let case=Case::new();let legacy=Uuid::new_v4();let current=Uuid::new_v4();
    case.reserve(legacy,true);case.reserve(current,false);let database=case.database();
    assert_eq!(storage::windows_recovery_reservation(&database,case.profile,legacy).unwrap().unwrap().windows_creation_protocol,0);
    assert_eq!(storage::windows_recovery_reservation(&database,case.profile,current).unwrap().unwrap().windows_creation_protocol,1);
    assert!(database.connection.execute("UPDATE native_work_storage SET windows_creation_protocol=1 WHERE work_id=?1",[legacy.to_string()]).is_err());
    assert!(database.connection.execute("UPDATE native_work_storage SET windows_creation_protocol=0 WHERE work_id=?1",[current.to_string()]).is_err());
}

#[test]
fn recovery_reports_legacy_retention_without_promoting_or_erasing_originals(){
    let case=Case::new();let missing=Uuid::new_v4();let partial=Uuid::new_v4();let fresh=Uuid::new_v4();
    case.reserve(missing,true);case.reserve(partial,true);let work=case.partial(partial);case.reserve(fresh,false);
    let summary=recover_windows_work(&case.root,case.profile,&AtomicBool::new(false),|_|{}).unwrap();
    assert_eq!((summary.attempted,summary.recovered,summary.retained),(3,1,2));
    assert_eq!((summary.retention.legacy_ownership,summary.retention.incomplete_ownership,summary.retention.locked,summary.retention.other_platform,summary.retention.unconfirmed),(2,0,0,0,0));
    assert_eq!(case.works(),2);assert_eq!(fs::read(work.join("game.bin")).unwrap(),b"test-game");
    for id in[missing,partial]{assert_eq!(storage::windows_recovery_reservation(&case.database(),case.profile,id).unwrap().unwrap().windows_creation_protocol,0);}
}

#[test]
fn recovery_distinguishes_locked_work_from_missing_ownership(){
    use std::os::windows::fs::OpenOptionsExt;
    let case=Case::new();let locked=Uuid::new_v4();let incomplete=Uuid::new_v4();
    case.reserve(locked,false);let work=case.partial(locked);
    let owner=OpenOptions::new().read(true).write(true).create_new(true).share_mode(1).open(work.join("owner.lease")).unwrap();
    case.reserve(incomplete,false);let other=case.partial(incomplete);creation_intent(&other,case.profile,incomplete).unwrap();
    let summary=recover_windows_work(&case.root,case.profile,&AtomicBool::new(false),|_|{}).unwrap();
    assert_eq!((summary.recovered,summary.retained,summary.retention.locked,summary.retention.incomplete_ownership),(0,2,1,1));
    assert_eq!(summary.retention.legacy_ownership+summary.retention.other_platform+summary.retention.unconfirmed,0);
    assert_eq!(fs::read(work.join("game.bin")).unwrap(),b"test-game");assert_eq!(case.works(),2);
    drop(owner);let retry=recover_windows_work(&case.root,case.profile,&AtomicBool::new(false),|_|{}).unwrap();
    assert_eq!((retry.recovered,retry.retained,retry.retention.locked,retry.retention.incomplete_ownership),(1,1,0,1));
    assert_eq!(fs::read(other.join("game.bin")).unwrap(),b"test-game");
}

#[test]
fn recovery_keeps_other_platform_and_invalid_owner_data_distinct(){
    let case=Case::new();let linux=Uuid::new_v4();let damaged=Uuid::new_v4();
    case.reserve(linux,false);let linux_work=case.partial(linux);fs::write(linux_work.join("linux-work-intent.json"),b"{}").unwrap();
    case.reserve(damaged,false);let damaged_work=case.partial(damaged);fs::write(damaged_work.join("owner.lease"),b"").unwrap();
    created(&damaged_work,Uuid::new_v4(),damaged).unwrap();
    let summary=recover_windows_work(&case.root,case.profile,&AtomicBool::new(false),|_|{}).unwrap();
    assert_eq!((summary.recovered,summary.retained,summary.retention.other_platform,summary.retention.unconfirmed),(0,2,1,1));
    assert_eq!(summary.retention.locked+summary.retention.legacy_ownership+summary.retention.incomplete_ownership,0);
    for work in[linux_work,damaged_work]{assert_eq!(fs::read(work.join("game.bin")).unwrap(),b"test-game");}
    assert_eq!(case.works(),2);
}

#[test]
fn quarantine_unowned_windows_work_preserves_originals_and_releases_legacy_debt(){
    let case=Case::new();let legacy=Uuid::new_v4();let incomplete=Uuid::new_v4();
    case.reserve(legacy,true);let legacy_work=case.partial(legacy);
    fs::create_dir_all(legacy_work.join("save")).unwrap();
    fs::write(legacy_work.join("save").join("battery.srm"),b"user-save-data").unwrap();

    case.reserve(incomplete,false);let inc_work=case.partial(incomplete);
    creation_intent(&inc_work,case.profile,incomplete).unwrap();

    let receipt1=quarantine_unowned_windows_work(&case.root,case.profile,legacy).unwrap();
    assert_eq!(receipt1.windows_creation_protocol,0);
    assert_eq!(receipt1.preserved_files,2);
    assert!(!legacy_work.exists());
    assert_eq!(fs::read(case.root.join("manual-recovery-staging").join(legacy.to_string()).join("save").join("battery.srm")).unwrap(),b"user-save-data");

    let receipt2=quarantine_unowned_windows_work(&case.root,case.profile,incomplete).unwrap();
    assert_eq!(receipt2.windows_creation_protocol,1);
    assert_eq!(receipt2.preserved_files,2);
    assert!(!inc_work.exists());
    assert_eq!(case.works(),0);
}
