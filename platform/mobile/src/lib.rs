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
        let handle=NEXT.fetch_update(Ordering::Relaxed,Ordering::Relaxed,|next|next.checked_add(1)).map_err(|_|error(ErrorCode::ResourceLimit,"Mobile handle limit reached"))?;
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

fn pairing_operation(handle:u64)->Result<(Arc<Mutex<LocalHost>>,Arc<pairing_credentials::NativePairingCredentialStore>,u64)>{
    let registry=hosts().lock().map_err(|_|error(ErrorCode::Internal,"Mobile registry is unavailable"))?;
    let entry=registry.get(&handle).filter(|entry|!entry.closing&&entry.foreground).ok_or_else(||error(ErrorCode::PermissionDenied,"Native pairing requires a live foreground host"))?;
    let provider=entry.pairing.clone().ok_or_else(||error(ErrorCode::UnsupportedCapability,"Native pairing provider is not attached"))?;
    let epoch=provider.operation_epoch()?;Ok((entry.host.clone(),provider,epoch))
}
/// Trusted native OS lifecycle/cancel event, independent of LocalHost's request mutex.
/// Acknowledges epoch invalidation (or an already retired handle), never stop of
/// a backend or rollback of committed work. Does not enable foreground itself.
#[no_mangle]
pub extern "C" fn om_mobile_pairing_invalidate_epoch(handle:u64)->u8{
    if handle==0{return 0;}
    let Ok(registry)=hosts().lock()else{return 0;};
    if handle>=NEXT.load(Ordering::Relaxed){return 0;}
    if let Some(entry)=registry.get(&handle){
        if let Some(provider)=&entry.pairing{
            if entry.closing{provider.close();}else{provider.set_available(entry.foreground);}
        }
    }
    // Not-yet-attached providers have no in-flight pairing calls; removed
    // entries were terminally closed before registry removal. Handles are unique.
    1
}
/// Native metadata only; intentionally absent from the renderer Command enum.
#[no_mangle]
pub extern "C" fn om_mobile_pairing_status(handle:u64)->OmniBuffer{result((||{
    let(host,provider,epoch)=pairing_operation(handle)?;
    let value=host.lock().map_err(|_|error(ErrorCode::Internal,"Mobile profile is unavailable"))?.native_pairing_status()?;
    provider.require_operation_epoch(epoch)?;Ok(value)
})())}
/// Explicit native preparation only. No secret/signing result or activation is
/// returned; installed disabled policy refuses before RNG/vault side effects.
#[no_mangle]
pub extern "C" fn om_mobile_pairing_prepare_identity_v1(handle:u64,expected_revision:u64,budget_ms:u32)->OmniBuffer{result((||{
    if !(1000..=25000).contains(&budget_ms){return Err(error(ErrorCode::InvalidRequest,"Native pairing operation budget is invalid"));}
    let deadline=std::time::Instant::now()+std::time::Duration::from_millis(u64::from(budget_ms));
    let(host,provider,epoch)=pairing_operation(handle)?;
    let host=host.lock().map_err(|_|error(ErrorCode::Internal,"Mobile profile is unavailable"))?;
    provider.require_operation_epoch(epoch)?;
    let candidate=host.native_pairing_prepare_identity(provider.as_ref(),expected_revision,deadline)?;
    provider.confirm_access(&candidate.address(),epoch)?;
    if std::time::Instant::now()>=deadline{return Err(error(ErrorCode::DeadlineExceeded,"Native identity preparation completed after its deadline; reload journal state"));}
    Ok(json!({"credential_protocol":1,"address":candidate.address(),"public_key":candidate.public_key(),"revision":candidate.revision(),"phase":"material_ready"}))
})())}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]struct PairingMetadataRecovery{address:omniplay_streaming::native_credentials::CredentialAddress,expected_revision:u64}
/// # Safety
/// Borrowed readable JSON at most 1024 bytes. Explicit trusted native recovery
/// action only: it may permanently fence a pending identity and all its pairs.
/// No caller-supplied retirement flags or secret bytes are accepted.
#[no_mangle]
pub unsafe extern "C" fn om_mobile_pairing_advance_metadata_v1(handle:u64,data:*const u8,length:usize,budget_ms:u32)->OmniBuffer{result((||{
    if !(1000..=25000).contains(&budget_ms)||length>1024{return Err(error(ErrorCode::InvalidRequest,"Native recovery arguments exceed their bounds"));}
    let deadline=std::time::Instant::now()+std::time::Duration::from_millis(u64::from(budget_ms));
    // SAFETY: caller supplies the bounded synchronous input borrow above.
    let request:PairingMetadataRecovery=serde_json::from_slice(unsafe{input(data,length)?}).map_err(|_|error(ErrorCode::InvalidRequest,"Native recovery arguments are invalid"))?;
    let(host,provider,epoch)=pairing_operation(handle)?;
    let mut host=host.lock().map_err(|_|error(ErrorCode::Internal,"Mobile profile is unavailable"))?;
    provider.require_operation_epoch(epoch)?;
    let value=host.native_pairing_advance_metadata(request.address,request.expected_revision,deadline)?;
    provider.require_operation_epoch(epoch)?;
    if std::time::Instant::now()>=deadline{return Err(error(ErrorCode::DeadlineExceeded,"Native recovery completed after its deadline; reload journal state"));}
    Ok(value)
})())}

/// # Safety
/// Same borrowed recovery request contract as advance_metadata_v1. Native user
/// approval selects exact-generation key removal, only after all retirement
/// receipts already exist. No flags/secret bytes or replacement key are accepted.
#[no_mangle]
pub unsafe extern "C" fn om_mobile_pairing_remove_material_v1(handle:u64,data:*const u8,length:usize,budget_ms:u32)->OmniBuffer{result((||{
    if !(1000..=25000).contains(&budget_ms)||length>1024{return Err(error(ErrorCode::InvalidRequest,"Native vault cleanup arguments exceed their bounds"));}
    let deadline=std::time::Instant::now()+std::time::Duration::from_millis(u64::from(budget_ms));
    // SAFETY: caller supplies the bounded synchronous input borrow above.
    let request:PairingMetadataRecovery=serde_json::from_slice(unsafe{input(data,length)?}).map_err(|_|error(ErrorCode::InvalidRequest,"Native vault cleanup arguments are invalid"))?;
    let(host,provider,epoch)=pairing_operation(handle)?;
    let mut host=host.lock().map_err(|_|error(ErrorCode::Internal,"Mobile profile is unavailable"))?;
    provider.confirm_access(&request.address,epoch)?;
    let value=host.native_pairing_remove_material(request.address,request.expected_revision,provider.as_ref(),deadline)?;
    provider.confirm_access(&request.address,epoch)?;
    if std::time::Instant::now()>=deadline{return Err(error(ErrorCode::DeadlineExceeded,"Native vault cleanup completed after its deadline; reload journal state"));}
    Ok(value)
})())}

/// # Safety
/// Same borrowed recovery request contract; native approval of authenticated
/// relay revocation only. No URL, identity key or receipt flags accepted.
#[no_mangle]
pub unsafe extern "C" fn om_mobile_pairing_revoke_relay_v1(handle:u64,data:*const u8,length:usize,budget_ms:u32)->OmniBuffer{result((||{
    if !(1000..=25000).contains(&budget_ms)||length>1024{return Err(error(ErrorCode::InvalidRequest,"Native relay cleanup arguments exceed their bounds"));}
    let deadline=std::time::Instant::now()+std::time::Duration::from_millis(u64::from(budget_ms));
    // SAFETY: caller supplies the bounded synchronous input borrow above.
    let request:PairingMetadataRecovery=serde_json::from_slice(unsafe{input(data,length)?}).map_err(|_|error(ErrorCode::InvalidRequest,"Native relay cleanup arguments are invalid"))?;
    let(host,provider,epoch)=pairing_operation(handle)?;
    let mut host=host.lock().map_err(|_|error(ErrorCode::Internal,"Mobile profile is unavailable"))?;
    provider.confirm_access(&request.address,epoch)?;
    let cancel=provider.operation_cancellation(epoch)?;
    let value=host.native_pairing_revoke_relay(request.address,request.expected_revision,provider.as_ref(),deadline,&cancel)?;
    provider.confirm_access(&request.address,epoch)?;
    if cancel.load(Ordering::Acquire)||std::time::Instant::now()>=deadline{return Err(error(ErrorCode::Cancelled,"Native relay cleanup became stale; reload journal state"));}
    Ok(value)
})())}

/// Metadata for a trusted native confirmation dialog, never renderer dispatch.
#[no_mangle]
pub extern "C" fn om_mobile_pairing_review_identity_v1(handle:u64,expected_revision:u64)->OmniBuffer{result((||{
    let(host,provider,epoch)=pairing_operation(handle)?;
    let mut host=host.lock().map_err(|_|error(ErrorCode::Internal,"Mobile profile is unavailable"))?;
    provider.require_operation_epoch(epoch)?;
    let review=host.native_pairing_review_identity(expected_revision,epoch)?;
    if let Err(error)=provider.require_operation_epoch(epoch){host.native_pairing_cancel_identity_review();return Err(error);}
    Ok(review)
})())}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]struct PairingIdentityConfirmation{review_id:Uuid}
/// # Safety
/// Input is a borrowed readable JSON object (at most 128 bytes). Trusted native
/// UI calls only after explicit user approval of the corresponding review.
/// A token does not attest to consent; no renderer may call this entry point.
#[no_mangle]
pub unsafe extern "C" fn om_mobile_pairing_confirm_identity_v1(handle:u64,data:*const u8,length:usize,budget_ms:u32)->OmniBuffer{result((||{
    if !(1000..=25000).contains(&budget_ms)||length>128{return Err(error(ErrorCode::InvalidRequest,"Native identity confirmation arguments exceed their bounds"));}
    let deadline=std::time::Instant::now()+std::time::Duration::from_millis(u64::from(budget_ms));
    // SAFETY: caller supplies the bounded synchronous input borrow above.
    let confirmation:PairingIdentityConfirmation=serde_json::from_slice(unsafe{input(data,length)?}).map_err(|_|error(ErrorCode::InvalidRequest,"Native identity confirmation is invalid"))?;
    let(host,provider,epoch)=pairing_operation(handle)?;
    let mut host=host.lock().map_err(|_|error(ErrorCode::Internal,"Mobile profile is unavailable"))?;
    if let Err(error)=provider.require_operation_epoch(epoch){host.native_pairing_cancel_identity_review();return Err(error);}
    let value=host.native_pairing_confirm_identity(provider.as_ref(),confirmation.review_id,epoch,deadline)?;
    provider.require_operation_epoch(epoch)?;
    if std::time::Instant::now()>=deadline{return Err(error(ErrorCode::DeadlineExceeded,"Native identity activation completed after its deadline; reload journal state"));}
    Ok(value)
})())}
/// Native dialog cancellation; no vault call or durable identity transition.
#[no_mangle]
pub extern "C" fn om_mobile_pairing_cancel_identity_review(handle:u64)->OmniBuffer{result((||{
    host(handle)?.lock().map_err(|_|error(ErrorCode::Internal,"Mobile profile is unavailable"))?.native_pairing_cancel_identity_review();
    Ok(json!({"cancelled":true}))
})())}

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
