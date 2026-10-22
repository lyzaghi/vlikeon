//! Immutable semantic origin records, separate from local game/history/CAS
//! projections. An origin hash is a consistency check, not a device attestation.
use crate::{Database,util::storage_error};
use omniplay_contracts::{ContentHash,ErrorCode,PlatformError,Result,SaveMetadata,SaveOrigin,SaveOriginKind,save_origin_hash};
use rusqlite::{Connection,OptionalExtension,params};
use uuid::Uuid;

pub(crate) enum NewOrigin{Captured,Imported(SaveMetadata),Remote(SaveMetadata)}
pub(crate) struct Prepared{value:SaveOrigin}
fn corrupt()->PlatformError{PlatformError::new(ErrorCode::CorruptData,"Saved progress origin metadata is inconsistent")}
fn conflict()->PlatformError{PlatformError::new(ErrorCode::Conflict,"This save identity already has a different canonical origin")}
fn parse(raw:&str)->Result<SaveMetadata>{if raw.len()>64*1024{return Err(corrupt());}let value:SaveMetadata=serde_json::from_str(raw).map_err(|_|corrupt())?;value.validate().map_err(|_|corrupt())?;Ok(value)}
fn encode(value:&SaveMetadata)->Result<String>{let raw=serde_json::to_string(value).map_err(storage_error)?;if raw.len()>64*1024{return Err(corrupt());}Ok(raw)}
fn kind(value:SaveOriginKind)->&'static str{match value{SaveOriginKind::Captured=>"captured",SaveOriginKind::AuthenticatedRemote=>"authenticated_remote",SaveOriginKind::Imported=>"imported",SaveOriginKind::LegacyProjection=>"legacy_projection"}}
fn parse_kind(value:&str)->Result<SaveOriginKind>{match value{"captured"=>Ok(SaveOriginKind::Captured),"authenticated_remote"=>Ok(SaveOriginKind::AuthenticatedRemote),"imported"=>Ok(SaveOriginKind::Imported),"legacy_projection"=>Ok(SaveOriginKind::LegacyProjection),_=>Err(corrupt())}}
fn save_kind(value:omniplay_contracts::SaveKind)->&'static str{match value{omniplay_contracts::SaveKind::InGame=>"in_game",omniplay_contracts::SaveKind::EmulatorState=>"emulator_state"}}
fn same_payload(a:&SaveMetadata,b:&SaveMetadata)->bool{
    a.game_content_hash==b.game_content_hash&&a.kind==b.kind&&a.slot==b.slot&&a.core_id==b.core_id&&a.core_version==b.core_version&&a.state_format_version==b.state_format_version&&a.platform==b.platform&&a.core_config_hash==b.core_config_hash&&a.payload_hash==b.payload_hash&&a.payload_bytes==b.payload_bytes
}
fn same_projection(local:&SaveMetadata,origin:&SaveMetadata)->bool{local.id==origin.id&&same_payload(local,origin)&&local.created_at_ms==origin.created_at_ms&&local.device_id==origin.device_id&&local.label==origin.label}
pub(crate) fn local_metadata(connection:&Connection,id:Uuid)->Result<SaveMetadata>{
    type Local=(String,String,String,u16,u64,Option<String>,String,u64,u64);
    let row:Option<Local>=connection.query_row("SELECT metadata_json,game_id,kind,slot,generation,parent_id,payload_hash,payload_bytes,created_at_ms FROM saves WHERE id=?1",[id.to_string()],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?))).optional().map_err(storage_error)?;
    let (raw,game,kind,slot,generation,parent,hash,bytes,created)=row.ok_or_else(||PlatformError::new(ErrorCode::NotFound,"The saved progress was not found"))?;let local=parse(&raw)?;
    if local.id!=id||local.game_id.to_string()!=game||save_kind(local.kind)!=kind||local.slot!=slot||local.generation!=generation||local.parent_id.map(|id|id.to_string())!=parent||local.payload_hash.0!=hash||local.payload_bytes!=bytes||local.created_at_ms!=created{return Err(corrupt());}Ok(local)
}
fn graph(connection:&Connection,origin:&SaveMetadata)->Result<()>{
    let parent=origin.parent_id.map(|id|id.to_string());
    let invalid_parent:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM saves s JOIN games g ON g.id=s.game_id WHERE s.id=?1 AND (g.content_hash!=?2 OR s.kind!=?3 OR s.slot!=?4))",params![parent,origin.game_content_hash.0,save_kind(origin.kind),origin.slot],|row|row.get(0)).map_err(storage_error)?;
    let invalid_child:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM save_origins o JOIN saves s ON s.id=o.save_id JOIN games g ON g.id=s.game_id WHERE o.canonical_parent_id=?1 AND (g.content_hash!=?2 OR s.kind!=?3 OR s.slot!=?4))",params![origin.id.to_string(),origin.game_content_hash.0,save_kind(origin.kind),origin.slot],|row|row.get(0)).map_err(storage_error)?;
    if invalid_parent||invalid_child{return Err(PlatformError::new(ErrorCode::ContentMismatch,"A canonical parent belongs to different saved progress"));}
    let referenced:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM save_origins WHERE canonical_parent_id=?1)",[origin.id.to_string()],|row|row.get(0)).map_err(storage_error)?;
    if !referenced{return Ok(());}
    // UNION deduplicates visited IDs, including a corrupt pre-existing cycle.
    // Missing parents stop the walk without rewriting the original reference.
    let bound=crate::save_maintenance::MAX_LOCAL_SAVE_COUNT.saturating_add(2);
    let (cycle,count):(bool,u64)=connection.query_row("WITH RECURSIVE ancestry(id) AS (SELECT ?1 WHERE ?1 IS NOT NULL UNION SELECT o.canonical_parent_id FROM save_origins o JOIN ancestry a ON a.id=o.save_id WHERE o.canonical_parent_id IS NOT NULL LIMIT ?3) SELECT EXISTS(SELECT 1 FROM ancestry WHERE id=?2),(SELECT count(*) FROM ancestry)",params![parent,origin.id.to_string(),bound],|row|Ok((row.get(0)?,row.get(1)?))).map_err(storage_error)?;
    if count>=bound{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Save ancestry exceeds its bounded history"));}
    if cycle{return Err(PlatformError::new(ErrorCode::Conflict,"A canonical save ancestry cannot contain a cycle"));}Ok(())
}
pub(crate) fn prepare(connection:&Connection,local:&SaveMetadata,source:NewOrigin)->Result<Prepared>{
    let (source_kind,canonical,imported)=match source{NewOrigin::Captured=>(SaveOriginKind::Captured,local.clone(),None),NewOrigin::Imported(previous)=>(SaveOriginKind::Imported,local.clone(),Some(previous)),NewOrigin::Remote(original)=>(SaveOriginKind::AuthenticatedRemote,original,None)};
    canonical.validate()?;if !same_projection(local,&canonical){return Err(conflict());}graph(connection,&canonical)?;
    if let Some(value)=&imported{value.validate()?;if value.id==canonical.id||!same_payload(value,&canonical){return Err(conflict());}}
    let canonical_hash=Some(save_origin_hash(&canonical)?);let imported_from_hash=imported.as_ref().map(save_origin_hash).transpose()?;
    Ok(Prepared{value:SaveOrigin{schema_version:1,save_id:local.id,kind:source_kind,canonical_metadata:Some(canonical),canonical_hash,imported_from:imported,imported_from_hash,missing_parent:false}})
}
pub(crate) fn insert(connection:&Connection,prepared:&Prepared)->Result<()>{
    let value=&prepared.value;let canonical=value.canonical_metadata.as_ref().ok_or_else(corrupt)?;let hash=value.canonical_hash.as_ref().ok_or_else(corrupt)?;
    let changed=connection.execute("UPDATE save_origins SET source_kind=?2,canonical_metadata_json=?3,canonical_hash=?4,canonical_parent_id=?5,imported_from_json=?6,imported_from_hash=?7 WHERE save_id=?1 AND source_kind='legacy_projection'",params![value.save_id.to_string(),kind(value.kind),encode(canonical)?,hash.0,canonical.parent_id.map(|id|id.to_string()),value.imported_from.as_ref().map(encode).transpose()?,value.imported_from_hash.as_ref().map(|hash|hash.0.as_str())]).map_err(storage_error)?;if changed!=1{return Err(corrupt());}Ok(())
}
fn origin(connection:&Connection,local:&SaveMetadata)->Result<SaveOrigin>{
    type Row=(String,Option<String>,Option<String>,Option<String>,Option<String>,Option<String>);
    let row:Option<Row>=connection.query_row("SELECT source_kind,canonical_metadata_json,canonical_hash,canonical_parent_id,imported_from_json,imported_from_hash FROM save_origins WHERE save_id=?1",[local.id.to_string()],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?))).optional().map_err(storage_error)?;
    let (source,canonical,hash,parent,imported,imported_hash)=row.ok_or_else(corrupt)?;let source=parse_kind(&source)?;
    let canonical=canonical.as_deref().map(parse).transpose()?;let hash=hash.map(ContentHash::parse).transpose().map_err(|_|corrupt())?;let imported=imported.as_deref().map(parse).transpose()?;let imported_hash=imported_hash.map(ContentHash::parse).transpose().map_err(|_|corrupt())?;
    if source==SaveOriginKind::LegacyProjection{if canonical.is_some()||hash.is_some()||parent.is_some()||imported.is_some()||imported_hash.is_some(){return Err(corrupt());}}
    else{let canonical=canonical.as_ref().ok_or_else(corrupt)?;if !same_projection(local,canonical)||hash.as_ref()!=Some(&save_origin_hash(canonical)?)||parent!=canonical.parent_id.map(|id|id.to_string()){return Err(corrupt());}}
    if (source==SaveOriginKind::Imported)!=imported.is_some()||imported.is_some()!=imported_hash.is_some(){return Err(corrupt());}
    if let Some(value)=&imported{if value.id==local.id||imported_hash.as_ref()!=Some(&save_origin_hash(value)?)||!same_payload(local,value){return Err(corrupt());}}
    let missing_parent=if let Some(parent)=canonical.as_ref().and_then(|value|value.parent_id){!connection.query_row("SELECT EXISTS(SELECT 1 FROM saves s JOIN games g ON g.id=s.game_id WHERE s.id=?1 AND g.content_hash=?2 AND s.kind=?3 AND s.slot=?4)",params![parent.to_string(),local.game_content_hash.0,save_kind(local.kind),local.slot],|row|row.get::<_,bool>(0)).map_err(storage_error)?}else{false};
    Ok(SaveOrigin{schema_version:1,save_id:local.id,kind:source,canonical_metadata:canonical,canonical_hash:hash,imported_from:imported,imported_from_hash:imported_hash,missing_parent})
}
pub fn get(database:&Database,id:Uuid)->Result<SaveOrigin>{let transaction=database.connection.unchecked_transaction().map_err(storage_error)?;let local=local_metadata(&transaction,id)?;let value=origin(&transaction,&local)?;transaction.commit().map_err(storage_error)?;Ok(value)}
/// Reuse an existing caller transaction without opening a nested one.
pub fn inspect(connection:&Connection,id:Uuid)->Result<(SaveMetadata,SaveOrigin)>{let local=local_metadata(connection,id)?;let value=origin(connection,&local)?;Ok((local,value))}
pub fn for_sync(database:&Database,id:Uuid)->Result<(SaveMetadata,bool)>{let transaction=database.connection.unchecked_transaction().map_err(storage_error)?;let local=local_metadata(&transaction,id)?;let value=origin(&transaction,&local)?;transaction.commit().map_err(storage_error)?;match value.canonical_metadata{Some(metadata)=>Ok((metadata,true)),None=>Ok((local,false))}}
/// Trusted callers must already authenticate/decrypt the remote envelope and
/// verify its payload. This is not exposed as a renderer write command.
pub(crate) fn attach_remote(connection:&Connection,local:&SaveMetadata,canonical:SaveMetadata)->Result<()>{
    let current=local_metadata(connection,local.id)?;if !same_projection(local,&current){return Err(conflict());}let local=&current;
    let existing=origin(connection,local)?;
    if let Some(hash)=existing.canonical_hash{if hash!=save_origin_hash(&canonical)?{return Err(conflict());}return Ok(());}
    let prepared=prepare(connection,local,NewOrigin::Remote(canonical))?;
    let canonical=prepared.value.canonical_metadata.as_ref().ok_or_else(corrupt)?;let hash=prepared.value.canonical_hash.as_ref().ok_or_else(corrupt)?;
    let changed=connection.execute("UPDATE save_origins SET source_kind='authenticated_remote',canonical_metadata_json=?1,canonical_hash=?2,canonical_parent_id=?3 WHERE save_id=?4 AND source_kind='legacy_projection'",params![encode(canonical)?,hash.0,canonical.parent_id.map(|id|id.to_string()),local.id.to_string()]).map_err(storage_error)?;
    if changed!=1{return Err(conflict());}Ok(())
}
