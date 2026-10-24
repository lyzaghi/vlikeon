#![deny(unsafe_op_in_unsafe_fn)]
//! C ABI only for the app's trusted Swift/JNI host. Filesystem authorization is a
//! different native entry point from the renderer's versioned control protocol.
use omniplay_contracts::*;
use omniplay_local_service::{LocalHost,SyncLifecycle};
use serde::Deserialize;
use serde_json::{Value,json};
use std::{collections::HashMap,path::PathBuf,sync::{Arc,Mutex,OnceLock,atomic::{AtomicU64,Ordering}}};
use zeroize::Zeroize;
use uuid::Uuid;
mod credentials;
pub use credentials::OmniCredentialCallbacks;
pub mod pairing_credentials;
pub use pairing_credentials::{OmniPairingAddress,OmniPairingCredentialCallbacks};
mod media_provider;
pub use media_provider::{OmniMediaCallbacks,OmniMediaStream};

struct HostEntry{profile_id:Uuid,device_id:Uuid,host:Arc<Mutex<LocalHost>>,lifecycle:Arc<SyncLifecycle>,ranges:Arc<omniplay_local_service::asset_ranges::AssetRanges>,background:Option<omniplay_local_service::SyncJobHandle>,pairing:Option<Arc<pairing_credentials::NativePairingCredentialStore>>,foreground:bool,closing:bool}
type Hosts=HashMap<u64,HostEntry>;
static HOSTS:OnceLock<Mutex<Hosts>>=OnceLock::new();
static NEXT:AtomicU64=AtomicU64::new(1);
fn hosts()->&'static Mutex<Hosts>{HOSTS.get_or_init(||Mutex::new(HashMap::new()))}
fn error(code:ErrorCode,message:&str)->PlatformError{PlatformError::new(code,message)}
fn host(handle:u64)->Result<Arc<Mutex<LocalHost>>>{hosts().lock().map_err(|_|error(ErrorCode::Internal,"Mobile registry is unavailable"))?.get(&handle).filter(|entry|!entry.closing).map(|entry|entry.host.clone()).ok_or_else(||error(ErrorCode::NotFound,"Mobile profile is not open"))}
fn collect_closed(){
    let retired=if let Ok(mut registry)=hosts().lock(){let ids=registry.iter().filter(|(_,entry)|entry.closing&&!entry.lifecycle.busy()&&!entry.ranges.busy()).map(|(id,_)|*id).collect::<Vec<_>>();ids.into_iter().filter_map(|id|registry.remove(&id)).collect::<Vec<_>>()}else{Vec::new()};
    // Native callback contexts are released outside the global registry lock.
    drop(retired);
}

#[repr(C)]
pub struct OmniBuffer {pub data:*mut u8,pub length:usize}
fn buffer(mut raw:Vec<u8>)->OmniBuffer {
    if raw.len()>MAX_CONTROL_MESSAGE_BYTES {raw.zeroize();raw=serde_json::to_vec(&json!({"status":"error","error":error(ErrorCode::ResourceLimit,"Response exceeds the mobile control bound")})).unwrap_or_default();}
    let mut raw=raw.into_boxed_slice();let value=OmniBuffer{data:raw.as_mut_ptr(),length:raw.len()};std::mem::forget(raw);value
}
fn result(value:Result<Value>)->OmniBuffer{let value=match value{Ok(result)=>json!({"status":"ok","result":result}),Err(error)=>json!({"status":"error","error":error})};buffer(serde_json::to_vec(&value).unwrap_or_default())}
fn clear_strings(value:&mut Value){match value{Value::String(text)=>text.zeroize(),Value::Array(values)=>for value in values{clear_strings(value);},Value::Object(values)=>for value in values.values_mut(){clear_strings(value);},_=>{}}}
unsafe fn input<'a>(data:*const u8,length:usize)->Result<&'a [u8]>{if data.is_null()||length==0||length>MAX_CONTROL_MESSAGE_BYTES{return Err(error(ErrorCode::InvalidRequest,"Native input is empty or exceeds its bound"));}Ok(unsafe{std::slice::from_raw_parts(data,length)})}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]struct Open {data_root:PathBuf,profile_id:Uuid}

/// # Safety
/// `data` must reference `length` readable bytes for the duration of this call.
#[no_mangle]
pub unsafe extern "C" fn om_mobile_open(data:*const u8,length:usize)->OmniBuffer{
    result((||open_profile(unsafe{input(data,length)?},None))())
}
/// # Safety
/// Follows om_mobile_open's byte contract and NativeCredentialStore::take's
/// native callback/thread/lifetime contract. The context is always consumed.
#[no_mangle]
pub unsafe extern "C" fn om_mobile_open_with_credentials_v2(data:*const u8,length:usize,callbacks:OmniCredentialCallbacks)->OmniBuffer{
    let provider=unsafe{credentials::NativeCredentialStore::take(callbacks)};
    result((||{let provider=provider?;open_profile(unsafe{input(data,length)?},Some(provider))})())
}
fn open_profile(raw:&[u8],credentials:Option<Arc<credentials::NativeCredentialStore>>)->Result<Value>{
        let request:Open=serde_json::from_slice(raw).map_err(|_|error(ErrorCode::InvalidRequest,"Mobile profile arguments are invalid"))?;
        if !request.data_root.is_absolute(){return Err(error(ErrorCode::PermissionDenied,"Native application storage must be absolute"));}
        if let Some(provider)=&credentials{provider.bind(request.profile_id)?;}
        collect_closed();
        let mut registry=hosts().lock().map_err(|_|error(ErrorCode::Internal,"Mobile registry is unavailable"))?;
        if registry.len()>=4||registry.values().any(|entry|entry.profile_id==request.profile_id){return Err(error(ErrorCode::Busy,"This profile is already open, finishing synchronization, or the mobile profile limit was reached"));}
        let value=match credentials{Some(provider)=>LocalHost::open_with_credentials(&request.data_root,request.profile_id,provider)?,None=>LocalHost::open(&request.data_root,request.profile_id)?};
        let handle=NEXT.fetch_add(1,Ordering::Relaxed);if handle==0{return Err(error(ErrorCode::ResourceLimit,"Mobile handle limit reached"));}
        let device_id=value.native_device_id();let lifecycle=value.sync_lifecycle();let ranges=value.asset_ranges();registry.insert(handle,HostEntry{profile_id:request.profile_id,device_id,host:Arc::new(Mutex::new(value)),lifecycle,ranges,background:None,pairing:None,foreground:false,closing:false});Ok(json!({"handle":handle,"profile_id":request.profile_id}))
}

/// Native scope for the OS provider factory. No renderer method or secret read.
#[no_mangle]
pub extern "C" fn om_mobile_pairing_scope(handle:u64)->OmniBuffer{result((||{
    let registry=hosts().lock().map_err(|_|error(ErrorCode::Internal,"Mobile registry is unavailable"))?;
    let entry=registry.get(&handle).filter(|entry|!entry.closing).ok_or_else(||error(ErrorCode::NotFound,"Mobile profile is closing"))?;
    Ok(json!({"profile_id":entry.profile_id,"device_id":entry.device_id}))
})())}

/// # Safety
/// Trusted native callback contract. Context is consumed on EVERY return,
/// including stale/invalid host, scope or duplicate attachment. No callback or
/// secret-record operation is performed by attachment itself.
#[no_mangle]
pub unsafe extern "C" fn om_mobile_attach_pairing_credentials_v1(handle:u64,callbacks:OmniPairingCredentialCallbacks)->OmniBuffer{
    let scope=(||{
        let registry=hosts().lock().map_err(|_|error(ErrorCode::Internal,"Mobile registry is unavailable"))?;
        let entry=registry.get(&handle).filter(|entry|!entry.closing).ok_or_else(||error(ErrorCode::NotFound,"Mobile profile is closing"))?;
        Ok::<_,PlatformError>((entry.profile_id,entry.device_id))
    })();
    let(profile,device)=scope.as_ref().copied().unwrap_or((Uuid::nil(),Uuid::nil()));
    // SAFETY: caller transfers the valid native table/context even on rejection.
    let provider=unsafe{pairing_credentials::NativePairingCredentialStore::take(callbacks,profile,device)};
    result((||{
        scope?;let provider=provider?;
        let mut registry=hosts().lock().map_err(|_|error(ErrorCode::Internal,"Mobile registry is unavailable"))?;
        let entry=registry.get_mut(&handle).filter(|entry|!entry.closing&&entry.profile_id==profile&&entry.device_id==device).ok_or_else(||error(ErrorCode::NotFound,"Mobile profile is closing"))?;
        if entry.pairing.is_some(){return Err(error(ErrorCode::Conflict,"Native pairing provider is already attached"));}
        provider.set_available(entry.foreground);entry.pairing=Some(provider);
        Ok(json!({"attached":true,"credential_abi":1}))
    })())
}

/// # Safety
/// The input follows `om_mobile_open`'s borrowed byte contract.
#[no_mangle]
pub unsafe extern "C" fn om_mobile_request(handle:u64,data:*const u8,length:usize)->OmniBuffer{
    let mut id=Uuid::nil();let mut sensitive=false;
    let response=(||{
        let raw=unsafe{input(data,length)?};
        if let Ok(mut value)=serde_json::from_slice::<Value>(raw){id=value.get("id").and_then(Value::as_str).and_then(|v|Uuid::parse_str(v).ok()).unwrap_or_default();clear_strings(&mut value);}
        let request:Request=serde_json::from_slice(raw).map_err(|_|error(ErrorCode::InvalidRequest,"Mobile request is invalid"))?;
        sensitive=request.command.contains_secrets();
        let host=host(handle)?;
        if matches!(&request.command,Command::AssetRangeRead{..}){
            let operation={let host=host.lock().map_err(|_|error(ErrorCode::Internal,"Mobile profile is unavailable"))?;host.prepare_range_read(&request,raw)?};
            let value=operation.run().and_then(|value|serde_json::to_value(value).map_err(|_|error(ErrorCode::Internal,"Native media reply could not be encoded")));return Ok(Response::result(request.id,value));
        }
        let mut host=host.lock().map_err(|_|error(ErrorCode::Internal,"Mobile profile is unavailable"))?;Ok(host.handle(request,raw))
    })();
    let mut response=response.unwrap_or_else(|error|Response::result(id,Err(error)));let mut bytes=serde_json::to_vec(&response).unwrap_or_default();if sensitive{if let ResponseOutcome::Ok{result}=&mut response.outcome{clear_strings(result);}}if bytes.len()>MAX_CONTROL_MESSAGE_BYTES{bytes.zeroize();bytes=serde_json::to_vec(&Response::result(id,Err(error(ErrorCode::ResourceLimit,"Request a smaller response page")))).unwrap_or_default();}buffer(bytes)
}

#[derive(Deserialize)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
enum Authorization {Import{path:PathBuf},Export{path:PathBuf},Folder{path:PathBuf},Firmware{path:PathBuf,system:GameSystem,role:String}}

/// # Safety
/// Input is created only by trusted native picker delegates, never forwarded from a
/// renderer. The paths must have been selected or copied with OS-authorized access.
#[no_mangle]
pub unsafe extern "C" fn om_mobile_authorize(handle:u64,data:*const u8,length:usize)->OmniBuffer{
    result((||{let authorization:Authorization=serde_json::from_slice(unsafe{input(data,length)?}).map_err(|_|error(ErrorCode::InvalidRequest,"Native picker authorization is invalid"))?;let host=host(handle)?;let mut host=host.lock().map_err(|_|error(ErrorCode::Internal,"Mobile profile is unavailable"))?;match authorization{Authorization::Import{path}=>host.selected_import(&path),Authorization::Export{path}=>host.selected_export(&path),Authorization::Folder{path}=>host.selected_folder(&path),Authorization::Firmware{path,system,role}=>host.import_selected_firmware(&path,system,&role)}})())
}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]struct MediaSelection{entries:Vec<String>,suggested_title:String}
/// # Safety
/// The metadata and callback context come only from a trusted native folder
/// picker. Follows input() and NativeDiscProvider::take's lifetime/thread rules.
/// The provider context is consumed even when metadata or the host is rejected.
#[no_mangle]
pub unsafe extern "C" fn om_mobile_authorize_media(handle:u64,data:*const u8,length:usize,callbacks:OmniMediaCallbacks)->OmniBuffer{
    let provider=unsafe{media_provider::NativeDiscProvider::take(callbacks)};
    result((||{let provider=provider?;let raw=unsafe{input(data,length)?};if raw.iter().copied().find(|byte|!byte.is_ascii_whitespace())!=Some(b'{'){return Err(error(ErrorCode::InvalidRequest,"Native folder metadata must be an object"));}let selected:MediaSelection=serde_json::from_slice(raw).map_err(|_|error(ErrorCode::InvalidRequest,"Native folder metadata is invalid"))?;let host=host(handle)?;let mut host=host.lock().map_err(|_|error(ErrorCode::Internal,"Mobile profile is unavailable"))?;provider.bind(host.profile_id)?;host.selected_disc_provider(Arc::new(media_provider::NativeDiscSource(provider)),selected.entries,selected.suggested_title)})())
}
#[no_mangle]
pub extern "C" fn om_mobile_set_foreground(handle:u64,foreground:u8){
    if foreground>1{return;}
    let lifecycle=if let Ok(mut registry)=hosts().lock(){registry.get_mut(&handle).map(|entry|{
        entry.foreground=foreground==1&&!entry.closing;
        if let Some(provider)=&entry.pairing{provider.set_available(entry.foreground);}
        (entry.lifecycle.clone(),entry.foreground)
    })}else{None};
    if let Some((lifecycle,foreground))=lifecycle{lifecycle.set_foreground(foreground);}
}
/// Trusted native scheduler policy, never a renderer-supplied root or account.
#[no_mangle]
pub extern "C" fn om_mobile_background_policy(handle:u64)->OmniBuffer{result((||{let host=host(handle)?;let value=host.lock().map_err(|_|error(ErrorCode::Internal,"Mobile profile is unavailable"))?.background_sync_policy();value})())}
#[no_mangle]
pub extern "C" fn om_mobile_background_start(handle:u64,budget_ms:u32)->OmniBuffer{result((||{
    let host=host(handle)?;let (snapshot,observer)=host.lock().map_err(|_|error(ErrorCode::Internal,"Mobile profile is unavailable"))?.start_background_sync(budget_ms)?;
    let mut registry=hosts().lock().map_err(|_|{let _=observer.cancel(snapshot.id);error(ErrorCode::Internal,"Mobile registry is unavailable")})?;
    let Some(entry)=registry.get_mut(&handle).filter(|entry|!entry.closing) else{let _=observer.cancel(snapshot.id);return Err(error(ErrorCode::NotFound,"Mobile profile is closing"));};
    entry.background=Some(observer);serde_json::to_value(snapshot).map_err(|_|error(ErrorCode::Internal,"Background sync status could not be encoded"))
})())}
/// # Safety
/// `data` borrows exactly 36 canonical ASCII UUID bytes, not a control request.
/// Poll/cancel bypass LocalHost's request mutex. A stale ID cannot cancel a newer
/// foreground/background run. Cooperative expiry does not free live workers.
#[no_mangle]
pub unsafe extern "C" fn om_mobile_background_status(handle:u64,data:*const u8,length:usize,cancel:u8)->OmniBuffer{result((||{
    if length!=36||cancel>1{return Err(error(ErrorCode::InvalidRequest,"Background sync ticket is invalid"));}
    let raw=unsafe{input(data,length)?};let text=std::str::from_utf8(raw).map_err(|_|error(ErrorCode::InvalidRequest,"Background sync ticket is invalid"))?;
    let id=Uuid::parse_str(text).map_err(|_|error(ErrorCode::InvalidRequest,"Background sync ticket is invalid"))?;
    if id.to_string()!=text{return Err(error(ErrorCode::InvalidRequest,"Background sync ticket is invalid"));}
    let observer=hosts().lock().map_err(|_|error(ErrorCode::Internal,"Mobile registry is unavailable"))?.get(&handle).and_then(|entry|entry.background.clone()).ok_or_else(||error(ErrorCode::NotFound,"Background sync ticket is no longer available"))?;
    let snapshot=if cancel==1{observer.cancel(id)?}else{observer.status(id)?};serde_json::to_value(snapshot).map_err(|_|error(ErrorCode::Internal,"Background sync status could not be encoded"))
})())}
#[no_mangle]
pub extern "C" fn om_mobile_expire_capabilities(handle:u64)->OmniBuffer{result((||{let host=host(handle)?;let mut host=host.lock().map_err(|_|error(ErrorCode::Internal,"Mobile profile is unavailable"))?;host.expire_capabilities();Ok(json!({"expired":true}))})())}
#[no_mangle]
pub extern "C" fn om_mobile_suspend(handle:u64)->OmniBuffer{result((||{let host=host(handle)?;let mut host=host.lock().map_err(|_|error(ErrorCode::Internal,"Mobile profile is unavailable"))?;host.suspend()?;Ok(json!({"paused":true}))})())}
#[no_mangle]
pub extern "C" fn om_mobile_detach(handle:u64)->OmniBuffer{result((||{let host=host(handle)?;let mut host=host.lock().map_err(|_|error(ErrorCode::Internal,"Mobile profile is unavailable"))?;host.detach();Ok(json!({"detached":true}))})())}
#[no_mangle]
pub extern "C" fn om_mobile_close(handle:u64){
    let value=hosts().lock().ok().and_then(|mut registry|registry.get_mut(&handle).map(|entry|{entry.closing=true;entry.foreground=false;if let Some(provider)=&entry.pairing{provider.close();}(entry.host.clone(),entry.lifecycle.clone())}));
    if let Some((host,lifecycle))=value{lifecycle.set_foreground(false);lifecycle.cancel();if let Ok(mut value)=host.lock(){value.close();};}collect_closed();
}

/// # Safety
/// Free exactly one previously returned, unchanged OmniBuffer. No other pointer or
/// length is valid. This is a native allocation contract, not an exposed JS method.
#[no_mangle]
pub unsafe extern "C" fn om_mobile_free(value:OmniBuffer){if !value.data.is_null(){let mut bytes=unsafe{Box::from_raw(std::ptr::slice_from_raw_parts_mut(value.data,value.length))};bytes.zeroize();}}
