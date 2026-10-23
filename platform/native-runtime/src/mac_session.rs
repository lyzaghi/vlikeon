//! XPC-owned macOS session with the shared bounded worker/save wire protocol.
use crate::{channel::{WorkerChannel,Reply},mac_staging::StagedInputs,registry::RuntimePackage,unavailable};
use omniplay_contracts::*;
use omniplay_sandbox::macos::{MacConnection,MacPair,MacLaunch};
use serde_json::json;
use std::{path::PathBuf,sync::{Arc,atomic::{AtomicBool,Ordering}},time::{Duration,Instant}};

pub(crate) struct MacSession{process:Option<MacPair>,channel:Option<WorkerChannel>,profile_root:PathBuf,pub package:RuntimePackage,pub inputs:StagedInputs,prepared:bool,haptics_ceiling:bool,launch_config:SessionConfig}
fn invalid()->PlatformError{unavailable("The owned macOS native session could not be composed")}
fn cancelled(cancel:&AtomicBool)->Result<()>{if cancel.load(Ordering::Acquire){Err(PlatformError::new(ErrorCode::Cancelled,"macOS session preparation was cancelled"))}else{Ok(())}}
impl MacSession{
    pub fn launch(connection:MacConnection,profile_root:PathBuf,package:RuntimePackage,mut inputs:StagedInputs,config:&SessionConfig,haptics:bool,cancel:Arc<AtomicBool>)->Result<Self>{
        config.validate()?;cancelled(&cancel)?;
        let reservation=WorkerChannel::reserve()?;let helper=package.presentation.as_ref().ok_or_else(invalid)?;
        inputs.launch_intent()?;
        let pair=connection.launch(MacLaunch{session:inputs.session(),runtime:&package.manifest.id,maximum_width:helper.maximum_width,maximum_height:helper.maximum_height,volume:(config.audio_volume*100.0).round()as u16,haptics:haptics&&package.manifest.capabilities.contains(&Capability::Haptics)})?;
        let mut result=Self{process:Some(pair.process),channel:None,profile_root,package,inputs,prepared:false,haptics_ceiling:haptics,launch_config:config.clone()};
        result.channel=Some(WorkerChannel::new_reserved(pair.input,pair.output,reservation)?);
        let hello=result.call("hello",json!({}),Vec::new(),Duration::from_secs(30),&cancel)?;let value=&hello.result;
        if value["runtime_id"].as_str()!=Some(result.package.manifest.id.as_str())||value["library_name"].as_str()!=Some(result.package.expected_library_name.as_str())||value["library_version"].as_str()!=Some(result.package.expected_library_version.as_str())||value["video_backend"].as_str()!=Some("software")||value["presentation_protocol"].as_u64()!=Some(2)||value["presentation_mode"].as_str()!=Some("brokered_rgba")||value["measurement_protocol"].as_u64()!=Some(0)||value["input_profile_protocol"].as_u64()!=Some(4){return Err(invalid());}
        if result.package.pause_barrier_protocol!=0&&value["pause_barrier_protocol"].as_u64()!=Some(u64::from(result.package.pause_barrier_protocol)){return Err(invalid());}
        if result.package.file_io_protocol==1&&(value["checked_file_io_protocol"].as_u64()!=Some(1)||value["persistent_files_protocol"].as_u64()!=Some(2)){return Err(invalid());}
        if result.package.filesystem_save_memory&&value["combined_in_game_protocol"].as_u64()!=Some(1){return Err(invalid());}
        if result.package.portable_in_game_format.as_deref()==Some("ares-md-canonical-files-v1")&&value["portable_file_protocol"].as_u64()!=Some(1){return Err(invalid());}
        if value["haptics_protocol"].as_u64()!=Some(1){result.package.manifest.capabilities.remove(&Capability::Haptics);}
        result.package.manifest.capabilities.remove(&Capability::PerformanceMeasurement);cancelled(&cancel)?;Ok(result)
    }
    pub fn call(&mut self,method:&str,params:serde_json::Value,payload:Vec<u8>,timeout:Duration,cancel:&AtomicBool)->Result<Reply>{
        let process=self.process.as_ref().ok_or_else(invalid)?;self.channel.as_mut().ok_or_else(invalid)?.call_cancellable(process,method,params,payload,timeout,cancel)
    }
    pub fn activate(&self)->Result<()>{self.process.as_ref().ok_or_else(invalid)?.activate()}
    pub fn prepare_core(&mut self,game:&GameManifest,config:&SessionConfig,mut input:GameInputProfile,cancel:&AtomicBool)->Result<()>{
        game.validate()?;config.validate()?;input.validate()?;if self.prepared||!self.inputs.matches_game(game)||serde_json::to_value(config).map_err(|_|invalid())?!=serde_json::to_value(&self.launch_config).map_err(|_|invalid())?{return Err(PlatformError::new(ErrorCode::InvalidState,"The macOS session input or configuration does not match this preparation"));}input.haptics_enabled&=self.haptics_ceiling&&self.package.manifest.capabilities.contains(&Capability::Haptics);
        let database=omniplay_core::Database::open(&self.profile_root)?;let selected=omniplay_core::core_options::load(&database,game.id,&self.package.manifest.id)?;let policy=CoreOptionPolicy{schema_version:1,definitions:self.package.user_core_options.clone()};self.package.core_options.extend(policy.resolve(&selected)?);let portable_md=self.package.portable_in_game_format.as_deref()==Some("ares-md-canonical-files-v1");
        input.raw_gamepads.clear(); // Keep Web device calibration out of native core IPC.
        let params=json!({"filename":self.inputs.filename(),"input_profile":input,"title":game.title,"bytes":self.inputs.primary_bytes(),"config":config,"core_options":self.package.core_options,"minimum_system_ram_bytes":self.package.minimum_system_ram_bytes,"reset_after_ram_load":self.package.reset_after_ram_load,"filesystem_save_directory":self.package.filesystem_save_directory,"core_save_directory":self.package.core_save_directory,"filesystem_save_memory":self.package.filesystem_save_memory,"file_io_protocol":self.package.file_io_protocol,"portable_files_profile":if portable_md{"ares_md_v1"}else{""},"has_system_assets":self.package.has_system_assets,"use_scratch_system_directory":self.package.use_scratch_system_directory,"video_backend":"software","gpu_max_width":0,"gpu_max_height":0,"analog_controls":matches!(game.system,GameSystem::N64|GameSystem::Psp),"prepare_completion":self.package.prepare_completion,"pause_barrier":self.package.pause_barrier,"pause_barrier_protocol":self.package.pause_barrier_protocol});
        let result=(||{let prepared=self.call("prepare",params,Vec::new(),Duration::from_secs(60),cancel)?;let value=&prepared.result;
            if portable_md&&value["portable_files_profile"].as_str()!=Some("ares_md_v1"){return Err(invalid());}let portable_available=if portable_md{value["portable_progress_available"].as_bool().ok_or_else(invalid)?}else{true};
            let memory=value["save_ram_bytes"].as_u64().zip(value["save_rtc_bytes"].as_u64()).and_then(|(ram,rtc)|ram.checked_add(rtc)).filter(|bytes|*bytes<=MAX_SAVE_BYTES).ok_or_else(invalid)?;let states=value["save_state_bytes"].as_u64().filter(|bytes|*bytes<=MAX_SAVE_BYTES).ok_or_else(invalid)?;
            let files=value["save_files_supported"].as_bool().ok_or_else(invalid)?;let combined=value["save_memory_files"].as_bool().ok_or_else(invalid)?;if files!=self.package.filesystem_save_directory.is_some()||combined!=self.package.filesystem_save_memory||files&&!combined&&memory>0||combined&&states==0{return Err(invalid());}
            let discs:DiscStatus=serde_json::from_value(value["disc"].clone()).map_err(|_|invalid())?;discs.validate()?;if game.media.as_ref().is_some_and(|media|media.format==ContentFormat::M3u&&(!discs.supported||discs.count as usize!=media.disc_entries.len())){return Err(invalid());}
            let status=self.call("status",json!({}),Vec::new(),Duration::from_secs(5),cancel)?;if status.result["state"].as_str()!=Some("ready"){return Err(invalid());}if status.result["close_requested"].as_bool().ok_or_else(invalid)?{return Err(PlatformError::new(ErrorCode::Cancelled,"The native game window closed during preparation"));}
            if memory==0&&!files||!portable_available{self.package.manifest.capabilities.remove(&Capability::SaveRam);}if states==0{self.package.manifest.capabilities.remove(&Capability::SaveState);}if !discs.supported{self.package.manifest.capabilities.remove(&Capability::DiscControl);}cancelled(cancel)?;self.prepared=true;Ok(())})();if result.is_err(){self.stop_owned();}result
    }
    fn ready(&self)->Result<()>{if !self.prepared{return Err(PlatformError::new(ErrorCode::InvalidState,"The macOS core is not prepared"));}Ok(())}
    pub fn start(&mut self,cancel:&AtomicBool)->Result<()>{self.ready()?;self.activate()?;self.call("start",json!({}),Vec::new(),Duration::from_secs(5),cancel).map(|_|())}
    pub fn pause(&mut self,cancel:&AtomicBool)->Result<()>{self.ready()?;self.call("pause",json!({}),Vec::new(),Duration::from_secs(5),cancel).map(|_|())}
    pub fn resume(&mut self,cancel:&AtomicBool)->Result<()>{self.ready()?;self.activate()?;self.call("resume",json!({}),Vec::new(),Duration::from_secs(5),cancel).map(|_|())}
    pub fn save(&mut self,kind:SaveKind,cancel:&AtomicBool)->Result<Vec<u8>>{self.ready()?;let payload=self.call("save",json!({"kind":kind}),Vec::new(),Duration::from_secs(30),cancel)?.payload;if payload.is_empty()||payload.len()as u64>MAX_SAVE_BYTES{return Err(invalid());}Ok(payload)}
    pub fn load(&mut self,kind:SaveKind,payload:&[u8],cancel:&AtomicBool)->Result<()>{self.ready()?;if payload.is_empty()||payload.len()as u64>MAX_SAVE_BYTES{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Native save size exceeds its policy"));}self.call("load",json!({"kind":kind}),payload.to_vec(),Duration::from_secs(30),cancel).map(|_|())}
    pub fn compatibility(&self,game:&GameManifest,kind:SaveKind)->Result<SaveCompatibility>{
        self.ready()?;if !self.inputs.matches_game(game){return Err(invalid());}let core=self.package.files.iter().find(|file|file.path==self.package.core_library).ok_or_else(invalid)?;let assets:std::collections::BTreeMap<_,_>=self.package.files.iter().filter(|file|file.path.contains("/assets/")).map(|file|(&file.path,&file.sha256)).collect();
        let mut effective=json!({"runtime":self.package.manifest.id,"core_artifact":core.sha256,"options":self.package.core_options,"firmware":self.inputs.firmware_hashes(),"system_assets":assets,"filesystem_save_directory":self.package.filesystem_save_directory,"reset_after_ram_load":self.package.reset_after_ram_load,"use_scratch_system_directory":self.package.use_scratch_system_directory,"video_backend":self.package.video_backend,"gpu_target":[self.package.gpu_max_width,self.package.gpu_max_height],"lifecycle_barriers":[self.package.prepare_completion,self.package.pause_barrier]});if self.package.file_io_protocol!=0||self.package.filesystem_save_memory||self.package.core_save_directory.is_some(){effective["persistent_file_policy"]=json!({"io":self.package.file_io_protocol,"memory":self.package.filesystem_save_memory,"core_directory":self.package.core_save_directory});}
        // Emulator states include the guest ABI and boot images. In-game
        // portable formats keep their existing cross-platform compatibility.
        if kind != SaveKind::InGame {
            let mac = self.package.macos.as_ref().ok_or_else(invalid)?;
            let kernel = self.package.files.iter().find(|f| f.path == mac.guest_kernel).ok_or_else(invalid)?;
            let initrd = self.package.files.iter().find(|f| f.path == mac.guest_initrd).ok_or_else(invalid)?;
            effective["guest"] = json!({"protocol":mac.guest_protocol,"kernel":kernel.sha256,"initrd":initrd.sha256});
        }
        if self.package.pause_barrier_protocol!=0{effective["pause_barrier_protocol"]=json!(self.package.pause_barrier_protocol);}
        Ok(SaveCompatibility{game_content_hash:game.content_hash.clone(),core_id:self.package.manifest.id.clone(),core_version:self.package.manifest.version.clone(),state_format_version:if kind==SaveKind::InGame{self.package.in_game_format_version.clone()}else{self.package.manifest.state_format_version.clone().ok_or_else(invalid)?},platform:self.package.manifest.platform.clone(),core_config_hash:omniplay_core::util::digest(&serde_json::to_vec(&effective).map_err(|_|invalid())?),portable_in_game_format:if kind==SaveKind::InGame{self.package.portable_in_game_format.clone()}else{None}})
    }
    pub fn observe(&mut self,cancel:&AtomicBool)->Result<omniplay_core::sessions::RuntimeObservation>{self.ready()?;let value=self.call("status",json!({}),Vec::new(),Duration::from_secs(2),cancel)?.result;let state=match value["state"].as_str(){Some("ready")=>SessionState::Ready,Some("running")=>SessionState::Running,Some("paused")=>SessionState::Paused,_=>return Err(invalid())};Ok(omniplay_core::sessions::RuntimeObservation{state:Some(state),close_requested:value["close_requested"].as_bool().ok_or_else(invalid)?,active_playtime_ms:Some(value["active_playtime_ms"].as_u64().ok_or_else(invalid)?)})}
    pub fn compatibility_for_config(&self,game:&GameManifest,config:&SessionConfig,kind:SaveKind)->Result<SaveCompatibility>{config.validate()?;if serde_json::to_value(config).map_err(|_|invalid())?!=serde_json::to_value(&self.launch_config).map_err(|_|invalid())?{return Err(invalid());}self.compatibility(game,kind)}
    pub fn disc_status(&mut self,index:Option<u32>,cancel:&AtomicBool)->Result<DiscStatus>{self.ready()?;if index.is_some_and(|index|index>=8){return Err(PlatformError::new(ErrorCode::InvalidRequest,"The requested disc index exceeds its policy"));}let(method,params,seconds)=match index{Some(index)=>("disc_select",json!({"index":index}),30),None=>("disc_status",json!({}),5)};let reply=self.call(method,params,Vec::new(),Duration::from_secs(seconds),cancel)?;let result:DiscStatus=serde_json::from_value(reply.result).map_err(|_|invalid())?;result.validate()?;if index.is_some_and(|index|!result.supported||result.index!=index){return Err(invalid());}Ok(result)}
    pub fn metrics(&mut self,session_id:uuid::Uuid,cancel:&AtomicBool)->Result<FrameMetrics>{
        self.ready()?;#[derive(serde::Deserialize)]#[serde(deny_unknown_fields)]struct Metric{window_duration_ms:u32,frames_presented:u32,dropped_frames:u32,audio_underruns:u32,frame_time_p50_ms:f64,frame_time_p95_ms:f64,frame_time_p99_ms:f64}
        let reply=self.call("metrics",json!({}),Vec::new(),Duration::from_secs(2),cancel)?;let value:Metric=serde_json::from_value(reply.result).map_err(|_|invalid())?;if session_id.is_nil()||value.window_duration_ms==0||value.window_duration_ms>300_000||[value.frame_time_p50_ms,value.frame_time_p95_ms,value.frame_time_p99_ms].iter().any(|value|!value.is_finite()||*value<0.0)||value.frame_time_p50_ms>value.frame_time_p95_ms||value.frame_time_p95_ms>value.frame_time_p99_ms{return Err(invalid());}
        Ok(FrameMetrics{session_id,sample_at_ms:omniplay_core::now_ms(),window_duration_ms:value.window_duration_ms,frames_presented:value.frames_presented,dropped_frames:value.dropped_frames,audio_underruns:value.audio_underruns,frame_time_p50_ms:value.frame_time_p50_ms,frame_time_p95_ms:value.frame_time_p95_ms,frame_time_p99_ms:value.frame_time_p99_ms,cpu_busy_percent:None,gpu_busy_percent:None,thermal_throttled:None,battery_percent:None,rtt_ms:None,packet_loss_percent:None,decode_queue_ms:None})
    }
    pub fn stop(&mut self,cancel:&AtomicBool)->Result<()>{let result=if self.process.as_ref().is_some_and(|process|!process.has_exited()){self.call("stop",json!({}),Vec::new(),Duration::from_secs(3),cancel).map(|_|())}else{Ok(())};self.stop_owned();result}
    pub fn has_exited(&self)->bool{self.process.as_ref().is_none_or(|process|process.has_exited())}

    pub fn stop_owned(&mut self){
        if let Some(process)=self.process.take(){
            process.terminate();let until=Instant::now()+Duration::from_secs(3);
            let ended=loop{match process.confirmed_ended(){Ok(true)=>break true,Err(_)=>break false,_=>{if Instant::now()>=until{break false;}std::thread::sleep(Duration::from_millis(20));}}};
            if !ended{if let Some(channel)=&mut self.channel{channel.abandon();}}
            if ended{let _=self.inputs.retire_confirmed();}
        }
        self.channel.take();self.prepared=false;
    }
}
impl Drop for MacSession{fn drop(&mut self){self.stop_owned();}}
