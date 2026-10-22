#![forbid(unsafe_code)]
mod store;
mod transfers;
mod cors;

use axum::{body::Body,extract::{DefaultBodyLimit,State},http::{HeaderMap,HeaderValue,StatusCode,header},response::{IntoResponse,Response},routing::{get,post},Json,Router};
use base64::{Engine,engine::general_purpose::URL_SAFE_NO_PAD};
use futures_util::StreamExt;
use omniplay_sync::{AuthenticatedRequest,BlobHeader,DeviceCertificate,RootOperation,RootRequest,SyncOperation,verify_device_request,verify_root_request,validate_audience};
use serde::Serialize;
use sha2::{Digest,Sha256};
use std::{net::{IpAddr,SocketAddr},path::{Path,PathBuf},sync::Arc};
use subtle::ConstantTimeEq;
use tokio::{io::AsyncWriteExt,sync::Mutex};
use tokio_util::io::ReaderStream;
use uuid::Uuid;

pub(crate) fn now_ms()->u64 {std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|value|value.as_millis().min(u64::MAX as u128) as u64).unwrap_or(0)}
pub(crate) type ApiResult<T>=std::result::Result<T,ApiError>;

#[derive(Serialize)]
pub(crate) struct ApiError {#[serde(skip)]status:StatusCode,code:&'static str,message:&'static str}
impl ApiError {
    fn unauthorized()->Self {Self{status:StatusCode::UNAUTHORIZED,code:"unauthorized",message:"The account authorization is invalid, expired or revoked"}}
    fn invalid()->Self {Self{status:StatusCode::BAD_REQUEST,code:"invalid_request",message:"The synchronization request is invalid"}}
    fn quota()->Self {Self{status:StatusCode::TOO_MANY_REQUESTS,code:"resource_limit",message:"The synchronization service limit was reached"}}
    fn not_found()->Self {Self{status:StatusCode::NOT_FOUND,code:"not_found",message:"The requested encrypted object was not found"}}
    fn conflict()->Self {Self{status:StatusCode::CONFLICT,code:"conflict",message:"This immutable object already exists with different content, or an upload is in progress"}}
    fn storage(_:impl std::fmt::Display)->Self {Self{status:StatusCode::INTERNAL_SERVER_ERROR,code:"storage_failure",message:"The synchronization storage operation failed"}}
}
impl IntoResponse for ApiError {fn into_response(self)->Response {(self.status,Json(self)).into_response()}}

struct Service {store:Mutex<store::Store>,audience:String,registration_token:String,root:PathBuf,transfer_permits:Arc<tokio::sync::Semaphore>,_root_lease:std::fs::File}
type Shared=Arc<Service>;

#[tokio::main]
async fn main(){
    if let Err(error)=run().await {eprintln!("OmniPlay synchronization service stopped: {}",error.code);std::process::exit(1);}
}

async fn run()->ApiResult<()> {
    let audience=std::env::var("OMNIPLAY_SYNC_AUDIENCE").map_err(|_|ApiError::invalid())?;
    validate_audience(&audience).map_err(|_|ApiError::invalid())?;
    let registration_token=std::env::var("OMNIPLAY_SYNC_REGISTRATION_TOKEN").map_err(|_|ApiError::invalid())?;
    if registration_token.len()<32||registration_token.len()>256||!registration_token.bytes().all(|byte|(33..=126).contains(&byte)){return Err(ApiError::invalid());}
    let root=PathBuf::from(std::env::var_os("OMNIPLAY_SYNC_DATA").ok_or_else(ApiError::invalid)?);
    if !root.is_absolute(){return Err(ApiError::invalid());}
    std::fs::create_dir_all(&root).map_err(ApiError::storage)?;
    let root=root.canonicalize().map_err(ApiError::storage)?;
    let _service_lease=omniplay_sync::transfer_files::lease(&root,"sync-service-v1.lock").map_err(ApiError::storage)?;
    #[cfg(unix)]
    {use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(&root,std::fs::Permissions::from_mode(0o700)).map_err(ApiError::storage)?;}
    let address:SocketAddr=std::env::var("OMNIPLAY_SYNC_LISTEN").unwrap_or_else(|_|"127.0.0.1:8792".into()).parse().map_err(|_|ApiError::invalid())?;
    // The process is a private HTTP upstream. A TLS ingress must terminate the
    // exact HTTPS audience; binding publicly without that boundary is rejected.
    if !matches!(address.ip(),IpAddr::V4(ip) if ip.is_loopback())&&!matches!(address.ip(),IpAddr::V6(ip) if ip.is_loopback()){return Err(ApiError::invalid());}
    let store=store::Store::open(&root,10*1024*1024*1024,32,1024*1024*1024)?;
    let cors=Arc::new(cors::Policy::from_environment()?);
    let shared=Arc::new(Service{store:Mutex::new(store),audience,registration_token,root,transfer_permits:Arc::new(tokio::sync::Semaphore::new(8)),_root_lease:_service_lease});
    let app=Router::new()
        .route("/health",get(||async{Json(serde_json::json!({"service":"omniplay-sync","protocol":1}))}))
        .route("/v1/account",post(account).layer(DefaultBodyLimit::max(32*1024)))
        .route("/v1/sync",post(synchronize).layer(DefaultBodyLimit::disable()))
        .layer(tower::limit::ConcurrencyLimitLayer::new(8))
        .layer(axum::middleware::from_fn_with_state(cors,cors::handle))
        .with_state(shared.clone());
    let listener=tokio::net::TcpListener::bind(address).await.map_err(ApiError::storage)?;
    let maintenance=transfers::maintenance(&shared);
    let result=axum::serve(listener,app).with_graceful_shutdown(async{let _=tokio::signal::ctrl_c().await;}).await.map_err(ApiError::storage);
    maintenance.abort();result
}

async fn account(State(service):State<Shared>,headers:HeaderMap,Json(request):Json<RootRequest>)->ApiResult<Json<serde_json::Value>> {
    let command=verify_root_request(&request,&service.audience,now_ms()).map_err(|_|ApiError::unauthorized())?;
    match command.operation {
        RootOperation::Create{root_public_key}=>{
            let token=headers.get("x-omniplay-registration").and_then(|value|value.to_str().ok()).unwrap_or("");
            if token.len()!=service.registration_token.len()||!bool::from(token.as_bytes().ct_eq(service.registration_token.as_bytes())) {return Err(ApiError::unauthorized());}
            service.store.lock().await.create_account(&command.account_id,&root_public_key,command.nonce)?;
            Ok(Json(serde_json::json!({"created":true,"account_id":command.account_id})))
        }
        RootOperation::RevokeDevice{device_id}=>{
            let mut store=service.store.lock().await;
            if store.root_key(&command.account_id)?!=request.root_public_key{return Err(ApiError::unauthorized());}
            store.revoke(&command.account_id,device_id,command.nonce)?;
            store.abort_account_uploads(&command.account_id)?;
            Ok(Json(serde_json::json!({"revoked":true})))
        }
        RootOperation::DeleteAccount=>{
            // Verify the root before choosing a server-generated, account-scoped
            // directory. No caller-supplied path participates in deletion.
            let mut store=service.store.lock().await;
            if store.retired(&command.account_id)? {return Ok(Json(serde_json::json!({"deleted":true})));}
            if store.root_key(&command.account_id)?!=request.root_public_key{return Err(ApiError::unauthorized());}
            store.begin_delete(&command.account_id,command.nonce)?;
            store.abort_account_uploads(&command.account_id)?;
            let directory=account_directory(&service.root,&command.account_id)?;
            if directory.exists(){
                let metadata=std::fs::symlink_metadata(&directory).map_err(ApiError::storage)?;
                if metadata.file_type().is_symlink()||!directory.starts_with(service.root.join("objects")){return Err(ApiError::storage("Unexpected account storage boundary"));}
                std::fs::remove_dir_all(&directory).map_err(ApiError::storage)?;
            }
            store.finish_delete(&command.account_id)?;
            Ok(Json(serde_json::json!({"deleted":true})))
        }
    }
}

fn request_from_headers(headers:&HeaderMap)->ApiResult<AuthenticatedRequest> {
    let encoded=headers.get("x-omniplay-authorization").and_then(|value|value.to_str().ok()).ok_or_else(ApiError::unauthorized)?;
    if encoded.len()>32768{return Err(ApiError::invalid());}
    let bytes=URL_SAFE_NO_PAD.decode(encoded).map_err(|_|ApiError::invalid())?;
    serde_json::from_slice(&bytes).map_err(|_|ApiError::invalid())
}

async fn authenticate(service:&Shared,request:&AuthenticatedRequest)->ApiResult<(DeviceCertificate,omniplay_sync::SyncRequest)> {
    if request.certificate.payload_base64.len()>12000{return Err(ApiError::invalid());}
    // This first parse only locates the account public key. It grants no rights.
    let untrusted=URL_SAFE_NO_PAD.decode(&request.certificate.payload_base64).map_err(|_|ApiError::unauthorized())?;
    let untrusted:DeviceCertificate=serde_json::from_slice(&untrusted).map_err(|_|ApiError::unauthorized())?;
    if !omniplay_sync::valid_account_id(&untrusted.account_id){return Err(ApiError::unauthorized());}
    let mut store=service.store.lock().await;
    let root=store.root_key(&untrusted.account_id)?;
    let (certificate,body)=verify_device_request(&root,request,&service.audience,now_ms()).map_err(|_|ApiError::unauthorized())?;
    store.authorize(&certificate,body.nonce)?;
    Ok((certificate,body))
}

async fn synchronize(State(service):State<Shared>,headers:HeaderMap,body:Body)->ApiResult<Response> {
    let auth=request_from_headers(&headers)?;
    let (certificate,request)=authenticate(&service,&auth).await?;
    match request.operation {
        SyncOperation::Put{header}=>{
            let declared=headers.get(header::CONTENT_LENGTH).and_then(|value|value.to_str().ok()).and_then(|value|value.parse::<u64>().ok()).ok_or_else(ApiError::invalid)?;
            if declared!=header.ciphertext_bytes{return Err(ApiError::invalid());}
            let reservation=service.store.lock().await.reserve(&header)?;
            let id=match reservation {store::Reservation::Existing=>return Ok(Json(serde_json::json!({"stored":true,"existing":true})).into_response()),store::Reservation::New(id)=>id};
            let pending=service.root.join("pending").join(id.to_string());
            let receive=tokio::time::timeout(std::time::Duration::from_secs(300),receive_blob(body,&pending,&header)).await;
            if !matches!(receive,Ok(Ok(()))) {
                let _=tokio::fs::remove_file(&pending).await;
                service.store.lock().await.cancel_reservation(id);
                return Err(ApiError::invalid());
            }
            let commit=publish(&service,&certificate,id,&header,&pending).await;
            let _=tokio::fs::remove_file(&pending).await;
            if commit.is_err(){service.store.lock().await.cancel_reservation(id);}
            commit?;
            Ok(Json(serde_json::json!({"stored":true,"existing":false})).into_response())
        }
        SyncOperation::Get{object_id}=>{
            let header=service.store.lock().await.object_header(&request.account_id,object_id)?;
            let path=account_directory(&service.root,&request.account_id)?.join(format!("{}.blob",header.ciphertext_sha256));
            let file=tokio::fs::File::open(path).await.map_err(|_|ApiError::not_found())?;
            if file.metadata().await.map_err(ApiError::storage)?.len()!=header.ciphertext_bytes{return Err(ApiError::storage("Encrypted object length mismatch"));}
            let mut response=Response::new(Body::from_stream(ReaderStream::new(file)));
            response.headers_mut().insert(header::CONTENT_TYPE,HeaderValue::from_static("application/octet-stream"));
            response.headers_mut().insert(header::CACHE_CONTROL,HeaderValue::from_static("no-store"));
            response.headers_mut().insert(header::CONTENT_LENGTH,HeaderValue::from_str(&header.ciphertext_bytes.to_string()).map_err(ApiError::storage)?);
            let metadata=URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).map_err(ApiError::storage)?);
            response.headers_mut().insert("x-omniplay-object",HeaderValue::from_str(&metadata).map_err(ApiError::storage)?);
            Ok(response)
        }
        SyncOperation::List{after_sequence,limit}=>Ok(Json(service.store.lock().await.list(&request.account_id,after_sequence,limit)?).into_response()),
        SyncOperation::Devices=>Ok(Json(service.store.lock().await.devices(&request.account_id)?).into_response()),
        operation=>transfers::handle(&service,&certificate,operation,headers,body).await,
    }
}

async fn receive_blob(body:Body,path:&Path,header:&BlobHeader)->ApiResult<()> {
    let mut file=tokio::fs::OpenOptions::new().write(true).create_new(true).open(path).await.map_err(ApiError::storage)?;
    let mut stream=body.into_data_stream();let mut length=0u64;let mut hasher=Sha256::new();
    while let Some(chunk)=stream.next().await {
        let bytes=chunk.map_err(|_|ApiError::invalid())?;
        length=length.checked_add(bytes.len() as u64).filter(|value|*value<=header.ciphertext_bytes).ok_or_else(ApiError::invalid)?;
        hasher.update(&bytes);file.write_all(&bytes).await.map_err(ApiError::storage)?;
    }
    if length!=header.ciphertext_bytes||format!("{:x}",hasher.finalize())!=header.ciphertext_sha256{return Err(ApiError::invalid());}
    file.sync_all().await.map_err(ApiError::storage)
}

async fn publish(service:&Shared,certificate:&DeviceCertificate,id:Uuid,header:&BlobHeader,pending:&Path)->ApiResult<()> {
    let mut store=service.store.lock().await;
    store.check_device_active(certificate)?;
    let directory=account_directory(&service.root,&header.binding.account_id)?;
    std::fs::create_dir_all(&directory).map_err(ApiError::storage)?;
    let target=directory.join(format!("{}.blob",header.ciphertext_sha256));
    if !target.exists(){std::fs::hard_link(pending,&target).map_err(ApiError::storage)?;}
    else {
        // Recovery after a crash between file publication and SQLite commit.
        let mut existing=std::fs::File::open(&target).map_err(ApiError::storage)?;
        let mut hasher=Sha256::new();let mut buffer=[0u8;65536];let mut length=0u64;
        loop{let count=std::io::Read::read(&mut existing,&mut buffer).map_err(ApiError::storage)?;if count==0{break;}length+=count as u64;if length>header.ciphertext_bytes{return Err(ApiError::storage("Existing blob length mismatch"));}hasher.update(&buffer[..count]);}
        if length!=header.ciphertext_bytes||format!("{:x}",hasher.finalize())!=header.ciphertext_sha256{return Err(ApiError::storage("Existing blob digest mismatch"));}
    }
    #[cfg(unix)]
    std::fs::File::open(&directory).and_then(|file|file.sync_all()).map_err(ApiError::storage)?;
    store.commit_object(id,header)
}

fn account_directory(root:&Path,account:&str)->ApiResult<PathBuf> {
    if !omniplay_sync::valid_account_id(account){return Err(ApiError::invalid());}
    Ok(root.join("objects").join(account))
}
