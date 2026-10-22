use crate::{ApiError,ApiResult,Shared,store::Store};
use axum::{body::{Body,Bytes},http::{HeaderMap,HeaderValue,header},response::Response};
use base64::{Engine,engine::general_purpose::URL_SAFE_NO_PAD};
use futures_util::StreamExt;
use omniplay_sync::{DeviceCertificate,SyncOperation,TransferCapabilities,TRANSFER_CHUNK_BYTES};
use sha2::{Digest,Sha256};
use tokio::sync::OwnedSemaphorePermit;

struct HeldBody{bytes:Vec<u8>,_permit:OwnedSemaphorePermit}
impl AsRef<[u8]> for HeldBody{fn as_ref(&self)->&[u8]{&self.bytes}}
fn held_body(bytes:Vec<u8>,permit:OwnedSemaphorePermit)->Body{Body::from(Bytes::from_owner(HeldBody{bytes,_permit:permit}))}
fn json<T:serde::Serialize>((value,permit):(T,OwnedSemaphorePermit))->ApiResult<Response>{
    let bytes=serde_json::to_vec(&value).map_err(ApiError::storage)?;
    if bytes.len()>16*1024{return Err(ApiError::storage("Transfer response exceeds its bound"));}
    let mut response=Response::new(held_body(bytes,permit));
    response.headers_mut().insert(header::CONTENT_TYPE,HeaderValue::from_static("application/json"));
    response.headers_mut().insert(header::CACHE_CONTROL,HeaderValue::from_static("no-store"));Ok(response)
}

async fn body_bytes(body:Body,limit:usize)->ApiResult<Vec<u8>>{
    let receive=async move{
        let mut bytes=Vec::with_capacity(limit);let mut stream=body.into_data_stream();
        while let Some(chunk)=stream.next().await{let chunk=chunk.map_err(|_|ApiError::invalid())?;if bytes.len().saturating_add(chunk.len())>limit{return Err(ApiError::invalid());}bytes.extend_from_slice(&chunk);}
        if bytes.len()!=limit{return Err(ApiError::invalid());}Ok(bytes)
    };
    tokio::time::timeout(std::time::Duration::from_secs(30),receive).await.map_err(|_|ApiError::invalid())?
}
async fn blocking<T:Send+'static>(service:Shared,permit:OwnedSemaphorePermit,operation:impl FnOnce(&mut Store)->ApiResult<T>+Send+'static)->ApiResult<(T,OwnedSemaphorePermit)>{
    // A dropped HTTP future cannot free admission while a blocking filesystem/DB
    // owner is queued or alive. On success the response's byte owner retains it.
    tokio::task::spawn_blocking(move||{let value=operation(&mut service.store.blocking_lock())?;Ok((value,permit))}).await.map_err(|_|ApiError::storage("Transfer worker ended"))?
}
pub(crate) fn maintenance(service:&Shared)->tokio::task::JoinHandle<()>{
    let weak=std::sync::Arc::downgrade(service);
    tokio::spawn(async move{
        let mut interval=tokio::time::interval(std::time::Duration::from_secs(60));interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop{
            interval.tick().await;
            let Some(service)=weak.upgrade() else{break;};
            let Ok(permit)=service.transfer_permits.clone().try_acquire_owned() else{continue;};
            // A cancelled async owner leaves blocking work holding admission
            // and the Service's kernel root lease until its actual return.
            if let Err(error)=blocking(service,permit,|store|store.collect_uploads()).await{eprintln!("OmniPlay sync staging maintenance deferred: {}",error.code);}
        }
    })
}
pub(crate) async fn handle(service:&Shared,certificate:&DeviceCertificate,operation:SyncOperation,headers:HeaderMap,body:Body)->ApiResult<Response>{
    let permit=service.transfer_permits.clone().try_acquire_owned().map_err(|_|ApiError::quota())?;
    let length=match &operation{SyncOperation::UploadChunk{bytes,..}=>*bytes as usize,_=>0};
    if length>TRANSFER_CHUNK_BYTES as usize{return Err(ApiError::invalid());}
    if length>0{
        let declared=headers.get(header::CONTENT_LENGTH).and_then(|value|value.to_str().ok()).and_then(|value|value.parse::<usize>().ok());
        if declared!=Some(length){return Err(ApiError::invalid());}
    }
    let bytes=body_bytes(body,length).await?;let shared=service.clone();let certificate=certificate.clone();
    match operation{
        SyncOperation::TransferCapabilities=>json(blocking(shared,permit,move|store|{store.check_device_active(&certificate)?;Ok(TransferCapabilities::default())}).await?),
        SyncOperation::UploadBegin{transfer_id,header}=>json(blocking(shared,permit,move|store|store.upload_begin(&certificate,transfer_id,header)).await?),
        SyncOperation::UploadChunk{transfer_id,offset,sha256,..}=>json(blocking(shared,permit,move|store|store.upload_chunk(&certificate,transfer_id,offset,&sha256,&bytes)).await?),
        SyncOperation::UploadCommit{transfer_id,header}=>json(blocking(shared,permit,move|store|store.upload_commit(&certificate,transfer_id,header)).await?),
        SyncOperation::UploadAbort{transfer_id}=>json(blocking(shared,permit,move|store|{store.upload_abort(&certificate,transfer_id)?;Ok(serde_json::json!({"aborted":true}))}).await?),
        SyncOperation::ObjectHeader{object_id}=>json(blocking(shared,permit,move|store|{store.check_device_active(&certificate)?;store.object_header(&certificate.account_id,object_id)}).await?),
        SyncOperation::ObjectChunk{object_id,offset,bytes:length}=>{
            let ((header,bytes),permit)=blocking(shared,permit,move|store|{store.check_device_active(&certificate)?;store.object_chunk(&certificate.account_id,object_id,offset,length)}).await?;
            let digest=format!("{:x}",Sha256::digest(&bytes));let encoded=URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).map_err(ApiError::storage)?);
            let mut response=Response::new(held_body(bytes,permit));
            response.headers_mut().insert(header::CONTENT_TYPE,HeaderValue::from_static("application/octet-stream"));
            response.headers_mut().insert(header::CACHE_CONTROL,HeaderValue::from_static("no-store"));
            response.headers_mut().insert(header::CONTENT_LENGTH,HeaderValue::from_str(&length.to_string()).map_err(ApiError::storage)?);
            response.headers_mut().insert("x-omniplay-object",HeaderValue::from_str(&encoded).map_err(ApiError::storage)?);
            response.headers_mut().insert("x-omniplay-chunk-sha256",HeaderValue::from_str(&digest).map_err(ApiError::storage)?);
            Ok(response)
        }
        _=>Err(ApiError::invalid()),
    }
}
