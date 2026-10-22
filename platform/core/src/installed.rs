use crate::{Database,now_ms,util::{hash_file,storage_error}};
use omniplay_contracts::{ContentHash,ErrorCode,GameManifest,GameSystem,PlatformError,Result};
use rusqlite::{OptionalExtension,params};
use serde::{Deserialize,Serialize};
use std::{collections::HashMap,fs::{File,OpenOptions},path::{Path,PathBuf},process::{Child,Command,Stdio}};
use uuid::Uuid;

#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct LaunchSpecification { executable:PathBuf,working_directory:PathBuf,sha256:ContentHash }

#[derive(Clone,Serialize)]
pub struct InstalledProcess { pub session_id:Uuid,pub game_id:Uuid,pub started_at_ms:u64,pub running:bool,pub exited_at_ms:Option<u64>,pub exit_success:Option<bool> }
struct RunningGame { child:Child,info:InstalledProcess }

#[derive(Default)]
pub struct InstalledGames { running:HashMap<Uuid,RunningGame>,finished:Vec<InstalledProcess> }

impl InstalledGames {
    /// Paths enter here only from the native picker, never from a renderer message.
    pub fn register_selected(database:&mut Database,path:&Path,title:&str)->Result<GameManifest> {
        if title.trim().is_empty()||title.len()>512 {return Err(PlatformError::new(ErrorCode::InvalidRequest,"Enter a game title"));}
        let canonical=path.canonicalize().map_err(storage_error)?;
        validate_executable(&canonical)?;
        let mut file=locked_executable(&canonical)?;
        let (hash,length)=hash_file(&mut file,omniplay_contracts::MAX_ROM_BYTES)?;
        let directory=canonical.parent().ok_or_else(||PlatformError::new(ErrorCode::InvalidRequest,"Game has no working directory"))?.to_owned();
        let existing:Option<String>=database.connection.query_row("SELECT id FROM games WHERE content_hash=?1",[&hash.0],|row|row.get(0)).optional().map_err(storage_error)?;
        let id=match existing {Some(value)=>Uuid::parse_str(&value).map_err(|_|PlatformError::new(ErrorCode::CorruptData,"Library game identity is invalid"))?,None=>Uuid::new_v4()};
        let game=GameManifest{schema_version:1,id,title:title.trim().into(),system:GameSystem::Pc,content_hash:hash.clone(),byte_length:length,region:None,nominal_refresh_hz:None,created_at_ms:now_ms(),media:None};
        game.validate()?;
        if let Some(original)=crate::library::Library::new(database).by_hash(&hash)? {if original.system!=GameSystem::Pc{return Err(PlatformError::new(ErrorCode::Conflict,"This file is already registered as a different game system"));}}
        let private=serde_json::to_string(&LaunchSpecification{executable:canonical,working_directory:directory,sha256:hash}).map_err(storage_error)?;
        let transaction=database.connection.transaction().map_err(storage_error)?;
        transaction.execute("INSERT INTO games(id,content_hash,title,search_title,system,manifest_json,created_at_ms) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(id) DO NOTHING",params![game.id.to_string(),game.content_hash.0,game.title,game.title.to_lowercase(),"\"pc\"",serde_json::to_string(&game).map_err(storage_error)?,game.created_at_ms]).map_err(storage_error)?;
        transaction.execute("INSERT INTO installed_games(game_id,private_launch_json,consented_at_ms) VALUES(?1,?2,?3) ON CONFLICT(game_id) DO UPDATE SET private_launch_json=excluded.private_launch_json,consented_at_ms=excluded.consented_at_ms",params![game.id.to_string(),private,now_ms()]).map_err(storage_error)?;
        transaction.commit().map_err(storage_error)?;
        database.record_audit("installed.register",Some(game.id))?;
        crate::library::Library::new(database).get(game.id)
    }

    pub fn launch(&mut self,database:&Database,game_id:Uuid)->Result<InstalledProcess> {
        self.poll(database)?;
        if self.running.values().any(|game|game.info.game_id==game_id){return Err(PlatformError::new(ErrorCode::Busy,"This installed game is already running"));}
        if self.running.len()>=4{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Close an installed game before starting another"));}
        let private:Option<String>=database.connection.query_row("SELECT private_launch_json FROM installed_games WHERE game_id=?1",[game_id.to_string()],|row|row.get(0)).optional().map_err(storage_error)?;
        let launch:LaunchSpecification=serde_json::from_str(&private.ok_or_else(||PlatformError::new(ErrorCode::NotFound,"Choose the installed game executable first"))?).map_err(|_|PlatformError::new(ErrorCode::CorruptData,"Installed game configuration is invalid"))?;
        validate_executable(&launch.executable)?;
        let mut file=locked_executable(&launch.executable)?;
        let (hash,_)=hash_file(&mut file,omniplay_contracts::MAX_ROM_BYTES)?;
        if hash!=launch.sha256{return Err(PlatformError::new(ErrorCode::ContentMismatch,"The installed game executable changed").recover("Select the updated executable again to approve it"));}
        // Direct process creation: no shell, command interpreter, renderer arguments,
        // DLL injection, anti-cheat hooks or elevated execution.
        let child=Command::new(&launch.executable).current_dir(&launch.working_directory).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|_|PlatformError::new(ErrorCode::RuntimeUnavailable,"The selected installed game could not start").recover("Try opening it from its normal launcher"))?;
        let info=InstalledProcess{session_id:Uuid::new_v4(),game_id,started_at_ms:now_ms(),running:true,exited_at_ms:None,exit_success:None};
        self.running.insert(info.session_id,RunningGame{child,info:info.clone()});
        database.record_audit("installed.launch",Some(game_id))?;
        Ok(info)
    }

    pub fn list(&mut self,database:&Database)->Result<Vec<InstalledProcess>> {
        self.poll(database)?;
        Ok(self.running.values().map(|value|value.info.clone()).chain(self.finished.iter().cloned()).collect())
    }

    pub fn is_running(&self,game_id:Uuid)->bool {self.running.values().any(|value|value.info.game_id==game_id)}

    #[cfg(windows)]
    pub fn stream_process(&mut self,database:&Database,game_id:Uuid)->Result<std::os::windows::io::OwnedHandle>{
        use std::os::windows::io::AsHandle;
        self.poll(database)?;
        let game=self.running.values().find(|value|value.info.game_id==game_id).ok_or_else(||PlatformError::new(ErrorCode::InvalidState,"Launch the selected installed game from OmniPlay before authorizing remote input"))?;
        game.child.as_handle().try_clone_to_owned().map_err(storage_error)
    }

    fn poll(&mut self,database:&Database)->Result<()> {
        let mut exited=Vec::new();
        for (id,game) in &mut self.running {
            if let Some(status)=game.child.try_wait().map_err(storage_error)? {
                game.info.running=false;game.info.exited_at_ms=Some(now_ms());game.info.exit_success=Some(status.success());exited.push(*id);
            }
        }
        for id in exited {
            if let Some(game)=self.running.remove(&id) {
                let ended=game.info.exited_at_ms.unwrap_or_else(now_ms);
                database.connection.execute("UPDATE games SET playtime_ms=playtime_ms+?1,last_played_at_ms=?2 WHERE id=?3",params![ended.saturating_sub(game.info.started_at_ms),ended,game.info.game_id.to_string()]).map_err(storage_error)?;
                self.finished.push(game.info);
            }
        }
        if self.finished.len()>32{self.finished.drain(..self.finished.len()-32);}
        Ok(())
    }
}

fn locked_executable(path:&Path)->Result<File> {
    let mut options=OpenOptions::new();options.read(true);
    #[cfg(windows)]
    {use std::os::windows::fs::OpenOptionsExt;options.share_mode(1);}
    let file=options.open(path).map_err(storage_error)?;
    if !file.metadata().map_err(storage_error)?.is_file(){return Err(PlatformError::new(ErrorCode::PermissionDenied,"Select a regular executable file"));}
    Ok(file)
}

fn validate_executable(path:&Path)->Result<()> {
    if !path.is_absolute(){return Err(PlatformError::new(ErrorCode::PermissionDenied,"Installed game path must be absolute"));}
    let name=path.file_name().and_then(|name|name.to_str()).unwrap_or("").to_ascii_lowercase();
    if ["cmd.exe","powershell.exe","pwsh.exe","wscript.exe","cscript.exe","mshta.exe","rundll32.exe","regsvr32.exe","bash","sh","zsh","fish","python","python3","node","env"].contains(&name.as_str()){
        return Err(PlatformError::new(ErrorCode::PermissionDenied,"Select the game executable, rather than a shell or script host"));
    }
    #[cfg(windows)]
    if path.extension().and_then(|extension|extension.to_str()).is_none_or(|extension|!extension.eq_ignore_ascii_case("exe")){return Err(PlatformError::new(ErrorCode::PermissionDenied,"Select a Windows game executable"));}
    Ok(())
}
