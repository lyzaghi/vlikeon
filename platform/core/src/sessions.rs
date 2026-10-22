use crate::{Database, now_ms, util::storage_error};
use omniplay_contracts::{Capability, ErrorCode, FrameMetrics, GameManifest, PlatformError, Result, RuntimeManifest, SaveCompatibility, SaveKind, SessionConfig, SessionInfo, SessionState};
use rusqlite::params;
use std::{collections::{BTreeMap, HashMap}, fs::File};
use uuid::Uuid;
use std::sync::{Arc,atomic::{AtomicBool,Ordering}};

pub trait RuntimeAdapter: Send {
    fn manifest(&self) -> &RuntimeManifest;
    fn prepare(&mut self, game: &GameManifest, asset: File, config: &SessionConfig) -> Result<()>;
    fn prepare_cancellable(&mut self,game:&GameManifest,asset:File,config:&SessionConfig,cancel:Arc<AtomicBool>)->Result<()>{if cancel.load(Ordering::Acquire){return Err(PlatformError::new(ErrorCode::Cancelled,"Runtime preparation was cancelled"));}self.prepare(game,asset,config)?;if cancel.load(Ordering::Acquire){let _=self.stop();return Err(PlatformError::new(ErrorCode::Cancelled,"Runtime preparation was cancelled"));}Ok(())}
    fn start(&mut self) -> Result<()>;
    fn pause(&mut self) -> Result<()>;
    fn resume(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
    fn save(&mut self, kind: SaveKind) -> Result<Vec<u8>>;
    fn load(&mut self, kind: SaveKind, payload: &[u8]) -> Result<()>;
    fn compatibility(&self, game: &GameManifest, config: &SessionConfig, kind: SaveKind) -> Result<SaveCompatibility>;
    fn metrics(&mut self, session_id: Uuid) -> Result<Option<FrameMetrics>>;
    fn measurement_start(&mut self,_:Uuid)->Result<omniplay_contracts::NativeMeasurementStatus>{Err(PlatformError::new(ErrorCode::UnsupportedCapability,"This runtime has no native measurement collector"))}
    fn measurement_status(&mut self)->Result<omniplay_contracts::NativeMeasurementStatus>{Err(PlatformError::new(ErrorCode::UnsupportedCapability,"This runtime has no native measurement collector"))}
    fn measurement_cancel(&mut self)->Result<omniplay_contracts::NativeMeasurementStatus>{Err(PlatformError::new(ErrorCode::UnsupportedCapability,"This runtime has no native measurement collector"))}
    fn measurement_read(&mut self,_:&GameManifest,_:&SessionConfig)->Result<omniplay_contracts::PerformanceRunInput>{Err(PlatformError::new(ErrorCode::UnsupportedCapability,"This runtime has no native measurement collector"))}
    fn observe(&mut self)->Result<RuntimeObservation>{Ok(RuntimeObservation::default())}
    fn cancel_close(&mut self)->Result<()>{Ok(())}
    fn disc_status(&mut self)->Result<omniplay_contracts::DiscStatus>{Err(PlatformError::new(ErrorCode::UnsupportedCapability,"This runtime has no disc controls"))}
    fn select_disc(&mut self,_:u32)->Result<omniplay_contracts::DiscStatus>{Err(PlatformError::new(ErrorCode::UnsupportedCapability,"This runtime has no disc controls"))}
}
#[derive(Default)]
pub struct RuntimeObservation {pub state:Option<SessionState>,pub close_requested:bool,pub active_playtime_ms:Option<u64>}

pub trait RuntimeFactory: Send { fn create(&self, id: &str) -> Result<Box<dyn RuntimeAdapter>>;fn list(&self)->Vec<RuntimeManifest>{Vec::new()}fn option_policy(&self,_:&str)->Result<omniplay_contracts::CoreOptionPolicy>{Err(PlatformError::new(ErrorCode::UnsupportedCapability,"This runtime factory has no declared core options"))} }

struct ActiveSession { pub info: SessionInfo, pub adapter: Box<dyn RuntimeAdapter>, pub config: SessionConfig, pub game: GameManifest, pub running_since_ms: Option<u64>, pub accumulated_playtime_ms: u64,pub runtime_playtime_ms:Option<u64>,measurement_summary:Option<omniplay_contracts::PerformanceRunSummary> }

pub struct SessionManager { active: HashMap<Uuid, ActiveSession>, max_active: usize }

impl SessionManager {
    pub fn new(max_active: usize) -> Self { Self { active: HashMap::new(), max_active: max_active.clamp(1, 8) } }
    pub fn has_room(&self)->bool{self.active.values().filter(|s|!s.info.state.is_terminal()).count()<self.max_active}
    pub fn adopt_prepared(&mut self,database:&Database,game:GameManifest,adapter:Box<dyn RuntimeAdapter>,config:SessionConfig)->Result<SessionInfo>{
        if !self.has_room(){return Err(PlatformError::new(ErrorCode::ResourceLimit,"Close an active game before starting another"));}
        game.validate()?;config.validate()?;adapter.manifest().validate()?;
        if !adapter.manifest().systems.contains(&game.system){return Err(PlatformError::new(ErrorCode::UnsupportedCapability,"Prepared runtime does not support this game"));}
        // Confirm that the game still belongs to this profile after asynchronous work.
        if crate::library::Library::new(database).get(game.id)?.content_hash!=game.content_hash{return Err(PlatformError::new(ErrorCode::ContentMismatch,"The game library changed during preparation"));}
        let now=now_ms();let info=SessionInfo{id:Uuid::new_v4(),game_id:game.id,runtime_id:adapter.manifest().id.clone(),runtime_kind:adapter.manifest().kind,state:SessionState::Ready,capabilities:adapter.manifest().capabilities.clone(),created_at_ms:now,updated_at_ms:now,error:None,close_requested:false};
        persist(database,&info)?;
        self.active.insert(info.id,ActiveSession{info:info.clone(),adapter,config,game,running_since_ms:None,accumulated_playtime_ms:0,runtime_playtime_ms:None,measurement_summary:None});Ok(info)
    }

    pub fn recover_interrupted(database: &Database) -> Result<usize> {
        // A previous process cannot still own these sessions after its IPC host died.
        // Preserve records and mark failure rather than pretending playback resumed.
        let mut statement = database.connection.prepare("SELECT id,info_json FROM sessions WHERE state NOT IN ('stopped','failed')").map_err(storage_error)?;
        let records = statement.query_map([], |row| Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?))).map_err(storage_error)?.collect::<std::result::Result<Vec<_>,_>>().map_err(storage_error)?;
        drop(statement);
        for (_, json) in &records {
            let mut info: SessionInfo = serde_json::from_str(json).map_err(|_| PlatformError::new(ErrorCode::CorruptData,"Session recovery record is invalid"))?;
            info.state = SessionState::Failed;
            info.updated_at_ms = now_ms();
            info.error = Some(PlatformError::new(ErrorCode::RuntimeCrashed,"The previous session ended unexpectedly").recover("Continue from the most recent compatible save"));
            persist(database,&info)?;
        }
        Ok(records.len())
    }

    pub fn prepare(&mut self, database: &Database, game: GameManifest, mut adapter: Box<dyn RuntimeAdapter>, asset: File, config: SessionConfig) -> Result<SessionInfo> {
        if self.active.values().filter(|s| !s.info.state.is_terminal()).count() >= self.max_active { return Err(PlatformError::new(ErrorCode::ResourceLimit,"Close an active game before starting another")); }
        game.validate()?;
        config.validate()?;
        adapter.manifest().validate()?;
        if !adapter.manifest().systems.contains(&game.system) { return Err(PlatformError::new(ErrorCode::UnsupportedCapability,"This runtime does not support the selected system")); }
        let now = now_ms();
        let mut info = SessionInfo { id: Uuid::new_v4(), game_id: game.id, runtime_id: adapter.manifest().id.clone(), runtime_kind: adapter.manifest().kind, state: SessionState::Created, capabilities: adapter.manifest().capabilities.clone(), created_at_ms: now, updated_at_ms: now, error: None,close_requested:false };
        info.state = info.state.transition(SessionState::Preparing)?;
        persist(database,&info)?;
        if let Err(error) = adapter.prepare(&game,asset,&config) {
            let _ = adapter.stop();
            info.state = SessionState::Failed;
            info.error = Some(error.clone());
            info.updated_at_ms = now_ms();
            persist(database,&info)?;
            return Err(error);
        }
        info.state = info.state.transition(SessionState::Ready)?;
        info.capabilities=adapter.manifest().capabilities.clone();
        info.updated_at_ms = now_ms();
        persist(database,&info)?;
        self.active.insert(info.id, ActiveSession { info: info.clone(), adapter, config, game, running_since_ms: None, accumulated_playtime_ms: 0,runtime_playtime_ms:None,measurement_summary:None });
        Ok(info)
    }

    pub fn change_state(&mut self, database: &Database, id: Uuid, target: SessionState) -> Result<SessionInfo> {
        self.refresh(database)?;
        let session = self.active.get_mut(&id).ok_or_else(|| PlatformError::new(ErrorCode::NotFound,"Active session was not found"))?;
        if session.info.state == target { return Ok(session.info.clone()); }
        if target==SessionState::Stopping&&session.info.state.is_terminal(){return Ok(session.info.clone());}
        session.info.state.transition(target)?;
        if matches!(target,SessionState::Paused) && !session.info.capabilities.contains(&Capability::Pause) { return Err(PlatformError::new(ErrorCode::UnsupportedCapability,"This runtime cannot pause")); }
        let action = match (session.info.state,target) {
            (SessionState::Ready,SessionState::Running) => session.adapter.start(),
            (SessionState::Paused,SessionState::Running) => session.adapter.resume(),
            (SessionState::Running,SessionState::Paused) => session.adapter.pause(),
            (_,SessionState::Stopping) => session.adapter.stop(),
            _ => Err(PlatformError::new(ErrorCode::InvalidState,"Requested transition is not a public action")),
        };
        let now = now_ms();
        if let Some(started) = session.running_since_ms.take() {if session.runtime_playtime_ms.is_none(){session.accumulated_playtime_ms = session.accumulated_playtime_ms.saturating_add(now.saturating_sub(started));}}
        match action {
            Ok(()) => {
                session.info.state = if target == SessionState::Stopping { SessionState::Stopped } else { target };
                if target == SessionState::Running { session.running_since_ms = Some(now); }
            }
            Err(error) if error.code==ErrorCode::PermissionDenied&&target==SessionState::Running&&matches!(session.info.state,SessionState::Ready|SessionState::Paused)=>{
                // A refused foreground/focus grant does not make a healthy,
                // already idle runtime terminal. Keep its loaded progress for
                // an explicit own-window action or a later Resume attempt.
                session.info.updated_at_ms=now;persist(database,&session.info)?;return Err(error);
            }
            Err(error) => {
                session.info.state = SessionState::Failed;
                session.info.error = Some(error.clone());
                session.info.updated_at_ms = now;
                let _ = session.adapter.stop();
                persist(database,&session.info)?;
                return Err(error);
            }
        }
        session.info.updated_at_ms = now;
        persist(database,&session.info)?;
        if session.info.state.is_terminal()&&session.accumulated_playtime_ms>0 {
            database.connection.execute("UPDATE games SET playtime_ms=playtime_ms+?1,last_played_at_ms=?2 WHERE id=?3", params![session.accumulated_playtime_ms,now,session.game.id.to_string()]).map_err(storage_error)?;
            session.accumulated_playtime_ms = 0;
        }
        Ok(session.info.clone())
    }

    pub fn list(&self) -> Vec<SessionInfo> {
        let mut values: Vec<_> = self.active.values().map(|s|s.info.clone()).collect();
        values.sort_by_key(|s|s.created_at_ms);
        values
    }

    pub fn snapshot(&mut self, id: Uuid, kind: SaveKind) -> Result<(Vec<u8>,SaveCompatibility,GameManifest)> {
        let session = self.live(id)?;
        check_save_capability(&session.info,kind)?;
        let compatibility = session.adapter.compatibility(&session.game,&session.config,kind)?;
        let payload = session.adapter.save(kind)?;
        if payload.is_empty() || payload.len() as u64 > omniplay_contracts::MAX_SAVE_BYTES { return Err(PlatformError::new(ErrorCode::ResourceLimit,"Runtime returned an invalid save size")); }
        Ok((payload,compatibility,session.game.clone()))
    }

    pub fn save_compatibility(&mut self,id:Uuid,kind:SaveKind)->Result<SaveCompatibility>{let session=self.live(id)?;check_save_capability(&session.info,kind)?;session.adapter.compatibility(&session.game,&session.config,kind)}
    pub fn cancel_close(&mut self,database:&Database,id:Uuid)->Result<SessionInfo>{let session=self.live(id)?;session.adapter.cancel_close()?;session.info.close_requested=false;session.info.updated_at_ms=now_ms();persist(database,&session.info)?;Ok(session.info.clone())}
    pub fn disc_status(&mut self,id:Uuid)->Result<omniplay_contracts::DiscStatus>{let session=self.live(id)?;if !session.info.capabilities.contains(&Capability::DiscControl){return Err(PlatformError::new(ErrorCode::UnsupportedCapability,"This session has no disc controls"));}let status=session.adapter.disc_status()?;status.validate()?;Ok(status)}
    pub fn select_disc(&mut self,id:Uuid,index:u32)->Result<omniplay_contracts::DiscStatus>{let session=self.live(id)?;if !session.info.capabilities.contains(&Capability::DiscControl)||index>=8{return Err(PlatformError::new(ErrorCode::UnsupportedCapability,"This disc selection is outside the runtime's capability"));}let status=session.adapter.select_disc(index)?;status.validate()?;Ok(status)}

    pub fn load_snapshot(&mut self, id: Uuid, metadata: &omniplay_contracts::SaveMetadata, payload: &[u8]) -> Result<()> {
        let session = self.live(id)?;
        check_save_capability(&session.info,metadata.kind)?;
        metadata.check_compatible(&session.adapter.compatibility(&session.game,&session.config,metadata.kind)?)?;
        session.adapter.load(metadata.kind,payload)
    }

    pub fn sample_metrics(&mut self,database:&Database) -> BTreeMap<Uuid,FrameMetrics> {
        let _=self.refresh(database);
        let mut samples = BTreeMap::new();
        for (id,session) in &mut self.active {
            if session.info.state == SessionState::Running {
                if let Ok(Some(metric)) = session.adapter.metrics(*id) { if metric.session_id == *id && metric.is_valid() { samples.insert(*id,metric); } }
            }
        }
        samples
    }

    pub fn measurement_start(&mut self,database:&Database,id:Uuid)->Result<omniplay_contracts::NativeMeasurementStatus>{
        {let session=self.live(id)?;if session.info.runtime_kind!=omniplay_contracts::RuntimeKind::NativeEmulator||!session.info.capabilities.contains(&Capability::PerformanceMeasurement)||!matches!(session.info.state,SessionState::Running|SessionState::Paused)||session.info.close_requested{return Err(PlatformError::new(ErrorCode::UnsupportedCapability,"Start a native game with a measurement collector before measuring"));}}
        self.change_state(database,id,SessionState::Running)?;
        let session=self.live(id)?;let status=session.adapter.measurement_start(Uuid::new_v4())?;session.measurement_summary=None;Ok(status)
    }
    pub fn measurement_cancel(&mut self,id:Uuid)->Result<omniplay_contracts::NativeMeasurementStatus>{self.live(id)?.adapter.measurement_cancel()}
    pub fn measurement_report(&mut self,database:&mut Database,id:Uuid)->Result<omniplay_contracts::NativeMeasurementReport>{
        let session=self.live(id)?;let capture=session.adapter.measurement_status()?;
        if capture.state==omniplay_contracts::NativeMeasurementState::Complete {
            if let Some(summary)=&session.measurement_summary {
                if summary.id!=capture.id{return Err(PlatformError::new(ErrorCode::CorruptData,"Native measurement identity changed after completion"));}
            }else{
                let input=session.adapter.measurement_read(&session.game,&session.config)?;
                if input.id!=capture.id{return Err(PlatformError::new(ErrorCode::CorruptData,"Native measurement result does not match its capture"));}
                session.measurement_summary=Some(crate::performance::PerformanceRuns::new(database).commit_native(input)?);
            }
        }
        // A completed capture is committed once. Polling cannot resurrect a
        // report the user subsequently deleted or that retention has removed.
        Ok(omniplay_contracts::NativeMeasurementReport{capture,summary:session.measurement_summary.clone()})
    }

    pub fn refresh(&mut self,database:&Database)->Result<()> {
        for session in self.active.values_mut().filter(|s|!s.info.state.is_terminal()) {
            let previous=session.info.state;let previous_close=session.info.close_requested;
            let observation=session.adapter.observe();
            match observation {
                Ok(observed)=>{
                    if let Some(played)=observed.active_playtime_ms {
                        let previous=session.runtime_playtime_ms.unwrap_or(0);
                        if played<previous {session.info.state=SessionState::Failed;session.info.error=Some(PlatformError::new(ErrorCode::RuntimeCrashed,"Native runtime playtime regressed"));let _=session.adapter.stop();}
                        else{session.accumulated_playtime_ms=session.accumulated_playtime_ms.saturating_add(played-previous);session.runtime_playtime_ms=Some(played);}
                    }
                    if !session.info.state.is_terminal(){if let Some(state)=observed.state {if state!=session.info.state {if matches!(state,SessionState::Ready|SessionState::Running|SessionState::Paused)&&session.info.state.transition(state).is_ok(){session.info.state=state;}else{session.info.state=SessionState::Failed;session.info.error=Some(PlatformError::new(ErrorCode::RuntimeCrashed,"Native runtime lifecycle is inconsistent"));let _=session.adapter.stop();}}}session.info.close_requested=observed.close_requested;}
                }
                Err(error)=>{session.info.state=SessionState::Failed;session.info.error=Some(error);let _=session.adapter.stop();}
            }
            let changed=previous!=session.info.state||previous_close!=session.info.close_requested;
            if changed||session.accumulated_playtime_ms>=30_000 {
                session.info.updated_at_ms=now_ms();
                if session.accumulated_playtime_ms>0 {database.connection.execute("UPDATE games SET playtime_ms=playtime_ms+?1,last_played_at_ms=?2 WHERE id=?3",params![session.accumulated_playtime_ms,session.info.updated_at_ms,session.game.id.to_string()]).map_err(storage_error)?;session.accumulated_playtime_ms=0;}
                persist(database,&session.info)?;
            }
        }
        Ok(())
    }

    pub fn stop_all(&mut self, database: &Database) {
        let ids: Vec<_> = self.active.keys().copied().collect();
        for id in ids { if self.active.get(&id).is_some_and(|s| !s.info.state.is_terminal()) { let _ = self.change_state(database,id,SessionState::Stopping); } }
    }

    fn live(&mut self, id: Uuid) -> Result<&mut ActiveSession> {
        let session = self.active.get_mut(&id).ok_or_else(|| PlatformError::new(ErrorCode::NotFound,"Active session was not found"))?;
        if !matches!(session.info.state,SessionState::Running | SessionState::Paused | SessionState::Ready) { return Err(PlatformError::new(ErrorCode::InvalidState,"Session cannot access saves in this state")); }
        Ok(session)
    }
}

fn check_save_capability(info: &SessionInfo, kind: SaveKind) -> Result<()> {
    let capability = match kind { SaveKind::InGame => Capability::SaveRam, SaveKind::EmulatorState => Capability::SaveState };
    if info.capabilities.contains(&capability) { return Ok(()); }
    Err(PlatformError::new(ErrorCode::UnsupportedCapability,"This runtime does not support the requested save type"))
}

fn persist(database: &Database, info: &SessionInfo) -> Result<()> {
    let state = serde_json::to_value(info.state).map_err(storage_error)?.as_str().ok_or_else(|| PlatformError::new(ErrorCode::Internal,"Invalid session state"))?.to_owned();
    database.connection.execute("INSERT INTO sessions(id,game_id,runtime_id,state,info_json,created_at_ms,updated_at_ms) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(id) DO UPDATE SET state=excluded.state,info_json=excluded.info_json,updated_at_ms=excluded.updated_at_ms", params![info.id.to_string(),info.game_id.to_string(),info.runtime_id,state,serde_json::to_string(info).map_err(storage_error)?,info.created_at_ms,info.updated_at_ms]).map_err(storage_error)?;
    Ok(())
}
