use crate::vault;
use base64::{Engine,engine::general_purpose::URL_SAFE_NO_PAD};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_core::{Database,now_ms,util::storage_error};
use omniplay_sync::*;
use reqwest::blocking::{Client,Response};
use rusqlite::OptionalExtension;
use serde::{Serialize,Deserialize};
use std::io::Read;
use std::time::{Duration,Instant};
use uuid::Uuid;

#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncConfiguration {
    pub schema_version:u32,pub profile_id:Uuid,pub audience:String,pub account_id:String,
    pub root_public_key:String,pub certificate:SignedDocument,pub after_sequence:u64,
    #[serde(default)]
    pub registered:bool,
}

#[derive(Serialize)]
pub struct CreatedAccount {pub account_id:String,pub recovery_key:String,pub registered:bool,pub registration_error:Option<ErrorCode>}

pub struct SyncClient {http:Client,pub configuration:SyncConfiguration,device:DeviceIdentity,pub cipher:SaveCipher,deadline:Option<Instant>}

pub fn load_configuration(database:&Database)->Result<Option<SyncConfiguration>> {
    let json:Option<String>=database.connection.query_row("SELECT value FROM preferences WHERE key='sync_configuration'",[],|row|row.get(0)).optional().map_err(storage_error)?;
    let configuration:Option<SyncConfiguration>=json.map(|json|serde_json::from_str(&json).map_err(|_|PlatformError::new(ErrorCode::CorruptData,"Synchronization configuration is invalid"))).transpose()?;
    if let Some(value)=&configuration {
        validate_audience(&value.audience)?;
        if value.schema_version!=1||account_id_for_key(&value.root_public_key)?!=value.account_id{return Err(PlatformError::new(ErrorCode::CorruptData,"Synchronization identity is inconsistent"));}
    }
    Ok(configuration)
}

pub(crate) fn save_configuration(database:&Database,configuration:&SyncConfiguration)->Result<()> {
    let json=serde_json::to_string(configuration).map_err(storage_error)?;
    database.connection.execute("INSERT INTO preferences(key,value) VALUES('sync_configuration',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[json]).map_err(storage_error)?;
    Ok(())
}

fn network_error(_:impl std::fmt::Display)->PlatformError {PlatformError::new(ErrorCode::NetworkUnavailable,"The encrypted save service could not complete this request").recover("Check the network and retry; local progress remains available")}

fn http()->Result<Client> {
    Client::builder().https_only(true).redirect(reqwest::redirect::Policy::none()).connect_timeout(std::time::Duration::from_secs(10)).timeout(std::time::Duration::from_secs(100)).user_agent("OmniPlay-Sync/1").build().map_err(network_error)
}

pub fn create_account(database:&Database,profile:Uuid,audience:&str)->Result<CreatedAccount> {
    create_account_with_store(database,profile,audience,&vault::OsCredentialStore)
}
pub fn create_account_with_store(database:&Database,profile:Uuid,audience:&str,store:&dyn vault::CredentialStore)->Result<CreatedAccount> {
    store.check_access()?;
    let _transport_lease=crate::transfers::profile_lease(database)?;
    validate_audience(audience)?;
    if load_configuration(database)?.is_some(){return Err(PlatformError::new(ErrorCode::Conflict,"A synchronization account is already connected"));}
    let account=AccountIdentity::generate()?;
    let device=DeviceIdentity::generate()?;
    let certificate=account.authorize_device(&device,audience,now_ms())?;
    let configuration=SyncConfiguration{schema_version:1,profile_id:profile,audience:audience.into(),account_id:account.account_id().into(),root_public_key:account.public_key_base64(),certificate,after_sequence:0,registered:false};
    // Credential storage must work before a remote identity is created.
    vault::store(store,profile,&device,&account.save_cipher()?)?;
    if let Err(error)=save_configuration(database,&configuration){let _=store.remove(profile);return Err(error);}
    // First display the recovery code. Remote registration is a second explicit
    // action which requires that code, proving the user retained their root.
    // A lost initial IPC response cannot strand any remote encrypted data.
    Ok(CreatedAccount{account_id:configuration.account_id,recovery_key:account.recovery_key().to_string(),registered:false,registration_error:None})
}

pub fn register_account(database:&Database,recovery:&str,registration:&str)->Result<()> {
    register_account_with_store(database,recovery,registration,&vault::OsCredentialStore)
}
pub fn register_account_with_store(database:&Database,recovery:&str,registration:&str,store:&dyn vault::CredentialStore)->Result<()> {
    store.check_access()?;
    let _transport_lease=crate::transfers::profile_lease(database)?;
    let mut configuration=load_configuration(database)?.ok_or_else(||PlatformError::new(ErrorCode::NotFound,"There is no pending synchronization account"))?;
    let account=AccountIdentity::recover(recovery)?;
    if account.account_id()!=configuration.account_id{return Err(PlatformError::new(ErrorCode::PermissionDenied,"Recovery key belongs to another account"));}
    let command=RootCommand{schema_version:1,account_id:account.account_id().into(),audience:configuration.audience.clone(),nonce:Uuid::new_v4(),issued_at_ms:now_ms(),operation:RootOperation::Create{root_public_key:account.public_key_base64()}};
    let request=RootRequest{root_public_key:account.public_key_base64(),command:account.sign_root(ROOT_COMMAND_DOMAIN,&command)?};
    let request=http()?.post(format!("{}/v1/account",configuration.audience)).header("x-omniplay-registration",registration).json(&request);store.check_access()?;
    ensure_success(request.send().map_err(network_error)?)?;
    configuration.registered=true;
    save_configuration(database,&configuration)?;
    let mut settings=database.settings()?;settings.cloud_sync_opt_in=true;database.set_settings(&settings)
}

pub fn recover_account(database:&Database,profile:Uuid,audience:&str,recovery:&str)->Result<SyncConfiguration> {
    recover_account_with_store(database,profile,audience,recovery,&vault::OsCredentialStore)
}
pub fn recover_account_with_store(database:&Database,profile:Uuid,audience:&str,recovery:&str,store:&dyn vault::CredentialStore)->Result<SyncConfiguration> {
    store.check_access()?;
    let _transport_lease=crate::transfers::profile_lease(database)?;
    validate_audience(audience)?;
    if load_configuration(database)?.is_some(){return Err(PlatformError::new(ErrorCode::Conflict,"Disconnect the current synchronization account before recovering another"));}
    let account=AccountIdentity::recover(recovery)?;let device=DeviceIdentity::generate()?;
    let configuration=SyncConfiguration{schema_version:1,profile_id:profile,audience:audience.into(),account_id:account.account_id().into(),root_public_key:account.public_key_base64(),certificate:account.authorize_device(&device,audience,now_ms())?,after_sequence:0,registered:true};
    let mut client=SyncClient{http:http()?,configuration:configuration.clone(),device,cipher:account.save_cipher()?,deadline:None};
    store.check_access()?;client.list(0,1)?;
    vault::store(store,profile,&client.device,&client.cipher)?;
    if let Err(error)=save_configuration(database,&configuration){let _=store.remove(profile);return Err(error);}
    let mut settings=database.settings()?;settings.cloud_sync_opt_in=true;database.set_settings(&settings)?;
    Ok(configuration)
}

pub fn revoke_device(database:&Database,recovery:&str,device_id:Uuid)->Result<()> {
    revoke_device_with_store(database,recovery,device_id,&vault::OsCredentialStore)
}
pub fn revoke_device_with_store(database:&Database,recovery:&str,device_id:Uuid,store:&dyn vault::CredentialStore)->Result<()> {
    root_operation(database,recovery,RootOperation::RevokeDevice{device_id},store)
}

pub fn renew_certificate(database:&Database,recovery:&str)->Result<()> {
    renew_certificate_with_store(database,recovery,&vault::OsCredentialStore)
}
pub fn renew_certificate_with_store(database:&Database,recovery:&str,store:&dyn vault::CredentialStore)->Result<()> {
    store.check_access()?;
    let _transport_lease=crate::transfers::profile_lease(database)?;
    let mut configuration=load_configuration(database)?.ok_or_else(||PlatformError::new(ErrorCode::NotFound,"No synchronization account is connected"))?;
    let account=AccountIdentity::recover(recovery)?;
    if account.account_id()!=configuration.account_id{return Err(PlatformError::new(ErrorCode::PermissionDenied,"Recovery key belongs to another account"));}
    let (device,_)=vault::load(store,configuration.profile_id,&configuration.account_id)?;
    configuration.certificate=account.authorize_device(&device,&configuration.audience,now_ms())?;
    // A revoked device ID stays revoked even if its old certificate is renewed.
    save_configuration(database,&configuration)
}

pub fn delete_account(database:&Database,recovery:&str)->Result<()> {
    delete_account_with_store(database,recovery,&vault::OsCredentialStore)
}
pub fn delete_account_with_store(database:&Database,recovery:&str,store:&dyn vault::CredentialStore)->Result<()> {
    store.check_access()?;let _transport_lease=crate::transfers::profile_lease(database)?;
    root_operation_locked(database,recovery,RootOperation::DeleteAccount,store)?;
    disconnect_locked(database,store)
}

fn root_operation(database:&Database,recovery:&str,operation:RootOperation,store:&dyn vault::CredentialStore)->Result<()> {
    store.check_access()?;let _transport_lease=crate::transfers::profile_lease(database)?;
    root_operation_locked(database,recovery,operation,store)
}
fn root_operation_locked(database:&Database,recovery:&str,operation:RootOperation,store:&dyn vault::CredentialStore)->Result<()> {
    store.check_access()?;
    let configuration=load_configuration(database)?.ok_or_else(||PlatformError::new(ErrorCode::NotFound,"No synchronization account is connected"))?;
    let account=AccountIdentity::recover(recovery)?;
    if account.account_id()!=configuration.account_id{return Err(PlatformError::new(ErrorCode::PermissionDenied,"Recovery key belongs to another account"));}
    let command=RootCommand{schema_version:1,account_id:configuration.account_id,audience:configuration.audience.clone(),nonce:Uuid::new_v4(),issued_at_ms:now_ms(),operation};
    let request=RootRequest{root_public_key:account.public_key_base64(),command:account.sign_root(ROOT_COMMAND_DOMAIN,&command)?};
    let request=http()?.post(format!("{}/v1/account",configuration.audience)).json(&request);store.check_access()?;
    ensure_success(request.send().map_err(network_error)?)?;
    Ok(())
}

pub fn disconnect(database:&Database)->Result<()> {
    disconnect_with_store(database,&vault::OsCredentialStore)
}
pub fn disconnect_with_store(database:&Database,store:&dyn vault::CredentialStore)->Result<()> {
    store.check_access()?;let _transport_lease=crate::transfers::profile_lease(database)?;
    disconnect_locked(database,store)
}
fn disconnect_locked(database:&Database,store:&dyn vault::CredentialStore)->Result<()> {
    store.check_access()?;
    database.set_mobile_background_sync(false)?;
    let mut settings=database.settings()?;settings.cloud_sync_opt_in=false;database.set_settings(&settings)?;
    crate::transfers::clear(database)?;
    crate::recovery::clear(database)?;
    if let Some(configuration)=load_configuration(database)? {store.remove(configuration.profile_id)?;}
    database.connection.execute("DELETE FROM preferences WHERE key='sync_configuration'",[]).map_err(storage_error)?;
    Ok(())
}

impl SyncClient {
    pub fn open(database:&Database)->Result<Self> {
        Self::open_with_store(database,&vault::OsCredentialStore)
    }
    pub fn open_with_store(database:&Database,store:&dyn vault::CredentialStore)->Result<Self> {
        store.check_access()?;
        if !database.settings()?.cloud_sync_opt_in{return Err(PlatformError::new(ErrorCode::PermissionDenied,"Synchronization is disabled"));}
        let configuration=load_configuration(database)?.ok_or_else(||PlatformError::new(ErrorCode::NotFound,"No synchronization account is connected"))?;
        if !configuration.registered{return Err(PlatformError::new(ErrorCode::PermissionDenied,"Complete synchronization account registration first"));}
        let (device,cipher)=vault::load(store,configuration.profile_id,&configuration.account_id)?;
        Ok(Self{http:http()?,configuration,device,cipher,deadline:None})
    }
    pub(crate) fn set_deadline(&mut self,deadline:Option<Instant>){self.deadline=deadline;}

    fn authorization(&self,operation:SyncOperation)->Result<String> {
        let request=SyncRequest{schema_version:1,account_id:self.configuration.account_id.clone(),device_id:self.device.id,audience:self.configuration.audience.clone(),nonce:Uuid::new_v4(),issued_at_ms:now_ms(),operation};
        let signed=self.device.sign_request(&request,self.configuration.certificate.clone())?;
        // Local validation catches an expired certificate before any network work.
        verify_device_request(&self.configuration.root_public_key,&signed,&self.configuration.audience,now_ms())?;
        let encoded=URL_SAFE_NO_PAD.encode(serde_json::to_vec(&signed).map_err(storage_error)?);
        if encoded.len()>32768{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Sync authorization header exceeds policy"));}
        Ok(encoded)
    }

    fn request(&self,operation:SyncOperation)->Result<reqwest::blocking::RequestBuilder> {
        let request=self.http.post(format!("{}/v1/sync",self.configuration.audience)).header("x-omniplay-authorization",self.authorization(operation)?);
        if let Some(deadline)=self.deadline{
            let remaining=deadline.saturating_duration_since(Instant::now());
            if remaining<Duration::from_millis(250){return Err(PlatformError::new(ErrorCode::Cancelled,"The synchronization opportunity ended"));}
            return Ok(request.timeout(remaining.min(Duration::from_secs(100))));
        }
        Ok(request)
    }

    pub fn list(&mut self,after_sequence:u64,limit:u16)->Result<Vec<ObjectSummary>> {
        let response=ensure_success(self.request(SyncOperation::List{after_sequence,limit})?.send().map_err(network_error)?)?;
        decode_small(response)
    }

    pub fn devices(&self)->Result<Vec<serde_json::Value>> {
        decode_small(ensure_success(self.request(SyncOperation::Devices)?.send().map_err(network_error)?)?)
    }
    pub(crate) fn transfer_capabilities(&self)->Result<TransferCapabilities>{
        let value:TransferCapabilities=decode_small(ensure_success(self.request(SyncOperation::TransferCapabilities)?.send().map_err(network_error)?)?)?;
        if value.protocol!=TRANSFER_PROTOCOL||value.chunk_bytes!=TRANSFER_CHUNK_BYTES||value.max_object_bytes<MAX_CIPHERTEXT_BYTES as u64||value.expiry_ms==0||value.expiry_ms>TRANSFER_TTL_MS{return Err(PlatformError::new(ErrorCode::UnsupportedProtocol,"The save service does not support the required resumable transport"));}Ok(value)
    }
    pub(crate) fn device_id(&self)->Uuid{self.device.id}
    pub(crate) fn upload_abort(&self,id:Uuid)->Result<()>{ensure_success(self.request(SyncOperation::UploadAbort{transfer_id:id})?.send().map_err(network_error)?)?;Ok(())}
    pub(crate) fn upload_begin(&self,id:Uuid,header:&BlobHeader)->Result<UploadProgress>{decode_small(ensure_success(self.request(SyncOperation::UploadBegin{transfer_id:id,header:header.clone()})?.send().map_err(network_error)?)?)}
    pub(crate) fn upload_chunk(&self,id:Uuid,offset:u64,bytes:Vec<u8>)->Result<UploadProgress>{
        let hash=omniplay_core::util::digest(&bytes).0;let length=bytes.len();
        decode_small(ensure_success(self.request(SyncOperation::UploadChunk{transfer_id:id,offset,bytes:length as u32,sha256:hash})?.header("content-type","application/octet-stream").header("content-length",length).body(bytes).send().map_err(network_error)?)?)
    }
    pub(crate) fn upload_commit(&self,id:Uuid,header:&BlobHeader)->Result<UploadProgress>{decode_small(ensure_success(self.request(SyncOperation::UploadCommit{transfer_id:id,header:header.clone()})?.send().map_err(network_error)?)?)}
    pub(crate) fn object_header(&self,id:Uuid)->Result<BlobHeader>{
        let header:BlobHeader=decode_small(ensure_success(self.request(SyncOperation::ObjectHeader{object_id:id})?.send().map_err(network_error)?)?)?;header.validate()?;
        if header.binding.object_id!=id||header.binding.account_id!=self.configuration.account_id{return Err(PlatformError::new(ErrorCode::CorruptData,"The remote object identity changed"));}Ok(header)
    }
    pub(crate) fn object_chunk(&self,expected:&BlobHeader,offset:u64,length:u32)->Result<Vec<u8>>{
        if chunk_length(expected.ciphertext_bytes,offset)!=Some(length){return Err(PlatformError::new(ErrorCode::InvalidRequest,"Invalid encrypted chunk range"));}
        let response=ensure_success(self.request(SyncOperation::ObjectChunk{object_id:expected.binding.object_id,offset,bytes:length})?.send().map_err(network_error)?)?;
        let metadata=response.headers().get("x-omniplay-object").and_then(|value|value.to_str().ok()).filter(|value|value.len()<4096).ok_or_else(||network_error("Missing object header"))?;
        let header:BlobHeader=serde_json::from_slice(&URL_SAFE_NO_PAD.decode(metadata).map_err(network_error)?).map_err(network_error)?;
        let hash=response.headers().get("x-omniplay-chunk-sha256").and_then(|value|value.to_str().ok()).filter(|value|value.len()==64).ok_or_else(||network_error("Missing chunk hash"))?.to_owned();
        if header!=*expected{return Err(PlatformError::new(ErrorCode::CorruptData,"The immutable encrypted header changed"));}
        let mut bytes=Vec::with_capacity(length as usize);response.take(u64::from(length)+1).read_to_end(&mut bytes).map_err(network_error)?;
        if bytes.len()!=length as usize||omniplay_core::util::digest(&bytes).0!=hash{return Err(PlatformError::new(ErrorCode::CorruptData,"Encrypted chunk integrity check failed"));}Ok(bytes)
    }

    pub fn put(&self,envelope:&EncryptedSave)->Result<()> {
        self.put_checked(envelope,||Ok(()))
    }
    pub(crate) fn put_checked(&self,envelope:&EncryptedSave,check:impl Fn()->Result<()>)->Result<()> {
        check()?;
        let (header,bytes)=envelope.to_blob()?;
        check()?;
        let response=self.request(SyncOperation::Put{header:header.clone()})?.header("content-type","application/octet-stream").header("content-length",bytes.len()).body(bytes).send().map_err(network_error)?;
        if response.status()==reqwest::StatusCode::CONFLICT {
            // A previous upload may have succeeded before its response was lost.
            // Re-encryption has a fresh nonce, so compare authenticated plaintext.
            check()?;let remote=self.get(header.binding.object_id)?;check()?;
            let (remote_metadata,remote_payload)=self.cipher.decrypt_save(&remote)?;
            check()?;
            let (local_metadata,local_payload)=self.cipher.decrypt_save(envelope)?;
            check()?;
            if remote_metadata.id==local_metadata.id&&remote_metadata.game_content_hash==local_metadata.game_content_hash&&remote_metadata.payload_hash==local_metadata.payload_hash&&remote_metadata.kind==local_metadata.kind&&remote_metadata.slot==local_metadata.slot&&remote_metadata.platform==local_metadata.platform&&remote_metadata.parent_id==local_metadata.parent_id&&remote_metadata.core_id==local_metadata.core_id&&remote_metadata.core_version==local_metadata.core_version&&remote_metadata.state_format_version==local_metadata.state_format_version&&remote_metadata.core_config_hash==local_metadata.core_config_hash&&remote_payload.as_slice()==local_payload.as_slice(){return Ok(());}
            return Err(PlatformError::new(ErrorCode::Conflict,"The remote immutable save contains different progress"));
        }
        ensure_success(response)?;Ok(())
    }

    pub fn get(&self,object_id:Uuid)->Result<EncryptedSave> {
        let response=ensure_success(self.request(SyncOperation::Get{object_id})?.send().map_err(network_error)?)?;
        let encoded=response.headers().get("x-omniplay-object").and_then(|value|value.to_str().ok()).filter(|value|value.len()<4096).ok_or_else(||network_error("Missing object metadata"))?;
        let header:BlobHeader=serde_json::from_slice(&URL_SAFE_NO_PAD.decode(encoded).map_err(network_error)?).map_err(network_error)?;
        header.validate()?;
        if header.binding.object_id!=object_id||header.binding.account_id!=self.configuration.account_id{return Err(network_error("Unexpected encrypted object"));}
        let mut bytes=Vec::new();response.take(header.ciphertext_bytes.saturating_add(1)).read_to_end(&mut bytes).map_err(network_error)?;
        EncryptedSave::from_blob(header,&bytes)
    }
}

fn decode_small<T:serde::de::DeserializeOwned>(response:Response)->Result<T> {
    let mut bytes=Vec::new();response.take(256*1024+1).read_to_end(&mut bytes).map_err(network_error)?;
    if bytes.len()>256*1024{return Err(network_error("Response size exceeds policy"));}
    serde_json::from_slice(&bytes).map_err(network_error)
}

fn ensure_success(response:Response)->Result<Response> {
    if response.status().is_success(){return Ok(response);}
    let code=match response.status().as_u16(){401|403=>ErrorCode::PermissionDenied,404=>ErrorCode::NotFound,409=>ErrorCode::Conflict,413|429=>ErrorCode::ResourceLimit,_=>ErrorCode::NetworkUnavailable};
    Err(PlatformError::new(code,"The encrypted save service rejected this operation").recover("Review the account connection and retry; local saves remain unchanged"))
}
