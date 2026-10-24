use omniplay_contracts::*;
use omniplay_core::{Database,assistance::{Assistant,session_preferences,set_session_preferences},diagnostics::DiagnosticPreview,firmware::FirmwareVault,library::Library,media_import::DirectoryGrants,performance::PerformanceRuns,permissions::{PermissionBroker,FileAccess},router::{self,RoutingPolicy},saves::SaveStore,sessions::SessionManager,util::storage_error,web_sessions::WebSessions};
use serde_json::{Value,json};
use std::{collections::{HashMap,VecDeque},fs::File,path::Path,sync::Arc};
use uuid::Uuid;

struct NativeIdentityReview{ id:Uuid,address:omniplay_streaming::native_credentials::CredentialAddress,
    revision:u64,provider_epoch:u64,deadline:std::time::Instant }
/// Trusted native UI supplies file capabilities. WebView requests contain UUIDs,
/// never arbitrary filesystem paths or native executable locations.
pub struct LocalHost {
    pub profile_id:Uuid,database:Database,owner:Uuid,permissions:PermissionBroker,closed:bool,
    directories:DirectoryGrants,transfers:crate::transfers::Transfers,imports:crate::import_jobs::ImportJobs,
    device_probe:omniplay_device_info::DeviceProbe,
    asset_ranges:Arc<crate::asset_ranges::AssetRanges>,
    credentials:Option<Arc<dyn omniplay_sync_client::CredentialStore>>,
    sync_jobs:crate::sync_jobs::SyncJobs,lifecycle:Arc<crate::SyncLifecycle>,
    sessions:WebSessions,assistant:Assistant,results:HashMap<Uuid,(Vec<u8>,Response)>,order:VecDeque<Uuid>,
    identity_review:Option<NativeIdentityReview>,
}
impl LocalHost {
    pub fn open(root:&Path,profile_id:Uuid)->Result<Self>{
        Self::open_inner(root,profile_id,None)
    }
    pub fn open_with_credentials(root:&Path,profile_id:Uuid,credentials:Arc<dyn omniplay_sync_client::CredentialStore>)->Result<Self>{
        Self::open_inner(root,profile_id,Some(credentials))
    }
    fn open_inner(root:&Path,profile_id:Uuid,credentials:Option<Arc<dyn omniplay_sync_client::CredentialStore>>)->Result<Self>{
        let database=Database::open(root)?;SessionManager::recover_interrupted(&database)?;
        let lifecycle=Arc::new(crate::SyncLifecycle::default());
        Ok(Self{profile_id,database,credentials,sync_jobs:crate::sync_jobs::SyncJobs::new(lifecycle.clone()),lifecycle,owner:Uuid::new_v4(),closed:false,asset_ranges:Arc::new(crate::asset_ranges::AssetRanges::default()),permissions:PermissionBroker::default(),directories:DirectoryGrants::default(),transfers:crate::transfers::Transfers::default(),imports:crate::import_jobs::ImportJobs::default(),device_probe:omniplay_device_info::DeviceProbe::default(),sessions:WebSessions::default(),assistant:Assistant::default(),results:HashMap::new(),order:VecDeque::new(),identity_review:None})
    }
    pub fn credential_store(&self)->Option<Arc<dyn omniplay_sync_client::CredentialStore>>{self.credentials.clone()}
    fn ensure_open(&self)->Result<()>{if self.closed{Err(PlatformError::new(ErrorCode::NotFound,"Mobile profile is closed"))}else{Ok(())}}
    pub fn asset_ranges(&self)->Arc<crate::asset_ranges::AssetRanges>{self.asset_ranges.clone()}
    pub fn prepare_range_read(&self,request:&Request,raw:&[u8])->Result<crate::asset_ranges::ReadOperation>{
        use sha2::Digest;
        self.ensure_open()?;request.validate(omniplay_core::now_ms())?;if request.profile_id!=self.profile_id{return Err(PlatformError::new(ErrorCode::PermissionDenied,"A media reader cannot switch profiles"));}
        if self.results.get(&request.id).is_some_and(|(previous,_)|previous!=raw){return Err(PlatformError::new(ErrorCode::Conflict,"Request ID was reused with different content"));}
        let Command::AssetRangeRead{set_id,asset_index,offset,limit}=&request.command else{return Err(PlatformError::new(ErrorCode::InvalidRequest,"A native media read command is required"));};
        self.asset_ranges.read(self.owner,request.id,sha2::Sha256::digest(raw).into(),*set_id,*asset_index,*offset,*limit,request.deadline_ms)
    }
    pub fn sync_lifecycle(&self)->Arc<crate::SyncLifecycle>{self.lifecycle.clone()}
    /// Trusted native scope, never a renderer-selected credential identity.
    pub fn native_device_id(&self)->Uuid{self.database.device_id}
    /// Native controller only, absent from Command/renderer dispatch. Metadata
    /// remains readable with streaming disabled for reviewed cleanup/recovery.
    pub fn native_pairing_status(&self)->Result<Value>{
        self.ensure_open()?;
        let snapshot=omniplay_core::stream_credentials::load(&self.database,self.profile_id)?;
        let observed_at_ms=omniplay_core::now_ms();
        let recovery:Vec<_>=snapshot.recovery_plan(observed_at_ms)?.iter().map(|action|
            json!({"address":action.address(),"next_step":action.step()})).collect();
        let records:Vec<_>=snapshot.records().iter().map(|record|json!({
            "address":record.address(),"phase":record.phase(),"public_key":record.public_key(),
            "writer_retired":record.writer_retired(),"material_absent":record.material_absent(),
            "local_retired":record.local_retired(),"relay_retired":record.relay_retired()
        })).collect();
        Ok(json!({"credential_protocol":1,"profile_id":self.profile_id,"device_id":self.database.device_id,
            "revision":snapshot.revision(),"policy_available":omniplay_core::stream_signal_authority::require_installed_pairing_policy().is_ok(),"records":records,
            "recovery_protocol":1,"observed_at_ms":observed_at_ms,"recovery":recovery}))
    }
    /// Explicit native recovery dispatch, not automatic startup cleanup. May
    /// permanently fence a candidate; caller must not use this as a status read.
    /// Disabled streaming does not prevent cleanup of existing generations.
    pub fn native_pairing_advance_metadata(&mut self,address:omniplay_streaming::native_credentials::CredentialAddress,
        expected:u64,deadline:std::time::Instant)->Result<Value>{
        self.ensure_open()?;
        if std::time::Instant::now()>=deadline{return Err(PlatformError::new(ErrorCode::DeadlineExceeded,"Native recovery operation expired before lease admission"));}
        let lease=omniplay_core::stream_credential_lease::CredentialLease::acquire(&self.database,self.profile_id)?;
        self.identity_review=None;
        let outcome=omniplay_core::stream_credential_recovery::advance_metadata(&self.database,&lease,self.profile_id,address,
            expected,omniplay_core::now_ms(),deadline)?;
        use omniplay_core::stream_credential_recovery::StepOutcome;
        let value=match outcome{
            StepOutcome::Committed{revision}=>json!({"outcome":"committed","revision":revision}),
            StepOutcome::Pending=>json!({"outcome":"pending"}),
            StepOutcome::RevalidationRequired=>json!({"outcome":"revalidation_required"}),
            StepOutcome::AlreadyRetired=>json!({"outcome":"already_retired"}),
        };
        Ok(json!({"recovery_protocol":1,"address":address,"result":value,"reload_required":true}))
    }
    /// Trusted native foreground/consent controller supplies the scoped store.
    /// Prepares MaterialReady only; no identity activation or pairing approval.
    pub fn native_pairing_prepare_identity(&self,store:&dyn omniplay_streaming::native_credentials::PairingSecretStore,
        expected:u64,deadline:std::time::Instant)->Result<omniplay_core::stream_credential_writer::Candidate>{
        self.ensure_open()?;
        omniplay_core::stream_signal_authority::require_installed_pairing_policy()?;
        let mut writer=omniplay_core::stream_credential_writer::NativeCredentialWriter::open(&self.database,self.profile_id,store)?;
        writer.prepare_identity(expected,deadline)
    }
    /// Native UI obtains this exact candidate before presenting local consent.
    /// A review token is only a one-use binding, not proof a dialog was shown.
    pub fn native_pairing_review_identity(&mut self,expected:u64,provider_epoch:u64)->Result<Value>{
        self.ensure_open()?;self.identity_review=None;
        omniplay_core::stream_signal_authority::require_installed_pairing_policy()?;
        if provider_epoch==0{return Err(PlatformError::new(ErrorCode::PermissionDenied,"Native pairing lifecycle is unavailable"));}
        let deadline=std::time::Instant::now()+std::time::Duration::from_secs(60);
        let snapshot=omniplay_core::stream_credentials::load(&self.database,self.profile_id)?;
        if snapshot.revision()!=expected{return Err(PlatformError::new(ErrorCode::Conflict,"Reload native pairing status before reviewing identity"));}
        let mut identities=snapshot.records().iter().filter(|record|
            record.address().kind()==omniplay_streaming::native_credentials::SecretKind::DeviceSigning&&
            record.phase()!=omniplay_core::stream_credentials::Phase::Retired);
        let record=identities.next().ok_or_else(||PlatformError::new(ErrorCode::Conflict,"Prepare an identity before reviewing activation"))?;
        if identities.next().is_some()||record.phase()!=omniplay_core::stream_credentials::Phase::MaterialReady||std::time::Instant::now()>=deadline{
            return Err(PlatformError::new(ErrorCode::Conflict,"Native identity is not ready for review"));
        }
        let id=Uuid::new_v4();
        self.identity_review=Some(NativeIdentityReview{id,address:record.address(),revision:expected,provider_epoch,deadline});
        Ok(json!({"credential_protocol":1,"review_id":id,"address":record.address(),"public_key":record.public_key(),
            "revision":expected,"expires_in_ms":deadline.saturating_duration_since(std::time::Instant::now()).as_millis(),
            "action":"activate_local_identity","approves_remote_peer":false}))
    }
    /// Trusted native caller invokes only after explicit approval of its review
    /// dialog. Always consumes the pending review, including rejected attempts.
    pub fn native_pairing_confirm_identity(&mut self,store:&dyn omniplay_streaming::native_credentials::PairingSecretStore,
        review_id:Uuid,provider_epoch:u64,deadline:std::time::Instant)->Result<Value>{
        self.ensure_open()?;
        let review=self.identity_review.take().ok_or_else(||PlatformError::new(ErrorCode::PermissionDenied,"Native identity review is missing or consumed"))?;
        let deadline=deadline.min(review.deadline);
        if review.id!=review_id||review.provider_epoch!=provider_epoch||std::time::Instant::now()>=deadline{
            return Err(PlatformError::new(ErrorCode::PermissionDenied,"Native identity review expired or changed lifecycle"));
        }
        omniplay_core::stream_signal_authority::require_installed_pairing_policy()?;
        let mut writer=omniplay_core::stream_credential_writer::NativeCredentialWriter::open(&self.database,self.profile_id,store)?;
        let revision=writer.activate_identity_after_local_consent(review.address,review.revision,omniplay_core::now_ms(),deadline)?;
        Ok(json!({"credential_protocol":1,"address":review.address,"revision":revision,"phase":"active","approves_remote_peer":false}))
    }
    pub fn native_pairing_cancel_identity_review(&mut self){self.identity_review=None;}
    pub fn background_sync_policy(&self)->Result<Value>{
        self.ensure_open()?;
        let opted_in=self.database.mobile_background_sync()?;
        let registered=omniplay_sync_client::load_configuration(&self.database)?.is_some_and(|value|value.registered&&value.profile_id==self.profile_id);
        Ok(json!({"protocol":1,"opt_in":opted_in,"eligible":opted_in&&registered&&self.credentials.is_some()&&self.database.settings()?.cloud_sync_opt_in}))
    }
    /// Native scheduler entry only. Renderer RPC has no background-start command.
    pub fn start_background_sync(&mut self,budget_ms:u32)->Result<(crate::SyncSnapshot,crate::SyncJobHandle)>{
        if self.background_sync_policy()?["eligible"]!=true{return Err(PlatformError::new(ErrorCode::PermissionDenied,"Background synchronization is disabled or its account is unavailable"));}
        let provider=self.sync_provider()?;self.sync_jobs.start_background(&self.database.root,provider,budget_ms)
    }
    pub fn expire_capabilities(&mut self){self.transfers.expire();self.permissions.expire();self.directories.expire();self.asset_ranges.expire();}
    fn sync_provider(&self)->Result<Arc<dyn omniplay_sync_client::CredentialStore>>{self.credentials.clone().ok_or_else(||PlatformError::new(ErrorCode::UnsupportedCapability,"Open this profile with its native secure credential provider"))}
    pub fn selected_import(&mut self,path:&Path)->Result<Value>{let grant_id=self.permissions.grant_selected_file(self.owner,path,FileAccess::ImportRead,300_000)?;Ok(json!({"grant_id":grant_id,"suggested_title":path.file_stem().and_then(|v|v.to_str()).unwrap_or("Game").chars().take(120).collect::<String>(),"format_hint":path.extension().and_then(|v|v.to_str()).and_then(ContentFormat::from_extension)}))}
    pub fn selected_export(&mut self,path:&Path)->Result<Value>{let grant_id=self.permissions.grant_selected_file(self.owner,path,FileAccess::ExportWrite,300_000)?;Ok(json!({"grant_id":grant_id}))}
    pub fn selected_folder(&mut self,path:&Path)->Result<Value>{encode(self.directories.selected(self.owner,path)?)}
    pub fn selected_disc_provider(&mut self,provider:Arc<dyn omniplay_core::media_import::DiscProvider>,entries:Vec<String>,title:String)->Result<Value>{encode(self.directories.selected_provider(self.owner,provider,entries,title)?)}
    pub fn import_selected_firmware(&mut self,path:&Path,system:GameSystem,role:&str)->Result<Value>{omniplay_core::firmware::role_sizes(system,role)?;let grant=self.permissions.grant_selected_file(self.owner,path,FileAccess::ImportRead,300_000)?;let file=self.permissions.take(self.owner,grant,FileAccess::ImportRead)?;encode(FirmwareVault::new(&mut self.database).import(system,role,file)?)}
    pub fn handle(&mut self,request:Request,raw:&[u8])->Response{
        let id=request.id;
        if let Err(error)=self.ensure_open().and_then(|_|self.asset_ranges.check_replay(self.owner,id,raw)){return Response::result(id,Err(error));}
        if request.profile_id!=self.profile_id{return Response::result(id,Err(PlatformError::new(ErrorCode::PermissionDenied,"A mobile channel cannot switch profiles")));}
        if let Some((previous,response))=self.results.get(&id){if previous==raw{return response.clone();}return Response::result(id,Err(PlatformError::new(ErrorCode::Conflict,"Request ID was reused with different content")));}
        let sensitive=request.command.contains_secrets();let started=std::time::Instant::now();let response=Response::result(id,request.validate(omniplay_core::now_ms()).and_then(|_|self.execute(request.command)));
        if started.elapsed().as_secs()>=1{self.sessions.defer_broker_work(started.elapsed());}
        if !sensitive{if self.order.len()>=32{if let Some(old)=self.order.pop_front(){self.results.remove(&old);}}self.results.insert(id,(raw.to_vec(),response.clone()));self.order.push_back(id);}response
    }
    fn execute(&mut self,command:Command)->Result<Value>{
        self.sessions.expire(&self.database)?;
        self.expire_capabilities();
        match command{
            Command::GameInputGet{game_id}=>encode(omniplay_core::input::load(&self.database,game_id)?),
            Command::GameInputSet{game_id,input}=>encode(omniplay_core::input::store(&self.database,game_id,input)?),
            Command::GameTouchGet{game_id}=>encode(omniplay_core::input::load_touch(&self.database,game_id)?),
            Command::GameTouchSet{game_id,touch}=>encode(omniplay_core::input::store_touch(&self.database,game_id,touch)?),
            Command::GameCoreOptionsGet{game_id,runtime_id}=>encode(omniplay_core::core_options::load(&self.database,game_id,&runtime_id)?),
            Command::GameCoreOptionsList{game_id}=>encode(omniplay_core::core_options::list(&self.database,game_id)?),
            Command::GameCoreOptionsSet{game_id,runtime_id,options}=>encode(omniplay_core::core_options::store(&self.database,game_id,&runtime_id,options)?),
            Command::RuntimeOptionsGet{..}=>Err(PlatformError::new(ErrorCode::UnsupportedCapability,"No native core options are installed on this mobile host")),
            Command::DeviceSnapshot=>encode(self.device_probe.sample()?),
            Command::Hello=>Ok(json!({"application":"OmniPlay","version":env!("CARGO_PKG_VERSION"),"protocol_major":PROTOCOL_MAJOR,"protocol_minor":PROTOCOL_MINOR,"storage_unused_protocol":1,"save_backup_protocol":1,"library_recovery_protocol":1,"manual_recovery_protocol":1,"save_orphan_protocol":1,"save_origin_protocol":1,"save_repair_protocol":1,"save_cloud_recovery_protocol":if self.credentials.is_some(){1}else{0},"native_asset_ranges":1,"device_id":self.database.device_id,"platform":std::env::consts::OS,"capabilities":["local_library","file_dialog","save_history","save_export","asset_transfer","save_transfer","local_settings","local_assistance","performance_measurements","firmware_vault"],"native_emulator":false,"installed_games":false,"encrypted_sync":self.credentials.is_some(),"sync_policy":"native_scheduled","background_sync_protocol":1,"desktop_updater":false})),
            Command::LibraryList{query,offset,limit}=>encode(Library::new(&self.database).list(query.as_deref(),offset,limit)?),
            Command::LibraryPage{query,system,after,limit,favorites_only}=>encode(Library::new(&self.database).page(query.as_deref(),system,after.as_deref(),limit,favorites_only)?),
            Command::LibraryFavoriteGet{game_id}=>encode(Library::new(&self.database).favorite(game_id)?),
            Command::LibraryFavoriteSet{game_id,favorite}=>encode(Library::new(&self.database).set_favorite(game_id,favorite)?),
            Command::LibraryGet{game_id}=>encode(Library::new(&self.database).get(game_id)?),
            Command::LibraryRemove{game_id}=>{if self.sessions.list().iter().any(|s|s.game_id==game_id&&!s.state.is_terminal()){return Err(PlatformError::new(ErrorCode::Busy,"Stop the game before removing it"));}Library::new(&self.database).remove(game_id)?;Ok(json!({"removed":true}))}
            Command::LibraryImportStart{grant_id,title,system,format_hint}=>{if self.imports.busy(){return Err(PlatformError::new(ErrorCode::Busy,"Wait for the current import to finish"));}let file=self.permissions.take(self.owner,grant_id,FileAccess::ImportRead)?;encode(self.imports.start(&self.database.root,title,system,crate::import_jobs::ImportSource::Single{file,format_hint})?)}
            Command::LibraryImportBundleStart{grant_id,entry_name,title,system,encoding}=>{if self.imports.busy(){return Err(PlatformError::new(ErrorCode::Busy,"Wait for the current import to finish"));}let scope=self.directories.take(self.owner,grant_id,&entry_name)?;encode(self.imports.start(&self.database.root,title,system,crate::import_jobs::ImportSource::Bundle{scope,entry:entry_name,encoding})?)}
            Command::LibraryImportStatus{job_id}=>encode(self.imports.status(job_id)?),Command::LibraryImportCancel{job_id}=>encode(self.imports.cancel(job_id)?),
            Command::PermissionRevoke{grant_id}=>{self.permissions.revoke(self.owner,grant_id)?;self.directories.revoke(self.owner,grant_id)?;Ok(json!({"revoked":true}))}
            Command::AssetReadStart{game_id,asset_name}=>{let library=Library::new(&self.database);let game=library.get(game_id)?;let(path,hash,bytes)=library.asset_details(&game,asset_name.as_deref())?;encode(self.transfers.open_download(File::open(path).map_err(storage_error)?,hash,bytes)?)}
            Command::AssetRangesStart{game_id}=>encode(self.asset_ranges.start(&self.database,self.owner,game_id)?),
            Command::FirmwareReadStart{game_id,role,expected_hash}=>{
                ContentHash::parse(expected_hash.0.clone())?;let game=Library::new(&self.database).get(game_id)?;
                let(asset,file)=FirmwareVault::new(&mut self.database).selected(game.system,&role)?;
                if asset.content_hash!=expected_hash{return Err(PlatformError::new(ErrorCode::Conflict,"The selected firmware changed; reopen the game to use its current selection"));}
                let transfer=self.transfers.open_download(file,asset.content_hash.clone(),asset.byte_length)?;Ok(json!({"role":role,"sha256":asset.content_hash,"byte_length":asset.byte_length,"transfer":transfer}))
            }
            Command::AssetRangesStatus{job_id}=>encode(self.asset_ranges.status(self.owner,job_id)?),
            Command::AssetRangesCollect{job_id}=>encode(self.asset_ranges.collect(self.owner,job_id)?),
            Command::AssetRangesCancel{job_id}=>encode(self.asset_ranges.cancel_job(self.owner,job_id)?),
            Command::AssetRangesKeepAlive{set_id}=>encode(self.asset_ranges.keep_alive(self.owner,set_id)?),
            Command::AssetRangesClose{set_id}=>{self.asset_ranges.close_set(self.owner,set_id)?;Ok(json!({"closed":true}))},
            Command::AssetRangeRead{..}=>Err(PlatformError::new(ErrorCode::UnsupportedCapability,"Use the native media I/O lane for positional reads")),
            Command::TransferRead{transfer_id,offset,limit}=>encode(self.transfers.read(transfer_id,offset,limit)?),Command::TransferAbort{transfer_id}=>{self.transfers.abort(transfer_id);Ok(json!({"aborted":true}))}
            Command::SaveList{game_id}=>encode(SaveStore::new(&mut self.database).list(game_id)?),Command::SaveHeads{game_id}=>encode(SaveStore::new(&mut self.database).heads(game_id)?),
            Command::SaveOriginGet{save_id}=>encode(omniplay_core::save_origins::get(&self.database,save_id)?),
            Command::SavePage{game_id,after,limit}=>encode(SaveStore::new(&mut self.database).page(game_id,after.as_deref(),limit)?),
            Command::SavePromote{save_id,expected_head_id}=>encode(SaveStore::new(&mut self.database).promote(save_id,expected_head_id)?),
            Command::SaveExportUploadStart{metadata,destination_grant_id}=>{metadata.validate()?;if Library::new(&self.database).get(metadata.game_id)?.content_hash!=metadata.game_content_hash{return Err(PlatformError::new(ErrorCode::ContentMismatch,"Export content is not in this library"));}let file=self.permissions.take(self.owner,destination_grant_id,FileAccess::ExportWrite)?;let transfer_id=self.transfers.start_export(metadata,destination_grant_id,file)?;Ok(json!({"transfer_id":transfer_id,"chunk_bytes":48*1024}))}
            Command::SaveExportUploadChunk{transfer_id,offset,payload_base64}=>Ok(json!({"next_offset":self.transfers.export_chunk(transfer_id,offset,&payload_base64)?})),
            Command::SaveExportUploadCommit{transfer_id,destination_grant_id}=>{self.transfers.commit_export(transfer_id,destination_grant_id)?;Ok(json!({"exported":true,"delivery":"file_handle"}))},
            Command::SavePurgePreview{game_id}=>encode(omniplay_core::save_maintenance::purge_preview(&self.database,game_id)?),
            Command::SavePurge{game_id,expected_revision}=>encode(omniplay_core::save_maintenance::purge(&mut self.database,game_id,expected_revision)?),
            Command::SaveCleanup=>encode(omniplay_core::save_maintenance::cleanup(&self.database,128)?),
            Command::SaveCleanupStatus=>encode(omniplay_core::save_maintenance::cleanup_status(&self.database)?),
            Command::StorageUnusedPreview{after_hash}=>encode(omniplay_core::asset_storage::preview(&self.database,after_hash.as_deref())?),
            Command::StorageUnusedRemove{after_hash,revision}=>encode(omniplay_core::asset_storage::remove(&self.database,after_hash.as_deref(),&revision)?),
            Command::SaveImport{grant_id,game_id}=>{let mut file=self.permissions.take(self.owner,grant_id,FileAccess::ImportRead)?;encode(SaveStore::new(&mut self.database).import_export(game_id,&mut file)?)}
            Command::SaveRepair{grant_id,save_id}=>{let mut file=self.permissions.take(self.owner,grant_id,FileAccess::ImportRead)?;encode(SaveStore::new(&mut self.database).repair_export(save_id,&mut file)?)}
            Command::SaveExport{save_id,destination_grant_id}=>{let mut file=self.permissions.take(self.owner,destination_grant_id,FileAccess::ExportWrite)?;SaveStore::new(&mut self.database).export(save_id,&mut file)?;Ok(json!({"exported":true}))}
            Command::SaveBackupExport{save_ids,destination_grant_id}=>{let mut file=self.permissions.take(self.owner,destination_grant_id,FileAccess::ExportWrite)?;encode(omniplay_core::save_backup::export(&mut self.database,&save_ids,&mut file)?)},
            Command::SaveBackupRestore{grant_id}=>{let mut file=self.permissions.take(self.owner,grant_id,FileAccess::ImportRead)?;encode(omniplay_core::save_backup::restore(&mut self.database,&mut file)?)},
            Command::LibraryRecoveryList{after}=>encode(omniplay_core::library_recovery::list(&self.database,after.as_deref())?),
            Command::LibraryRecoveryImport{candidate_id,revision,title,system}=>encode(omniplay_core::library_recovery::recover(&self.database,&candidate_id,&revision,title,system)?),
            Command::SaveOrphanPreview{save_id,after}=>encode(omniplay_core::save_orphan_recovery::preview(&self.database,save_id,after.as_deref())?),
            Command::SaveOrphanRecover{save_id,candidate_id,revision}=>encode(omniplay_core::save_orphan_recovery::recover(&mut self.database,save_id,&candidate_id,&revision)?),
            Command::ManualRecoveryList{after}=>encode(omniplay_core::manual_recovery::list(&self.database,after.as_ref())?),
            Command::ManualRecoveryInspect{candidate_id}=>encode(omniplay_core::manual_recovery::inspect(&self.database,&candidate_id)?),
            Command::ManualRecoveryExport{candidate_id,revision,destination_grant_id}=>{let mut file=self.permissions.take(self.owner,destination_grant_id,FileAccess::ExportWrite)?;encode(omniplay_core::manual_recovery::export(&self.database,&candidate_id,&revision,&mut file)?)},
            Command::ManualRecoveryExtract{grant_id,destination_grant_id}=>{let mut source=self.permissions.take(self.owner,grant_id,FileAccess::ImportRead)?;let mut destination=self.permissions.take(self.owner,destination_grant_id,FileAccess::ExportWrite)?;encode(omniplay_core::manual_recovery::extract(&mut source,&mut destination)?)},
            Command::SaveReadStart{save_id,compatibility}=>{let metadata=SaveStore::new(&mut self.database).metadata(save_id)?;metadata.check_compatible(&compatibility)?;let file=SaveStore::new(&mut self.database).open_payload(&metadata)?;let transfer=self.transfers.open_download(file,metadata.payload_hash.clone(),metadata.payload_bytes)?;Ok(json!({"metadata":metadata,"transfer":transfer}))}
            Command::SaveUploadStart{metadata,expected_parent_id}=>{if Library::new(&self.database).get(metadata.game_id)?.content_hash!=metadata.game_content_hash{return Err(PlatformError::new(ErrorCode::ContentMismatch,"Save belongs to different game content"));}let transfer_id=self.transfers.start_upload(&self.database.root,metadata,expected_parent_id)?;Ok(json!({"transfer_id":transfer_id,"chunk_bytes":48*1024}))}
            Command::SaveUploadChunk{transfer_id,offset,payload_base64}=>Ok(json!({"next_offset":self.transfers.upload_chunk(transfer_id,offset,&payload_base64)?})),Command::SaveUploadCommit{transfer_id}=>{let(metadata,payload,parent)=self.transfers.finish_upload(transfer_id)?;encode(SaveStore::new(&mut self.database).commit(metadata,&payload,parent)?)}
            Command::SettingsGet=>encode(self.database.settings()?),Command::SettingsSet{mut settings}=>{
                if settings.cloud_sync_opt_in{self.sync_provider()?;if !omniplay_sync_client::load_configuration(&self.database)?.is_some_and(|value|value.registered&&value.profile_id==self.profile_id){return Err(PlatformError::new(ErrorCode::PermissionDenied,"Connect and register a synchronization account before enabling it"));}}
                settings.telemetry_opt_in=false;settings.suspend_background_tabs=false;self.database.set_settings(&settings)?;if !settings.cloud_sync_opt_in{self.sync_jobs.cancel_all();}encode(settings)
            }
            Command::SyncStatus=>{self.sync_provider()?;let configuration=omniplay_sync_client::load_configuration(&self.database)?;Ok(json!({"connected":configuration.is_some(),"enabled":self.database.settings()?.cloud_sync_opt_in,"account":configuration,"running":self.sync_jobs.busy(),"job":self.sync_jobs.current(),"action":self.sync_jobs.current_action(self.owner),"action_protocol":1,"foreground":self.lifecycle.foreground(),"policy":"native_scheduled","background":self.background_sync_policy()?}))}
            Command::SyncBackgroundConfigure{enabled}=>{
                if !self.lifecycle.foreground(){return Err(PlatformError::new(ErrorCode::PermissionDenied,"Open Game Hub before changing background synchronization"));}
                if enabled{self.sync_provider()?;if !self.database.settings()?.cloud_sync_opt_in||!omniplay_sync_client::load_configuration(&self.database)?.is_some_and(|value|value.registered&&value.profile_id==self.profile_id){return Err(PlatformError::new(ErrorCode::PermissionDenied,"Enable a registered synchronization account first"));}}
                self.database.set_mobile_background_sync(enabled)?;if !enabled{self.lifecycle.cancel_background();}self.background_sync_policy()
            }
            Command::SyncActionStart{action}=>{let provider=self.sync_provider()?;encode(self.sync_jobs.start_action(&self.database.root,self.profile_id,self.owner,provider,action)?)}
            Command::SyncActionStatus{action_id}=>encode(self.sync_jobs.action_status(self.owner,action_id)?),
            Command::SyncActionCollect{action_id}=>self.sync_jobs.collect_action(self.owner,action_id),
            Command::SyncActionCancel{action_id}=>encode(self.sync_jobs.cancel_action(self.owner,action_id)?),
            Command::SyncActionDiscard{action_id}=>encode(self.sync_jobs.discard_action(self.owner,action_id)?),
            Command::SyncCreate{audience}=>{let provider=self.sync_provider()?;encode(self.sync_jobs.account(provider,|store|omniplay_sync_client::create_account_with_store(&self.database,self.profile_id,&audience,store))?)}
            Command::SyncRegister{recovery_key,registration_token}=>{let recovery=zeroize::Zeroizing::new(recovery_key);let registration=zeroize::Zeroizing::new(registration_token);self.sync_jobs.account(self.sync_provider()?,|store|omniplay_sync_client::register_account_with_store(&self.database,&recovery,&registration,store))?;Ok(json!({"registered":true}))}
            Command::SyncRecover{audience,recovery_key}=>{let recovery=zeroize::Zeroizing::new(recovery_key);encode(self.sync_jobs.account(self.sync_provider()?,|store|omniplay_sync_client::recover_account_with_store(&self.database,self.profile_id,&audience,&recovery,store))?)}
            Command::SyncRenew{recovery_key}=>{let recovery=zeroize::Zeroizing::new(recovery_key);self.sync_jobs.account(self.sync_provider()?,|store|omniplay_sync_client::renew_certificate_with_store(&self.database,&recovery,store))?;Ok(json!({"renewed":true}))}
            Command::SyncDevices=>encode(self.sync_jobs.account(self.sync_provider()?,|store|omniplay_sync_client::SyncClient::open_with_store(&self.database,store)?.devices())?),
            Command::SyncRevokeDevice{recovery_key,device_id}=>{let recovery=zeroize::Zeroizing::new(recovery_key);self.sync_jobs.account(self.sync_provider()?,|store|omniplay_sync_client::revoke_device_with_store(&self.database,&recovery,device_id,store))?;Ok(json!({"revoked":true}))}
            Command::SyncDeleteAccount{recovery_key}=>{let recovery=zeroize::Zeroizing::new(recovery_key);self.sync_jobs.account(self.sync_provider()?,|store|omniplay_sync_client::delete_account_with_store(&self.database,&recovery,store))?;Ok(json!({"deleted":true}))}
            Command::SyncDisconnect=>{self.sync_jobs.account(self.sync_provider()?,|store|omniplay_sync_client::disconnect_with_store(&self.database,store))?;Ok(json!({"disconnected":true}))}
            Command::SyncRunStart=>{let provider=self.sync_provider()?;if !self.database.settings()?.cloud_sync_opt_in{return Err(PlatformError::new(ErrorCode::PermissionDenied,"Synchronization is disabled"));}encode(self.sync_jobs.start(&self.database.root,provider)?)}
            Command::SyncRunStatus{job_id}=>encode(self.sync_jobs.status(job_id)?),Command::SyncRunCancel{job_id}=>encode(self.sync_jobs.cancel(job_id)?),
            Command::SyncSaveRecoveryRequest{save_id}=>{let provider=self.sync_provider()?;encode(self.sync_jobs.account(provider,|store|omniplay_sync_client::request_save_recovery(&self.database,save_id,store))?)},
            Command::SyncSaveRecoveryList{game_id}=>encode(omniplay_sync_client::list_save_recoveries(&self.database,game_id)?),
            Command::SyncSaveRecoveryCancel{save_id}=>encode(self.sync_jobs.cancel_recovery(&self.database,save_id,self.sync_provider()?)?),
            Command::GameSettingsGet{game_id}=>encode(session_preferences(&self.database,game_id)?),Command::GameSettingsSet{game_id,config}=>{set_session_preferences(&self.database,game_id,&config)?;encode(config)}
            Command::RuntimeRank{game_id,device,mut candidates}=>{for c in &mut candidates{if c.kind!=RuntimeKind::WebWasm{c.available=false;}c.benchmark=None;}let game=Library::new(&self.database).get(game_id)?;encode(router::rank(&game,&device,&candidates,&RoutingPolicy{native_allowed:false,remote_allowed:false,paired_host_available:false,device_class:"unclassified".into()},omniplay_core::now_ms())?)}
            Command::AssistantAdvise{game_id,topic}=>encode(self.assistant.advise(&self.database,game_id,topic)?),Command::AssistantAccept{proposal_id}=>encode(self.assistant.accept(&self.database,proposal_id)?),Command::AssistantDismiss{proposal_id}=>{self.assistant.dismiss(proposal_id);Ok(json!({"dismissed":true}))}
            Command::WebSessionBegin{game_id,runtime_id}=>encode(self.sessions.begin(&self.database,game_id,&runtime_id)?),Command::WebSessionHeartbeat{session_id,state}=>encode(self.sessions.heartbeat(&self.database,session_id,state)?),Command::SessionList=>encode(self.sessions.list()),
            Command::RuntimeList=>Ok(json!([])),Command::MetricsSnapshot=>Ok(json!({})),
            Command::PerformanceRunCommit{run}=>encode(PerformanceRuns::new(&mut self.database).commit(run)?),Command::PerformanceRunList{game_id}=>encode(PerformanceRuns::new(&mut self.database).list(game_id)?),Command::PerformanceRunCompare{baseline_id,candidate_id}=>encode(PerformanceRuns::new(&mut self.database).compare(baseline_id,candidate_id)?),
            Command::PerformanceRunExport{run_id,destination_grant_id}=>{let mut file=self.permissions.take(self.owner,destination_grant_id,FileAccess::ExportWrite)?;PerformanceRuns::new(&mut self.database).export(run_id,&mut file)?;Ok(json!({"exported":true}))}
            Command::PerformanceRunDelete{run_id}=>{PerformanceRuns::new(&mut self.database).delete(run_id)?;Ok(json!({"deleted":true}))}Command::PerformanceRunsClear=>{PerformanceRuns::new(&mut self.database).clear()?;Ok(json!({"deleted":true}))}
            Command::DiagnosticsPreview=>encode(DiagnosticPreview::collect(&self.database,&self.sessions.list())?),Command::DiagnosticsExport{destination_grant_id}=>{let mut file=self.permissions.take(self.owner,destination_grant_id,FileAccess::ExportWrite)?;DiagnosticPreview::collect(&self.database,&self.sessions.list())?.write(&mut file)?;Ok(json!({"exported":true}))}
            Command::FirmwareList=>encode(FirmwareVault::new(&mut self.database).list()?),Command::FirmwareSelect{system,role,content_hash}=>{FirmwareVault::new(&mut self.database).select(system,&role,&content_hash)?;Ok(json!({"selected":true}))}Command::FirmwareForget{content_hash}=>{FirmwareVault::new(&mut self.database).forget(&content_hash)?;Ok(json!({"removed":true}))}
            Command::Shutdown=>{self.close();Ok(json!({"stopped":true}))}
            _=>Err(PlatformError::new(ErrorCode::UnsupportedCapability,"This operation is not enabled by the current mobile platform policy")),
        }
    }
    fn clear_transient(&mut self){self.identity_review=None;self.asset_ranges.close_owner(self.owner);self.sync_jobs.cancel_all();self.sync_jobs.abandon_actions(self.owner);self.imports.cancel_all();self.sessions.close(&self.database);self.transfers.clear();self.permissions.revoke_owner(self.owner);self.directories.revoke_owner(self.owner);}
    pub fn close(&mut self){self.closed=true;self.lifecycle.set_foreground(false);self.clear_transient();}
    pub fn detach(&mut self){self.clear_transient();self.owner=Uuid::new_v4();self.transfers=crate::transfers::Transfers::default();self.results.clear();self.order.clear();}
    pub fn suspend(&mut self)->Result<()>{let ids=self.sessions.list().into_iter().filter(|s|s.state==SessionState::Running).map(|s|s.id).collect::<Vec<_>>();for id in ids{self.sessions.heartbeat(&self.database,id,SessionState::Paused)?;}Ok(())}
}
impl Drop for LocalHost{fn drop(&mut self){self.close();}}
fn encode(value:impl serde::Serialize)->Result<Value>{serde_json::to_value(value).map_err(|_|PlatformError::new(ErrorCode::Internal,"Local response could not be encoded"))}
