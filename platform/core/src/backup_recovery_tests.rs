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
