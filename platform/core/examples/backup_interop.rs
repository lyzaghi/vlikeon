//! Test-only endpoint for exchanging actual archives with browser QA.
use base64::{Engine, engine::general_purpose::STANDARD};
use omniplay_contracts::{ContentHash, GameManifest, GameSystem, SaveKind, SaveMetadata};
use omniplay_core::{Database, library::Library, saves::SaveStore, save_backup, save_origins, save_maintenance, util::digest};
use serde_json::{Value, json};
use std::{fs::{self, File, OpenOptions}, io::Read, path::{Path, PathBuf}};
use uuid::Uuid;
type AnyResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;
fn leaf(root:&Path, value:&str)->AnyResult<PathBuf>{
    if value.is_empty()||value.len()>100||value=="."||value==".."||!value.bytes().all(|c|c.is_ascii_alphanumeric()||b"._-".contains(&c)){return Err("Expected a single QA filename".into());}
    Ok(root.join(value))
}
fn new_file(path:&Path)->AnyResult<File>{Ok(OpenOptions::new().read(true).write(true).create_new(true).open(path)?)}
fn metadata(db:&Database,game:&GameManifest,payload:&[u8],slot:u16,label:Option<&str>)->SaveMetadata{
    SaveMetadata{schema_version:1,id:Uuid::new_v4(),game_id:game.id,game_content_hash:game.content_hash.clone(),kind:SaveKind::InGame,slot,generation:1,parent_id:None,core_id:"interop-fixture".into(),core_version:"1.2.3+test".parse().unwrap(),state_format_version:"portable-fixture-v1".into(),platform:"interop-fixture".into(),core_config_hash:ContentHash("a".repeat(64)),payload_hash:digest(payload),payload_bytes:payload.len() as u64,created_at_ms:omniplay_core::now_ms(),device_id:db.device_id,label:label.map(str::to_owned),conflict:false}
}
fn snapshot(db:&mut Database)->AnyResult<Value>{
    let ids={let mut statement=db.connection.prepare("SELECT id FROM saves ORDER BY id")?;let rows=statement.query_map([],|row|row.get::<_,String>(0))?;rows.collect::<std::result::Result<Vec<_>,_>>()?};
    let mut records=Vec::new();
    for raw in ids{
        let id=Uuid::parse_str(&raw)?;let(metadata,origin)=save_origins::inspect(&db.connection,id)?;
        let reference=db.connection.query_row("SELECT value FROM preferences WHERE key=?1",[format!("backup_reference.{id}")],|row|row.get::<_,String>(0)).ok();
        let payload=match SaveStore::new(db).open_payload(&metadata){Ok(mut file)=>{let mut bytes=Vec::new();if metadata.payload_bytes<=4096{file.read_to_end(&mut bytes)?;}json!({"valid":true,"base64":if metadata.payload_bytes<=4096{Some(STANDARD.encode(bytes))}else{None}})},Err(error)=>json!({"valid":false,"error":error.code})};
        records.push(json!({"metadata":metadata,"origin":origin,"reference":reference.map(|raw|serde_json::from_str::<Value>(&raw)).transpose()?,"payload":payload}));
    }
    let games=Library::new(db).list(None,0,100)?;let mut heads=Vec::new();for game in &games{heads.extend(SaveStore::new(db).heads(game.id)?);}heads.sort_by_key(|metadata|metadata.id);
    let markers={let mut statement=db.connection.prepare("SELECT save_id FROM locally_removed_saves ORDER BY save_id")?;let rows=statement.query_map([],|row|row.get::<_,String>(0))?;rows.collect::<std::result::Result<Vec<_>,_>>()?};
    Ok(json!({"records":records,"heads":heads,"removed":markers}))
}
fn initialize(root:&Path,request:&Value)->AnyResult<Value>{
    let profile=leaf(root,request["profile"].as_str().ok_or("profile")?)?;fs::create_dir(&profile)?;let mut db=Database::open(&profile)?;
    let count=request["games"].as_u64().unwrap_or(2);if count>2{return Err("At most two synthetic games".into());}let mut games=Vec::new();
    for n in 0..count{let content:Vec<u8>=(0..64).map(|i|(i+n*17) as u8).collect();let selected=profile.join(format!("fixture-source-{n}.bin"));fs::write(&selected,&content)?;games.push(Library::new(&db).import(File::open(selected)?,format!("Synthetic game {n}"),GameSystem::Nes)?);}
    if request["seed"]==true{
        let game=games.first().ok_or("seed needs game")?;
        let first=metadata(&db,game,b"captured first",0,Some("Tiến trình Việt 日本語 🎮"));let first=SaveStore::new(&mut db).commit(first,b"captured first",None)?;
        let mut child=metadata(&db,game,b"captured child",0,None);child.parent_id=Some(first.id);SaveStore::new(&mut db).commit(child,b"captured child",Some(first.id))?;
        let mut state=metadata(&db,game,b"\0state\xff\x80",1,Some("quotes \" \\ \n emoji 🚀"));state.kind=SaveKind::EmulatorState;SaveStore::new(&mut db).commit(state,b"\0state\xff\x80",None)?;
        let legacy=metadata(&db,game,b"legacy",2,Some(""));let legacy=SaveStore::new(&mut db).commit(legacy,b"legacy",None)?;
        // Deliberately emulate a row from the pre-origin schema in this fixture.
        db.connection.execute("UPDATE save_origins SET source_kind='legacy_projection',canonical_metadata_json=NULL,canonical_hash=NULL,canonical_parent_id=NULL,imported_from_json=NULL,imported_from_hash=NULL WHERE save_id=?1",[legacy.id.to_string()])?;
        let mut remote=metadata(&db,game,b"remote canonical",3,Some("Remote original"));remote.game_id=Uuid::new_v4();remote.device_id=Uuid::new_v4();remote.parent_id=Some(Uuid::new_v4());remote.generation=999;
        SaveStore::new(&mut db).ingest_remote(remote,b"remote canonical")?.ok_or("remote fixture missing")?;
        let single=profile.join("selected.opsave");let mut file=new_file(&single)?;SaveStore::new(&mut db).export(first.id,&mut file)?;drop(file);SaveStore::new(&mut db).import_export(game.id,&mut File::open(single)?)?;
        if let Some(other)=games.get(1){let m=metadata(&db,other,b"other game",0,None);SaveStore::new(&mut db).commit(m,b"other game",None)?;}
    }else if request["heads"]==true{for game in &games{let m=metadata(&db,game,b"existing target head",0,Some("Keep selected"));SaveStore::new(&mut db).commit(m,b"existing target head",None)?;}}
    Ok(json!({"games":games,"snapshot":snapshot(&mut db)?}))
}
fn execute(root:&Path,request:&Value)->AnyResult<Value>{
    if request["operation"]=="init"{return initialize(root,request);}
    let profile=leaf(root,request["profile"].as_str().ok_or("profile")?)?;if !profile.join("omniplay.sqlite3").is_file(){return Err("Initialize QA profile first".into());}let mut db=Database::open(&profile)?;let before=snapshot(&mut db)?;
    let result=match request["operation"].as_str().ok_or("operation")?{
        "restore"=>{let archive=leaf(root,request["archive"].as_str().ok_or("archive")?)?;match save_backup::restore(&mut db,&mut File::open(archive)?){Ok(value)=>json!({"ok":true,"value":value}),Err(error)=>json!({"ok":false,"error":error.code})}},
        "export"=>{let ids:Vec<Uuid>=serde_json::from_value(request["ids"].clone())?;let archive=leaf(root,request["archive"].as_str().ok_or("archive")?)?;let mut file=new_file(&archive)?;match save_backup::export(&mut db,&ids,&mut file){Ok(value)=>json!({"ok":true,"value":value}),Err(error)=>json!({"ok":false,"error":error.code})}},
        "purge"=>{let game_id:Uuid=serde_json::from_value(request["game_id"].clone())?;let preview=save_maintenance::purge_preview(&db,game_id)?;json!({"ok":true,"value":save_maintenance::purge(&mut db,game_id,preview.revision)?})},
        "active"=>{let game_id:Uuid=serde_json::from_value(request["game_id"].clone())?;db.connection.execute("INSERT INTO sessions(id,game_id,runtime_id,state,info_json,created_at_ms,updated_at_ms) VALUES(?1,?2,'interop-fixture','paused','{}',1,1)",rusqlite::params![Uuid::new_v4().to_string(),game_id.to_string()])?;json!({"ok":true})},
        "stop"=>{db.connection.execute("UPDATE sessions SET state='stopped'",[])?;json!({"ok":true})},
        "damage"=>{let id:Uuid=serde_json::from_value(request["save_id"].clone())?;let raw:String=db.connection.query_row("SELECT file_id FROM save_payload_locations WHERE save_id=?1",[id.to_string()],|row|row.get(0))?;let file_id=Uuid::parse_str(&raw)?;fs::write(db.root.join("repaired-saves-v1").join(format!("{file_id}.bin")),b"damaged fixture")?;json!({"ok":true})},
        "snapshot"=>json!({"ok":true}),_=>return Err("Unknown QA operation".into()),
    };Ok(json!({"result":result,"before":before,"after":snapshot(&mut db)?}))
}
fn main()->AnyResult<()>{let args:Vec<_>=std::env::args().collect();if args.len()!=3{return Err("usage: backup_interop QA_ROOT REQUEST_LEAF.json".into());}let root=Path::new(&args[1]).canonicalize()?;let request:Value=serde_json::from_slice(&fs::read(leaf(&root,&args[2])?)?)?;println!("{}",serde_json::to_string(&execute(&root,&request)?)?);Ok(())}
