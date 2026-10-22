use crate::{package::MAX_PACKAGE_BYTES,rejected};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_core::{updates::{ReleaseManifest,SignedEnvelope,TrustedRoot},util::{PendingFile,storage_error}};
use reqwest::blocking::Client;
use sha2::{Digest,Sha256};
use std::{io::{Read,Write},path::Path,time::{Duration,Instant}};
use std::sync::atomic::{AtomicBool,Ordering};

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootRotation {pub old_signed:SignedEnvelope,pub new_signed:SignedEnvelope}

pub fn root_rotation(url:&str,root:&TrustedRoot)->Result<Option<RootRotation>> {
    let url=validate_url(url,root)?;
    let mut response=client()?.get(url).header("Accept-Encoding","identity").timeout(Duration::from_secs(30)).send().map_err(|_|network())?;
    if response.status()==reqwest::StatusCode::NOT_FOUND{return Ok(None);}
    if !response.status().is_success(){return Err(network());}
    let mut raw=Vec::new();response.by_ref().take(1024*1024+1).read_to_end(&mut raw).map_err(|_|network())?;
    if raw.len()>1024*1024{return Err(rejected("Root rotation exceeds its bound"));}
    serde_json::from_slice(&raw).map(Some).map_err(|_|rejected("Root rotation response is invalid"))
}

pub fn validate_url(value:&str,root:&TrustedRoot)->Result<reqwest::Url> {
    let url=reqwest::Url::parse(value).map_err(|_|rejected("Update URL is invalid"))?;
    if url.scheme()!="https"||!url.username().is_empty()||url.password().is_some()||url.fragment().is_some()||url.port().is_some()||!url.host_str().is_some_and(|h|root.release_hosts.contains(h))||value.contains('\\')||value.chars().any(char::is_control){return Err(rejected("Update URL is outside the embedded release policy"));}
    Ok(url)
}
fn client()->Result<Client>{Client::builder().https_only(true).redirect(reqwest::redirect::Policy::none()).connect_timeout(Duration::from_secs(10)).timeout(Duration::from_secs(20*60)).build().map_err(|_|network())}
fn network()->PlatformError {PlatformError::new(ErrorCode::NetworkUnavailable,"The trusted update service is unavailable").recover("Retry on a working network connection")}

pub fn metadata(url:&str,root:&TrustedRoot)->Result<SignedEnvelope> {
    root.validate(omniplay_core::now_ms())?;
    let url=validate_url(url,root)?;
    let mut response=client()?.get(url).header("Accept-Encoding","identity").timeout(Duration::from_secs(30)).send().map_err(|_|network())?;
    if !response.status().is_success(){return Err(network());}let mut bytes=Vec::new();
    if response.content_length().is_some_and(|v|v>1024*1024){return Err(rejected("Update envelope exceeds its bound"));}
    response.by_ref().take(1024*1024+1).read_to_end(&mut bytes).map_err(|_|network())?;
    if bytes.len()>1024*1024{return Err(rejected("Update envelope exceeds its bound"));}
    serde_json::from_slice(&bytes).map_err(|_|rejected("Update envelope is not valid JSON"))
}

pub fn copy_verified(source:&mut impl Read,target:&mut std::fs::File,release:&ReleaseManifest)->Result<()> {copy_verified_cancellable(source,target,release,&AtomicBool::new(false))}

pub fn copy_verified_cancellable(source:&mut impl Read,target:&mut std::fs::File,release:&ReleaseManifest,cancel:&AtomicBool)->Result<()> {
    if release.artifact_bytes==0||release.artifact_bytes>MAX_PACKAGE_BYTES{return Err(rejected("Update artifact exceeds its bound"));}
    let mut hash=Sha256::new();let mut size=0u64;let mut buffer=[0u8;65536];let started=Instant::now();
    loop {if cancel.load(Ordering::Relaxed){return Err(PlatformError::new(ErrorCode::Cancelled,"Update download was cancelled"));}if started.elapsed()>Duration::from_secs(20*60){return Err(rejected("Update transfer exceeded its time bound"));}let read=source.read(&mut buffer).map_err(|_|network())?;if read==0{break;}size=size.checked_add(read as u64).ok_or_else(||rejected("Update length overflow"))?;if size>release.artifact_bytes{return Err(rejected("Update artifact exceeds its signed length"));}hash.update(&buffer[..read]);target.write_all(&buffer[..read]).map_err(storage_error)?;}
    if size!=release.artifact_bytes||format!("{:x}",hash.finalize())!=release.artifact_sha256.0{return Err(rejected("Update artifact length or checksum failed"));}
    target.sync_all().map_err(storage_error)
}

pub fn download(directory:&Path,root:&TrustedRoot,release:&ReleaseManifest)->Result<PendingFile> {
    download_cancellable(directory,root,release,&AtomicBool::new(false))
}
pub fn download_cancellable(directory:&Path,root:&TrustedRoot,release:&ReleaseManifest,cancel:&AtomicBool)->Result<PendingFile> {
    let url=validate_url(&release.artifact_url,root)?;
    if release.artifact_bytes==0||release.artifact_bytes>MAX_PACKAGE_BYTES{return Err(rejected("Update artifact exceeds its bound"));}
    let runtime=tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|_|network())?;
    runtime.block_on(async {
        let client=reqwest::Client::builder().https_only(true).redirect(reqwest::redirect::Policy::none()).connect_timeout(Duration::from_secs(10)).read_timeout(Duration::from_secs(30)).timeout(Duration::from_secs(20*60)).build().map_err(|_|network())?;
        let mut response=receive(client.get(url).header("Accept-Encoding","identity").send(),cancel).await?;
        if !response.status().is_success(){return Err(network());}
        if response.content_length().is_some_and(|v|v!=release.artifact_bytes){return Err(rejected("Update content length differs from signed metadata"));}
        let mut pending=PendingFile::create(directory)?;let mut size=0u64;let mut hash=Sha256::new();let started=Instant::now();
        while let Some(chunk)=receive(response.chunk(),cancel).await? {
            if started.elapsed()>Duration::from_secs(20*60){return Err(rejected("Update transfer exceeded its time bound"));}
            size=size.checked_add(chunk.len()as u64).ok_or_else(||rejected("Update length overflow"))?;
            if size>release.artifact_bytes{return Err(rejected("Update artifact exceeds its signed length"));}
            hash.update(&chunk);pending.file.write_all(&chunk).map_err(storage_error)?;
        }
        if size!=release.artifact_bytes||format!("{:x}",hash.finalize())!=release.artifact_sha256.0{return Err(rejected("Update artifact length or checksum failed"));}
        pending.file.sync_all().map_err(storage_error)?;Ok(pending)
    })
}

async fn receive<T>(future:impl std::future::Future<Output=std::result::Result<T,reqwest::Error>>,cancel:&AtomicBool)->Result<T> {
    // Keep the same pinned receive future between one-second cancellation polls.
    // Restarting a response read at every poll could lose partially received chunks.
    let mut future=std::pin::pin!(future);let started=Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed){return Err(PlatformError::new(ErrorCode::Cancelled,"Update download was cancelled"));}
        if started.elapsed()>Duration::from_secs(30){return Err(network());}
        match tokio::time::timeout(Duration::from_secs(1),future.as_mut()).await {Ok(result)=>return result.map_err(|_|network()),Err(_)=>continue}
    }
}
