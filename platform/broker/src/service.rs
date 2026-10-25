use omniplay_contracts::*;
use omniplay_core::{Database, diagnostics::DiagnosticPreview, library::Library, permissions::{FileAccess,PermissionBroker}, saves::SaveStore, sessions::{RuntimeFactory,SessionManager}, util::storage_error};
use serde_json::{Value,json};
use std::collections::{HashMap,VecDeque};
use uuid::Uuid;

pub struct Broker {
    database: Database,
    permissions: PermissionBroker,
    sessions: SessionManager,
    runtimes: Box<dyn RuntimeFactory>,
    client_id: Uuid,
    results: HashMap<Uuid,(Vec<u8>,Response)>,
    result_order: VecDeque<Uuid>,
    pub shutdown_requested: bool,
    transfers: crate::transfers::Transfers,
    installed:omniplay_core::installed::InstalledGames,
    jobs:crate::jobs::Jobs,
    profile_id:Uuid,
    update_jobs:crate::update_jobs::UpdateJobs,
    assistant:omniplay_core::assistance::Assistant,
    web_sessions:omniplay_core::web_sessions::WebSessions,
    directory_grants:omniplay_core::media_import::DirectoryGrants,
    import_jobs:crate::import_jobs::ImportJobs,
    preparations:crate::preparation_jobs::Preparations,
    stream_inputs:crate::stream_input::StreamInputs,
    #[cfg(windows)]
    receive_video:crate::stream_video_registry::ReceiveRegistry,
    #[cfg(windows)]
    receive_document:crate::stream_video_registry::ReceiveDocument,
    #[cfg(windows)]
    receive_transport:crate::stream_video_ingress::TransportSlot,
    gamepad_recovery:crate::stream_gamepad_recovery::RecoveryLane,
    device_probe:omniplay_device_info::DeviceProbe,
    native_work_jobs:crate::native_work_jobs::NativeWorkJobs,
    asset_ranges:std::sync::Arc<omniplay_local_service::asset_ranges::AssetRanges>,
    benchmark_registry:omniplay_core::benchmark_registry::BenchmarkRegistry,
}

impl Broker {
    pub fn new(database: Database, runtimes: Box<dyn RuntimeFactory>,profile_id:Uuid) -> Result<Self> {
        SessionManager::recover_interrupted(&database)?;
        let mut broker=Self {
            database, permissions:PermissionBroker::default(), sessions:SessionManager::new(2),
            runtimes, client_id:Uuid::new_v4(), results:HashMap::new(), result_order:VecDeque::new(),
            shutdown_requested:false, profile_id,
            asset_ranges:std::sync::Arc::new(omniplay_local_service::asset_ranges::AssetRanges::default()),
            transfers:crate::transfers::Transfers::default(),
            installed:omniplay_core::installed::InstalledGames::default(), jobs:crate::jobs::Jobs::default(),
            update_jobs:crate::update_jobs::UpdateJobs::default(),
            assistant:omniplay_core::assistance::Assistant::default(),
            web_sessions:omniplay_core::web_sessions::WebSessions::default(),
            directory_grants:omniplay_core::media_import::DirectoryGrants::default(),
            import_jobs:crate::import_jobs::ImportJobs::default(),
            preparations:crate::preparation_jobs::Preparations::default(),
            stream_inputs:crate::stream_input::StreamInputs::default(),
            #[cfg(windows)]
            receive_video:crate::stream_video_registry::ReceiveRegistry::default(),
            #[cfg(windows)]
            receive_document:crate::stream_video_registry::ReceiveDocument::new(),
            #[cfg(windows)]
            receive_transport:crate::stream_video_ingress::TransportSlot::default(),
            gamepad_recovery:crate::stream_gamepad_recovery::RecoveryLane::default(),
            device_probe:omniplay_device_info::DeviceProbe::default(),
            native_work_jobs:crate::native_work_jobs::NativeWorkJobs::default(),
            benchmark_registry:omniplay_core::benchmark_registry::BenchmarkRegistry::default(),
        };
        // The system-owned first pass has no renderer capability. It shares the
        // same single worker/collector lease with later explicit user passes.
        if crate::native_work_jobs::enabled(){let _=broker.native_work_jobs.start(&broker.database.root,profile_id);}
        if crate::stream_gamepad_recovery::enabled(){let _=broker.gamepad_recovery.start(&broker.database.root,profile_id,broker.database.device_id);}
        Ok(broker)
    }
    pub fn prepare_range_read(&self,request:&Request,raw:&[u8])->Result<omniplay_local_service::asset_ranges::ReadOperation>{
        use sha2::Digest;request.validate(omniplay_core::now_ms())?;if self.shutdown_requested||request.profile_id!=self.profile_id{return Err(PlatformError::new(ErrorCode::PermissionDenied,"Native media cannot use another profile or closed channel"));}
        if self.results.get(&request.id).is_some_and(|(previous,_)|previous!=raw){return Err(PlatformError::new(ErrorCode::Conflict,"Request ID was reused with different content"));}
        let Command::AssetRangeRead{set_id,asset_index,offset,limit}=&request.command else{return Err(PlatformError::new(ErrorCode::InvalidRequest,"A media range command is required"));};
        self.asset_ranges.read(self.client_id,request.id,sha2::Sha256::digest(raw).into(),*set_id,*asset_index,*offset,*limit,request.deadline_ms)
    }

    pub fn handle(&mut self, request: Request, encoded: &[u8]) -> Response {
        if let Err(error)=self.asset_ranges.check_replay(self.client_id,request.id,encoded){return Response::result(request.id,Err(error));}
        // A repeated request ID must carry the exact same request. In particular, retries
        // of save/import operations cannot silently replay a different mutation.
        if let Some((previous,response)) = self.results.get(&request.id) {
            if previous == encoded { return response.clone(); }
            return Response::result(request.id,Err(PlatformError::new(ErrorCode::Conflict,"Request ID was reused with different content")));
        }
        let id = request.id;
        let sensitive=request.command.contains_secrets();
        let processing_started=std::time::Instant::now();
        let value = request.validate(omniplay_core::now_ms()).and_then(|_|self.execute(request.command,request.deadline_ms));
        let elapsed=processing_started.elapsed();if elapsed.as_secs()>=1{self.web_sessions.defer_broker_work(elapsed);}
        let response = Response::result(id,value);
        if sensitive{return response;}
        if self.result_order.len() >= 64 { if let Some(old) = self.result_order.pop_front() { self.results.remove(&old); } }
        self.results.insert(id,(encoded.to_vec(),response.clone()));
        self.result_order.push_back(id);
        response
    }

    fn execute(&mut self, command: Command, deadline_ms:u64) -> Result<Value> {
        self.web_sessions.expire(&self.database)?;
        self.transfers.expire();self.permissions.expire();
        match command {
            Command::GameInputGet{game_id}=>encode(omniplay_core::input::load(&self.database,game_id)?),
            Command::GameInputSet{game_id,input}=>encode(omniplay_core::input::store(&self.database,game_id,input)?),
            Command::GameTouchGet{game_id}=>encode(omniplay_core::input::load_touch(&self.database,game_id)?),
            Command::GameTouchSet{game_id,touch}=>encode(omniplay_core::input::store_touch(&self.database,game_id,touch)?),
            Command::GameCoreOptionsGet{game_id,runtime_id}=>encode(omniplay_core::core_options::load(&self.database,game_id,&runtime_id)?),
            Command::GameCoreOptionsList{game_id}=>encode(omniplay_core::core_options::list(&self.database,game_id)?),
            Command::GameCoreOptionsSet{game_id,runtime_id,options}=>encode(omniplay_core::core_options::store(&self.database,game_id,&runtime_id,options)?),
            Command::RuntimeOptionsGet{runtime_id}=>encode(self.runtimes.option_policy(&runtime_id)?),
            Command::DeviceSnapshot=>encode(self.device_probe.sample()?),
            Command::Hello => Ok(json!({"application":"OmniPlay","version":env!("CARGO_PKG_VERSION"),"protocol_major":PROTOCOL_MAJOR,"protocol_minor":PROTOCOL_MINOR,"storage_unused_protocol":1,"save_backup_protocol":1,"library_recovery_protocol":1,"manual_recovery_protocol":1,"save_orphan_protocol":1,"save_origin_protocol":1,"save_repair_protocol":1,"save_cloud_recovery_protocol":1,"encrypted_sync":true,"native_asset_ranges":1,"native_work_maintenance_protocol":if crate::native_work_jobs::enabled(){1}else{0},"client_id":self.client_id,"device_id":self.database.device_id,"platform":std::env::consts::OS,"capabilities":["local_library","file_dialog","save_history","save_export","asset_transfer","save_transfer","diagnostics_preview","local_settings","disc_import","firmware_vault","local_assistance","performance_measurements"],"native_remote_input":crate::stream_input::available(),"native_owned_capture":crate::stream_input::capture_available(),"runtime_capabilities_require_manifest":true})),
            Command::NativeWorkStatus=>{
                if !crate::native_work_jobs::enabled(){return Err(PlatformError::new(ErrorCode::UnsupportedCapability,"Native temporary-session recovery is not available on this platform"));}
                let mut status=omniplay_core::native_work_storage::status(&self.database,self.profile_id)?;
                status.recovery_running=self.native_work_jobs.busy();encode(status)
            }
            Command::NativeWorkRecoverStart=>encode(self.native_work_jobs.start(&self.database.root,self.profile_id)?),
            Command::NativeWorkRecoverStatus{job_id}=>encode(self.native_work_jobs.status(job_id)?),
            Command::NativeWorkRecoverCancel{job_id}=>encode(self.native_work_jobs.cancel(job_id)?),
            Command::StreamCapturePrepare{game_id,pair_id,session_id,host_public_key,grant}=>self.stream_inputs.prepare_capture(&self.database,&mut self.installed,game_id,pair_id,session_id,host_public_key,grant,deadline_ms),
            Command::StreamCaptureSelect{capture_id,session_id,choice_id}=>self.stream_inputs.select_capture(&self.database,&mut self.installed,capture_id,session_id,choice_id,deadline_ms),
            Command::StreamCaptureRelease{capture_id,session_id}=>self.stream_inputs.release_capture(capture_id,session_id),
            Command::StreamInputArm{game_id,pair_id,session_id,host_public_key,grant,capture_id,capture_window}=>encode(self.stream_inputs.arm(&self.database,&mut self.installed,game_id,pair_id,session_id,host_public_key,grant,capture_id,capture_window,deadline_ms)?),
            Command::StreamInputFrame{grant_id,session_id,sequence,frame}=>encode(self.stream_inputs.frame(grant_id,session_id,sequence,frame)?),
            Command::StreamInputRelease{grant_id,session_id}=>encode(self.stream_inputs.release(grant_id,session_id)?),
            Command::StreamInputStatus{grant_id,session_id}=>encode(self.stream_inputs.status(grant_id,session_id)?),
            Command::StreamInputRevoke{grant_id,session_id}=>encode(self.stream_inputs.revoke(grant_id,session_id)?),
            Command::FileSelectImport => {
                let path = rfd::FileDialog::new().set_title("OmniPlay — chọn game bạn có quyền sử dụng").add_filter("Game files",&["nes","gb","gbc","sfc","smc","gba","md","gen","bin","iso","chd","nds","pbp","opsave"]).pick_file().ok_or_else(||PlatformError::new(ErrorCode::Cancelled,"No file was selected"))?;
                let suggested_title = path.file_stem().and_then(|v|v.to_str()).unwrap_or("Game").chars().take(120).collect::<String>();
                let grant_id = self.permissions.grant_selected_file(self.client_id,&path,FileAccess::ImportRead,120_000)?;
                let format_hint=path.extension().and_then(|v|v.to_str()).and_then(ContentFormat::from_extension);
                Ok(json!({"grant_id":grant_id,"suggested_title":suggested_title,"format_hint":format_hint}))
            }
            Command::FileSelectExport { suggested_name } => {
                if suggested_name.is_empty() || suggested_name.len()>128 || !suggested_name.bytes().all(|v|v.is_ascii_alphanumeric() || [b'-',b'_',b'.'].contains(&v)) { return Err(PlatformError::new(ErrorCode::InvalidRequest,"Export file name is invalid")); }
                let path = rfd::FileDialog::new().set_title("OmniPlay — xuất dữ liệu").set_file_name(suggested_name).save_file().ok_or_else(||PlatformError::new(ErrorCode::Cancelled,"No destination was selected"))?;
                let grant_id = self.permissions.grant_selected_file(self.client_id,&path,FileAccess::ExportWrite,120_000)?;
                Ok(json!({"grant_id":grant_id}))
            }
            Command::LibraryList { query,offset,limit } => encode(Library::new(&self.database).list(query.as_deref(),offset,limit)?),
            Command::LibraryPage{query,system,after,limit,favorites_only}=>encode(Library::new(&self.database).page(query.as_deref(),system,after.as_deref(),limit,favorites_only)?),
            Command::LibraryFavoriteGet{game_id}=>encode(Library::new(&self.database).favorite(game_id)?),
            Command::LibraryFavoriteSet{game_id,favorite}=>encode(Library::new(&self.database).set_favorite(game_id,favorite)?),
            Command::LibraryGet { game_id } => encode(Library::new(&self.database).get(game_id)?),
            Command::LibraryImport { grant_id,title,system } => {
                if system==GameSystem::Pc {return Err(PlatformError::new(ErrorCode::InvalidRequest,"Use the installed PC game picker to retain the game's working directory"));}
                let source = self.permissions.take(self.client_id,grant_id,FileAccess::ImportRead)?;
                encode(Library::new(&self.database).import(source,title,system)?)
            }
            Command::LibraryImportStart{grant_id,title,system,format_hint}=>{
                if self.import_jobs.busy(){return Err(PlatformError::new(ErrorCode::Busy,"Wait for the current import to finish"));}
                let file=self.permissions.take(self.client_id,grant_id,FileAccess::ImportRead)?;
                encode(self.import_jobs.start(&self.database.root,title,system,crate::import_jobs::ImportSource::Single{file,format_hint})?)
            }
            Command::DiscSelectFolder=>{
                let path=rfd::FileDialog::new().set_title("OmniPlay — chọn thư mục chứa bộ đĩa bạn có quyền sử dụng").pick_folder().ok_or_else(||PlatformError::new(ErrorCode::Cancelled,"No disc folder was selected"))?;
                encode(self.directory_grants.selected(self.client_id,&path)?)
            }
            Command::LibraryImportBundleStart{grant_id,entry_name,title,system,encoding}=>{
                if self.import_jobs.busy(){return Err(PlatformError::new(ErrorCode::Busy,"Wait for the current import to finish"));}
                let scope=self.directory_grants.take(self.client_id,grant_id,&entry_name)?;
                encode(self.import_jobs.start(&self.database.root,title,system,crate::import_jobs::ImportSource::Bundle{scope,entry:entry_name,encoding})?)
            }
            Command::LibraryImportStatus{job_id}=>encode(self.import_jobs.status(job_id)?),
            Command::LibraryImportCancel{job_id}=>encode(self.import_jobs.cancel(job_id)?),
            Command::LibraryRemove { game_id } => {
                self.installed.list(&self.database)?;
                if self.installed.is_running(game_id){return Err(PlatformError::new(ErrorCode::Busy,"Exit the installed game before removing its library entry"));}
                if self.sessions.list().iter().any(|s|s.game_id==game_id && !s.state.is_terminal()) { return Err(PlatformError::new(ErrorCode::Busy,"Stop the game before removing it")); }
                if self.web_sessions.list().iter().any(|s|s.game_id==game_id&&!s.state.is_terminal()){return Err(PlatformError::new(ErrorCode::Busy,"Stop the web game before removing it"));}
                Library::new(&self.database).remove(game_id)?;
                Ok(json!({"removed":true}))
            }
            Command::InstalledGameSelect {title} => {
                let mut picker=rfd::FileDialog::new().set_title("OmniPlay — chọn tệp chạy của game đã cài");
                if cfg!(windows){picker=picker.add_filter("Installed game",&["exe"]);}
                let path=picker.pick_file().ok_or_else(||PlatformError::new(ErrorCode::Cancelled,"No installed game was selected"))?;
                encode(omniplay_core::installed::InstalledGames::register_selected(&mut self.database,&path,&title)?)
            }
            Command::InstalledGameLaunch {game_id} => encode(self.installed.launch(&self.database,game_id)?),
            Command::InstalledGameProcesses => encode(self.installed.list(&self.database)?),
            Command::AssetReadStart {game_id,asset_name} => {
                let library=Library::new(&self.database);let game=library.get(game_id)?;
                let(path,hash,bytes)=library.asset_details(&game,asset_name.as_deref())?;
                let file=std::fs::File::open(path).map_err(storage_error)?;
                encode(self.transfers.open_download(file,hash,bytes)?)
            }
            Command::AssetRangesStart{game_id}=>encode(self.asset_ranges.start(&self.database,self.client_id,game_id)?),
            Command::FirmwareReadStart{game_id,role,expected_hash}=>{
                ContentHash::parse(expected_hash.0.clone())?;let game=Library::new(&self.database).get(game_id)?;
                let(asset,file)=omniplay_core::firmware::FirmwareVault::new(&mut self.database).selected(game.system,&role)?;
                if asset.content_hash!=expected_hash{return Err(PlatformError::new(ErrorCode::Conflict,"The selected firmware changed; reopen the game to use its current selection"));}
                let transfer=self.transfers.open_download(file,asset.content_hash.clone(),asset.byte_length)?;Ok(json!({"role":role,"sha256":asset.content_hash,"byte_length":asset.byte_length,"transfer":transfer}))
            }
            Command::AssetRangesStatus{job_id}=>encode(self.asset_ranges.status(self.client_id,job_id)?),
            Command::AssetRangesCollect{job_id}=>encode(self.asset_ranges.collect(self.client_id,job_id)?),
            Command::AssetRangesCancel{job_id}=>encode(self.asset_ranges.cancel_job(self.client_id,job_id)?),
            Command::AssetRangesKeepAlive{set_id}=>encode(self.asset_ranges.keep_alive(self.client_id,set_id)?),
            Command::AssetRangesClose{set_id}=>{self.asset_ranges.close_set(self.client_id,set_id)?;Ok(json!({"closed":true}))},
            Command::AssetRangeRead{..}=>Err(PlatformError::new(ErrorCode::UnsupportedCapability,"Native media reads require their bounded I/O lane")),
            Command::SaveReadStart {save_id,compatibility} => {
                let metadata=SaveStore::new(&mut self.database).metadata(save_id)?;
                metadata.check_compatible(&compatibility)?;
                let file=SaveStore::new(&mut self.database).open_payload(&metadata)?;
                let info=self.transfers.open_download(file,metadata.payload_hash.clone(),metadata.payload_bytes)?;
                Ok(json!({"transfer":info,"metadata":metadata}))
            }
            Command::TransferRead {transfer_id,offset,limit} => encode(self.transfers.read(transfer_id,offset,limit)?),
            Command::SaveUploadStart {metadata,expected_parent_id} => {
                let game=Library::new(&self.database).get(metadata.game_id)?;
                if game.content_hash!=metadata.game_content_hash{return Err(PlatformError::new(ErrorCode::ContentMismatch,"Save belongs to different game content"));}
                let transfer_id=self.transfers.start_upload(&self.database.root,metadata,expected_parent_id)?;
                Ok(json!({"transfer_id":transfer_id,"chunk_bytes":48*1024}))
            }
            Command::SaveUploadChunk {transfer_id,offset,payload_base64} => Ok(json!({"next_offset":self.transfers.upload_chunk(transfer_id,offset,&payload_base64)?})),
            Command::SaveUploadCommit {transfer_id} => {
                let (metadata,payload,parent)=self.transfers.finish_upload(transfer_id)?;
                encode(SaveStore::new(&mut self.database).commit(metadata,&payload,parent)?)
            }
            Command::TransferAbort {transfer_id} => {self.transfers.abort(transfer_id);Ok(json!({"aborted":true}))}
            Command::SessionPrepare { game_id,runtime_id,config } => {
                let library = Library::new(&self.database);
                let game = library.get(game_id)?;
                let source = std::fs::File::open(library.asset_path(&game)?).map_err(storage_error)?;
                let adapter = self.runtimes.create(&runtime_id)?;
                encode(self.sessions.prepare(&self.database,game,adapter,source,config)?)
            }
            Command::SessionPrepareStart{game_id,runtime_id,config}=>{
                if !self.sessions.has_room(){return Err(PlatformError::new(ErrorCode::ResourceLimit,"Close an active native game before starting another"));}
                let library=Library::new(&self.database);let game=library.get(game_id)?;
                let source=std::fs::File::open(library.asset_path(&game)?).map_err(storage_error)?;let adapter=self.runtimes.create(&runtime_id)?;
                encode(self.preparations.start(game,adapter,source,config)?)
            }
            Command::SessionPrepareStatus{job_id}=>encode(self.preparations.status(job_id,&self.database,&mut self.sessions)?),
            Command::SessionPrepareCancel{job_id}=>{
                let status=self.preparations.cancel(job_id)?;
                if let Some(session)=&status.session{let _=self.sessions.change_state(&self.database,session.id,SessionState::Stopping);}
                encode(status)
            }
            Command::SessionStart { session_id } | Command::SessionResume { session_id } => encode(self.sessions.change_state(&self.database,session_id,SessionState::Running)?),
            Command::SessionPause { session_id } => encode(self.sessions.change_state(&self.database,session_id,SessionState::Paused)?),
            Command::SessionStop { session_id } => encode(self.sessions.change_state(&self.database,session_id,SessionState::Stopping)?),
            Command::SessionCancelClose{session_id}=>encode(self.sessions.cancel_close(&self.database,session_id)?),
            Command::SessionDetach{session_id}=>self.detach_session(session_id),
            Command::SessionDiscStatus{session_id}=>encode(self.sessions.disc_status(session_id)?),
            Command::SessionSelectDisc{session_id,index}=>encode(self.sessions.select_disc(session_id,index)?),
            Command::SessionList => {self.sessions.refresh(&self.database)?;let mut sessions=self.sessions.list();sessions.extend(self.web_sessions.list());encode(sessions)}
            Command::RuntimeList=>encode(self.runtimes.list()),
            Command::FirmwareList=>encode(omniplay_core::firmware::FirmwareVault::new(&mut self.database).list()?),
            Command::FirmwareImportSelected{system,role}=>{
                omniplay_core::firmware::role_sizes(system,&role)?;
                let path=rfd::FileDialog::new().set_title("OmniPlay — chọn firmware bạn có quyền sử dụng").pick_file().ok_or_else(||PlatformError::new(ErrorCode::Cancelled,"No firmware was selected"))?;
                let grant=self.permissions.grant_selected_file(self.client_id,&path,FileAccess::ImportRead,120_000)?;let file=self.permissions.take(self.client_id,grant,FileAccess::ImportRead)?;
                encode(omniplay_core::firmware::FirmwareVault::new(&mut self.database).import(system,&role,file)?)
            }
            Command::FirmwareSelect{system,role,content_hash}=>{omniplay_core::firmware::FirmwareVault::new(&mut self.database).select(system,&role,&content_hash)?;Ok(json!({"selected":true}))}
            Command::FirmwareForget{content_hash}=>{omniplay_core::firmware::FirmwareVault::new(&mut self.database).forget(&content_hash)?;Ok(json!({"removed":true}))}
            Command::WebSessionBegin{game_id,runtime_id}=>encode(self.web_sessions.begin(&self.database,game_id,&runtime_id)?),
            Command::WebSessionHeartbeat{session_id,state}=>encode(self.web_sessions.heartbeat(&self.database,session_id,state)?),
            Command::MetricsSnapshot => encode(self.sessions.sample_metrics(&self.database)),
            Command::PerformanceRunCommit{run}=>encode(omniplay_core::performance::PerformanceRuns::new(&mut self.database).commit(run)?),
            Command::PerformanceNativeStart{session_id}=>encode(self.sessions.measurement_start(&self.database,session_id)?),
            Command::PerformanceNativeStatus{session_id}=>encode(self.sessions.measurement_report(&mut self.database,session_id)?),
            Command::PerformanceNativeCancel{session_id}=>encode(self.sessions.measurement_cancel(session_id)?),
            Command::PerformanceRunList{game_id}=>encode(omniplay_core::performance::PerformanceRuns::new(&mut self.database).list(game_id)?),
            Command::PerformanceRunCompare{baseline_id,candidate_id}=>encode(omniplay_core::performance::PerformanceRuns::new(&mut self.database).compare(baseline_id,candidate_id)?),
            Command::PerformanceRunExport{run_id,destination_grant_id}=>{let mut destination=self.permissions.take(self.client_id,destination_grant_id,FileAccess::ExportWrite)?;omniplay_core::performance::PerformanceRuns::new(&mut self.database).export(run_id,&mut destination)?;Ok(json!({"exported":true}))}
            Command::PerformanceRunDelete{run_id}=>{omniplay_core::performance::PerformanceRuns::new(&mut self.database).delete(run_id)?;Ok(json!({"deleted":true}))}
            Command::PerformanceRunsClear=>{omniplay_core::performance::PerformanceRuns::new(&mut self.database).clear()?;Ok(json!({"deleted":true}))}
            Command::SaveList { game_id } => encode(SaveStore::new(&mut self.database).list(game_id)?),
            Command::SavePage{game_id,after,limit}=>encode(SaveStore::new(&mut self.database).page(game_id,after.as_deref(),limit)?),
            Command::SaveHeads {game_id} => encode(SaveStore::new(&mut self.database).heads(game_id)?),
            Command::SaveOriginGet{save_id}=>encode(omniplay_core::save_origins::get(&self.database,save_id)?),
            Command::SavePromote {save_id,expected_head_id} => encode(SaveStore::new(&mut self.database).promote(save_id,expected_head_id)?),
            Command::SaveExportUploadStart{metadata,destination_grant_id}=>{metadata.validate()?;if Library::new(&self.database).get(metadata.game_id)?.content_hash!=metadata.game_content_hash{return Err(PlatformError::new(ErrorCode::ContentMismatch,"Export content is not in this library"));}let file=self.permissions.take(self.client_id,destination_grant_id,FileAccess::ExportWrite)?;let transfer_id=self.transfers.start_export(metadata,destination_grant_id,file)?;Ok(json!({"transfer_id":transfer_id,"chunk_bytes":48*1024}))}
            Command::SaveExportUploadChunk{transfer_id,offset,payload_base64}=>Ok(json!({"next_offset":self.transfers.export_chunk(transfer_id,offset,&payload_base64)?})),
            Command::SaveExportUploadCommit{transfer_id,destination_grant_id}=>{self.transfers.commit_export(transfer_id,destination_grant_id)?;Ok(json!({"exported":true,"delivery":"file_handle"}))},
            Command::SavePurgePreview{game_id}=>encode(omniplay_core::save_maintenance::purge_preview(&self.database,game_id)?),
            Command::SavePurge{game_id,expected_revision}=>encode(omniplay_core::save_maintenance::purge(&mut self.database,game_id,expected_revision)?),
            Command::SaveCleanup=>encode(omniplay_core::save_maintenance::cleanup(&self.database,128)?),
            Command::SaveCleanupStatus=>encode(omniplay_core::save_maintenance::cleanup_status(&self.database)?),
            Command::StorageUnusedPreview{after_hash}=>{if self.installed.list(&self.database)?.iter().any(|session|session.running){return Err(PlatformError::new(ErrorCode::Busy,"Exit installed games before collecting unused content"));}encode(omniplay_core::asset_storage::preview(&self.database,after_hash.as_deref())?)},
            Command::StorageUnusedRemove{after_hash,revision}=>{if self.installed.list(&self.database)?.iter().any(|session|session.running){return Err(PlatformError::new(ErrorCode::Busy,"Exit installed games before collecting unused content"));}encode(omniplay_core::asset_storage::remove(&self.database,after_hash.as_deref(),&revision)?)},
            Command::SaveImport {grant_id,game_id} => {
                let mut source=self.permissions.take(self.client_id,grant_id,FileAccess::ImportRead)?;
                encode(SaveStore::new(&mut self.database).import_export(game_id,&mut source)?)
            }
            Command::SaveRepair{grant_id,save_id}=>{
                let mut file=self.permissions.take(self.client_id,grant_id,FileAccess::ImportRead)?;
                encode(SaveStore::new(&mut self.database).repair_export(save_id,&mut file)?)
            }
            Command::SaveCreate { session_id,kind,slot,expected_parent_id } => {
                let (payload,compatibility,game) = self.sessions.snapshot(session_id,kind)?;
                let device_id = self.database.device_id;
                let mut store = SaveStore::new(&mut self.database);
                let generation = match expected_parent_id { Some(id) => store.metadata(id)?.generation.checked_add(1).ok_or_else(||PlatformError::new(ErrorCode::ResourceLimit,"Save generation limit reached"))?, None => 1 };
                let metadata = SaveMetadata { schema_version:1,id:Uuid::new_v4(),game_id:game.id,game_content_hash:game.content_hash,kind,slot,generation,parent_id:expected_parent_id,core_id:compatibility.core_id,core_version:compatibility.core_version,state_format_version:compatibility.state_format_version,platform:compatibility.platform,core_config_hash:compatibility.core_config_hash,payload_hash:omniplay_core::util::digest(&payload),payload_bytes:payload.len() as u64,created_at_ms:omniplay_core::now_ms(),device_id,label:None,conflict:false };
                encode(store.commit(metadata,&payload,expected_parent_id)?)
            }
            Command::SaveLiveExport{session_id,kind,destination_grant_id}=>{
                if !self.sessions.list().iter().any(|session|session.id==session_id&&session.state==SessionState::Paused){return Err(PlatformError::new(ErrorCode::InvalidState,"Pause the active game before exporting live progress"));}
                let mut destination=self.permissions.take(self.client_id,destination_grant_id,FileAccess::ExportWrite)?;
                let(payload,compatibility,game)=self.sessions.snapshot(session_id,kind)?;
                let metadata=SaveMetadata{schema_version:1,id:Uuid::new_v4(),game_id:game.id,game_content_hash:game.content_hash,kind,slot:0,generation:1,parent_id:None,core_id:compatibility.core_id,core_version:compatibility.core_version,state_format_version:compatibility.state_format_version,platform:compatibility.platform,core_config_hash:compatibility.core_config_hash,payload_hash:omniplay_core::util::digest(&payload),payload_bytes:payload.len()as u64,created_at_ms:omniplay_core::now_ms(),device_id:self.database.device_id,label:Some("Live progress export".into()),conflict:false};
                omniplay_core::saves::write_export(&metadata,&payload,&mut destination)?;Ok(json!({"exported":true,"delivery":"file_handle"}))
            }
            Command::SessionSaveCompatibility{session_id,kind}=>encode(self.sessions.save_compatibility(session_id,kind)?),
            Command::SaveLoad { session_id,save_id } => {
                let (metadata,payload) = SaveStore::new(&mut self.database).read(save_id,None)?;
                self.sessions.load_snapshot(session_id,&metadata,&payload)?;
                Ok(json!({"loaded":true}))
            }
            Command::SaveExport { save_id,destination_grant_id } => {
                let mut file = self.permissions.take(self.client_id,destination_grant_id,FileAccess::ExportWrite)?;
                SaveStore::new(&mut self.database).export(save_id,&mut file)?;
                Ok(json!({"exported":true}))
            }
            Command::SaveBackupExport{save_ids,destination_grant_id}=>{let mut file=self.permissions.take(self.client_id,destination_grant_id,FileAccess::ExportWrite)?;encode(omniplay_core::save_backup::export(&mut self.database,&save_ids,&mut file)?)},
            Command::SaveBackupRestore{grant_id}=>{let mut file=self.permissions.take(self.client_id,grant_id,FileAccess::ImportRead)?;encode(omniplay_core::save_backup::restore(&mut self.database,&mut file)?)},
            Command::LibraryRecoveryList{after}=>encode(omniplay_core::library_recovery::list(&self.database,after.as_deref())?),
            Command::LibraryRecoveryImport{candidate_id,revision,title,system}=>encode(omniplay_core::library_recovery::recover(&self.database,&candidate_id,&revision,title,system)?),
            Command::SaveOrphanPreview{save_id,after}=>encode(omniplay_core::save_orphan_recovery::preview(&self.database,save_id,after.as_deref())?),
            Command::SaveOrphanRecover{save_id,candidate_id,revision}=>encode(omniplay_core::save_orphan_recovery::recover(&mut self.database,save_id,&candidate_id,&revision)?),
            Command::ManualRecoveryList{after}=>encode(omniplay_core::manual_recovery::list(&self.database,after.as_ref())?),
            Command::ManualRecoveryInspect{candidate_id}=>encode(omniplay_core::manual_recovery::inspect(&self.database,&candidate_id)?),
            Command::ManualRecoveryExport{candidate_id,revision,destination_grant_id}=>{let mut file=self.permissions.take(self.client_id,destination_grant_id,FileAccess::ExportWrite)?;encode(omniplay_core::manual_recovery::export(&self.database,&candidate_id,&revision,&mut file)?)},
            Command::ManualRecoveryExtract{grant_id,destination_grant_id}=>{let mut source=self.permissions.take(self.client_id,grant_id,FileAccess::ImportRead)?;let mut destination=self.permissions.take(self.client_id,destination_grant_id,FileAccess::ExportWrite)?;encode(omniplay_core::manual_recovery::extract(&mut source,&mut destination)?)},
            Command::PermissionRevoke { grant_id } => { self.permissions.revoke(self.client_id,grant_id)?;self.directory_grants.revoke(self.client_id,grant_id)?;Ok(json!({"revoked":true})) }
            Command::DiagnosticsPreview => encode(self.diagnostic_preview()?),
            Command::DiagnosticsExport { destination_grant_id } => {
                let mut file = self.permissions.take(self.client_id,destination_grant_id,FileAccess::ExportWrite)?;
                self.diagnostic_preview()?.write(&mut file)?;
                Ok(json!({"exported":true}))
            }
            Command::SettingsGet => encode(self.database.settings()?),
            Command::SettingsSet { settings } => { self.database.set_settings(&settings)?; encode(settings) }
            Command::GameSettingsGet{game_id}=>encode(omniplay_core::assistance::session_preferences(&self.database,game_id)?),
            Command::GameSettingsSet{game_id,config}=>{omniplay_core::assistance::set_session_preferences(&self.database,game_id,&config)?;encode(config)}
            Command::RuntimeRank{game_id,device,mut candidates}=>{
                let game=Library::new(&self.database).get(game_id)?;
                let now = omniplay_core::now_ms();
                let installed_manifests = self.runtimes.list();
                let paired_host_available = omniplay_core::stream_credentials::load(&self.database, self.profile_id)
                    .map(|j| j.records().iter().any(|r| r.phase() == omniplay_core::stream_credentials::Phase::Active && r.expires_at_ms().map_or(true, |exp| exp > now)))
                    .unwrap_or(false);
                let native_allowed = !cfg!(target_arch = "wasm32");
                let remote_allowed = true;
                let device_class = if device.memory_mb.is_some_and(|m| m >= 8192) && device.webgpu {
                    "desktop-x86_64-dedicated-gpu".to_string()
                } else if device.touch {
                    "mobile-arm64".to_string()
                } else {
                    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
                };

                for candidate in &mut candidates {
                    match candidate.kind {
                        RuntimeKind::WebWasm => {
                            if !device.wasm {
                                candidate.available = false;
                            }
                        }
                        RuntimeKind::NativeEmulator => {
                            if let Some(installed) = installed_manifests.iter().find(|m| m.id == candidate.id) {
                                if installed.systems.contains(&game.system) {
                                    candidate.available = true;
                                    candidate.distribution_approved = true;
                                } else {
                                    candidate.available = false;
                                }
                            } else {
                                candidate.available = false;
                            }
                        }
                        RuntimeKind::Remote => {
                            if paired_host_available && remote_allowed {
                                candidate.available = true;
                                candidate.distribution_approved = true;
                            } else {
                                candidate.available = false;
                            }
                        }
                        RuntimeKind::InstalledGame => {
                            let installed_list = self.installed.list(&self.database).unwrap_or_default();
                            candidate.available = installed_list.iter().any(|entry| entry.game_id == game_id);
                        }
                    }
                    if candidate.available {
                        if let Some(bench) = self.benchmark_registry.lookup(&candidate.id, &game.content_hash, &device_class, now) {
                            candidate.benchmark = Some(bench.to_runtime_benchmark());
                        }
                    }
                }

                let policy = omniplay_core::router::RoutingPolicy {
                    native_allowed,
                    remote_allowed,
                    paired_host_available,
                    device_class,
                };
                encode(omniplay_core::router::rank(&game, &device, &candidates, &policy, now)?)
            }
            Command::AssistantAdvise{game_id,topic}=>encode(self.assistant.advise(&self.database,game_id,topic)?),
            Command::AssistantAccept{proposal_id}=>encode(self.assistant.accept(&self.database,proposal_id)?),
            Command::AssistantDismiss{proposal_id}=>{self.assistant.dismiss(proposal_id);Ok(json!({"dismissed":true}))}
            Command::SyncStatus => {
                let configuration=omniplay_sync_client::load_configuration(&self.database)?;
                Ok(json!({"connected":configuration.is_some(),"enabled":self.database.settings()?.cloud_sync_opt_in,"account":configuration,"running":self.jobs.busy()}))
            }
            Command::SyncBackgroundConfigure{..}=>Err(PlatformError::new(ErrorCode::UnsupportedCapability,"This scheduling preference belongs to the native mobile application")),
            Command::SyncActionStart{..}|Command::SyncActionStatus{..}|Command::SyncActionCollect{..}|Command::SyncActionCancel{..}|Command::SyncActionDiscard{..}=>Err(PlatformError::new(ErrorCode::UnsupportedCapability,"This desktop host uses its existing account command protocol")),
            Command::SyncCreate {audience} => {
                if self.jobs.busy(){return Err(PlatformError::new(ErrorCode::Busy,"Stop synchronization before changing its account"));}
                encode(omniplay_sync_client::create_account(&self.database,self.profile_id,&audience)?)
            }
            Command::SyncRegister {recovery_key,registration_token} => {
                let recovery=zeroize::Zeroizing::new(recovery_key);let registration=zeroize::Zeroizing::new(registration_token);
                omniplay_sync_client::register_account(&self.database,&recovery,&registration)?;Ok(json!({"registered":true}))
            }
            Command::SyncRecover {audience,recovery_key} => {
                let recovery=zeroize::Zeroizing::new(recovery_key);
                if self.jobs.busy(){return Err(PlatformError::new(ErrorCode::Busy,"Stop synchronization before changing its account"));}
                encode(omniplay_sync_client::recover_account(&self.database,self.profile_id,&audience,&recovery)?)
            }
            Command::SyncRenew {recovery_key} => {
                let recovery=zeroize::Zeroizing::new(recovery_key);
                if self.jobs.busy(){return Err(PlatformError::new(ErrorCode::Busy,"Stop synchronization before renewing its certificate"));}
                omniplay_sync_client::renew_certificate(&self.database,&recovery)?;Ok(json!({"renewed":true}))
            }
            Command::SyncDevices => encode(omniplay_sync_client::SyncClient::open(&self.database)?.devices()?),
            Command::SyncRevokeDevice {recovery_key,device_id} => {
                let recovery=zeroize::Zeroizing::new(recovery_key);
                if self.jobs.busy(){return Err(PlatformError::new(ErrorCode::Busy,"Stop synchronization before revoking a device"));}
                omniplay_sync_client::revoke_device(&self.database,&recovery,device_id)?;Ok(json!({"revoked":true}))
            }
            Command::SyncDeleteAccount {recovery_key} => {
                let recovery=zeroize::Zeroizing::new(recovery_key);
                if self.jobs.busy(){return Err(PlatformError::new(ErrorCode::Busy,"Stop synchronization before deleting its account"));}
                omniplay_sync_client::delete_account(&self.database,&recovery)?;Ok(json!({"deleted":true}))
            }
            Command::SyncDisconnect => {
                if self.jobs.busy(){return Err(PlatformError::new(ErrorCode::Busy,"Cancel the current synchronization and wait for it to stop"));}
                omniplay_sync_client::disconnect(&self.database)?;Ok(json!({"disconnected":true}))
            }
            Command::SyncRunStart => {
                if !self.database.settings()?.cloud_sync_opt_in{return Err(PlatformError::new(ErrorCode::PermissionDenied,"Synchronization is disabled"));}
                encode(self.jobs.start_sync(&self.database.root)?)
            }
            Command::SyncRunStatus {job_id} => encode(self.jobs.status(job_id)?),
            Command::SyncRunCancel {job_id} => encode(self.jobs.cancel(job_id)?),
            Command::SyncSaveRecoveryRequest{save_id}=>{
                if self.jobs.busy(){return Err(PlatformError::new(ErrorCode::Busy,"Wait for the current synchronization to finish before requesting recovery"));}
                encode(omniplay_sync_client::request_save_recovery(&self.database,save_id,&omniplay_sync_client::OsCredentialStore)?)
            }
            Command::SyncSaveRecoveryList{game_id}=>encode(omniplay_sync_client::list_save_recoveries(&self.database,game_id)?),
            Command::SyncSaveRecoveryCancel{save_id}=>encode(omniplay_sync_client::cancel_save_recovery(&self.database,save_id,&omniplay_sync_client::OsCredentialStore)?),
            Command::UpdateStatus=>crate::update_jobs::status(),
            Command::UpdateDownloadStart=>encode(self.update_jobs.start()?),
            Command::UpdateJobStatus{job_id}=>encode(self.update_jobs.status(job_id)?),
            Command::UpdateJobCancel{job_id}=>encode(self.update_jobs.cancel(job_id)?),
            Command::UpdateActivate{release_id}=>{
                if self.update_jobs.busy(){return Err(PlatformError::new(ErrorCode::Busy,"Wait for the update download to finish"));}
                encode(crate::update_jobs::ReleaseSummary::from(crate::update_jobs::open()?.activate(release_id)?))
            }
            Command::UpdateRollback=>{
                if self.update_jobs.busy(){return Err(PlatformError::new(ErrorCode::Busy,"Wait for the update download to finish"));}
                encode(crate::update_jobs::ReleaseSummary::from(crate::update_jobs::open()?.rollback()?))
            }
            Command::Shutdown => { self.shutdown_requested=true; self.close(); Ok(json!({"stopped":true})) }
        }
    }

    fn detach_session(&mut self,id:Uuid)->Result<Value>{
        self.sessions.refresh(&self.database)?;
        let info=self.sessions.list().into_iter().find(|s|s.id==id).ok_or_else(||PlatformError::new(ErrorCode::NotFound,"Native session was not found"))?;
        if info.state.is_terminal(){return Ok(json!({"stopped":true,"recovered_saves":[]}));}
        let mut recovered=Vec::new();
        for(kind,capability)in[(SaveKind::InGame,Capability::SaveRam),(SaveKind::EmulatorState,Capability::SaveState)]{
            if !info.capabilities.contains(&capability){continue;}
            let saved=(||{
                let(payload,compatibility,game)=self.sessions.snapshot(id,kind)?;
                let metadata=SaveMetadata{schema_version:1,id:Uuid::new_v4(),game_id:game.id,game_content_hash:game.content_hash,kind,slot:0,generation:1,parent_id:None,core_id:compatibility.core_id,core_version:compatibility.core_version,state_format_version:compatibility.state_format_version,platform:compatibility.platform,core_config_hash:compatibility.core_config_hash,payload_hash:omniplay_core::util::digest(&payload),payload_bytes:payload.len()as u64,created_at_ms:omniplay_core::now_ms(),device_id:self.database.device_id,label:Some("Recovered when the control window closed".into()),conflict:false};
                // The UI's causal head is no longer available. Preserve a branch instead
                // of replacing potentially newer progress from another active window.
                SaveStore::new(&mut self.database).commit(metadata,&payload,None)
            })();
            match saved{Ok(metadata)=>recovered.push(metadata.id),Err(_)=>{let _=self.database.record_audit("native.detach_save_failed",Some(id));}}
        }
        let stopped=self.sessions.change_state(&self.database,id,SessionState::Stopping).is_ok();
        Ok(json!({"stopped":stopped,"recovered_saves":recovered}))
    }

    fn diagnostic_preview(&self)->Result<DiagnosticPreview>{
        let mut sessions=self.sessions.list();sessions.extend(self.web_sessions.list());
        let mut preview=DiagnosticPreview::collect(&self.database,&sessions)?;
        if crate::native_work_jobs::enabled(){
            let pending=self.native_work_jobs.busy();
            let(storage,storage_error)=match omniplay_core::native_work_storage::status(&self.database,self.profile_id){Ok(mut status)=>{status.recovery_running=pending;(Some(status),None)},Err(error)=>(None,Some(error.code))};
            let(recovery,recovery_status_error)=match self.native_work_jobs.diagnostic(){Ok(value)=>(value,None),Err(error)=>(None,Some(error.code))};
            preview.native_work=Some(omniplay_core::diagnostics::NativeWorkDiagnostic{observed_at_ms:omniplay_core::now_ms(),storage,storage_error,recovery,recovery_status_error,worker_pending:pending});
        }
        Ok(preview)
    }

    /// Broker-channel native lifetime only. Navigation/focus-specific documents
    /// still require their own native issuer; no renderer-selected scope exists.
    #[cfg(windows)]
    pub(crate) fn open_native_receive(&mut self,configuration:crate::stream_video_session::WindowsReceiveConfiguration,source:impl crate::stream_video_authority::NativeVideoAuthority,deadline:std::time::Instant)->Result<()>{
        self.receive_video.open(&self.receive_document,configuration,source,deadline)
    }
    pub(crate) fn maintain_native_receive(&mut self){
        #[cfg(windows)]{
            let deadline=std::time::Instant::now()+std::time::Duration::from_millis(25);
            let _=self.receive_transport.maintain(&mut self.receive_video,&self.receive_document,deadline);
            if !self.receive_transport.occupied(){let _=self.receive_video.maintain(deadline);}
        }
    }
    #[cfg(windows)]
    pub(crate) fn attach_native_video_transport<T:crate::stream_video_ingress::NativeSrtpReceiver+'static,C:crate::stream_video_ingress::NativeDatagramReceipt+'static>(&mut self,transport:&mut Option<crate::stream_video_ingress::UdpVideoIngress<T,C>>)->Result<()>{
        self.receive_transport.attach(transport,&self.receive_video,&self.receive_document)
    }
    pub fn close(&mut self) {
        #[cfg(windows)]{self.receive_document.close();}
        #[cfg(windows)]{let _=self.receive_transport.close_until(&mut self.receive_video,&self.receive_document,std::time::Instant::now()+std::time::Duration::from_millis(1000));}
        #[cfg(windows)]{if !self.receive_transport.occupied(){let _=self.receive_video.close_until(std::time::Instant::now()+std::time::Duration::from_millis(1000));}}
        self.gamepad_recovery.cancel();self.native_work_jobs.cancel_all();self.asset_ranges.close_owner(self.client_id);self.stream_inputs.close();self.jobs.cancel_all();self.update_jobs.cancel_all();self.import_jobs.cancel_all();self.preparations.cancel_all();let ids=self.sessions.list().into_iter().filter(|s|!s.state.is_terminal()).map(|s|s.id).collect::<Vec<_>>();for id in ids{let _=self.detach_session(id);}self.sessions.stop_all(&self.database);self.web_sessions.close(&self.database);self.transfers.clear();self.permissions.revoke_owner(self.client_id);self.directory_grants.revoke_owner(self.client_id);
    }
}

fn encode(value: impl serde::Serialize) -> Result<Value> { serde_json::to_value(value).map_err(|_|PlatformError::new(ErrorCode::Internal,"Response could not be encoded")) }
