use crate::{Database,library::Library,now_ms,util::storage_error};
use omniplay_contracts::*;
use rusqlite::params;
use std::{collections::HashMap,time::Instant};
use uuid::Uuid;

struct Observed {info:SessionInfo,last_seen:Instant,last_state:SessionState,pending_playtime_ms:u64,last_flush:Instant}
#[derive(Default)]
pub struct WebSessions {active:HashMap<Uuid,Observed>}

impl WebSessions {
    pub fn begin(&mut self,database:&Database,game_id:Uuid,runtime_id:&str)->Result<SessionInfo> {
        self.expire(database)?;
        if self.active.values().filter(|v|!v.info.state.is_terminal()).count()>=2{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Close an active web game before starting another"));}
        let game=Library::new(database).get(game_id)?;
        if matches!(game.system,GameSystem::Pc|GameSystem::Web)||!valid_identifier(runtime_id){return Err(PlatformError::new(ErrorCode::InvalidRequest,"Observed runtime identity is invalid"));}
        let now=now_ms();let info=SessionInfo{id:Uuid::new_v4(),game_id,runtime_id:runtime_id.into(),runtime_kind:RuntimeKind::WebWasm,state:SessionState::Ready,capabilities:[Capability::Pause,Capability::KeyboardMouse,Capability::Gamepad,Capability::TouchControls].into_iter().collect(),created_at_ms:now,updated_at_ms:now,error:None,close_requested:false};
        persist(database,&info)?;self.active.insert(info.id,Observed{info:info.clone(),last_seen:Instant::now(),last_state:SessionState::Ready,pending_playtime_ms:0,last_flush:Instant::now()});
        Ok(info)
    }

    pub fn heartbeat(&mut self,database:&Database,id:Uuid,state:SessionState)->Result<SessionInfo> {
        self.expire(database)?;
        if !matches!(state,SessionState::Running|SessionState::Paused|SessionState::Stopped|SessionState::Failed){return Err(PlatformError::new(ErrorCode::InvalidRequest,"Web session observation is invalid"));}
        let value=self.active.get_mut(&id).ok_or_else(||PlatformError::new(ErrorCode::NotFound,"Web session was not found"))?;
        if value.info.state.is_terminal(){return Err(PlatformError::new(ErrorCode::InvalidState,"Web session has ended"));}
        let next=if state==SessionState::Stopped{SessionState::Stopping}else{state};
        value.info.state.transition(next)?;
        let elapsed=value.last_seen.elapsed().as_millis().min(15_000)as u64;
        if value.last_state==SessionState::Running {value.pending_playtime_ms=value.pending_playtime_ms.saturating_add(elapsed);}
        let changed=value.info.state!=state;value.info.state=state;value.info.updated_at_ms=now_ms();value.last_seen=Instant::now();value.last_state=state;
        if changed||state.is_terminal()||value.last_flush.elapsed().as_secs()>=10{flush(database,value)?;}
        Ok(value.info.clone())
    }

    pub fn expire(&mut self,database:&Database)->Result<()> {
        for value in self.active.values_mut().filter(|v|!v.info.state.is_terminal()&&v.last_seen.elapsed().as_secs()>if v.info.state==SessionState::Paused{24*60*60}else{30}){
            // Unconfirmed elapsed time after a lost renderer heartbeat is not counted.
            value.info.state=SessionState::Failed;value.info.updated_at_ms=now_ms();value.info.error=Some(PlatformError::new(ErrorCode::RuntimeCrashed,"The web game stopped reporting its lifecycle").recover("Reopen the game and continue from a compatible save"));flush(database,value)?;
        }
        if self.active.len()>32{self.active.retain(|_,v|!v.info.state.is_terminal());}Ok(())
    }
    pub fn list(&self)->Vec<SessionInfo>{self.active.values().map(|v|v.info.clone()).collect()}
    pub fn defer_broker_work(&mut self,duration:std::time::Duration){for value in self.active.values_mut().filter(|v|!v.info.state.is_terminal()){if let Some(adjusted)=value.last_seen.checked_add(duration){value.last_seen=adjusted.min(Instant::now());}}}
    pub fn close(&mut self,database:&Database){for value in self.active.values_mut().filter(|v|!v.info.state.is_terminal()){value.info.state=SessionState::Failed;value.info.updated_at_ms=now_ms();value.info.error=Some(PlatformError::new(ErrorCode::RuntimeCrashed,"The browser connection ended"));let _=flush(database,value);}}
}

fn flush(database:&Database,value:&mut Observed)->Result<()> {
    // Write playtime and its observed state together. A failed transaction leaves the
    // pending counter intact; a later retry cannot double-credit elapsed time.
    let tx=database.connection.unchecked_transaction().map_err(storage_error)?;
    if value.pending_playtime_ms>0 {tx.execute("UPDATE games SET playtime_ms=playtime_ms+?1,last_played_at_ms=?2 WHERE id=?3",params![value.pending_playtime_ms,value.info.updated_at_ms,value.info.game_id.to_string()]).map_err(storage_error)?;}
    let state=serde_json::to_value(value.info.state).map_err(storage_error)?;
    tx.execute("UPDATE sessions SET state=?1,info_json=?2,updated_at_ms=?3 WHERE id=?4",params![state.as_str(),serde_json::to_string(&value.info).map_err(storage_error)?,value.info.updated_at_ms,value.info.id.to_string()]).map_err(storage_error)?;
    tx.commit().map_err(storage_error)?;value.pending_playtime_ms=0;value.last_flush=Instant::now();Ok(())
}
fn persist(database:&Database,info:&SessionInfo)->Result<()> {
    database.connection.execute("INSERT INTO sessions(id,game_id,runtime_id,state,info_json,created_at_ms,updated_at_ms) VALUES(?1,?2,?3,'ready',?4,?5,?5)",params![info.id.to_string(),info.game_id.to_string(),info.runtime_id,serde_json::to_string(info).map_err(storage_error)?,info.created_at_ms]).map_err(storage_error)?;Ok(())
}
