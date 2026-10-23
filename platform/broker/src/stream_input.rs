use omniplay_contracts::*;
use omniplay_core::{Database,installed::InstalledGames};
use omniplay_stream_input::{InputWorker,protocol::Observation};
use omniplay_streaming::{SignedDocument,PairGrant,inspect,verify_grant,key_id};
use serde::Deserialize;
use std::{fs::File,path::PathBuf};
use uuid::Uuid;

#[derive(Deserialize)]#[serde(deny_unknown_fields)]
struct Policy{schema_version:u32,enabled:bool,relay_origins:Vec<String>,allow_browser_host:bool,allow_remote_input:bool,default_profile:String}
fn policy()->Option<Policy>{
    let value:Policy=serde_json::from_str(include_str!("../../../security/streaming-channel.json")).ok()?;
    if value.schema_version!=1||value.relay_origins.len()>8||value.relay_origins.iter().any(|v|omniplay_streaming::validate_audience(v).is_err())||!["p720_30","p720_60","p1080_30","p1080_60"].contains(&value.default_profile.as_str()){return None;}Some(value)
}
pub fn available()->bool{
    if !omniplay_stream_input::supported()||!policy().is_some_and(|p|p.enabled&&p.allow_browser_host&&p.allow_remote_input&&!p.relay_origins.is_empty()){return false;}
    #[cfg(windows)]{approved_helper().is_ok()}
    #[cfg(not(windows))]{false}
}
struct Active{status:StreamInputStatus,worker:Option<InputWorker>,_artifact:File,#[cfg(windows)]_process:std::os::windows::io::OwnedHandle}
struct Capture{id:Uuid,game:Uuid,pair:Uuid,session:Uuid,expires:u64,#[cfg(windows)]_process:std::os::windows::io::OwnedHandle,target:omniplay_stream_input::protocol::Target}
#[derive(Default)]pub struct StreamInputs{active:Option<Active>,capture:Option<Capture>}
pub fn capture_available()->bool{cfg!(windows)&&policy().is_some_and(|p|p.enabled&&p.allow_browser_host&&!p.relay_origins.is_empty())}
fn unavailable()->PlatformError{PlatformError::new(ErrorCode::UnsupportedCapability,"Owned native capture or input is unavailable for this target or installed product policy")}
impl StreamInputs{
    pub fn prepare_capture(&mut self,database:&Database,installed:&mut InstalledGames,game_id:Uuid,pair_id:Uuid,session_id:Uuid,host_public_key:String,document:StreamGrantDocument)->Result<serde_json::Value>{
        if !capture_available(){return Err(unavailable());}
        if self.active.as_ref().is_some_and(|a|a.status.state!=StreamInputState::Revoked){return Err(PlatformError::new(ErrorCode::Busy,"Revoke native input before preparing another capture"));}
        if self.capture.as_ref().is_some_and(|c|c.expires>omniplay_core::now_ms()){return Err(PlatformError::new(ErrorCode::Busy,"Close the current owned capture before selecting another game"));}
        let policy=policy().ok_or_else(unavailable)?;let document=SignedDocument{payload:document.payload,signature:document.signature};let body:PairGrant=inspect(&document,8192)?;
        if !policy.relay_origins.contains(&body.audience)||body.host_public_key!=host_public_key||body.id!=pair_id{return Err(PlatformError::new(ErrorCode::PermissionDenied,"This pairing does not authorize this host capture context"));}
        let grant=verify_grant(&document,&body.audience,omniplay_core::now_ms())?;
        #[cfg(not(windows))]{let _=(database,installed,game_id,session_id,grant);Err(unavailable())}
        #[cfg(windows)]{
            use std::os::windows::io::AsHandle;
            let process=installed.stream_process(database,game_id)?;let target=omniplay_stream_input::target_identity(process.as_handle())?;
            if target.process_id>i32::MAX as u32{return Err(unavailable());}
            let id=Uuid::new_v4();let expires=grant.expires_at_ms.min(omniplay_core::now_ms()+30*60*1000);
            let result=serde_json::json!({"capture_id":id,"game_id":game_id,"pair_id":pair_id,"session_id":session_id,"expires_at_ms":expires,"process_id":target.process_id,"creation_time":target.creation_time.to_string()});
            database.record_audit("stream_capture.prepared",Some(game_id))?;self.capture=Some(Capture{id,game:game_id,pair:pair_id,session:session_id,expires,_process:process,target});Ok(result)
        }
    }
    pub fn release_capture(&mut self,id:Uuid,session:Uuid)->Result<serde_json::Value>{
        if let Some(capture)=&self.capture{if capture.id!=id||capture.session!=session{return Err(PlatformError::new(ErrorCode::PermissionDenied,"Capture belongs to another session"));}self.capture=None;if let Some(active)=self.active.as_mut(){stop_active(active);}}
        Ok(serde_json::json!({"released":true,"capture_id":id}))
    }
    pub fn arm(&mut self,database:&Database,installed:&mut InstalledGames,game_id:Uuid,pair_id:Uuid,session_id:Uuid,host_public_key:String,document:StreamGrantDocument,capture_id:Uuid,capture_window:String,deadline_ms:u64)->Result<StreamInputStatus>{
        if !available(){return Err(unavailable());}
        if self.active.as_ref().is_some_and(|v|v.status.state!=StreamInputState::Revoked){return Err(PlatformError::new(ErrorCode::Busy,"Revoke the current native input grant before authorizing another game"));}
        let policy=policy().ok_or_else(unavailable)?;let document=SignedDocument{payload:document.payload,signature:document.signature};let body:PairGrant=inspect(&document,8192)?;
        if !policy.relay_origins.contains(&body.audience)||body.host_public_key!=host_public_key||body.id!=pair_id||!body.input_allowed{return Err(PlatformError::new(ErrorCode::PermissionDenied,"The signed pairing does not authorize input from this host context"));}
        let grant=verify_grant(&document,&body.audience,omniplay_core::now_ms())?;
        let game=omniplay_core::library::Library::new(database).get(game_id)?;if game.system!=GameSystem::Pc{return Err(unavailable());}
        let capture=self.capture.as_ref().ok_or_else(unavailable)?;
        if capture.id!=capture_id||capture.game!=game_id||capture.pair!=pair_id||capture.session!=session_id||capture.expires<=omniplay_core::now_ms(){return Err(unavailable());}
        let window=capture_window.parse::<u64>().ok().filter(|id|*id>0&&id.to_string()==capture_window).ok_or_else(unavailable)?;
        let captured_target=capture.target.clone();
        let capture_expires=capture.expires;
        #[cfg(not(windows))]{let _=(installed,session_id,grant,game);Err(unavailable())}
        #[cfg(windows)]{
            use std::os::windows::io::AsHandle;
            let process=installed.stream_process(database,game_id)?;let mut target=omniplay_stream_input::target_identity(process.as_handle())?;
            if target.process_id!=captured_target.process_id||target.creation_time!=captured_target.creation_time{return Err(unavailable());}target.window=Some(window);
            let title:String=game.title.chars().filter(|c|!c.is_control()).take(100).collect();let client=key_id(&grant.client_public_key)?;let vi=database.settings()?.locale=="vi";
            let description=if vi{format!("Cho thiết bị đã ghép nối điều khiển bàn phím và chuột của game:\n{title}\n\nBản nhận diện thiết bị nhận:\n{client}\n\nChỉ áp dụng cho tiến trình game do OmniPlay đã mở. Đưa game ra phía trước trong 30 giây. Đổi cửa sổ, dùng chuột/bàn phím tại host hoặc nhấn F12/Ctrl/Alt/Windows sẽ thu hồi quyền. Hiệu lực tối đa 30 phút. Tay cầm ảo chưa khả dụng.")}else{format!("Allow the paired device to control keyboard and mouse for:\n{title}\n\nViewer fingerprint:\n{client}\n\nApplies only to the game process launched by OmniPlay. Bring it to the foreground within 30 seconds. Switching windows, using the host keyboard/mouse or pressing F12/Ctrl/Alt/Windows revokes this grant. Maximum lifetime: 30 minutes. Virtual gamepads are unavailable.")};
            let answer=rfd::MessageDialog::new().set_title("OmniPlay — Remote input").set_description(&description).set_buttons(rfd::MessageButtons::YesNo).set_level(rfd::MessageLevel::Warning).show();
            if answer!=rfd::MessageDialogResult::Yes{return Err(PlatformError::new(ErrorCode::Cancelled,"Native remote input was not authorized"));}
            // A native consent dialog can outlast the certificate or game process.
            let now=omniplay_core::now_ms();if now>=deadline_ms||now>=capture_expires{return Err(PlatformError::new(ErrorCode::DeadlineExceeded,"Native input consent exceeded the capture or page lifetime"));}grant.validate(&grant.audience,now)?;let actual=omniplay_stream_input::target_identity(process.as_handle())?;if actual.process_id!=target.process_id||actual.creation_time!=target.creation_time{return Err(unavailable());}
            let remaining=(grant.expires_at_ms-now).min(capture_expires-now).min(30*60*1000)as u32;let(path,artifact)=approved_helper()?;let grant_id=Uuid::new_v4();
            let worker=InputWorker::spawn(&path,target,grant_id,session_id,now+u64::from(remaining))?;
            if omniplay_core::now_ms()>=deadline_ms{return Err(PlatformError::new(ErrorCode::DeadlineExceeded,"Native input setup exceeded the requesting page's deadline"));}
            let status=StreamInputStatus{grant_id,game_id,pair_id,session_id,state:StreamInputState::WaitingForTarget,reason:None,keyboard_mouse:true,gamepad:false,expires_at_ms:now+u64::from(remaining)};
            self.active=Some(Active{status:status.clone(),worker:Some(worker),_artifact:artifact,_process:process});if let Err(error)=database.record_audit("stream_input.authorized",Some(game_id)){self.close();return Err(error);}Ok(status)
        }
    }
    fn active(&mut self,id:Uuid,session:Uuid)->Result<&mut Active>{let active=self.active.as_mut().ok_or_else(||PlatformError::new(ErrorCode::NotFound,"Native input grant is absent"))?;if active.status.grant_id!=id||active.status.session_id!=session{return Err(PlatformError::new(ErrorCode::PermissionDenied,"Native input capability belongs to another session"));}Ok(active)}
    pub fn frame(&mut self,id:Uuid,session:Uuid,sequence:u64,frame:RemoteInputFrame)->Result<StreamInputStatus>{let active=self.active(id,session)?;frame.validate()?;if matches!(active.status.state,StreamInputState::Revoked|StreamInputState::CleanupRequired){return Ok(active.status.clone());}let result=active.worker.as_mut().ok_or_else(unavailable)?.input(sequence,frame);observe(active,result)}
    pub fn release(&mut self,id:Uuid,session:Uuid)->Result<StreamInputStatus>{let active=self.active(id,session)?;if matches!(active.status.state,StreamInputState::Revoked|StreamInputState::CleanupRequired){return Ok(active.status.clone());}let result=active.worker.as_mut().ok_or_else(unavailable)?.release();observe(active,result)}
    pub fn status(&mut self,id:Uuid,session:Uuid)->Result<StreamInputStatus>{let active=self.active(id,session)?;if matches!(active.status.state,StreamInputState::Revoked|StreamInputState::CleanupRequired){return Ok(active.status.clone());}let result=active.worker.as_mut().ok_or_else(unavailable)?.status();observe(active,result)}
    pub fn revoke(&mut self,id:Uuid,session:Uuid)->Result<StreamInputStatus>{let active=self.active(id,session)?;stop_active(active);Ok(active.status.clone())}
    pub fn close(&mut self){self.active.take();self.capture.take();}
}
impl Drop for StreamInputs{fn drop(&mut self){self.close();}}
fn stop_active(active:&mut Active){
    if matches!(active.status.state,StreamInputState::Revoked|StreamInputState::CleanupRequired){return;}
    let observed=active.worker.take().ok_or_else(unavailable).and_then(|mut worker|worker.stop_confirmed());
    match observed{
        Ok(value)=>{active.status.state=StreamInputState::Revoked;active.status.reason=value.reason;},
        Err(_)=>{active.status.state=StreamInputState::CleanupRequired;active.status.reason=Some("helper_disconnected".into());},
    }
}
fn observe(active:&mut Active,result:Result<Observation>)->Result<StreamInputStatus>{
    match result{
        Ok(value)=>{active.status.state=match value.state.as_str(){"waiting_for_target"=>StreamInputState::WaitingForTarget,"active"=>StreamInputState::Active,"idle"=>StreamInputState::Idle,_=>if value.reason.as_deref()==Some("release_failed"){StreamInputState::CleanupRequired}else{StreamInputState::Revoked}};active.status.reason=value.reason;if matches!(active.status.state,StreamInputState::Revoked|StreamInputState::CleanupRequired){active.worker.take();}Ok(active.status.clone())},
        Err(error)=>{active.worker.take();active.status.state=StreamInputState::CleanupRequired;active.status.reason=Some("helper_disconnected".into());Err(error)},
    }
}
#[cfg(windows)]fn approved_helper()->Result<(PathBuf,File)>{
    use std::os::windows::fs::OpenOptionsExt;
    use omniplay_updater::{channel::Channel,store::Store,package::verify_entry};
    let store=Store::open(&omniplay_updater::install_root()?,Channel::embedded()?)?;let active=store.active()?;let root=store.version_directory(active.id);
    let broker=verify_entry(&root,&active.manifest,&active.manifest.broker_entry)?;
    if std::env::current_exe().and_then(|p|p.canonicalize()).map_err(omniplay_core::util::storage_error)?!=broker{return Err(unavailable());}
    let relative="helpers/omniplay-input-worker.exe";let path=root.join(relative);let guard=std::fs::OpenOptions::new().read(true).share_mode(1).open(&path).map_err(omniplay_core::util::storage_error)?;
    let verified=verify_entry(&root,&active.manifest,relative)?;if path.canonicalize().map_err(omniplay_core::util::storage_error)?!=verified{return Err(unavailable());}Ok((verified,guard))
}
