//! Isolated storage QA endpoint; callbacks and TEMP triggers are test-only.
use omniplay_contracts::{ContentHash,GameManifest,GameSystem,SaveKind,SaveMetadata};
use omniplay_core::{Database,library::Library,saves::SaveStore,util::digest};
use rusqlite::{Connection,functions::FunctionFlags};
use serde_json::{Value,json};
use std::{fs::{self,File},io::Write,path::{Path,PathBuf},time::{Duration,Instant}};
use uuid::Uuid;
type AnyResult<T>=std::result::Result<T,Box<dyn std::error::Error>>;
fn leaf(root:&Path,value:&str)->AnyResult<PathBuf>{if value.is_empty()||value.len()>100||value=="."||value==".."||!value.bytes().all(|c|c.is_ascii_alphanumeric()||b"._-".contains(&c)){return Err("Expected a QA leaf".into());}Ok(root.join(value))}
fn new_metadata(device_id:Uuid,game:&GameManifest,id:Uuid,parent:Option<Uuid>,payload:&[u8])->SaveMetadata{SaveMetadata{schema_version:1,id,game_id:game.id,game_content_hash:game.content_hash.clone(),kind:SaveKind::InGame,slot:0,generation:1,parent_id:parent,core_id:"storage-fixture".into(),core_version:"1.0.0".parse().unwrap(),state_format_version:"fixture-v1".into(),platform:"fixture".into(),core_config_hash:ContentHash("b".repeat(64)),payload_hash:digest(payload),payload_bytes:payload.len()as u64,created_at_ms:1,device_id,label:Some("Preserve progress 🎮".into()),conflict:false}}
fn snapshot(db:&mut Database)->AnyResult<Value>{
    let games=Library::new(db).list(None,0,100)?;let mut saves=Vec::new();let mut heads=Vec::new();for game in games{for metadata in SaveStore::new(db).list(game.id)?{let result=SaveStore::new(db).read(metadata.id,None);saves.push(json!({"metadata":metadata,"valid":result.is_ok(),"payload_hash":result.ok().map(|(_,bytes)|digest(&bytes))}));}heads.extend(SaveStore::new(db).heads(game.id)?);}saves.sort_by(|a,b|a["metadata"]["id"].as_str().cmp(&b["metadata"]["id"].as_str()));heads.sort_by_key(|head|head.id);
    let integrity:String=db.connection.query_row("PRAGMA integrity_check",[],|row|row.get(0))?;let foreign_keys=db.connection.prepare("PRAGMA foreign_key_check")?.exists([])?;
    let version:u32=db.connection.query_row("PRAGMA user_version",[],|row|row.get(0))?;
    let debt:i64=db.connection.query_row("SELECT count(*) FROM save_repair_files WHERE state!='live'",[],|row|row.get(0))?;
    Ok(json!({"saves":saves,"heads":heads,"integrity":integrity,"foreign_key_errors":foreign_keys,"schema":version,"device_id":db.device_id,"staged_or_retired":debt}))
}
fn seed(root:&Path)->AnyResult<Value>{
    fs::create_dir(root)?;let mut db=Database::open(root)?;let source=root.join("synthetic.nes");fs::write(&source,[23;64])?;let game=Library::new(&db).import(File::open(source)?,"Synthetic fixture".into(),GameSystem::Nes)?;
    let metadata=new_metadata(db.device_id,&game,Uuid::new_v4(),None,b"old progress");let saved=SaveStore::new(&mut db).commit(metadata,b"old progress",None)?;Ok(json!({"game":game,"old":saved,"snapshot":snapshot(&mut db)?}))
}
fn pause(marker:&Path,phase:&str)->AnyResult<()>{let mut file=File::create(marker)?;file.write_all(phase.as_bytes())?;file.sync_all()?;let deadline=Instant::now()+Duration::from_secs(90);while Instant::now()<deadline{std::thread::sleep(Duration::from_millis(20));}Err("QA child not terminated within its pause deadline".into())}
fn write_save(root:&Path,request:&Value,marker:&Path)->AnyResult<Value>{
    let mut db=Database::open(root)?;let game_id:Uuid=serde_json::from_value(request["game_id"].clone())?;let game=Library::new(&db).get(game_id)?;
    let id:Uuid=serde_json::from_value(request["save_id"].clone())?;let parent:Uuid=serde_json::from_value(request["parent_id"].clone())?;let phase=request["phase"].as_str().unwrap_or("normal");
    if matches!(phase,"reservation"|"payload_synced"|"metadata_inserted"|"pointer_published"|"head_updated"){
        let marker=marker.to_owned();let phase_owned=phase.to_owned();db.connection.create_scalar_function("qa_pause",0,FunctionFlags::SQLITE_UTF8|FunctionFlags::SQLITE_INNOCUOUS,move |_|{pause(&marker,&phase_owned).map_err(|error|rusqlite::Error::UserFunctionError(error.to_string().into()))?;Ok(0i64)})?;
        let timing=match phase{"reservation"=>"BEFORE INSERT ON save_repair_files","payload_synced"=>"BEFORE INSERT ON saves","metadata_inserted"=>"AFTER INSERT ON saves","pointer_published"=>"AFTER INSERT ON save_payload_locations","head_updated"=>"AFTER INSERT ON audit WHEN NEW.event='save.commit'",_=>unreachable!()};
        db.connection.execute_batch(&format!("CREATE TEMP TRIGGER qa_interrupt {timing} BEGIN SELECT qa_pause(); END;"))?;
    }
    if phase=="abort"{db.connection.execute_batch("CREATE TEMP TRIGGER qa_abort BEFORE INSERT ON saves BEGIN SELECT RAISE(ABORT,'qa_forced_abort'); END;")?;}
    if phase=="sqlite_full"{
        db.connection.execute_batch("CREATE TABLE qa_capacity(payload BLOB);")?;let pages:u64=db.connection.query_row("PRAGMA page_count",[],|row|row.get(0))?;db.connection.pragma_update(None,"max_page_count",pages+1)?;
        db.connection.execute_batch("CREATE TEMP TRIGGER qa_full BEFORE INSERT ON saves BEGIN INSERT INTO qa_capacity VALUES(zeroblob(1048576)); END;")?;
    }
    let payload=if phase=="disk_full"{vec![37;8*1024*1024]}else{b"new progress".to_vec()};let metadata=new_metadata(db.device_id,&game,id,Some(parent),&payload);
    let result=SaveStore::new(&mut db).commit(metadata,&payload,Some(parent));
    if phase=="committed"&&result.is_ok(){pause(marker,phase)?;}
    if phase=="sqlite_full"{db.connection.pragma_update(None,"max_page_count",4294967294u64)?;}
    Ok(json!({"ok":result.is_ok(),"error":result.as_ref().err().map(|error|error.code),"snapshot":snapshot(&mut db)?}))
}
const MIGRATIONS:[&str;15]=[include_str!("../migrations/0001.sql"),include_str!("../migrations/0002.sql"),include_str!("../migrations/0003.sql"),include_str!("../migrations/0004.sql"),include_str!("../migrations/0005.sql"),include_str!("../migrations/0006.sql"),include_str!("../migrations/0007.sql"),include_str!("../migrations/0008.sql"),include_str!("../migrations/0009.sql"),include_str!("../migrations/0010.sql"),include_str!("../migrations/0011.sql"),include_str!("../migrations/0012.sql"),include_str!("../migrations/0013.sql"),include_str!("../migrations/0014.sql"),include_str!("../migrations/0015.sql")];
fn migration(root:&Path,version:usize,interrupt:bool)->AnyResult<Value>{
    if version>15{return Err("Unsupported fixture schema".into());}fs::create_dir(root)?;
    let connection=Connection::open(root.join("omniplay.sqlite3"))?;connection.execute_batch("PRAGMA foreign_keys=ON;PRAGMA journal_mode=WAL;")?;
    let device=Uuid::new_v4();let mut preserved=None;
    if version>0{
        connection.execute_batch(MIGRATIONS[0])?;
        connection.execute("INSERT INTO preferences(key,value) VALUES('device_id',?1)",[device.to_string()])?;connection.execute("INSERT INTO preferences(key,value) VALUES('qa_preserved','migration sentinel')",[])?;
        let content=[23;64];let game=GameManifest{schema_version:1,id:Uuid::new_v4(),title:"Migration game 🎮".into(),system:GameSystem::Nes,content_hash:digest(&content),byte_length:64,region:None,nominal_refresh_hz:None,created_at_ms:1,media:None};
        connection.execute("INSERT INTO games(id,content_hash,title,search_title,system,manifest_json,created_at_ms) VALUES(?1,?2,?3,?3,'nes',?4,1)",rusqlite::params![game.id.to_string(),game.content_hash.0,game.title,serde_json::to_string(&game)?])?;
        fs::create_dir(root.join("assets"))?;fs::write(root.join("assets").join(&game.content_hash.0),content)?;
        let save=new_metadata(device,&game,Uuid::new_v4(),None,b"old progress");connection.execute("INSERT INTO saves(id,game_id,kind,slot,generation,parent_id,metadata_json,payload_hash,payload_bytes,created_at_ms) VALUES(?1,?2,'in_game',0,1,NULL,?3,?4,?5,1)",rusqlite::params![save.id.to_string(),game.id.to_string(),serde_json::to_string(&save)?,save.payload_hash.0,save.payload_bytes])?;
        connection.execute("INSERT INTO save_heads(game_id,kind,slot,save_id) VALUES(?1,'in_game',0,?2)",rusqlite::params![game.id.to_string(),save.id.to_string()])?;
        let saves=root.join("saves").join(game.id.to_string());fs::create_dir_all(&saves)?;fs::write(saves.join(format!("{}.bin",save.id)),b"old progress")?;preserved=Some(save.id);
        for sql in MIGRATIONS.iter().take(version).skip(1){connection.execute_batch(sql)?;}
    }
    if interrupt{
        if !(10..=13).contains(&version){return Err("Migration FULL fixture supports schemas 10 through 13".into());}
        let pages:u64=connection.query_row("PRAGMA page_count",[],|row|row.get(0))?;connection.pragma_update(None,"max_page_count",pages)?;
        let error=connection.execute_batch(MIGRATIONS[version]).expect_err("Migration unexpectedly fit without allocating a page");
        assert_eq!(error.sqlite_error_code(),Some(rusqlite::ErrorCode::DiskFull));
        if !connection.is_autocommit(){connection.execute_batch("ROLLBACK;")?;}
        let after:u32=connection.query_row("PRAGMA user_version",[],|row|row.get(0))?;assert_eq!(after as usize,version);
        let count:u32=connection.query_row("SELECT count(*) FROM saves",[],|row|row.get(0))?;assert_eq!(count,1);
        connection.pragma_update(None,"max_page_count",4294967294u64)?;
    }
    drop(connection);let mut db=Database::open(root)?;
    if version>0{assert_eq!(db.device_id,device);let sentinel:String=db.connection.query_row("SELECT value FROM preferences WHERE key='qa_preserved'",[],|row|row.get(0))?;assert_eq!(sentinel,"migration sentinel");}
    let first=snapshot(&mut db)?;if let Some(id)=preserved{assert_eq!(first["saves"].as_array().unwrap().len(),1);assert_eq!(first["saves"][0]["valid"],true);assert_eq!(first["heads"][0]["id"],id.to_string());}
    drop(db);let mut db=Database::open(root)?;assert_eq!(snapshot(&mut db)?,first);let mut value=first;value["migration_rollback_verified"]=json!(interrupt);Ok(value)
}
fn main()->AnyResult<()>{
    let args:Vec<_>=std::env::args().collect();if args.len()!=3{return Err("usage: storage_faults QA_ROOT REQUEST_LEAF.json".into());}let base=Path::new(&args[1]).canonicalize()?;let request:Value=serde_json::from_slice(&fs::read(leaf(&base,&args[2])?)?)?;
    let profile=leaf(&base,request["profile"].as_str().ok_or("profile")?)?;
    let value=match request["operation"].as_str().ok_or("operation")?{"init"=>seed(&profile)?,"write"=>write_save(&profile,&request,&leaf(&base,request["marker"].as_str().unwrap_or("marker"))?)?,"snapshot"=>snapshot(&mut Database::open(&profile)?)?,"migration"=>migration(&profile,request["version"].as_u64().ok_or("version")? as usize,request["interrupt"]==true)?,_=>return Err("Unknown fixture operation".into())};println!("{}",serde_json::to_string(&value)?);Ok(())
}
