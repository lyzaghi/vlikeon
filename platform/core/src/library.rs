use crate::{Database, now_ms, util::{PendingFile, storage_error}};
use omniplay_contracts::{ContentHash,ContentFormat,GameMedia,MediaAsset,ImportProgress,ErrorCode,GameManifest,GameSystem,PlatformError,Result};
use rusqlite::{OptionalExtension,Transaction,TransactionBehavior,params};
use sha2::{Digest, Sha256};
use std::{fs::File, io::{Read, Seek, SeekFrom, Write}, path::PathBuf};
use uuid::Uuid;
use std::sync::atomic::{AtomicBool,Ordering};
use base64::{Engine,engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Serialize,Deserialize};

#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {version:u32,title:String,id:Uuid,filter_hash:ContentHash}

pub struct Library<'a> { database: &'a Database }

impl<'a> Library<'a> {
    pub fn new(database: &'a Database) -> Self { Self { database } }

    pub fn import(&self,source:File,title:String,system:GameSystem)->Result<GameManifest>{self.import_controlled(source,title,system,None,&AtomicBool::new(false),|_|{})}

    pub fn import_controlled(&self,mut source:File,title:String,system:GameSystem,format_hint:Option<ContentFormat>,cancel:&AtomicBool,mut progress:impl FnMut(ImportProgress))->Result<GameManifest>{
        let _asset_lease=crate::asset_storage::AssetLease::acquire(self.database)?;
        if title.trim().is_empty() || title.len() > 512 { return Err(PlatformError::new(ErrorCode::InvalidRequest, "Game title is required and must fit within 512 bytes")); }
        let format=detect_format(&mut source)?.or(format_hint).unwrap_or(ContentFormat::RawRom);
        let extension=format.extension(system)?;
        if matches!(format,ContentFormat::Cue|ContentFormat::M3u){return Err(PlatformError::new(ErrorCode::UnsupportedCapability,"Import a cue sheet or playlist with the disc-folder picker so its referenced tracks are explicitly authorized"));}
        source.seek(SeekFrom::Start(0)).map_err(storage_error)?;
        let expected=source.metadata().map_err(storage_error)?.len();
        if expected==0||expected>omniplay_contracts::MAX_ROM_BYTES{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Content exceeds the import limit"));}
        let directory = self.database.root.join("assets");
        let mut pending = PendingFile::create(&directory)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        let mut total = 0u64;
        loop {
            if cancel.load(Ordering::Relaxed){return Err(PlatformError::new(ErrorCode::Cancelled,"Game import was cancelled"));}
            let count = source.read(&mut buffer).map_err(storage_error)?;
            if count == 0 { break; }
            total = total.checked_add(count as u64).ok_or_else(|| PlatformError::new(ErrorCode::ResourceLimit, "Content is too large"))?;
            if total > omniplay_contracts::MAX_ROM_BYTES { return Err(PlatformError::new(ErrorCode::ResourceLimit, "Content exceeds the import limit")); }
            hasher.update(&buffer[..count]);
            pending.file.write_all(&buffer[..count]).map_err(storage_error)?;
            progress(ImportProgress{phase:"copying".into(),copied_bytes:total,total_bytes:expected,asset_index:1,asset_count:1});
        }
        let hash = ContentHash(format!("{:x}", hasher.finalize()));
        if total!=expected{return Err(PlatformError::new(ErrorCode::ContentMismatch,"Content changed during import"));}
        let name=format!("game.{extension}");let media=GameMedia{schema_version:1,format,entry_name:name.clone(),assets:vec![MediaAsset{name,content_hash:hash.clone(),byte_length:total}],disc_entries:Vec::new()};
        let manifest = GameManifest { schema_version: 1, id: Uuid::new_v4(), title: title.trim().into(), system, content_hash: hash.clone(), byte_length: total, region: None, nominal_refresh_hz: None, created_at_ms: now_ms(),media:Some(media) };
        manifest.validate()?;
        let target = directory.join(&hash.0);
        if target.exists() {
            let mut file = File::open(&target).map_err(storage_error)?;
            let (existing_hash, size) = crate::util::hash_file(&mut file, omniplay_contracts::MAX_ROM_BYTES)?;
            if existing_hash != hash || size != total { return Err(PlatformError::new(ErrorCode::CorruptData, "Imported asset conflicts with local content")); }
        } else { pending.commit_new(&target)?; }
        if cancel.load(Ordering::Relaxed){return Err(PlatformError::new(ErrorCode::Cancelled,"Game import was cancelled before publication"));}
        self.insert_manifest(manifest)
    }

    pub(crate) fn insert_manifest(&self,manifest:GameManifest)->Result<GameManifest>{
        manifest.validate()?;
        // Serialize deduplication with publication across import workers and
        // brokers. The unique hash constraint is the final invariant, not the
        // normal conflict resolution after a stale pre-insert lookup.
        let tx=Transaction::new_unchecked(&self.database.connection,TransactionBehavior::Immediate).map_err(storage_error)?;
        if let Some(mut existing)=self.by_hash(&manifest.content_hash)?{
            if existing.system!=manifest.system{return Err(PlatformError::new(ErrorCode::Conflict,"The same content is already registered for another system"));}
            if existing.media.is_none()&&manifest.media.is_some(){existing.media=manifest.media;existing.validate()?;self.database.connection.execute("UPDATE games SET manifest_json=?1 WHERE id=?2",params![serde_json::to_string(&existing).map_err(storage_error)?,existing.id.to_string()]).map_err(storage_error)?;}
            tx.commit().map_err(storage_error)?;return Ok(existing);
        }
        self.database.connection.execute("INSERT INTO games(id,content_hash,title,search_title,system,manifest_json,created_at_ms) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![manifest.id.to_string(),manifest.content_hash.0,manifest.title,manifest.title.to_lowercase(),serde_json::to_string(&manifest.system).map_err(storage_error)?,serde_json::to_string(&manifest).map_err(storage_error)?,manifest.created_at_ms]).map_err(storage_error)?;
        self.database.record_audit("library.import",Some(manifest.id))?;tx.commit().map_err(storage_error)?;Ok(manifest)
    }

    pub fn get(&self, id: Uuid) -> Result<GameManifest> {
        let json: Option<String> = self.database.connection.query_row("SELECT manifest_json FROM games WHERE id=?1", [id.to_string()], |row| row.get(0)).optional().map_err(storage_error)?;
        decode(json.ok_or_else(|| PlatformError::new(ErrorCode::NotFound, "Game was not found"))?)
    }

    pub fn by_hash(&self, hash: &ContentHash) -> Result<Option<GameManifest>> {
        let json: Option<String> = self.database.connection.query_row("SELECT manifest_json FROM games WHERE content_hash=?1", [&hash.0], |row| row.get(0)).optional().map_err(storage_error)?;
        json.map(decode).transpose()
    }

    pub fn list(&self, query: Option<&str>, offset: u32, limit: u16) -> Result<Vec<GameManifest>> {
        if limit == 0 || limit > 200 || query.is_some_and(|q| q.len() > 512) { return Err(PlatformError::new(ErrorCode::InvalidRequest, "Invalid library query")); }
        // instr() treats percent and underscore literally, unlike LIKE wildcard searches.
        let mut statement = self.database.connection.prepare("SELECT manifest_json FROM games WHERE (?1='' OR instr(search_title,?1)>0) ORDER BY search_title,id LIMIT ?2 OFFSET ?3").map_err(storage_error)?;
        let rows = statement.query_map(params![query.unwrap_or("").to_lowercase(), limit, offset], |row| row.get::<_, String>(0)).map_err(storage_error)?;
        rows.map(|row| decode(row.map_err(storage_error)?)).collect()
    }

    pub fn favorite(&self,id:Uuid)->Result<omniplay_contracts::LibraryFavorite>{
        let value:Option<bool>=self.database.connection.query_row("SELECT favorite FROM games WHERE id=?1",[id.to_string()],|row|row.get(0)).optional().map_err(storage_error)?;
        Ok(omniplay_contracts::LibraryFavorite{game_id:id,favorite:value.ok_or_else(||PlatformError::new(ErrorCode::NotFound,"Game was not found"))?})
    }
    pub fn set_favorite(&self,id:Uuid,favorite:bool)->Result<omniplay_contracts::LibraryFavorite>{
        let tx=self.database.connection.unchecked_transaction().map_err(storage_error)?;
        if tx.execute("UPDATE games SET favorite=?1 WHERE id=?2",params![favorite,id.to_string()]).map_err(storage_error)?==0{return Err(PlatformError::new(ErrorCode::NotFound,"Game was not found"));}
        self.database.record_audit("library.favorite",Some(id))?;tx.commit().map_err(storage_error)?;
        Ok(omniplay_contracts::LibraryFavorite{game_id:id,favorite})
    }
    pub fn page(&self,query:Option<&str>,system:Option<GameSystem>,after:Option<&str>,limit:u16,favorites_only:bool)->Result<omniplay_contracts::LibraryPage>{
        if limit==0||limit>200||query.is_some_and(|q|q.len()>512)||after.is_some_and(|c|c.len()>2048){return Err(PlatformError::new(ErrorCode::InvalidRequest,"Invalid library page query"));}
        let query=query.unwrap_or("").to_lowercase();let system=system.map(|s|serde_json::to_string(&s).map_err(storage_error)).transpose()?.unwrap_or_default();
        let filter_hash=crate::util::digest(&serde_json::to_vec(&(&query,&system,favorites_only)).map_err(storage_error)?);
        let cursor=after.map(|raw|->Result<Cursor>{let raw=URL_SAFE_NO_PAD.decode(raw).map_err(|_|PlatformError::new(ErrorCode::InvalidRequest,"Library cursor encoding is invalid"))?;let cursor:Cursor=serde_json::from_slice(&raw).map_err(|_|PlatformError::new(ErrorCode::InvalidRequest,"Library cursor is invalid"))?;if cursor.version!=2||cursor.title.len()>2048||cursor.filter_hash!=filter_hash{return Err(PlatformError::new(ErrorCode::InvalidRequest,"Library cursor belongs to a different query"));}Ok(cursor)}).transpose()?;
        let tx=self.database.connection.unchecked_transaction().map_err(storage_error)?;
        let total_all=tx.query_row("SELECT count(*) FROM games",[],|r|r.get(0)).map_err(storage_error)?;
        // Only fixed clauses are interpolated. Values always remain bound.
        // Equality predicates expose the compound indexes for selected filters.
        let system_filter=if system.is_empty(){"?2=''"}else{"system=?2"};
        let count_favorite=if favorites_only{"?3=1 AND favorite=1"}else{"?3=0"};
        let page_favorite=if favorites_only{"?7=1 AND favorite=1"}else{"?7=0"};
        let count_sql=format!("SELECT count(*) FROM games WHERE (?1='' OR instr(search_title,?1)>0) AND ({system_filter}) AND ({count_favorite})");
        let total_matching=tx.query_row(&count_sql,params![query,system,favorites_only],|r|r.get(0)).map_err(storage_error)?;
        let page_sql=format!("SELECT manifest_json,search_title,last_played_at_ms,playtime_ms,favorite FROM games WHERE (?1='' OR instr(search_title,?1)>0) AND ({system_filter}) AND ({page_favorite}) AND (?3=0 OR search_title>?4 OR (search_title=?4 AND id>?5)) ORDER BY search_title,id LIMIT ?6");
        let mut statement=tx.prepare(&page_sql).map_err(storage_error)?;
        let rows=statement.query_map(params![query,system,cursor.is_some(),cursor.as_ref().map(|c|c.title.as_str()).unwrap_or(""),cursor.as_ref().map(|c|c.id.to_string()).unwrap_or_default(),u32::from(limit)+1,favorites_only],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,Option<u64>>(2)?,r.get::<_,u64>(3)?,r.get::<_,bool>(4)?))).map_err(storage_error)?;
        let mut games=Vec::new();let mut last=None;let mut has_more=false;
        for row in rows {
            if games.len()>=usize::from(limit){has_more=true;break;}
            let(raw,title,last_played_at_ms,playtime_ms,favorite)=row.map_err(storage_error)?;let game=decode(raw)?;
            last=Some(Cursor{version:2,title,id:game.id,filter_hash:filter_hash.clone()});
            games.push(omniplay_contracts::GameSummary{id:game.id,title:game.title,system:game.system,content_hash:game.content_hash,byte_length:game.byte_length,created_at_ms:game.created_at_ms,last_played_at_ms,playtime_ms,favorite,content_format:game.media.as_ref().map(|m|m.format),asset_count:game.media.as_ref().map(|m|m.assets.len()as u16).unwrap_or(1)});
        }
        drop(statement);tx.commit().map_err(storage_error)?;
        let next_cursor=if has_more{last.map(|c|serde_json::to_vec(&c).map(|b|URL_SAFE_NO_PAD.encode(b)).map_err(storage_error)).transpose()?}else{None};
        Ok(omniplay_contracts::LibraryPage{games,next_cursor,total_matching,total_all})
    }

    pub fn asset_path(&self, manifest: &GameManifest) -> Result<PathBuf> {
        self.asset_details(manifest,None).map(|(path,_,_)|path)
    }

    pub fn asset_details(&self,manifest:&GameManifest,name:Option<&str>)->Result<(PathBuf,ContentHash,u64)>{
        manifest.validate()?;
        let(hash,bytes)=if let Some(media)=&manifest.media{let selected=name.unwrap_or(&media.entry_name);let asset=media.assets.iter().find(|a|a.name==selected).ok_or_else(||PlatformError::new(ErrorCode::NotFound,"Requested asset is not part of this game"))?;(asset.content_hash.clone(),asset.byte_length)}else{if name.is_some(){return Err(PlatformError::new(ErrorCode::NotFound,"This legacy entry has no named asset"));}(manifest.content_hash.clone(),manifest.byte_length)};
        Ok((self.database.root.join("assets").join(&hash.0),hash,bytes))
    }

    pub fn remove(&self, id: Uuid) -> Result<()> {
        let tx=Transaction::new_unchecked(&self.database.connection,TransactionBehavior::Immediate).map_err(storage_error)?;
        let active:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM sessions WHERE game_id=?1 AND state NOT IN ('stopped','failed'))",[id.to_string()],|row|row.get(0)).map_err(storage_error)?;
        if active{return Err(PlatformError::new(ErrorCode::Busy,"Stop the game before removing this library entry"));}
        let saves: i64 = tx.query_row("SELECT count(*) FROM saves WHERE game_id=?1", [id.to_string()], |row| row.get(0)).map_err(storage_error)?;
        if saves > 0 { return Err(PlatformError::new(ErrorCode::Conflict, "Export or retain game saves before removing this game").recover("Manage saved progress first")); }
        let changed = tx.execute("DELETE FROM games WHERE id=?1", [id.to_string()]).map_err(storage_error)?;
        if changed == 0 { return Err(PlatformError::new(ErrorCode::NotFound, "Game was not found")); }
        // Content is intentionally retained for recovery. A separate explicit storage action may remove orphaned assets.
        tx.execute("DELETE FROM preferences WHERE key GLOB ?1",[format!("core_options.{id}.*")]).map_err(storage_error)?;
        self.database.record_audit("library.remove", Some(id))?;tx.commit().map_err(storage_error)
    }
}

fn detect_format(source:&mut File)->Result<Option<ContentFormat>>{
    source.seek(SeekFrom::Start(0)).map_err(storage_error)?;let mut header=[0u8;16];let count=source.read(&mut header).map_err(storage_error)?;
    let detected=if count>=8&&&header[..8]==b"MComprHD"{Some(ContentFormat::Chd)}else if count>=4{match &header[..4]{b"\0PBP"=>Some(ContentFormat::Pbp),b"CISO"|b"ZISO"=>Some(ContentFormat::Cso),[0x80,0x37,0x12,0x40]=>Some(ContentFormat::N64Z64),[0x37,0x80,0x40,0x12]=>Some(ContentFormat::N64V64),[0x40,0x12,0x37,0x80]=>Some(ContentFormat::N64N64),_=>None}}else{None};
    if detected.is_some(){source.seek(SeekFrom::Start(0)).map_err(storage_error)?;return Ok(detected);}
    if source.metadata().map_err(storage_error)?.len()>=0x8006{source.seek(SeekFrom::Start(0x8001)).map_err(storage_error)?;let mut id=[0u8;5];source.read_exact(&mut id).map_err(storage_error)?;source.seek(SeekFrom::Start(0)).map_err(storage_error)?;if &id==b"CD001"{return Ok(Some(ContentFormat::Iso));}}
    source.seek(SeekFrom::Start(0)).map_err(storage_error)?;Ok(None)
}

fn decode(json: String) -> Result<GameManifest> {
    let manifest: GameManifest = serde_json::from_str(&json).map_err(|_| PlatformError::new(ErrorCode::CorruptData, "Library record is invalid"))?;
    manifest.validate()?;
    Ok(manifest)
}
