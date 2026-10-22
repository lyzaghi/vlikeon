use crate::{Database,now_ms,util::{PendingFile,bounded_read,digest,storage_error}};
use omniplay_contracts::*;
use rusqlite::{OptionalExtension,params};
use serde::{Deserialize,Serialize};
use std::{fs::{File,OpenOptions},io::Write};

#[derive(Clone,Serialize,Deserialize)]
pub struct FirmwareAsset {pub content_hash:ContentHash,pub byte_length:u64,pub created_at_ms:u64,pub selected_roles:Vec<String>}

pub fn role_sizes(system:GameSystem,role:&str)->Result<&'static [u64]> {
    match (system,role){
        (GameSystem::Snes,"snes_spc_ipl")=>Ok(&[64]),
        (GameSystem::MegaDrive,"mega_drive_svp")=>Ok(&[2048]),
        (GameSystem::Ps1,"ps1_bios"|"ps1_bios_jp"|"ps1_bios_eu")=>Ok(&[512*1024]),
        (GameSystem::Nds,"nds_bios7")=>Ok(&[16*1024]),
        (GameSystem::Nds,"nds_bios9")=>Ok(&[4*1024]),
        (GameSystem::Nds,"nds_firmware")=>Ok(&[128*1024,256*1024,512*1024]),
        _=>Err(PlatformError::new(ErrorCode::UnsupportedCapability,"This firmware role is not supported")),
    }
}
fn system_key(system:GameSystem)->Result<String>{serde_json::to_value(system).map_err(storage_error)?.as_str().map(str::to_owned).ok_or_else(||PlatformError::new(ErrorCode::InvalidRequest,"Firmware system is invalid"))}

pub struct FirmwareVault<'a>{database:&'a mut Database}
impl<'a> FirmwareVault<'a> {
    pub fn new(database:&'a mut Database)->Self{Self{database}}
    pub fn import(&mut self,system:GameSystem,role:&str,mut file:File)->Result<FirmwareAsset> {
        let sizes=role_sizes(system,role)?;let payload=bounded_read(&mut file,*sizes.iter().max().expect("nonempty role"))?;
        if !sizes.contains(&(payload.len()as u64)){return Err(PlatformError::new(ErrorCode::ContentMismatch,"Selected firmware has the wrong size for its role"));}
        // Shape validation does not assert provenance or successful hardware compatibility.
        let hash=digest(&payload);let existing:bool=self.database.connection.query_row("SELECT EXISTS(SELECT 1 FROM firmware_assets WHERE content_hash=?1)",[&hash.0],|r|r.get(0)).map_err(storage_error)?;
        if !existing{
            let(count,total):(u64,u64)=self.database.connection.query_row("SELECT count(*),COALESCE(sum(byte_length),0) FROM firmware_assets",[],|r|Ok((r.get(0)?,r.get(1)?))).map_err(storage_error)?;
            if count>=128||total.saturating_add(payload.len()as u64)>1024*1024*1024{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Firmware storage reached its local quota").recover("Delete firmware versions you no longer need before importing another"));}
            let directory=self.database.root.join("firmware");let target=directory.join(format!("{}.bin",hash.0));
            if target.exists(){let bytes=bounded_read(&mut File::open(&target).map_err(storage_error)?,payload.len()as u64)?;if digest(&bytes)!=hash{return Err(PlatformError::new(ErrorCode::CorruptData,"An existing firmware object failed its checksum"));}}
            else{let mut pending=PendingFile::create(&directory)?;pending.write_all(&payload)?;pending.commit_new(&target)?;}
        }
        let tx=self.database.connection.transaction().map_err(storage_error)?;
        tx.execute("INSERT OR IGNORE INTO firmware_assets(content_hash,byte_length,created_at_ms) VALUES(?1,?2,?3)",params![hash.0,payload.len()as u64,now_ms()]).map_err(storage_error)?;
        tx.execute("INSERT INTO firmware_selections(system,role,content_hash) VALUES(?1,?2,?3) ON CONFLICT(system,role) DO UPDATE SET content_hash=excluded.content_hash",params![system_key(system)?,role,hash.0]).map_err(storage_error)?;
        tx.commit().map_err(storage_error)?;
        self.list()?.into_iter().find(|a|a.content_hash==hash).ok_or_else(||PlatformError::new(ErrorCode::Internal,"Imported firmware was not recorded"))
    }
    pub fn list(&self)->Result<Vec<FirmwareAsset>> {
        let mut query=self.database.connection.prepare("SELECT content_hash,byte_length,created_at_ms FROM firmware_assets ORDER BY created_at_ms DESC LIMIT 128").map_err(storage_error)?;
        let rows=query.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get(1)?,r.get(2)?))).map_err(storage_error)?;
        let mut assets=Vec::new();for row in rows {let(hash,byte_length,created_at_ms)=row.map_err(storage_error)?;let mut selected=self.database.connection.prepare("SELECT system||':'||role FROM firmware_selections WHERE content_hash=?1 ORDER BY system,role").map_err(storage_error)?;let selected_roles=selected.query_map([&hash],|r|r.get(0)).map_err(storage_error)?.collect::<std::result::Result<Vec<_>,_>>().map_err(storage_error)?;assets.push(FirmwareAsset{content_hash:ContentHash::parse(hash)?,byte_length,created_at_ms,selected_roles});}Ok(assets)
    }
    pub fn selected(&self,system:GameSystem,role:&str)->Result<(FirmwareAsset,File)> {
        let allowed=role_sizes(system,role)?;
        let hash:Option<String>=self.database.connection.query_row("SELECT content_hash FROM firmware_selections WHERE system=?1 AND role=?2",params![system_key(system)?,role],|r|r.get(0)).optional().map_err(storage_error)?;
        let hash=hash.ok_or_else(||PlatformError::new(ErrorCode::NotFound,"This core requires user-supplied firmware").recover("Import firmware you are authorized to use in Settings before starting the game"))?;
        let asset=self.list()?.into_iter().find(|a|a.content_hash.0==hash).ok_or_else(||PlatformError::new(ErrorCode::CorruptData,"Firmware selection references a missing object"))?;
        if !allowed.contains(&asset.byte_length){return Err(PlatformError::new(ErrorCode::ContentMismatch,"Selected firmware size no longer matches its role"));}
        let mut options=OpenOptions::new();options.read(true);
        #[cfg(windows)]{use std::os::windows::fs::OpenOptionsExt;options.share_mode(1);}
        let mut file=options.open(self.database.root.join("firmware").join(format!("{hash}.bin"))).map_err(storage_error)?;
        let bytes=bounded_read(&mut file,asset.byte_length)?;if bytes.len()as u64!=asset.byte_length||digest(&bytes)!=asset.content_hash{return Err(PlatformError::new(ErrorCode::CorruptData,"Selected firmware failed its checksum"));}
        use std::io::{Seek,SeekFrom};file.seek(SeekFrom::Start(0)).map_err(storage_error)?;Ok((asset,file))
    }
    pub fn select(&mut self,system:GameSystem,role:&str,hash:&ContentHash)->Result<()> {
        ContentHash::parse(hash.0.clone())?;let sizes=role_sizes(system,role)?;
        let bytes:Option<u64>=self.database.connection.query_row("SELECT byte_length FROM firmware_assets WHERE content_hash=?1",[&hash.0],|r|r.get(0)).optional().map_err(storage_error)?;
        if !bytes.is_some_and(|v|sizes.contains(&v)){return Err(PlatformError::new(ErrorCode::ContentMismatch,"Firmware is not suitable for this role"));}
        self.database.connection.execute("INSERT INTO firmware_selections(system,role,content_hash) VALUES(?1,?2,?3) ON CONFLICT(system,role) DO UPDATE SET content_hash=excluded.content_hash",params![system_key(system)?,role,hash.0]).map_err(storage_error)?;Ok(())
    }
    pub fn forget(&mut self,hash:&ContentHash)->Result<()> {
        ContentHash::parse(hash.0.clone())?;
        let tx=self.database.connection.transaction().map_err(storage_error)?;tx.execute("DELETE FROM firmware_selections WHERE content_hash=?1",[&hash.0]).map_err(storage_error)?;tx.execute("DELETE FROM firmware_assets WHERE content_hash=?1",[&hash.0]).map_err(storage_error)?;tx.commit().map_err(storage_error)?;
        let target=self.database.root.join("firmware").join(format!("{}.bin",hash.0));
        // A single validated hash filename, never a renderer-supplied filesystem path.
        match std::fs::remove_file(target){Ok(())=>Ok(()),Err(e)if e.kind()==std::io::ErrorKind::NotFound=>Ok(()),Err(e)=>Err(storage_error(e))}
    }
}
