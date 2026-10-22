use crate::{Database,library::Library,util::storage_error};
use omniplay_contracts::{core_option_identifier,GameCoreOptions,ErrorCode,PlatformError,Result,MAX_CORE_OPTION_PROFILE_BYTES};
use rusqlite::{OptionalExtension,params};
use uuid::Uuid;
fn key(game_id:Uuid,runtime:&str)->Result<String>{if !core_option_identifier(runtime){return Err(PlatformError::new(ErrorCode::InvalidRequest,"Core option runtime identity is invalid"));}Ok(format!("core_options.{game_id}.{runtime}"))}
pub fn list(database:&Database,game_id:Uuid)->Result<Vec<String>>{
    Library::new(database).get(game_id)?;let prefix=format!("core_options.{game_id}.");
    let mut statement=database.connection.prepare("SELECT key FROM preferences WHERE key GLOB ?1 ORDER BY key LIMIT 33").map_err(storage_error)?;
    let keys=statement.query_map([format!("{prefix}*")],|row|row.get::<_,String>(0)).map_err(storage_error)?.collect::<std::result::Result<Vec<_>,_>>().map_err(storage_error)?;
    if keys.len()>32{return Err(PlatformError::new(ErrorCode::CorruptData,"Core option profile storage needs recovery"));}
    keys.into_iter().map(|key|{let id=key.strip_prefix(&prefix).filter(|id|core_option_identifier(id)).ok_or_else(||PlatformError::new(ErrorCode::CorruptData,"Stored core option identity is invalid"))?;Ok(id.to_owned())}).collect()
}
pub fn load(database:&Database,game_id:Uuid,runtime:&str)->Result<GameCoreOptions>{
    let key=key(game_id,runtime)?;Library::new(database).get(game_id)?;
    let raw:Option<String>=database.connection.query_row("SELECT value FROM preferences WHERE key=?1",[key],|row|row.get(0)).optional().map_err(storage_error)?;
    let value=match raw{Some(raw)if raw.len()<=MAX_CORE_OPTION_PROFILE_BYTES=>serde_json::from_str(&raw).map_err(|_|PlatformError::new(ErrorCode::CorruptData,"Core options need review or reset"))?,Some(_)=>return Err(PlatformError::new(ErrorCode::CorruptData,"Stored core options exceed their bound")),None=>GameCoreOptions::default()};value.validate()?;Ok(value)
}
pub fn store(database:&Database,game_id:Uuid,runtime:&str,value:GameCoreOptions)->Result<GameCoreOptions>{
    let key=key(game_id,runtime)?;value.validate()?;let raw=serde_json::to_string(&value).map_err(storage_error)?;
    let tx=database.connection.unchecked_transaction().map_err(storage_error)?;Library::new(database).get(game_id)?;
    if value.values.is_empty(){tx.execute("DELETE FROM preferences WHERE key=?1",[&key]).map_err(storage_error)?;}
    else{
        let exists:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM preferences WHERE key=?1)",[&key],|r|r.get(0)).map_err(storage_error)?;
        if !exists{
            let per_game:u64=tx.query_row("SELECT count(*) FROM preferences WHERE key GLOB ?1",[format!("core_options.{game_id}.*")],|r|r.get(0)).map_err(storage_error)?;
            let total:u64=tx.query_row("SELECT count(*) FROM preferences WHERE key GLOB 'core_options.*'",[],|r|r.get(0)).map_err(storage_error)?;
            if per_game>=32||total>=65536{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Core option profile storage is full").recover("Reset unused runtime options before adding another profile"));}
        }
        tx.execute("INSERT INTO preferences(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,raw]).map_err(storage_error)?;
    }
    database.record_audit("core_options.update",Some(game_id))?;tx.commit().map_err(storage_error)?;Ok(value)
}
