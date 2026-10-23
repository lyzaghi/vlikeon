use crate::{Database,library::Library,util::storage_error};
use omniplay_contracts::{GameInputProfile,GameTouchProfile,ErrorCode,PlatformError,Result};
use rusqlite::{OptionalExtension,params};
use uuid::Uuid;
pub fn load(database:&Database,game_id:Uuid)->Result<GameInputProfile>{
    Library::new(database).get(game_id)?;let raw:Option<String>=database.connection.query_row("SELECT substr(value,1,16385) FROM preferences WHERE key=?1",[format!("input.{game_id}")],|row|row.get(0)).optional().map_err(storage_error)?;
    let profile=match raw{Some(raw)if raw.len()<=16384=>serde_json::from_str(&raw).map_err(|_|PlatformError::new(ErrorCode::CorruptData,"Game input settings need recovery"))?,Some(_)=>return Err(PlatformError::new(ErrorCode::CorruptData,"Game input settings exceed their bound")),None=>GameInputProfile::default()};profile.validate()?;Ok(profile)
}
pub fn store(database:&Database,game_id:Uuid,input:GameInputProfile)->Result<GameInputProfile>{
    input.validate()?;let raw=serde_json::to_string(&input).map_err(storage_error)?;if raw.len()>16384{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Input settings exceed their storage bound"));}let tx=database.connection.unchecked_transaction().map_err(storage_error)?;Library::new(database).get(game_id)?;
    tx.execute("INSERT INTO preferences(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![format!("input.{game_id}"),raw]).map_err(storage_error)?;
    database.record_audit("input.update",Some(game_id))?;tx.commit().map_err(storage_error)?;Ok(input)
}
pub fn load_touch(database:&Database,game_id:Uuid)->Result<GameTouchProfile>{
    Library::new(database).get(game_id)?;let raw:Option<String>=database.connection.query_row("SELECT substr(value,1,8193) FROM preferences WHERE key=?1",[format!("touch.{game_id}")],|row|row.get(0)).optional().map_err(storage_error)?;
    let profile=match raw{Some(raw)if raw.len()<=8192=>serde_json::from_str(&raw).map_err(|_|PlatformError::new(ErrorCode::CorruptData,"Touch settings need recovery"))?,Some(_)=>return Err(PlatformError::new(ErrorCode::CorruptData,"Touch settings exceed their bound")),None=>GameTouchProfile::default()};profile.validate()?;Ok(profile)
}
pub fn store_touch(database:&Database,game_id:Uuid,touch:GameTouchProfile)->Result<GameTouchProfile>{
    touch.validate()?;let raw=serde_json::to_string(&touch).map_err(storage_error)?;if raw.len()>8192{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Touch settings exceed their storage bound"));}let tx=database.connection.unchecked_transaction().map_err(storage_error)?;Library::new(database).get(game_id)?;
    tx.execute("INSERT INTO preferences(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![format!("touch.{game_id}"),raw]).map_err(storage_error)?;
    database.record_audit("touch.update",Some(game_id))?;tx.commit().map_err(storage_error)?;Ok(touch)
}
