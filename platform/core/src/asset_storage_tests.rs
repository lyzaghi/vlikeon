//! Authored fixtures only. Execute when the owner resumes verification.
use super::*;
use crate::library::Library;
use std::io::Write;
use uuid::Uuid;

struct Directory(PathBuf);
impl Drop for Directory{fn drop(&mut self){let _=fs::remove_dir_all(&self.0);}}
// Fields drop in declaration order: close SQLite before removing its files.
struct Fixture{database:Database,_directory:Directory,original:PathBuf}
impl Fixture{
    fn new()->Self{
        let base=std::env::temp_dir().join(format!("vlikeon-asset-fixture-{}",Uuid::new_v4()));fs::create_dir(&base).unwrap();
        let original=base.join("selected.nes");File::create(&original).unwrap().write_all(&[17;64]).unwrap();
        let database=Database::open(&base.join("profile")).unwrap();Self{database,_directory:Directory(base),original}
    }
    fn import(&self)->GameManifest{Library::new(&self.database).import(File::open(&self.original).unwrap(),"Fixture".into(),GameSystem::Nes).unwrap()}
}

#[test]
fn live_content_is_retained_and_removed_library_content_is_explicit(){
    let f=Fixture::new();let game=f.import();let path=Library::new(&f.database).asset_path(&game).unwrap();
    let live=preview(&f.database,None).unwrap();assert_eq!(live.unused_assets,0);assert!(path.exists());
    Library::new(&f.database).remove(game.id).unwrap();assert!(path.exists());
    let reviewed=preview(&f.database,None).unwrap();assert_eq!(reviewed.unused_assets,1);
    let result=remove(&f.database,None,&reviewed.revision).unwrap();assert_eq!(result.removed_assets,1);assert_eq!(result.retained_assets,0);assert!(!path.exists());assert!(f.original.exists());
}

#[test]
fn preview_does_not_authorize_removal_after_reimport(){
    let f=Fixture::new();let first=f.import();Library::new(&f.database).remove(first.id).unwrap();
    let reviewed=preview(&f.database,None).unwrap();let replacement=f.import();
    assert!(remove(&f.database,None,&reviewed.revision).is_err());assert!(Library::new(&f.database).asset_path(&replacement).unwrap().exists());
}

#[test]
fn active_import_lease_blocks_preview_removal_and_another_import(){
    let f=Fixture::new();let game=f.import();Library::new(&f.database).remove(game.id).unwrap();let reviewed=preview(&f.database,None).unwrap();
    let lease=AssetLease::acquire(&f.database).unwrap();assert!(preview(&f.database,None).is_err());assert!(remove(&f.database,None,&reviewed.revision).is_err());
    assert!(Library::new(&f.database).import(File::open(&f.original).unwrap(),"Busy".into(),GameSystem::Nes).is_err());drop(lease);
    assert_eq!(preview(&f.database,None).unwrap().unused_assets,1);
}

#[test]
fn unknown_and_pending_files_are_never_collection_candidates(){
    let f=Fixture::new();let game=f.import();Library::new(&f.database).remove(game.id).unwrap();let directory=f.database.root.join("assets");
    for name in ["notes.txt",".pending-unknown"]{fs::write(directory.join(name),b"retain").unwrap();}
    let reviewed=preview(&f.database,None).unwrap();assert_eq!(reviewed.retained_unknown_entries,2);
    remove(&f.database,None,&reviewed.revision).unwrap();for name in ["notes.txt",".pending-unknown"]{assert_eq!(fs::read(directory.join(name)).unwrap(),b"retain");}
}

#[test]
fn reviewed_page_is_bounded_and_cursor_survives_removal(){
    let f=Fixture::new();let directory=f.database.root.join("assets");fs::create_dir_all(&directory).unwrap();
    for value in 0u8..65{let payload=[value];fs::write(directory.join(crate::util::digest(&payload).0),payload).unwrap();}
    let first=preview(&f.database,None).unwrap();assert_eq!(first.scanned_assets,64);assert_eq!(first.unused_assets,64);assert!(first.more_available);
    let cursor=first.next_after.clone().unwrap();let removed=remove(&f.database,None,&first.revision).unwrap();assert_eq!(removed.removed_assets,64);assert_eq!(removed.next_after.as_deref(),Some(cursor.as_str()));
    let last=preview(&f.database,Some(&cursor)).unwrap();assert_eq!(last.scanned_assets,1);assert_eq!(last.unused_assets,1);assert!(!last.more_available);assert_eq!(last.next_after,None);
    remove(&f.database,Some(&cursor),&last.revision).unwrap();assert_eq!(preview(&f.database,None).unwrap().scanned_assets,0);
}

#[test]
fn changed_candidate_requires_another_review(){
    let f=Fixture::new();let game=f.import();let path=Library::new(&f.database).asset_path(&game).unwrap();Library::new(&f.database).remove(game.id).unwrap();
    let reviewed=preview(&f.database,None).unwrap();fs::write(&path,b"different size").unwrap();
    assert!(remove(&f.database,None,&reviewed.revision).is_err());assert_eq!(fs::read(&path).unwrap(),b"different size");
}

#[test]
fn corrupt_ownership_refuses_the_entire_removal(){
    let f=Fixture::new();let game=f.import();let directory=f.database.root.join("assets");let unused=directory.join("a".repeat(64));fs::write(&unused,b"unused").unwrap();
    let reviewed=preview(&f.database,None).unwrap();assert_eq!(reviewed.unused_assets,1);
    f.database.connection.execute("UPDATE games SET manifest_json='{}' WHERE id=?1",[game.id.to_string()]).unwrap();
    assert!(preview(&f.database,None).is_err());assert!(remove(&f.database,None,&reviewed.revision).is_err());assert_eq!(fs::read(&unused).unwrap(),b"unused");
}

#[test]
fn persisted_active_game_blocks_collection_even_without_ui_guard(){
    let f=Fixture::new();let game=f.import();
    f.database.connection.execute("INSERT INTO sessions(id,game_id,runtime_id,state,info_json,created_at_ms,updated_at_ms) VALUES(?1,?2,'fixture','running','{}',1,1)",rusqlite::params![Uuid::new_v4().to_string(),game.id.to_string()]).unwrap();
    assert!(preview(&f.database,None).is_err());assert!(Library::new(&f.database).remove(game.id).is_err());
}

#[test]
fn current_save_promotion_is_fenced_by_the_persisted_session(){
    use omniplay_contracts::{SaveMetadata,SaveKind};
    let mut f=Fixture::new();let game=f.import();let payload=b"progress";
    let metadata=SaveMetadata{schema_version:1,id:Uuid::new_v4(),game_id:game.id,game_content_hash:game.content_hash.clone(),kind:SaveKind::InGame,slot:0,generation:1,parent_id:None,core_id:"fixture".into(),core_version:"1.0.0".parse().unwrap(),state_format_version:"1".into(),platform:"fixture".into(),core_config_hash:ContentHash("a".repeat(64)),payload_hash:crate::util::digest(payload),payload_bytes:payload.len()as u64,created_at_ms:crate::now_ms(),device_id:f.database.device_id,label:None,conflict:false};
    let saved=crate::saves::SaveStore::new(&mut f.database).commit(metadata,payload,None).unwrap();
    f.database.connection.execute("INSERT INTO sessions(id,game_id,runtime_id,state,info_json,created_at_ms,updated_at_ms) VALUES(?1,?2,'fixture','paused','{}',1,1)",rusqlite::params![Uuid::new_v4().to_string(),game.id.to_string()]).unwrap();
    assert!(crate::saves::SaveStore::new(&mut f.database).promote(saved.id,Some(saved.id)).is_err());
    let head:String=f.database.connection.query_row("SELECT save_id FROM save_heads WHERE game_id=?1",[game.id.to_string()],|row|row.get(0)).unwrap();assert_eq!(head,saved.id.to_string());
}

#[cfg(unix)]
#[test]
fn hash_shaped_symlink_is_uncertainty_and_does_not_reach_original(){
    let f=Fixture::new();let game=f.import();Library::new(&f.database).remove(game.id).unwrap();
    std::os::unix::fs::symlink(&f.original,f.database.root.join("assets").join("a".repeat(64))).unwrap();
    assert!(preview(&f.database,None).is_err());assert_eq!(fs::read(&f.original).unwrap(),[17;64]);
}
