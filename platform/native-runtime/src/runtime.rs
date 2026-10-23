use crate::{channel::{Reply,WorkerChannel},registry::{RuntimePackage,approved_file},unavailable};
use omniplay_contracts::*;
use omniplay_core::{sessions::{RuntimeAdapter,RuntimeObservation},util::{PendingFile,digest,storage_error}};
use omniplay_updater::package::PackageManifest;
use omniplay_sandbox::{ContainedProcess,ContainerIdentity,Limits,PresentationHost,PresentationLaunch};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest,Sha256};
use std::{fs::{File,OpenOptions},io::{Read,Write},path::PathBuf,time::Duration};
use uuid::Uuid;
use std::sync::{Arc,atomic::{AtomicBool,Ordering}};

pub struct NativeAdapter {root:PathBuf,package:RuntimePackage,profile_root:PathBuf,profile_id:Uuid,inventory:Option<PackageManifest>,work:Option<PathBuf>,process:Option<ContainedProcess>,presentation:Option<PresentationHost>,channel:Option<WorkerChannel>,file_guards:Vec<File>,prepared:bool,firmware_hashes:std::collections::BTreeMap<String,ContentHash>,lease:Option<File>,cancel:Option<Arc<AtomicBool>>,measurement_id:Option<Uuid>,container:Option<ContainerIdentity>,storage_reservation:Option<Uuid>}
impl NativeAdapter {
    pub fn new(root:PathBuf,package:RuntimePackage,profile_root:PathBuf,profile_id:Uuid,inventory:Option<PackageManifest>)->Self{Self{root,package,profile_root,profile_id,inventory,work:None,process:None,presentation:None,channel:None,file_guards:Vec::new(),prepared:false,firmware_hashes:std::collections::BTreeMap::new(),lease:None,cancel:None,measurement_id:None,container:None,storage_reservation:None}}
    fn call(&mut self,method:&str,params:serde_json::Value,payload:Vec<u8>,seconds:u64)->Result<Reply>{let process=self.process.as_ref().ok_or_else(||unavailable("No contained native runtime is active"))?;let cancel=self.cancel.as_deref().unwrap_or(&NEVER_CANCELLED);self.channel.as_mut().ok_or_else(||unavailable("Native runtime control is unavailable"))?.call_cancellable(process,method,params,payload,Duration::from_secs(seconds),cancel)}
    fn cleanup(&mut self){
        let mut actor_stopped=true;let mut presentation_stopped=true;
        if let Some(process)=self.process.take(){process.terminate();actor_stopped=process.has_exited();drop(process);}
        if let Some(presentation)=self.presentation.take(){presentation.terminate();presentation_stopped=presentation.has_exited();drop(presentation);}
        let stopped=actor_stopped&&presentation_stopped;
        if !actor_stopped{if let Some(channel)=&mut self.channel{channel.abandon();}}
        self.channel.take();self.file_guards.clear();self.prepared=false;self.measurement_id=None;self.firmware_hashes.clear();
        let retired=self.container.take().map(|container|container.retire().is_ok()).unwrap_or(true);
        self.lease.take();
        if let Some(work)=self.work.take(){if stopped&&retired{let _=crate::work::collect_one(&self.profile_root,self.profile_id,&work);}}
        if let Some(id)=self.storage_reservation.take(){if stopped&&retired{if let Ok(database)=omniplay_core::Database::open(&self.profile_root){let _=omniplay_core::native_work_storage::release_uncreated(&database,self.profile_id,id);}}}
    }
    fn parse_measurement_status(&self,reply:Reply,expected:Uuid)->Result<NativeMeasurementStatus>{
        if !reply.payload.is_empty(){return Err(unavailable("Native measurement status contains an unexpected payload"));}
        let status:NativeMeasurementStatus=serde_json::from_value(reply.result).map_err(|_|unavailable("Native measurement status is malformed"))?;status.validate()?;
        if status.id!=expected{return Err(unavailable("Native measurement identity changed"));}Ok(status)
    }
}
impl Drop for NativeAdapter{fn drop(&mut self){self.cleanup();}}

impl RuntimeAdapter for NativeAdapter {
    fn manifest(&self)->&RuntimeManifest{&self.package.manifest}
    fn prepare_cancellable(&mut self,game:&GameManifest,asset:File,config:&SessionConfig,cancel:Arc<AtomicBool>)->Result<()>{self.cancel=Some(cancel);let result=self.prepare(game,asset,config);if result.is_err(){self.cleanup();}self.cancel=None;result}
    fn prepare(&mut self,game:&GameManifest,asset:File,config:&SessionConfig)->Result<()> {
        if self.prepared||self.process.is_some()||self.presentation.is_some(){return Err(PlatformError::new(ErrorCode::InvalidState,"The native runtime is already prepared"));}
        self.package.manifest.validate()?;game.validate()?;config.validate()?;
        check_cancel(self.cancel.as_deref().unwrap_or(&NEVER_CANCELLED))?;
        let control_reservation=WorkerChannel::reserve()?;
        let mut input_profile={let database=omniplay_core::Database::open(&self.profile_root)?;
            let profile=omniplay_core::core_options::load(&database,game.id,&self.package.manifest.id)?;
            let policy=CoreOptionPolicy{schema_version:1,definitions:self.package.user_core_options.clone()};
            self.package.core_options.extend(policy.resolve(&profile)?);
            omniplay_core::input::load(&database,game.id)?};
        check_cancel(self.cancel.as_deref().unwrap_or(&NEVER_CANCELLED))?;
        if game.byte_length>self.package.maximum_rom_bytes{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Game exceeds this native core's ROM bound"));}
        for entry in &self.package.files{self.file_guards.push(approved_file(&self.root,&entry.path,self.inventory.as_ref(),Some(entry))?);}
        let format=game.media.as_ref().map(|m|m.format).unwrap_or(ContentFormat::RawRom);
        if !self.package.content_formats.is_empty()&&!self.package.content_formats.contains(&format){return Err(PlatformError::new(ErrorCode::UnsupportedCapability,"This native core does not support the selected content format"));}
        let extension=format.extension(game.system)?;
        let parent=self.profile_root.join("native-work");std::fs::create_dir_all(&parent).map_err(storage_error)?;
        let(staging_bytes,staging_files)=self.package.staging_budget(game)?;let work_id=Uuid::new_v4();
        // Protect the reservation -> directory -> exclusive owner handoff.
        // Recovery must not interpret a live creator's not-yet-created path as
        // abandoned. Only this gated Windows path stamps creation protocol 1.
        #[cfg(windows)]let creator=crate::work::creation_guard(&self.profile_root)?;
        #[cfg(windows)]omniplay_core::native_work_storage::reserve_windows(&mut omniplay_core::Database::open(&self.profile_root)?,self.profile_id,work_id,staging_bytes,staging_files)?;
        #[cfg(not(windows))]omniplay_core::native_work_storage::reserve(&mut omniplay_core::Database::open(&self.profile_root)?,self.profile_id,work_id,staging_bytes,staging_files)?;
        self.storage_reservation=Some(work_id);
        let work=parent.join(work_id.to_string());let mut directory=std::fs::DirBuilder::new();
        #[cfg(unix)]{use std::os::unix::fs::DirBuilderExt;directory.mode(0o700);}
        directory.create(&work).map_err(storage_error)?;self.work=Some(work.clone());
        let mut lease=OpenOptions::new();lease.read(true).write(true).create_new(true);
        #[cfg(unix)]{use std::os::unix::fs::OpenOptionsExt;lease.mode(0o600);}
        #[cfg(windows)]{use std::os::windows::fs::OpenOptionsExt;lease.share_mode(1);}
        self.lease=Some(lease.open(work.join("owner.lease")).map_err(storage_error)?);
        #[cfg(windows)]drop(creator);
        let(filename,primary_bytes)=if let Some(media)=&game.media{
            let mut primary=Some(asset);
            for entry in &media.assets{
                let source=if entry.name==media.entry_name{primary.take().ok_or_else(||unavailable("The primary media asset is duplicated"))?}else{let mut options=OpenOptions::new();options.read(true);#[cfg(windows)]{use std::os::windows::fs::OpenOptionsExt;options.share_mode(1);}options.open(self.profile_root.join("assets").join(&entry.content_hash.0)).map_err(storage_error)?};
                materialize(&work,&entry.name,&entry.content_hash,entry.byte_length,source,self.cancel.as_deref().unwrap_or(&NEVER_CANCELLED))?;
            }
            (media.entry_name.clone(),media.entry()?.byte_length)
        }else{let filename=format!("game.{extension}");materialize(&work,&filename,&game.content_hash,game.byte_length,asset,self.cancel.as_deref().unwrap_or(&NEVER_CANCELLED))?;(filename,game.byte_length)};
        if !self.package.firmware_requirements.is_empty(){
            let mut database=omniplay_core::Database::open(&self.profile_root)?;
            let mut groups=std::collections::BTreeMap::<String,bool>::new();
            for requirement in &self.package.firmware_requirements {
                if let Some(group)=&requirement.required_group{groups.entry(group.clone()).or_insert(false);}
                let selected=omniplay_core::firmware::FirmwareVault::new(&mut database).selected(game.system,&requirement.role);
                let(metadata,mut source)=match selected{Ok(value)=>value,Err(error)if error.code==ErrorCode::NotFound&&(requirement.required_group.is_some()||requirement.optional)=>continue,Err(error)=>return Err(error)};
                if !requirement.allowed_bytes.contains(&metadata.byte_length){return Err(PlatformError::new(ErrorCode::ContentMismatch,"Selected firmware does not match this core's declared shape"));}
                let payload=omniplay_core::util::bounded_read(&mut source,metadata.byte_length)?;if digest(&payload)!=metadata.content_hash{return Err(PlatformError::new(ErrorCode::CorruptData,"Firmware changed while preparing the runtime"));}
                let mut pending=PendingFile::create(&work)?;pending.write_all(&payload)?;pending.commit_new(&work.join(&requirement.filename))?;
                self.firmware_hashes.insert(requirement.role.clone(),metadata.content_hash);
                if let Some(group)=&requirement.required_group{groups.insert(group.clone(),true);}
            }
            if groups.values().any(|found|!*found){return Err(PlatformError::new(ErrorCode::NotFound,"This core needs at least one user-imported firmware file for its declared firmware group").recover("Import firmware for the region of the game in Settings"));}
        }
        let staged=crate::work::staging_ready(&work,&json!({"schema_version":1,"work_id":work_id,"game_id":game.id,"game_content_hash":game.content_hash,"game_bytes":game.byte_length,"firmware_hashes":self.firmware_hashes}))?;
        omniplay_core::native_work_storage::staged(&omniplay_core::Database::open(&self.profile_root)?,self.profile_id,work_id,&staged)?;
        crate::work::creation_intent(&work,self.profile_id,work_id)?;
        self.container=Some(ContainerIdentity::create_for_session(&crate::work::container_name(work_id))?);
        crate::work::created(&work,self.profile_id,work_id)?;
        let identity=self.container.as_ref().ok_or_else(||unavailable("The session container identity is absent"))?;
        identity.grant_game_read(&work)?;
        // A memory-only declaration is not authority to write arbitrary OS
        // scratch files. Keep the legacy grant only for explicitly declared
        // disk-backed save/system routes; protocol 1 uses the bounded store.
        let legacy_os_writes=self.package.legacy_os_writes();
        let scratch=work.join("scratch");std::fs::create_dir(&scratch).map_err(storage_error)?;
        if legacy_os_writes{identity.grant_private_write(&scratch)?;}
        if self.package.filesystem_save_directory.is_some(){let saves=work.join("save");std::fs::create_dir(&saves).map_err(storage_error)?;if legacy_os_writes{identity.grant_private_write(&saves)?;}}
        let worker=self.root.join(&self.package.worker);let runtime_directory=worker.parent().ok_or_else(||unavailable("Native worker has no package directory"))?;
        omniplay_sandbox::grant_public_runtime_read(runtime_directory)?;
        crate::work::launch_intent(&work,work_id)?;
        let limits=Limits{memory_bytes:self.package.memory_limit_mb as usize*1024*1024};
        let launched=if let Some(presentation)=&self.package.presentation{
            let temporary=work.join("presentation-tmp");std::fs::create_dir(&temporary).map_err(storage_error)?;
            let executable=self.root.join(&presentation.executable);let session=work_id.to_string();
            let spec=PresentationLaunch{executable:&executable,working_directory:runtime_directory,temporary_directory:&temporary,session:&session,maximum_width:presentation.maximum_width,maximum_height:presentation.maximum_height,volume_percent:(config.audio_volume*100.0).round()as u16,haptics:input_profile.haptics_enabled&&self.package.manifest.capabilities.contains(&Capability::Haptics),memory_bytes:presentation.memory_limit_mb as usize*1024*1024};
            crate::work::presentation_launch_intent(&work,work_id)?;
            let pair=identity.launch_presented(&worker,&work,&limits,&spec,|process|crate::work::process(&work,work_id,process),|process|crate::work::presentation_process(&work,work_id,process))?;
            self.presentation=Some(pair.presentation);pair.actor
        }else{identity.launch(&worker,&work,&limits,|process|crate::work::process(&work,work_id,process))?};
        self.process=Some(launched.process);self.channel=Some(WorkerChannel::new_reserved(launched.input,launched.output,control_reservation)?);
        let hello=self.call("hello",json!({}),Vec::new(),10)?;
        if hello.result["runtime_id"].as_str()!=Some(self.package.manifest.id.as_str())||hello.result["library_name"].as_str()!=Some(self.package.expected_library_name.as_str())||hello.result["library_version"].as_str()!=Some(self.package.expected_library_version.as_str())||hello.result["video_backend"].as_str()!=Some(self.package.video_backend.as_str()){return Err(unavailable("The native core identity or video policy differs from its approved catalog"));}
        if hello.result["input_profile_protocol"].as_u64()!=Some(4){return Err(unavailable("The installed native worker does not support validated input profiles"));}
        if self.package.pause_barrier_protocol!=0&&hello.result["pause_barrier_protocol"].as_u64()!=Some(u64::from(self.package.pause_barrier_protocol)){return Err(unavailable("The installed worker lacks the declared checked pause barrier"));}
        if self.package.writable_files_protocol==1&&hello.result["writable_files_protocol"].as_u64()!=Some(1){return Err(unavailable("The installed native worker does not support the declared bounded memory files"));}
        if self.package.presentation.is_some(){
            if hello.result["presentation_protocol"].as_u64()!=Some(2)||hello.result["presentation_mode"].as_str()!=Some("brokered_rgba")||hello.result["measurement_protocol"].as_u64()!=Some(0){return Err(unavailable("The installed native worker differs from its private presentation declaration"));}
        }else if hello.result.get("presentation_mode").is_some_and(|value|value.as_str()!=Some("direct")){return Err(unavailable("The installed native worker selected an undeclared presentation path"));}
        if self.package.file_io_protocol==1&&(hello.result["checked_file_io_protocol"].as_u64()!=Some(1)||hello.result["persistent_files_protocol"].as_u64()!=Some(2)){return Err(unavailable("The installed worker does not support checked persistent file capture"));}
        let portable_md=self.package.portable_in_game_format.as_deref()==Some("ares-md-canonical-files-v1");
        if portable_md&&hello.result["portable_file_protocol"].as_u64()!=Some(1){return Err(unavailable("The installed worker does not support portable MD progress"));}
        if self.package.filesystem_save_memory&&hello.result["combined_in_game_protocol"].as_u64()!=Some(1){return Err(unavailable("The installed worker does not support complete memory and file progress"));}
        if hello.result["haptics_protocol"].as_u64()!=Some(1){self.package.manifest.capabilities.remove(&Capability::Haptics);}
        input_profile.haptics_enabled&=self.package.manifest.capabilities.contains(&Capability::Haptics);
        // Browser device identifiers/calibration never enter the native core.
        // SDL mappings and the shared physical-axis tuning apply here.
        input_profile.raw_gamepads.clear();
        if hello.result["measurement_protocol"].as_u64()!=Some(1){self.package.manifest.capabilities.remove(&Capability::PerformanceMeasurement);}
        let prepared=self.call("prepare",json!({"filename":filename,"input_profile":input_profile,"title":game.title,"bytes":primary_bytes,"config":config,"core_options":self.package.core_options,"minimum_system_ram_bytes":self.package.minimum_system_ram_bytes,"reset_after_ram_load":self.package.reset_after_ram_load,"filesystem_save_directory":self.package.filesystem_save_directory,"core_save_directory":self.package.core_save_directory,"filesystem_save_memory":self.package.filesystem_save_memory,"file_io_protocol":self.package.file_io_protocol,"writable_files_protocol":self.package.writable_files_protocol,"portable_files_profile":if portable_md{"ares_md_v1"}else{""},"has_system_assets":self.package.has_system_assets,"use_scratch_system_directory":self.package.use_scratch_system_directory,"video_backend":self.package.video_backend,"gpu_max_width":self.package.gpu_max_width,"gpu_max_height":self.package.gpu_max_height,"analog_controls":matches!(game.system,GameSystem::N64|GameSystem::Psp),"prepare_completion":self.package.prepare_completion,"pause_barrier":self.package.pause_barrier,"pause_barrier_protocol":self.package.pause_barrier_protocol}),Vec::new(),60)?;
        if portable_md&&prepared.result["portable_files_profile"].as_str()!=Some("ares_md_v1"){return Err(unavailable("The native worker did not acknowledge its portable save descriptor"));}
        if self.package.writable_files_protocol==1&&prepared.result["writable_files_protocol"].as_u64()!=Some(1){return Err(unavailable("The native core did not acknowledge its bounded memory-file installation"));}
        let portable_available=if portable_md{prepared.result["portable_progress_available"].as_bool().ok_or_else(||unavailable("The native portable save availability is invalid"))?}else{true};
        let memory=prepared.result["save_ram_bytes"].as_u64().zip(prepared.result["save_rtc_bytes"].as_u64()).and_then(|(a,b)|a.checked_add(b)).filter(|v|*v<=MAX_SAVE_BYTES).ok_or_else(||unavailable("The native core returned an invalid save memory bound"))?;
        let states=prepared.result["save_state_bytes"].as_u64().filter(|v|*v<=MAX_SAVE_BYTES).ok_or_else(||unavailable("The native core returned an invalid state memory bound"))?;
        let files=prepared.result["save_files_supported"].as_bool().unwrap_or(false);
        if files!=self.package.filesystem_save_directory.is_some(){return Err(unavailable("The native persistent file capability differs from its declaration"));}
        if prepared.result["save_memory_files"].as_bool().unwrap_or(false)!=self.package.filesystem_save_memory||files&&!self.package.filesystem_save_memory&&memory>0{return Err(unavailable("The native persistent save memory/file policy differs from its declaration"));}
        if self.package.filesystem_save_memory&&states==0{return Err(unavailable("Combined native saves require a rollback snapshot"));}
        if memory==0&&!files||!portable_available{self.package.manifest.capabilities.remove(&Capability::SaveRam);}
        if states==0{self.package.manifest.capabilities.remove(&Capability::SaveState);}
        let discs:DiscStatus=serde_json::from_value(prepared.result["disc"].clone()).map_err(|_|unavailable("The native disc interface is invalid"))?;discs.validate()?;
        if game.media.as_ref().is_some_and(|media|media.format==ContentFormat::M3u&&(!discs.supported||discs.count as usize!=media.disc_entries.len())){return Err(unavailable("The native disc list differs from the selected playlist"));}
        if !discs.supported{self.package.manifest.capabilities.remove(&Capability::DiscControl);}
        let window=self.call("status",json!({}),Vec::new(),5)?;
        if window.result["close_requested"].as_bool()==Some(true){return Err(PlatformError::new(ErrorCode::Cancelled,"The native game window was closed during preparation"));}
        self.prepared=true;Ok(())
    }
    fn start(&mut self)->Result<()>{if let Some(presentation)=&self.presentation{presentation.activate()?;}self.call("start",json!({}),Vec::new(),5).map(|_|())}
    fn pause(&mut self)->Result<()>{self.call("pause",json!({}),Vec::new(),5).map(|_|())}
    fn resume(&mut self)->Result<()>{if let Some(presentation)=&self.presentation{presentation.activate()?;}self.call("resume",json!({}),Vec::new(),5).map(|_|())}
    fn stop(&mut self)->Result<()>{let result=if self.process.as_ref().is_some_and(|p|!p.has_exited()){self.call("stop",json!({}),Vec::new(),3).map(|_|())}else{Ok(())};self.cleanup();result}
    fn save(&mut self,kind:SaveKind)->Result<Vec<u8>>{self.call("save",json!({"kind":kind}),Vec::new(),30).map(|r|r.payload)}
    fn load(&mut self,kind:SaveKind,payload:&[u8])->Result<()>{if payload.is_empty()||payload.len()as u64>MAX_SAVE_BYTES{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Native save size is invalid"));}self.call("load",json!({"kind":kind}),payload.to_vec(),30).map(|_|())}
    fn compatibility(&self,game:&GameManifest,_config:&SessionConfig,kind:SaveKind)->Result<SaveCompatibility>{
        let core=self.package.files.iter().find(|f|f.path==self.package.core_library).ok_or_else(||unavailable("The native core artifact is undeclared"))?;
        let assets:std::collections::BTreeMap<_,_>=self.package.files.iter().filter(|f|f.path.contains("/assets/")).map(|f|(&f.path,&f.sha256)).collect();
        let mut effective_config=json!({"runtime":self.package.manifest.id,"core_artifact":core.sha256,"options":self.package.core_options,"firmware":self.firmware_hashes,"system_assets":assets,"filesystem_save_directory":self.package.filesystem_save_directory,"reset_after_ram_load":self.package.reset_after_ram_load,"use_scratch_system_directory":self.package.use_scratch_system_directory,"video_backend":self.package.video_backend,"gpu_target":[self.package.gpu_max_width,self.package.gpu_max_height],"lifecycle_barriers":[self.package.prepare_completion,self.package.pause_barrier]});
        if self.package.file_io_protocol!=0||self.package.filesystem_save_memory||self.package.core_save_directory.is_some(){effective_config["persistent_file_policy"]=json!({"io":self.package.file_io_protocol,"memory":self.package.filesystem_save_memory,"core_directory":self.package.core_save_directory});}
        if self.package.pause_barrier_protocol!=0{effective_config["pause_barrier_protocol"]=json!(self.package.pause_barrier_protocol);}
        if self.package.writable_files_protocol!=0{effective_config["writable_files_protocol"]=json!(self.package.writable_files_protocol);}
        // Only opaque state identity changes for the newly readonly legacy
        // route. Do not invent a state conversion or invalidate in-game bytes.
        if kind==SaveKind::EmulatorState&&self.package.writable_files_protocol==0&&!self.package.legacy_os_writes(){effective_config["os_scratch_policy"]=json!(1);}
        let config_hash=digest(&serde_json::to_vec(&effective_config).map_err(storage_error)?);
        Ok(SaveCompatibility{game_content_hash:game.content_hash.clone(),core_id:self.package.manifest.id.clone(),core_version:self.package.manifest.version.clone(),state_format_version:if kind==SaveKind::InGame{self.package.in_game_format_version.clone()}else{self.package.manifest.state_format_version.clone().ok_or_else(||unavailable("Native state format is undeclared"))?},platform:self.package.manifest.platform.clone(),core_config_hash:config_hash,portable_in_game_format:if kind==SaveKind::InGame{self.package.portable_in_game_format.clone()}else{None}})
    }
    fn metrics(&mut self,session_id:Uuid)->Result<Option<FrameMetrics>> {
        #[derive(Deserialize)]struct Metric {window_duration_ms:u32,frames_presented:u32,dropped_frames:u32,audio_underruns:u32,frame_time_p50_ms:f64,frame_time_p95_ms:f64,frame_time_p99_ms:f64}
        let reply=self.call("metrics",json!({}),Vec::new(),2)?;
        let measured:Metric=serde_json::from_value(reply.result).map_err(|_|PlatformError::new(ErrorCode::CorruptData,"Native metrics are invalid"))?;
        Ok(Some(FrameMetrics{session_id,sample_at_ms:omniplay_core::now_ms(),window_duration_ms:measured.window_duration_ms,frames_presented:measured.frames_presented,dropped_frames:measured.dropped_frames,audio_underruns:measured.audio_underruns,frame_time_p50_ms:measured.frame_time_p50_ms,frame_time_p95_ms:measured.frame_time_p95_ms,frame_time_p99_ms:measured.frame_time_p99_ms,cpu_busy_percent:None,gpu_busy_percent:None,thermal_throttled:None,battery_percent:None,rtt_ms:None,packet_loss_percent:None,decode_queue_ms:None}))
    }
    fn measurement_start(&mut self,id:Uuid)->Result<NativeMeasurementStatus>{
        if !self.package.manifest.capabilities.contains(&Capability::PerformanceMeasurement){return Err(PlatformError::new(ErrorCode::UnsupportedCapability,"This native worker has no measurement collector"));}
        let reply=self.call("measurement_start",json!({"id":id}),Vec::new(),5)?;
        let status=self.parse_measurement_status(reply,id)?;self.measurement_id=Some(id);Ok(status)
    }
    fn measurement_status(&mut self)->Result<NativeMeasurementStatus>{let id=self.measurement_id.ok_or_else(||PlatformError::new(ErrorCode::InvalidState,"No native measurement was started"))?;let reply=self.call("measurement_status",json!({}),Vec::new(),2)?;self.parse_measurement_status(reply,id)}
    fn measurement_cancel(&mut self)->Result<NativeMeasurementStatus>{let id=self.measurement_id.ok_or_else(||PlatformError::new(ErrorCode::InvalidState,"No native measurement was started"))?;let reply=self.call("measurement_cancel",json!({}),Vec::new(),2)?;self.parse_measurement_status(reply,id)}
    fn measurement_read(&mut self,game:&GameManifest,config:&SessionConfig)->Result<PerformanceRunInput>{
        #[derive(Deserialize)]#[serde(deny_unknown_fields)]struct Header {id:Uuid,nominal_refresh_hz:f64,observed_duration_ms:u32,warmup_ms:u32,frames_presented:u32,frames_emulated:u32,dropped_frames:u32,audio_underruns:u32,interval_count:u32,interval_encoding:String}
        let status=self.measurement_status()?;if status.state!=NativeMeasurementState::Complete{return Err(PlatformError::new(ErrorCode::InvalidState,"The native measurement is incomplete"));}
        let reply=self.call("measurement_read",json!({}),Vec::new(),5)?;
        let header:Header=serde_json::from_value(reply.result).map_err(|_|unavailable("Native measurement header is malformed"))?;
        if header.id!=status.id||header.observed_duration_ms!=status.elapsed_ms||header.warmup_ms!=status.warmup_ms||header.interval_encoding!="float64_le"||!(100..=36000).contains(&header.interval_count)||header.frames_emulated!=header.interval_count||reply.payload.len()!=header.interval_count as usize*8{return Err(unavailable("Native measurement samples are incomplete or exceed their bound"));}
        let intervals:Vec<f64>=reply.payload.chunks_exact(8).map(|bytes|{let mut raw=[0u8;8];raw.copy_from_slice(bytes);f64::from_le_bytes(raw)}).collect();
        if intervals.iter().any(|sample|!sample.is_finite()||*sample<=0.0||*sample>5000.0){return Err(unavailable("Native measurement intervals are invalid"));}
        let compatibility=self.compatibility(game,config,SaveKind::InGame)?;
        let config_hash=digest(&serde_json::to_vec(&json!({"native_core_config":compatibility.core_config_hash,"session":config,"measurement_protocol":1})).map_err(storage_error)?);
        let artifacts:std::collections::BTreeMap<_,_>=self.package.files.iter().map(|file|(&file.path,(&file.sha256,file.bytes))).collect();
        let build_hash=digest(&serde_json::to_vec(&artifacts).map_err(storage_error)?);
        Ok(PerformanceRunInput{id:status.id,game_id:game.id,game_content_hash:game.content_hash.clone(),runtime_id:self.package.manifest.id.clone(),runtime_version:self.package.manifest.version.clone(),source_revision:self.package.manifest.source_revision.clone(),runtime_build_hash:build_hash,core_config_hash:config_hash,preset:config.preset,nominal_refresh_hz:header.nominal_refresh_hz,observed_duration_ms:header.observed_duration_ms,warmup_ms:header.warmup_ms,frames_presented:header.frames_presented,frames_emulated:header.frames_emulated,dropped_frames:header.dropped_frames,audio_underruns:header.audio_underruns,frame_intervals_ms:intervals})
    }
    fn observe(&mut self)->Result<RuntimeObservation>{
        if !self.prepared{return Ok(RuntimeObservation::default());}
        let value=self.call("status",json!({}),Vec::new(),2)?.result;
        let state=match value["state"].as_str(){Some("ready")=>SessionState::Ready,Some("running")=>SessionState::Running,Some("paused")=>SessionState::Paused,_=>return Err(PlatformError::new(ErrorCode::RuntimeCrashed,"The native runtime reported an invalid lifecycle"))};
        Ok(RuntimeObservation{state:Some(state),close_requested:value["close_requested"].as_bool().ok_or_else(||unavailable("Native window state is invalid"))?,active_playtime_ms:Some(value["active_playtime_ms"].as_u64().ok_or_else(||unavailable("Native playtime is invalid"))?)})
    }
    fn cancel_close(&mut self)->Result<()>{self.call("cancel_close",json!({}),Vec::new(),5).map(|_|())}
    fn disc_status(&mut self)->Result<DiscStatus>{let reply=self.call("disc_status",json!({}),Vec::new(),5)?;serde_json::from_value(reply.result).map_err(|_|unavailable("The native disc status is invalid"))}
    fn select_disc(&mut self,index:u32)->Result<DiscStatus>{let reply=self.call("disc_select",json!({"index":index}),Vec::new(),30)?;serde_json::from_value(reply.result).map_err(|_|unavailable("The native disc selection result is invalid"))}
}

static NEVER_CANCELLED:AtomicBool=AtomicBool::new(false);
fn check_cancel(cancel:&AtomicBool)->Result<()>{if cancel.load(Ordering::Acquire){return Err(PlatformError::new(ErrorCode::Cancelled,"Runtime preparation was cancelled"));}Ok(())}
fn materialize(directory:&std::path::Path,name:&str,expected_hash:&ContentHash,expected_bytes:u64,mut source:File,cancel:&AtomicBool)->Result<()>{
    if !valid_media_name(name){return Err(PlatformError::new(ErrorCode::InvalidRequest,"Native media filename is invalid"));}
    let mut staged=PendingFile::create(directory)?;let mut hash=Sha256::new();let mut bytes=0u64;let mut buffer=[0u8;65536];
    loop{check_cancel(cancel)?;let read=source.read(&mut buffer).map_err(storage_error)?;if read==0{break;}bytes=bytes.checked_add(read as u64).ok_or_else(||unavailable("Native content length overflow"))?;if bytes>expected_bytes{return Err(PlatformError::new(ErrorCode::ContentMismatch,"Game content length changed"));}hash.update(&buffer[..read]);staged.file.write_all(&buffer[..read]).map_err(storage_error)?;}
    if bytes!=expected_bytes||format!("{:x}",hash.finalize())!=expected_hash.0{return Err(PlatformError::new(ErrorCode::ContentMismatch,"Game asset failed its integrity check"));}
    staged.commit_new(&directory.join(name))
}
