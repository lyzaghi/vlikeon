use crate::{Database, util::{bounded_read, digest, storage_error}};
use omniplay_contracts::{ErrorCode, PlatformError, Result, SaveCompatibility, SaveKind, SaveMetadata};
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use std::{fs::File, io::{Read, Write,Seek,SeekFrom}};
use uuid::Uuid;
use base64::{Engine,engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Serialize,Deserialize};

const EXPORT_MAGIC: &[u8; 8] = b"OPSV0001";
const MAX_METADATA_BYTES: u32 = 64 * 1024;
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryCursor{version:u32,game_id:Uuid,generation:u64,created_at_ms:u64,id:Uuid}

pub struct SaveStore<'a> { database: &'a mut Database }

impl<'a> SaveStore<'a> {
    pub fn new(database: &'a mut Database) -> Self { Self { database } }

    pub fn commit(&mut self, metadata: SaveMetadata, payload: &[u8], expected_parent: Option<Uuid>) -> Result<SaveMetadata> {
        self.commit_origin(metadata,payload,expected_parent,crate::save_origins::NewOrigin::Captured)
    }
    fn commit_origin(&mut self, mut metadata:SaveMetadata,payload:&[u8],expected_parent:Option<Uuid>,origin:crate::save_origins::NewOrigin)->Result<SaveMetadata>{
        metadata.validate()?;
        if payload.len() as u64 != metadata.payload_bytes || digest(payload) != metadata.payload_hash {
            return Err(PlatformError::new(ErrorCode::ContentMismatch, "Save payload does not match its metadata"));
        }
        if metadata.parent_id != expected_parent { return Err(PlatformError::new(ErrorCode::InvalidRequest, "Save parent does not match its expected parent")); }
        let encoded=serde_json::to_vec(&metadata).map_err(storage_error)?;if encoded.len()>MAX_METADATA_BYTES as usize{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Save metadata is too large"));}
        let kind = kind_name(metadata.kind);
        let staged=crate::save_payloads::stage(self.database,&metadata,payload)?;
        let transaction = self.database.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage_error)?;
        let game_hash: Option<String> = transaction.query_row("SELECT content_hash FROM games WHERE id=?1", [metadata.game_id.to_string()], |row| row.get(0)).optional().map_err(storage_error)?;
        if game_hash.as_deref() != Some(metadata.game_content_hash.0.as_str()) { return Err(PlatformError::new(ErrorCode::ContentMismatch, "Save game content is not in this library")); }
        let removed:bool=transaction.query_row("SELECT EXISTS(SELECT 1 FROM locally_removed_saves WHERE save_id=?1)",[metadata.id.to_string()],|row|row.get(0)).map_err(storage_error)?;
        if removed{return Err(PlatformError::new(ErrorCode::Conflict,"This save identity was intentionally removed locally"));}
        let current: Option<(String, u64)> = transaction.query_row("SELECT s.id,s.generation FROM save_heads h JOIN saves s ON h.save_id=s.id WHERE h.game_id=?1 AND h.kind=?2 AND h.slot=?3", params![metadata.game_id.to_string(), kind, metadata.slot], |row| Ok((row.get(0)?,row.get(1)?))).optional().map_err(storage_error)?;
        let current_id = current.as_ref().map(|(id,_)| Uuid::parse_str(id)).transpose().map_err(|_| PlatformError::new(ErrorCode::CorruptData, "Save head is invalid"))?;
        if let Some(parent) = expected_parent {
            let valid_parent: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM saves WHERE id=?1 AND game_id=?2 AND kind=?3 AND slot=?4)",params![parent.to_string(),metadata.game_id.to_string(),kind,metadata.slot],|row|row.get(0)).map_err(storage_error)?;
            if !valid_parent { return Err(PlatformError::new(ErrorCode::ContentMismatch,"Save parent belongs to a different game, slot or save type")); }
        }
        // Preserve concurrent progress as a branch. Wall-clock ordering never decides
        // which player's progress wins. The transaction assigns a local sequence.
        let greatest: u64 = transaction.query_row("SELECT coalesce(max(generation),0) FROM saves WHERE game_id=?1 AND kind=?2 AND slot=?3",params![metadata.game_id.to_string(),kind,metadata.slot],|row|row.get(0)).map_err(storage_error)?;
        metadata.generation = greatest.checked_add(1).filter(|value|*value<=9_007_199_254_740_991).ok_or_else(||PlatformError::new(ErrorCode::ResourceLimit,"Save generation limit reached"))?;
        metadata.conflict = current_id != expected_parent || metadata.conflict;
        let origin=crate::save_origins::prepare(&transaction,&metadata,origin)?;
        let json = serde_json::to_string(&metadata).map_err(storage_error)?;
        transaction.execute("INSERT INTO saves(id,game_id,kind,slot,generation,parent_id,metadata_json,payload_hash,payload_bytes,created_at_ms) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)", params![metadata.id.to_string(), metadata.game_id.to_string(), kind, metadata.slot, metadata.generation, metadata.parent_id.map(|id|id.to_string()), json, metadata.payload_hash.0, metadata.payload_bytes, metadata.created_at_ms]).map_err(storage_error)?;
        crate::save_origins::insert(&transaction,&origin)?;
        staged.publish(&transaction,&metadata)?;
        if !metadata.conflict {
            transaction.execute("INSERT INTO save_heads(game_id,kind,slot,save_id) VALUES(?1,?2,?3,?4) ON CONFLICT(game_id,kind,slot) DO UPDATE SET save_id=excluded.save_id", params![metadata.game_id.to_string(),kind,metadata.slot,metadata.id.to_string()]).map_err(storage_error)?;
        }
        transaction.execute("INSERT INTO audit(event,entity_id,created_at_ms) VALUES('save.commit',?1,?2)", params![metadata.id.to_string(), crate::now_ms()]).map_err(storage_error)?;
        transaction.commit().map_err(storage_error)?;
        Ok(metadata)
    }

    pub fn metadata(&self, id: Uuid) -> Result<SaveMetadata> {
        crate::save_origins::local_metadata(&self.database.connection,id)
    }

    pub fn list(&self, game_id: Uuid) -> Result<Vec<SaveMetadata>> {
        let mut statement = self.database.connection.prepare("SELECT metadata_json FROM saves WHERE game_id=?1 ORDER BY generation DESC,created_at_ms DESC LIMIT 500").map_err(storage_error)?;
        let rows = statement.query_map([game_id.to_string()], |row| row.get::<_,String>(0)).map_err(storage_error)?;
        rows.map(|row| parse_metadata(row.map_err(storage_error)?.as_bytes())).collect()
    }

    pub fn heads(&self,game_id:Uuid)->Result<Vec<SaveMetadata>> {
        let mut statement=self.database.connection.prepare("SELECT s.metadata_json FROM save_heads h JOIN saves s ON h.save_id=s.id WHERE h.game_id=?1 ORDER BY h.kind,h.slot").map_err(storage_error)?;
        let rows=statement.query_map([game_id.to_string()],|row|row.get::<_,String>(0)).map_err(storage_error)?;
        rows.map(|row|parse_metadata(row.map_err(storage_error)?.as_bytes())).collect()
    }

    pub fn page(&self,game_id:Uuid,after:Option<&str>,limit:u16)->Result<omniplay_contracts::SaveHistoryPage>{
        let invalid=||PlatformError::new(ErrorCode::InvalidRequest,"Save history cursor or page bound is invalid");
        if limit==0||limit>100||after.is_some_and(|raw|raw.len()>1024){return Err(invalid());}
        let cursor=after.map(|raw|->Result<HistoryCursor>{let bytes=URL_SAFE_NO_PAD.decode(raw).map_err(|_|invalid())?;if URL_SAFE_NO_PAD.encode(&bytes)!=raw{return Err(invalid());}let cursor:HistoryCursor=serde_json::from_slice(&bytes).map_err(|_|invalid())?;if cursor.version!=1||cursor.game_id!=game_id||cursor.generation==0||cursor.generation>9_007_199_254_740_991||cursor.created_at_ms>9_007_199_254_740_991{return Err(invalid());}Ok(cursor)}).transpose()?;
        let tx=self.database.connection.unchecked_transaction().map_err(storage_error)?;
        let exists:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM games WHERE id=?1)",[game_id.to_string()],|row|row.get(0)).map_err(storage_error)?;if !exists{return Err(PlatformError::new(ErrorCode::NotFound,"Game was not found"));}
        let total=tx.query_row("SELECT count(*) FROM saves WHERE game_id=?1",[game_id.to_string()],|row|row.get(0)).map_err(storage_error)?;
        let mut statement=tx.prepare("SELECT s.metadata_json,h.save_id,s.id,s.generation,s.created_at_ms FROM saves s LEFT JOIN save_heads h ON h.game_id=s.game_id AND h.kind=s.kind AND h.slot=s.slot WHERE s.game_id=?1 AND (?2=0 OR s.generation<?3 OR (s.generation=?3 AND s.created_at_ms<?4) OR (s.generation=?3 AND s.created_at_ms=?4 AND s.id<?5)) ORDER BY s.generation DESC,s.created_at_ms DESC,s.id DESC LIMIT ?6").map_err(storage_error)?;
        let rows=statement.query_map(params![game_id.to_string(),cursor.is_some(),cursor.as_ref().map(|c|c.generation).unwrap_or(0),cursor.as_ref().map(|c|c.created_at_ms).unwrap_or(0),cursor.as_ref().map(|c|c.id.to_string()).unwrap_or_default(),u32::from(limit)+1],|row|Ok((row.get::<_,String>(0)?,row.get::<_,Option<String>>(1)?,row.get::<_,String>(2)?,row.get::<_,u64>(3)?,row.get::<_,u64>(4)?))).map_err(storage_error)?;
        let mut entries=Vec::new();let mut bytes=0usize;let mut last=None;let mut more=false;
        for row in rows{
            let(raw,head,id,generation,created)=row.map_err(storage_error)?;
            let metadata=parse_metadata(raw.as_bytes())?;if metadata.game_id!=game_id||metadata.id.to_string()!=id||metadata.generation!=generation||metadata.created_at_ms!=created{return Err(PlatformError::new(ErrorCode::CorruptData,"Save history content identity is inconsistent"));}
            if entries.len()>=usize::from(limit)||bytes.saturating_add(raw.len()+128)>256*1024{more=true;break;}
            let current_head_id=head.map(|id|Uuid::parse_str(&id).map_err(storage_error)).transpose()?;
            last=Some(HistoryCursor{version:1,game_id,generation:metadata.generation,created_at_ms:metadata.created_at_ms,id:metadata.id});bytes+=raw.len()+128;
            entries.push(omniplay_contracts::SaveHistoryEntry{metadata,current_head_id});
        }
        drop(statement);tx.commit().map_err(storage_error)?;
        let next_cursor=if more{last.map(|cursor|serde_json::to_vec(&cursor).map(|bytes|URL_SAFE_NO_PAD.encode(bytes)).map_err(storage_error)).transpose()?}else{None};
        Ok(omniplay_contracts::SaveHistoryPage{entries,next_cursor,total})
    }

    pub fn promote(&mut self,id:Uuid,expected_head:Option<Uuid>)->Result<SaveMetadata> {
        // Check the actual bytes before allowing this generation to become current.
        let (mut metadata,_)=self.read(id,None)?;
        let kind=kind_name(metadata.kind);
        let transaction=self.database.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage_error)?;
        let existing:Option<String>=transaction.query_row("SELECT save_id FROM save_heads WHERE game_id=?1 AND kind=?2 AND slot=?3",params![metadata.game_id.to_string(),kind,metadata.slot],|row|row.get(0)).optional().map_err(storage_error)?;
        if existing!=expected_head.map(|value|value.to_string()) {return Err(PlatformError::new(ErrorCode::Conflict,"Current progress changed while choosing a save").recover("Refresh the save history and choose again"));}
        metadata.conflict=false;
        transaction.execute("UPDATE saves SET metadata_json=?1 WHERE id=?2",params![serde_json::to_string(&metadata).map_err(storage_error)?,id.to_string()]).map_err(storage_error)?;
        transaction.execute("INSERT INTO save_heads(game_id,kind,slot,save_id) VALUES(?1,?2,?3,?4) ON CONFLICT(game_id,kind,slot) DO UPDATE SET save_id=excluded.save_id",params![metadata.game_id.to_string(),kind,metadata.slot,id.to_string()]).map_err(storage_error)?;
        transaction.execute("INSERT INTO audit(event,entity_id,created_at_ms) VALUES('save.promote',?1,?2)",params![id.to_string(),crate::now_ms()]).map_err(storage_error)?;
        transaction.commit().map_err(storage_error)?;
        Ok(metadata)
    }

    pub fn import_export(&mut self,game_id:Uuid,source:&mut impl Read)->Result<SaveMetadata> {
        let (mut metadata,payload)=Self::decode_export(source)?;
        let imported_from=metadata.clone();
        metadata.id=Uuid::new_v4();
        metadata.game_id=game_id;
        metadata.parent_id=None;
        metadata.generation=1;
        metadata.conflict=false;
        metadata.label=Some("Imported save".into());
        // Existing heads cause a preserved conflict rather than being overwritten.
        self.commit_origin(metadata,&payload,None,crate::save_origins::NewOrigin::Imported(imported_from))
    }

    pub fn ingest_remote(&mut self,mut metadata:SaveMetadata,payload:&[u8])->Result<Option<SaveMetadata>> {
        metadata.validate()?;
        if payload.len() as u64!=metadata.payload_bytes||digest(payload)!=metadata.payload_hash {return Err(PlatformError::new(ErrorCode::CorruptData,"Remote save integrity check failed"));}
        if crate::save_maintenance::locally_removed(self.database,metadata.id)?{return Ok(None);}
        let Some(game)=crate::library::Library::new(self.database).by_hash(&metadata.game_content_hash)? else{return Ok(None);};
        let exists:bool=self.database.connection.query_row("SELECT EXISTS(SELECT 1 FROM saves WHERE id=?1)",[metadata.id.to_string()],|row|row.get(0)).map_err(storage_error)?;
        if exists {
            let existing=self.metadata(metadata.id)?;
            if existing.game_content_hash!=metadata.game_content_hash||existing.payload_hash!=metadata.payload_hash||existing.kind!=metadata.kind||existing.slot!=metadata.slot||existing.platform!=metadata.platform||existing.core_id!=metadata.core_id||existing.core_version!=metadata.core_version||existing.state_format_version!=metadata.state_format_version||existing.core_config_hash!=metadata.core_config_hash {return Err(PlatformError::new(ErrorCode::Conflict,"A remote save identity conflicts with different local progress"));}
            return crate::save_payloads::repair(self.database,&existing,payload,Some(metadata)).map(|result|Some(result.metadata));
        }
        let original=metadata.clone();
        metadata.game_id=game.id;
        if let Some(parent)=metadata.parent_id {
            let parent_exists:bool=self.database.connection.query_row("SELECT EXISTS(SELECT 1 FROM saves WHERE id=?1 AND game_id=?2 AND kind=?3 AND slot=?4)",params![parent.to_string(),game.id.to_string(),kind_name(metadata.kind),metadata.slot],|row|row.get(0)).map_err(storage_error)?;
            if !parent_exists {metadata.parent_id=None;}
        }
        let parent=metadata.parent_id;
        self.commit_origin(metadata,payload,parent,crate::save_origins::NewOrigin::Remote(original)).map(Some)
    }

    fn verify_payload(&self,metadata:&SaveMetadata)->Result<()>{
        crate::save_payloads::open(self.database,metadata).map(|_|())
    }
    pub fn open_payload(&self,metadata:&SaveMetadata)->Result<File>{crate::save_payloads::open(self.database,metadata)}
    /// A selected OPSV file can restore only the bytes of its exact existing
    /// identity. It never rewrites metadata or upgrades untrusted provenance.
    pub fn repair_export(&mut self,id:Uuid,source:&mut impl Read)->Result<omniplay_contracts::SaveRepairResult>{
        let (input,payload)=Self::decode_export(source)?;let local=self.metadata(id)?;
        if input.id!=id||input.game_content_hash!=local.game_content_hash||input.kind!=local.kind||input.slot!=local.slot||input.core_id!=local.core_id||input.core_version!=local.core_version||input.state_format_version!=local.state_format_version||input.platform!=local.platform||input.core_config_hash!=local.core_config_hash||input.payload_hash!=local.payload_hash||input.payload_bytes!=local.payload_bytes{return Err(PlatformError::new(ErrorCode::ContentMismatch,"This recovery file belongs to different saved progress"));}
        crate::save_payloads::repair(self.database,&local,&payload,None)
    }
    /// Only trusted synchronization callers may supply an authenticated origin.
    /// There is intentionally no renderer command to write or replace one.
    pub fn attach_authenticated_origin(&mut self,canonical:SaveMetadata)->Result<()>{
        canonical.validate()?;let local=self.metadata(canonical.id)?;self.verify_payload(&local)?;
        let transaction=self.database.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage_error)?;
        crate::save_origins::attach_remote(&transaction,&local,canonical)?;transaction.commit().map_err(storage_error)
    }
    pub fn read_for_sync(&self,id:Uuid)->Result<(SaveMetadata,Vec<u8>,bool)>{
        let (metadata,canonical)=crate::save_origins::for_sync(self.database,id)?;let (_,payload)=self.read(id,None)?;Ok((metadata,payload,canonical))
    }
    /// Pins verified bytes without a second full payload allocation. Projection
    /// and canonical origin remain separate; callers compare the returned origin
    /// to their outgoing fingerprint before committing a transport slot.
    pub fn open_for_sync(&self,id:Uuid)->Result<(SaveMetadata,File,bool)>{
        let local=self.metadata(id)?;let file=self.open_payload(&local)?;
        let (metadata,canonical)=crate::save_origins::for_sync(self.database,id)?;Ok((metadata,file,canonical))
    }

    pub fn read(&self, id: Uuid, compatibility: Option<&SaveCompatibility>) -> Result<(SaveMetadata,Vec<u8>)> {
        let metadata = self.metadata(id)?;
        if let Some(target) = compatibility { metadata.check_compatible(target)?; }
        let mut file = self.open_payload(&metadata)?;
        let payload = bounded_read(&mut file, omniplay_contracts::MAX_SAVE_BYTES)?;
        if payload.len() as u64 != metadata.payload_bytes || digest(&payload) != metadata.payload_hash { return Err(PlatformError::new(ErrorCode::CorruptData, "Save integrity check failed").recover("Choose an earlier saved generation")); }
        Ok((metadata,payload))
    }

    pub fn export(&self, id: Uuid, destination: &mut File) -> Result<()> {
        let (metadata,payload,_) = self.read_for_sync(id)?;
        write_export(&metadata,&payload,destination)
    }

    pub fn decode_export(source: &mut impl Read) -> Result<(SaveMetadata,Vec<u8>)> {
        let mut header = [0u8;12];
        source.read_exact(&mut header).map_err(|_| PlatformError::new(ErrorCode::CorruptData,"Save export header is truncated"))?;
        if &header[..8] != EXPORT_MAGIC { return Err(PlatformError::new(ErrorCode::CorruptData,"Unknown save export format")); }
        let length = u32::from_le_bytes(header[8..12].try_into().expect("fixed-size header"));
        if length == 0 || length > MAX_METADATA_BYTES { return Err(PlatformError::new(ErrorCode::ResourceLimit,"Save metadata size is invalid")); }
        let mut json = vec![0u8;length as usize];
        source.read_exact(&mut json).map_err(|_| PlatformError::new(ErrorCode::CorruptData,"Save metadata is truncated"))?;
        let metadata = parse_metadata(&json)?;
        let payload = bounded_read(source,omniplay_contracts::MAX_SAVE_BYTES)?;
        if payload.len() as u64 != metadata.payload_bytes || digest(&payload) != metadata.payload_hash { return Err(PlatformError::new(ErrorCode::CorruptData,"Save export integrity check failed")); }
        Ok((metadata,payload))
    }
}

// Publish the magic only after complete, verified bytes have been flushed. A
// cancelled/incomplete export must not masquerade as a complete OPSV envelope.
pub fn export_prefix(metadata:&SaveMetadata)->Result<Vec<u8>>{
    metadata.validate()?;let json=serde_json::to_vec(metadata).map_err(storage_error)?;
    if json.len()>MAX_METADATA_BYTES as usize{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Save metadata is too large"));}
    let mut output=vec![0u8;8];output.extend_from_slice(&(json.len()as u32).to_le_bytes());output.extend_from_slice(&json);Ok(output)
}
pub fn finalize_export(destination:&mut File)->Result<()>{destination.sync_all().map_err(storage_error)?;destination.seek(SeekFrom::Start(0)).map_err(storage_error)?;destination.write_all(EXPORT_MAGIC).map_err(storage_error)?;destination.sync_all().map_err(storage_error)}
pub fn write_export(metadata:&SaveMetadata,payload:&[u8],destination:&mut File)->Result<()>{
    if metadata.payload_bytes!=payload.len()as u64||digest(payload)!=metadata.payload_hash{return Err(PlatformError::new(ErrorCode::ContentMismatch,"Export bytes do not match their metadata"));}
    let prefix=export_prefix(metadata)?;destination.seek(SeekFrom::Start(0)).map_err(storage_error)?;destination.set_len(0).map_err(storage_error)?;destination.write_all(&prefix).map_err(storage_error)?;destination.write_all(payload).map_err(storage_error)?;finalize_export(destination)
}

pub fn kind_name(kind: SaveKind) -> &'static str { match kind { SaveKind::InGame => "in_game", SaveKind::EmulatorState => "emulator_state" } }

fn parse_metadata(bytes: &[u8]) -> Result<SaveMetadata> {
    if bytes.len() > MAX_METADATA_BYTES as usize { return Err(PlatformError::new(ErrorCode::ResourceLimit,"Save metadata is too large")); }
    let metadata: SaveMetadata = serde_json::from_slice(bytes).map_err(|_| PlatformError::new(ErrorCode::CorruptData,"Save metadata is invalid"))?;
    metadata.validate()?;
    Ok(metadata)
}
