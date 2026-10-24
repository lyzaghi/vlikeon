#![forbid(unsafe_code)]
#![cfg_attr(all(windows,not(debug_assertions)),windows_subsystem="windows")]
mod framing;
mod service;
mod transfers;
mod jobs;
mod update_jobs;
mod import_jobs;
mod preparation_jobs;
mod native_work_jobs;
mod stream_input;
mod stream_gamepad;
mod stream_gamepad_recovery;
#[cfg(windows)]mod stream_video;
#[cfg(windows)]mod stream_video_authority;
#[cfg(windows)]mod stream_video_installation;
#[cfg(windows)]mod stream_video_receive;
#[cfg(windows)]mod stream_video_codec;
#[cfg(windows)]mod stream_video_session;

use omniplay_contracts::{ErrorCode, PlatformError, Request, Response};
use omniplay_core::Database;
use std::{io, path::PathBuf,sync::{Arc,Mutex,mpsc::{self,SyncSender}}};
use zeroize::{Zeroize,Zeroizing};

// The distribution tool writes the component-extension identity to this file.
// No arbitrary website origin or renderer-provided filesystem path is accepted.
const ALLOWED_ORIGIN: &str = include_str!("../../../security/game_hub_origin.txt");
type Output=Arc<Mutex<io::Stdout>>;
type RangeWork=(uuid::Uuid,omniplay_local_service::asset_ranges::ReadOperation);
fn publish(output:&Output,encoded:&[u8])->omniplay_contracts::Result<()>{let mut writer=output.lock().map_err(|_|PlatformError::new(ErrorCode::Internal,"Native output lane is unavailable"))?;framing::write_message(&mut *writer,encoded)}
fn range_lane(output:Output)->omniplay_contracts::Result<SyncSender<RangeWork>>{
    let(sender,receiver)=mpsc::sync_channel::<RangeWork>(1);
    std::thread::Builder::new().name("omniplay-media-read".into()).spawn(move||while let Ok((id,operation))=receiver.recv(){
        let result=operation.run().and_then(|value|serde_json::to_value(value).map_err(|_|PlatformError::new(ErrorCode::Internal,"Native media response encoding failed")));let response=Response::result(id,result);drop(operation);
        let Ok(encoded)=serde_json::to_vec(&response)else{break;};let encoded=Zeroizing::new(encoded);if encoded.len()>omniplay_contracts::MAX_CONTROL_MESSAGE_BYTES||publish(&output,&encoded).is_err(){break;}
    }).map_err(|_|PlatformError::new(ErrorCode::ResourceLimit,"The native media I/O lane could not start"))?;Ok(sender)
}

fn data_directory(profile_id: uuid::Uuid) -> omniplay_contracts::Result<PathBuf> {
    let base = if cfg!(target_os="windows") { std::env::var_os("LOCALAPPDATA").map(PathBuf::from) }
        else if cfg!(target_os="macos") { std::env::var_os("HOME").map(|p|PathBuf::from(p).join("Library/Application Support")) }
        else { std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(||std::env::var_os("HOME").map(|p|PathBuf::from(p).join(".local/share"))) };
    base.map(|p|p.join("OmniPlay").join("profiles").join(profile_id.to_string())).ok_or_else(||PlatformError::new(ErrorCode::StorageFailure,"Application data directory is unavailable"))
}

fn native_registry(profile_root:&std::path::Path,profile_id:uuid::Uuid)->omniplay_native_runtime::NativeRegistry{
    #[cfg(all(target_os="macos",target_arch="aarch64"))]
    if let Ok(registry)=omniplay_native_runtime::NativeRegistry::installed_macos(profile_root,profile_id){return registry;}
    #[cfg(all(target_os="linux",target_arch="x86_64",target_env="gnu"))]
    {
        if let Some(runtime)=std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).filter(|path|path.is_absolute()){
            let socket=runtime.join("omniplay/native-host.sock");
            // Socket activation, exact profile cgroup fd transfer and retained-work
            // recovery must all succeed before Linux native packages become visible.
            if let Ok((registry,_summary))=omniplay_native_runtime::NativeRegistry::installed_linux_service(profile_root,profile_id,&socket,512*1024*1024){return registry;}
        }
    }
    omniplay_native_runtime::NativeRegistry::installed(profile_root,profile_id)
}

fn run() -> omniplay_contracts::Result<()> {
    let origin = std::env::args().nth(1).unwrap_or_default();
    if origin != ALLOWED_ORIGIN.trim() { return Err(PlatformError::new(ErrorCode::PermissionDenied,"Native host caller is not authorized")); }
    let mut input = io::stdin().lock();
    let output:Output=Arc::new(Mutex::new(io::stdout()));
    let mut media_lane=None;
    let mut broker: Option<service::Broker> = None;
    let mut profile_id = None;
    while let Some(bytes) = framing::read_message(&mut input)? {
        let bytes=Zeroizing::new(bytes);
        let mut request = match serde_json::from_slice::<Request>(&bytes) {
            Ok(request) => request,
            Err(_) => {
                // A malformed uncorrelated frame terminates the channel instead of emitting
                // guessed request identifiers or continuing with ambiguous framing.
                if let Some(broker) = broker.as_mut() { broker.close(); }
                return Err(PlatformError::new(ErrorCode::InvalidRequest,"Native message is not a valid request"));
            }
        };
        if let Err(error)=request.validate(omniplay_core::now_ms()){
            clear_request_secrets(&mut request.command);
            let response=Response::result(request.id,Err(error));
            let encoded=Zeroizing::new(serde_json::to_vec(&response).map_err(|_|PlatformError::new(ErrorCode::Internal,"Response encoding failed"))?);
            publish(&output,&encoded)?;
            // An expired command queued behind a native file dialog must not kill an
            // unrelated running game or the entire profile's native connection.
            continue;
        }
        if profile_id.is_some_and(|profile|profile!=request.profile_id) {
            return Err(PlatformError::new(ErrorCode::PermissionDenied,"Native channel cannot switch browser profiles"));
        }
        if broker.is_none() {
            profile_id = Some(request.profile_id);
            let database = Database::open(&data_directory(request.profile_id)?)?;
            omniplay_updater::note_opened_profile_schema(omniplay_core::database::CURRENT_SCHEMA_VERSION)?;
            let runtimes=native_registry(&database.root,request.profile_id);
            broker = Some(service::Broker::new(database,Box::new(runtimes),request.profile_id)?);
            // The Game Hub successfully connected and the local profile opened. An
            // update health record is never acknowledged merely because spawn worked.
            omniplay_updater::acknowledge_broker_health()?;
        }
        let active_broker = broker.as_mut().expect("broker was initialized above");
        if matches!(&request.command,omniplay_contracts::Command::AssetRangeRead{..}){
            let id=request.id;let admission=(||{
                let operation=active_broker.prepare_range_read(&request,&bytes)?;
                if media_lane.is_none(){media_lane=Some(range_lane(output.clone())?);}
                media_lane.as_ref().expect("initialized media lane").try_send((id,operation)).map_err(|_|PlatformError::new(ErrorCode::Busy,"A native media read is still finishing"))
            })();
            if let Err(error)=admission{let encoded=serde_json::to_vec(&Response::result(id,Err(error))).map_err(|_|PlatformError::new(ErrorCode::Internal,"Native response encoding failed"))?;publish(&output,&encoded)?;}continue;
        }
        let sensitive=request.command.contains_secrets();
        let mut response: Response = active_broker.handle(request,&bytes);
        let mut encoded = Zeroizing::new(serde_json::to_vec(&response).map_err(|_|PlatformError::new(ErrorCode::Internal,"Response encoding failed"))?);
        if encoded.len()>omniplay_contracts::MAX_CONTROL_MESSAGE_BYTES{
            clear_strings_if_sensitive(&mut response,sensitive);
            response=Response::result(response.id,Err(PlatformError::new(ErrorCode::ResourceLimit,"The result exceeds the control channel's size bound; request a smaller page")));
            encoded=Zeroizing::new(serde_json::to_vec(&response).map_err(|_|PlatformError::new(ErrorCode::Internal,"Response encoding failed"))?);
        }
        let write_result=publish(&output,&encoded);
        clear_strings_if_sensitive(&mut response,sensitive);
        write_result?;
        if active_broker.shutdown_requested { break; }
    }
    if let Some(broker) = broker.as_mut() { broker.close(); }
    Ok(())
}
fn clear_strings_if_sensitive(response:&mut Response,sensitive:bool){if sensitive{if let omniplay_contracts::ResponseOutcome::Ok{result}=&mut response.outcome{clear_strings(result);}}}

fn clear_strings(value:&mut serde_json::Value) {
    match value {serde_json::Value::String(value)=>value.zeroize(),serde_json::Value::Array(values)=>for value in values{clear_strings(value);},serde_json::Value::Object(values)=>for value in values.values_mut(){clear_strings(value);},_=>{}}
}
fn clear_request_secrets(command:&mut omniplay_contracts::Command){
    use omniplay_contracts::Command;
    match command{
        Command::SyncActionStart{action}=>match action{
            omniplay_contracts::SyncAccountAction::Register{recovery_key,registration_token}=>{recovery_key.zeroize();registration_token.zeroize();},
            omniplay_contracts::SyncAccountAction::Recover{recovery_key,..}|omniplay_contracts::SyncAccountAction::Renew{recovery_key}|omniplay_contracts::SyncAccountAction::RevokeDevice{recovery_key,..}|omniplay_contracts::SyncAccountAction::DeleteAccount{recovery_key}=>recovery_key.zeroize(),_=>{}
        },
        Command::SyncRegister{recovery_key,registration_token}=>{recovery_key.zeroize();registration_token.zeroize();}
        Command::SyncRecover{recovery_key,..}|Command::SyncRenew{recovery_key}|Command::SyncRevokeDevice{recovery_key,..}|Command::SyncDeleteAccount{recovery_key}=>recovery_key.zeroize(),
        _=>{}
    }
}

fn main() {
    if let Err(error) = run() {
        // Only the stable error code is logged; paths and user content never enter stderr.
        eprintln!("OmniPlay host terminated: {:?}",error.code);
        std::process::exit(1);
    }
}
