//! Browser-only cryptographic adapter. The host supplies a finite WASIp1 ABI,
//! not files, sockets, environment, arguments, clocks or a generic WASI runtime.
//! All cryptographic/wire algorithms are the same pinned native Rust code.
#![deny(unsafe_op_in_unsafe_fn)]
use omniplay_contracts::{ErrorCode,PlatformError,Result,SaveMetadata};
use omniplay_sync::{AccountIdentity,DeviceIdentity,SaveCipher,SignedDocument,BlobHeader,SyncOperation,SyncRequest,RootOperation,RootCommand,RootRequest};
use serde::{Deserialize,Serialize};
use serde_json::json;
use std::{collections::BTreeMap,sync::{Mutex,LazyLock}};
use uuid::Uuid;
use zeroize::{Zeroize,ZeroizeOnDrop,Zeroizing};

const MAX_JSON:usize=1024*1024;
const MAX_BINARY:usize=omniplay_sync::MAX_CIPHERTEXT_BYTES;
const CREDENTIAL_BYTES:usize=84;
const MAX_TIME:u64=8_640_000_000_000_000;
fn invalid()->PlatformError{PlatformError::new(ErrorCode::InvalidRequest,"The browser crypto command exceeds its finite protocol")}
fn unavailable()->PlatformError{PlatformError::new(ErrorCode::PermissionDenied,"Unlock the current synchronization device before this operation")}
fn resource()->PlatformError{PlatformError::new(ErrorCode::ResourceLimit,"The browser crypto buffer budget is exhausted")}
fn now(value:u64)->Result<()>{if value==0||value>MAX_TIME{Err(invalid())}else{Ok(())}}

#[derive(Deserialize,Zeroize,ZeroizeOnDrop)]
#[serde(tag="method",rename_all="snake_case",deny_unknown_fields)]
enum Command{
    Create{audience:String,now_ms:u64},
    Recover{recovery_key:String,audience:String,now_ms:u64},
    Load{account_id:String,root_public_key:String,audience:String,#[zeroize(skip)]certificate:SignedDocument,now_ms:u64},
    Sign{#[zeroize(skip)]operation:SyncOperation,now_ms:u64},
    Root{recovery_key:String,#[zeroize(skip)]operation:RootOperation,now_ms:u64},
    Renew{recovery_key:String,now_ms:u64},
    Seal{#[zeroize(skip)]metadata:SaveMetadata},
    Open{#[zeroize(skip)]header:BlobHeader},
    Clear,
}
struct Session{device:DeviceIdentity,cipher:SaveCipher,root_public_key:String,audience:String,certificate:SignedDocument}
impl Session{
    fn binding(&self,now_ms:u64)->Result<bool>{
        now(now_ms)?;omniplay_sync::validate_audience(&self.audience)?;
        let payload=omniplay_sync::verify_document(&omniplay_sync::parse_key(&self.root_public_key)?,omniplay_sync::CERTIFICATE_DOMAIN,&self.certificate,8192)?;
        let certificate:omniplay_sync::DeviceCertificate=serde_json::from_slice(&payload).map_err(|_|invalid())?;
        if certificate.schema_version!=1||certificate.account_id!=self.cipher.account_id()||certificate.account_id!=omniplay_sync::account_id_for_key(&self.root_public_key)?||certificate.device_id!=self.device.id||certificate.device_public_key!=self.device.public_key_base64()||certificate.audience!=self.audience||certificate.expires_at_ms<=certificate.issued_at_ms||certificate.expires_at_ms-certificate.issued_at_ms>90*24*60*60*1000||certificate.issued_at_ms>now_ms.saturating_add(120_000){return Err(unavailable());}
        // Expiry prevents signed network requests, but must not prevent a root
        // holder from loading its own device solely to renew the certificate.
        Ok(certificate.expires_at_ms>now_ms)
    }
    fn signed(&self,operation:SyncOperation,now_ms:u64)->Result<omniplay_sync::AuthenticatedRequest>{
        now(now_ms)?;let request=SyncRequest{schema_version:1,account_id:self.cipher.account_id().into(),device_id:self.device.id,audience:self.audience.clone(),nonce:Uuid::new_v4(),issued_at_ms:now_ms,operation};
        let signed=self.device.sign_request(&request,self.certificate.clone())?;
        // Reuse native certificate, domain, audience, identity and operation
        // admission before any signed request is returned to a browser transport.
        omniplay_sync::verify_device_request(&self.root_public_key,&signed,&self.audience,now_ms)?;Ok(signed)
    }
    fn root(&self,recovery:&str)->Result<AccountIdentity>{let root=AccountIdentity::recover(recovery)?;if root.account_id()!=self.cipher.account_id()||root.public_key_base64()!=self.root_public_key{return Err(unavailable());}Ok(root)}
}
struct State{
    buffers:BTreeMap<u32,Zeroizing<Vec<u8>>>,
    json:Zeroizing<Vec<u8>>,binary:Zeroizing<Vec<u8>>,session:Option<Session>,
}
impl State{
    fn new()->Self{Self{buffers:BTreeMap::new(),json:Zeroizing::new(Vec::new()),binary:Zeroizing::new(Vec::new()),session:None}}
    fn reset_result(&mut self){self.json=Zeroizing::new(Vec::new());self.binary=Zeroizing::new(Vec::new());}
    fn write<T:Serialize>(&mut self,value:&T)->Result<()>{
        // Result structures are finite public metadata plus, only for Create,
        // a borrowed recovery string. Serialize directly into zeroizing storage.
        self.json.zeroize();self.json.clear();serde_json::to_writer(&mut *self.json,value).map_err(|_|invalid())?;
        if self.json.len()>MAX_JSON{self.reset_result();return Err(resource());}Ok(())
    }
    fn provision(&mut self,root:AccountIdentity,audience:&str,now_ms:u64,reveal:bool)->Result<()>{
        now(now_ms)?;omniplay_sync::validate_audience(audience)?;let device=DeviceIdentity::generate()?;let cipher=root.save_cipher()?;let certificate=root.authorize_device(&device,audience,now_ms)?;
        let session=Session{device,cipher,root_public_key:root.public_key_base64(),audience:audience.into(),certificate};
        session.signed(SyncOperation::TransferCapabilities,now_ms)?;
        let mut credentials=Zeroizing::new(Vec::new());credentials.try_reserve_exact(CREDENTIAL_BYTES).map_err(|_|resource())?;credentials.extend_from_slice(b"OPK1");credentials.extend_from_slice(session.device.id.as_bytes());credentials.extend_from_slice(session.device.secret_bytes().as_ref());credentials.extend_from_slice(session.cipher.secret_bytes().as_ref());
        #[derive(Serialize)]struct Provision<'a>{schema_version:u32,account_id:&'a str,device_id:Uuid,root_public_key:&'a str,audience:&'a str,certificate:&'a SignedDocument,#[serde(skip_serializing_if="Option::is_none")]recovery_key:Option<&'a str>}
        let recovery=if reveal{Some(root.recovery_key())}else{None};
        self.write(&Provision{schema_version:1,account_id:session.cipher.account_id(),device_id:session.device.id,root_public_key:&session.root_public_key,audience:&session.audience,certificate:&session.certificate,recovery_key:recovery.as_ref().map(|value|value.as_str())})?;
        self.binary=credentials;self.session=Some(session);Ok(())
    }
    fn execute(&mut self,command:&Command,mut binary:Zeroizing<Vec<u8>>)->Result<()>{
        if !matches!(command,Command::Load{..}|Command::Seal{..}|Command::Open{..})&&!binary.is_empty(){return Err(invalid());}
        match command{
            Command::Create{audience,now_ms}=>{self.session=None;self.provision(AccountIdentity::generate()?,audience,*now_ms,true)},
            Command::Recover{recovery_key,audience,now_ms}=>{self.session=None;self.provision(AccountIdentity::recover(recovery_key)?,audience,*now_ms,false)},
            Command::Load{account_id,root_public_key,audience,certificate,now_ms}=>{
                self.session=None;now(*now_ms)?;if binary.len()!=CREDENTIAL_BYTES||&binary[..4]!=b"OPK1"{return Err(invalid());}
                let id=Uuid::from_slice(&binary[4..20]).map_err(|_|invalid())?;if id.is_nil(){return Err(invalid());}
                let mut seed=Zeroizing::new([0u8;32]);seed.copy_from_slice(&binary[20..52]);let mut data=Zeroizing::new([0u8;32]);data.copy_from_slice(&binary[52..84]);
                let session=Session{device:DeviceIdentity::from_seed(id,*seed)?,cipher:SaveCipher::from_secret(account_id.clone(),*data)?,root_public_key:root_public_key.clone(),audience:audience.clone(),certificate:certificate.clone()};
                let current=session.binding(*now_ms)?;self.write(&json!({"loaded":true,"device_id":session.device.id,"account_id":session.cipher.account_id(),"certificate_current":current}))?;self.session=Some(session);Ok(())
            },
            Command::Sign{operation,now_ms}=>{let value=self.session.as_ref().ok_or_else(unavailable)?.signed(operation.clone(),*now_ms)?;self.write(&value)},
            Command::Root{recovery_key,operation,now_ms}=>{
                now(*now_ms)?;let session=self.session.as_ref().ok_or_else(unavailable)?;let root=session.root(recovery_key)?;
                let command=RootCommand{schema_version:1,account_id:root.account_id().into(),audience:session.audience.clone(),nonce:Uuid::new_v4(),issued_at_ms:*now_ms,operation:operation.clone()};
                let request=RootRequest{root_public_key:root.public_key_base64(),command:root.sign_root(omniplay_sync::ROOT_COMMAND_DOMAIN,&command)?};omniplay_sync::verify_root_request(&request,&session.audience,*now_ms)?;self.write(&request)
            },
            Command::Renew{recovery_key,now_ms}=>{
                now(*now_ms)?;let session=self.session.as_mut().ok_or_else(unavailable)?;let root=session.root(recovery_key)?;let certificate=root.authorize_device(&session.device,&session.audience,*now_ms)?;session.certificate=certificate.clone();self.write(&certificate)
            },
            Command::Seal{metadata}=>{
                let session=self.session.as_ref().ok_or_else(unavailable)?;let(header,encrypted)=session.cipher.encrypt_blob(metadata,&binary)?;self.write(&header)?;self.binary=Zeroizing::new(encrypted);Ok(())
            },
            Command::Open{header}=>{
                let session=self.session.as_ref().ok_or_else(unavailable)?;let(metadata,payload)=session.cipher.decrypt_owned_blob_checked(header,std::mem::take(&mut binary),&||Ok(()))?;self.write(&metadata)?;self.binary=payload;Ok(())
            },
            Command::Clear=>{self.session=None;self.buffers.clear();self.write(&json!({"cleared":true}))},
        }
    }
}
static STATE:LazyLock<Mutex<State>>=LazyLock::new(||Mutex::new(State::new()));

/// Only registered, zero-initialized allocations are accepted by op_command.
/// No arbitrary caller pointer is dereferenced by Rust at this ABI boundary.
#[no_mangle]pub extern "C" fn op_alloc(bytes:u32)->u32{
    let Ok(mut state)=STATE.lock()else{return 0};let length=bytes as usize;
    if length==0||length>MAX_BINARY||state.buffers.len()>=2||state.buffers.values().map(|v|v.len()).sum::<usize>().saturating_add(length)>MAX_BINARY+MAX_JSON{return 0;}
    let mut buffer=Zeroizing::new(Vec::new());if buffer.try_reserve_exact(length).is_err(){return 0;}buffer.resize(length,0);let pointer=buffer.as_ptr()as usize as u32;if pointer==0||state.buffers.contains_key(&pointer){return 0;}state.buffers.insert(pointer,buffer);pointer
}
#[no_mangle]pub extern "C" fn op_free(pointer:u32)->u32{STATE.lock().map(|mut state|u32::from(state.buffers.remove(&pointer).is_some())).unwrap_or(0)}
#[no_mangle]pub extern "C" fn op_command(control:u32,binary:u32)->u32{
    let Ok(mut state)=STATE.lock()else{return 1};state.reset_result();
    let result=(||{
        if control==0||control==binary{return Err(invalid());}let source=state.buffers.remove(&control).ok_or_else(invalid)?;if source.len()>MAX_JSON||source.iter().copied().find(|byte|!byte.is_ascii_whitespace())!=Some(b'{'){return Err(invalid());}
        let payload=if binary==0{Zeroizing::new(Vec::new())}else{state.buffers.remove(&binary).ok_or_else(invalid)?};if !state.buffers.is_empty(){return Err(invalid());}
        let command:Command=serde_json::from_slice(&source).map_err(|_|invalid())?;state.execute(&command,payload)
    })();
    match result{Ok(())=>0,Err(error)=>{state.reset_result();state.buffers.clear();if state.write(&error).is_err(){state.json.extend_from_slice(b"{\"code\":\"resource_limit\",\"message\":\"Browser crypto output failed\"}");}1}}
}
#[no_mangle]pub extern "C" fn op_result_ptr()->u32{STATE.lock().map(|state|state.json.as_ptr()as usize as u32).unwrap_or(0)}
#[no_mangle]pub extern "C" fn op_result_len()->u32{STATE.lock().map(|state|state.json.len()as u32).unwrap_or(0)}
#[no_mangle]pub extern "C" fn op_binary_ptr()->u32{STATE.lock().map(|state|state.binary.as_ptr()as usize as u32).unwrap_or(0)}
#[no_mangle]pub extern "C" fn op_binary_len()->u32{STATE.lock().map(|state|state.binary.len()as u32).unwrap_or(0)}
#[no_mangle]pub extern "C" fn op_clear_result(){if let Ok(mut state)=STATE.lock(){state.reset_result();}}
#[no_mangle]pub extern "C" fn op_clear(){if let Ok(mut state)=STATE.lock(){state.reset_result();state.buffers.clear();state.session=None;}}
#[no_mangle]pub extern "C" fn op_protocol()->u32{1}
