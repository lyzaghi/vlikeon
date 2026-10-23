use crate::{Database,now_ms,util::{hash_file,storage_error}};
use omniplay_contracts::{ContentHash,ErrorCode,GameManifest,GameSystem,PlatformError,Result};
use rusqlite::{OptionalExtension,Transaction,TransactionBehavior,params};
use serde::{Deserialize,Serialize};
use std::{collections::HashMap,fs::{File,OpenOptions},path::{Path,PathBuf}};
#[cfg(not(windows))]use std::process::{Child,Command,Stdio};
#[cfg(windows)]use omniplay_stream_input::game_process::OwnedGameProcess as Child;
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
        let transaction=Transaction::new_unchecked(&database.connection,TransactionBehavior::Immediate).map_err(storage_error)?;
        let existing:Option<String>=database.connection.query_row("SELECT id FROM games WHERE content_hash=?1",[&hash.0],|row|row.get(0)).optional().map_err(storage_error)?;
        let id=match existing {Some(value)=>Uuid::parse_str(&value).map_err(|_|PlatformError::new(ErrorCode::CorruptData,"Library game identity is invalid"))?,None=>Uuid::new_v4()};
        let game=GameManifest{schema_version:1,id,title:title.trim().into(),system:GameSystem::Pc,content_hash:hash.clone(),byte_length:length,region:None,nominal_refresh_hz:None,created_at_ms:now_ms(),media:None};
        game.validate()?;
        if let Some(original)=crate::library::Library::new(database).by_hash(&hash)? {if original.system!=GameSystem::Pc{return Err(PlatformError::new(ErrorCode::Conflict,"This file is already registered as a different game system"));}}
        let private=serde_json::to_string(&LaunchSpecification{executable:canonical,working_directory:directory,sha256:hash}).map_err(storage_error)?;
        transaction.execute("INSERT INTO games(id,content_hash,title,search_title,system,manifest_json,created_at_ms) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(id) DO NOTHING",params![game.id.to_string(),game.content_hash.0,game.title,game.title.to_lowercase(),"\"pc\"",serde_json::to_string(&game).map_err(storage_error)?,game.created_at_ms]).map_err(storage_error)?;
        transaction.execute("INSERT INTO installed_games(game_id,private_launch_json,consented_at_ms) VALUES(?1,?2,?3) ON CONFLICT(game_id) DO UPDATE SET private_launch_json=excluded.private_launch_json,consented_at_ms=excluded.consented_at_ms",params![game.id.to_string(),private,now_ms()]).map_err(storage_error)?;
        transaction.execute("INSERT INTO audit(event,entity_id,created_at_ms) VALUES('installed.register',?1,?2)",params![game.id.to_string(),now_ms()]).map_err(storage_error)?;
        transaction.commit().map_err(storage_error)?;
        crate::library::Library::new(database).get(game.id)
    }

    pub fn launch(&mut self,database:&Database,game_id:Uuid)->Result<InstalledProcess> {
        self.poll(database)?;
        let game=crate::library::Library::new(database).get(game_id)?;
        if game.system!=GameSystem::Pc{return Err(PlatformError::new(ErrorCode::ContentMismatch,"This library entry is not an installed PC game"));}
        if self.running.values().any(|game|game.info.game_id==game_id){return Err(PlatformError::new(ErrorCode::Busy,"This installed game is already running"));}
        if self.running.len()>=4{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Close an installed game before starting another"));}
        let private:Option<String>=database.connection.query_row("SELECT private_launch_json FROM installed_games WHERE game_id=?1",[game_id.to_string()],|row|row.get(0)).optional().map_err(storage_error)?;
        let launch:LaunchSpecification=serde_json::from_str(&private.ok_or_else(||PlatformError::new(ErrorCode::NotFound,"Choose the installed game executable first"))?).map_err(|_|PlatformError::new(ErrorCode::CorruptData,"Installed game configuration is invalid"))?;
        if !launch.working_directory.is_absolute()||launch.executable.parent()!=Some(launch.working_directory.as_path())||launch.sha256!=game.content_hash{return Err(PlatformError::new(ErrorCode::CorruptData,"Installed game registration needs renewed local selection"));}
        validate_executable(&launch.executable)?;
        let mut file=locked_executable(&launch.executable)?;
        let (hash,bytes)=hash_file(&mut file,omniplay_contracts::MAX_ROM_BYTES)?;
        if hash!=launch.sha256||bytes!=game.byte_length{return Err(PlatformError::new(ErrorCode::ContentMismatch,"The installed game executable changed").recover("Select the updated executable again to approve it"));}
        database.record_audit("installed.launch_requested",Some(game_id))?;
        // Direct process creation: no shell, command interpreter, renderer arguments,
        // DLL injection, anti-cheat hooks or elevated execution.
        #[cfg(not(windows))]
        let child=Command::new(&launch.executable).current_dir(&launch.working_directory).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|_|PlatformError::new(ErrorCode::RuntimeUnavailable,"The selected installed game could not start").recover("Try opening it from its normal launcher"))?;
        #[cfg(windows)]
        let child=Child::launch(&launch.executable,&launch.working_directory)?;
        let info=InstalledProcess{session_id:Uuid::new_v4(),game_id,started_at_ms:now_ms(),running:true,exited_at_ms:None,exit_success:None};
        self.running.insert(info.session_id,RunningGame{child,info:info.clone()});
        // The process exists and is retained above. A diagnostic write failure
        // must not turn that successful launch into a retryable launch refusal.
        let _=database.record_audit("installed.launch",Some(game_id));
        Ok(info)
    }

    pub fn list(&mut self,database:&Database)->Result<Vec<InstalledProcess>> {
        self.poll(database)?;
        Ok(self.running.values().map(|value|value.info.clone()).chain(self.finished.iter().cloned()).collect())
    }

    pub fn is_running(&self,game_id:Uuid)->bool {self.running.values().any(|value|value.info.game_id==game_id)}

    #[cfg(windows)]
    pub fn stream_process(&mut self,database:&Database,game_id:Uuid)->Result<std::os::windows::io::OwnedHandle>{
        self.poll(database)?;
        let game=self.running.values().find(|value|value.info.game_id==game_id).ok_or_else(||PlatformError::new(ErrorCode::InvalidState,"Launch the selected installed game from OmniPlay before authorizing remote input"))?;
        game.child.root_handle()
    }

    /// Native-host selection only; not a renderer/remote process lookup API.
    /// The eventual chooser must retain this returned exact process instance
    /// together with its consented HWND and capture owner, then recheck liveness.
    #[cfg(windows)]
    pub fn stream_selected_process(&mut self,database:&Database,game_id:Uuid,selected:std::os::windows::io::BorrowedHandle<'_>)->Result<std::os::windows::io::OwnedHandle>{
        self.poll(database)?;
        let game=self.running.values().find(|value|value.info.game_id==game_id).ok_or_else(||PlatformError::new(ErrorCode::InvalidState,"The selected installed game session is no longer active"))?;
        game.child.admit_selected(selected)
    }

    fn poll(&mut self,database:&Database)->Result<()> {
        let mut exited=Vec::new();
        for (id,game) in &mut self.running {
            #[cfg(not(windows))]
            let status=game.child.try_wait().map_err(storage_error)?.map(|value|Some(value.success()));
            #[cfg(windows)]
            let status=game.child.poll_exit()?;
            if let Some(success)=status {
                if game.info.running{game.info.running=false;game.info.exited_at_ms=Some(now_ms());game.info.exit_success=success;}exited.push(*id);
            }
        }
        for id in exited {
            if let Some(game)=self.running.get(&id) {
                let ended=game.info.exited_at_ms.unwrap_or_else(now_ms);
                let tx=Transaction::new_unchecked(&database.connection,TransactionBehavior::Immediate).map_err(storage_error)?;
                // A failed/uncertain commit must retain the owned observation,
                // but retry must not add its playtime twice. The same durable
                // transaction publishes an exact session-ID audit receipt.
                let accounted:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM audit WHERE event='installed.exit_accounted' AND entity_id=?1)",[id.to_string()],|row|row.get(0)).map_err(storage_error)?;
                if !accounted{
                    let elapsed=ended.saturating_sub(game.info.started_at_ms).min(9_007_199_254_740_991);
                    tx.execute("UPDATE games SET playtime_ms=MIN(9007199254740991,playtime_ms+?1),last_played_at_ms=MAX(coalesce(last_played_at_ms,0),?2) WHERE id=?3",params![elapsed,ended,game.info.game_id.to_string()]).map_err(storage_error)?;
                    tx.execute("INSERT INTO audit(event,entity_id,created_at_ms) VALUES('installed.exit_accounted',?1,?2)",params![id.to_string(),ended]).map_err(storage_error)?;
                }
                tx.commit().map_err(storage_error)?;
                if let Some(game)=self.running.remove(&id){self.finished.push(game.info);}
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
