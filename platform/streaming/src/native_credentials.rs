//! Native Remote Play secret material, separate from save-sync credentials.
//! The durable lifecycle journal must publish/fence these generation addresses.
//! A vault write/delete alone is never pairing activation or revocation.
use crate::{decode,document_hash,key,verify_grant,invalid,SignedDocument,
    REQUEST_DOMAIN,INVITE_DOMAIN,HELLO_DOMAIN,GRANT_DOMAIN,MAX_REQUEST_BYTES};
use base64::{Engine,engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{Signer,SigningKey};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use serde::{Deserialize,Serialize};
use serde::de::{IgnoredAny,MapAccess,Visitor};
use sha2::{Digest,Sha256};
use std::time::Instant;
use uuid::Uuid;
use zeroize::Zeroizing;

const MAGIC:&[u8;8]=b"VLKRP001";
pub const SECRET_RECORD_BYTES:usize=137;
pub const SECRET_SERVICE:&str="org.vlikeon.remote-play.credentials.v1";

#[derive(Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum SecretKind{DeviceSigning,PairEncryption}

/// Non-secret generation address. Deserialization does not establish lifecycle
/// authority; every vault operation validates it and the caller must consult its
/// durable profile/device journal before loading or publishing the generation.
#[derive(Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialAddress{profile:Uuid,device:Uuid,kind:SecretKind,id:Uuid,generation:Uuid}
impl CredentialAddress{
    pub fn new_identity(profile:Uuid,device:Uuid)->Result<Self>{Self::new(profile,device,SecretKind::DeviceSigning,device)}
    pub fn new_pair(profile:Uuid,device:Uuid,pair:Uuid)->Result<Self>{Self::new(profile,device,SecretKind::PairEncryption,pair)}
    fn new(profile:Uuid,device:Uuid,kind:SecretKind,id:Uuid)->Result<Self>{
        let address=Self{profile,device,kind,id,generation:Uuid::new_v4()};address.validate()?;Ok(address)
    }
    pub fn profile(&self)->Uuid{self.profile}
    pub fn device(&self)->Uuid{self.device}
    pub fn id(&self)->Uuid{self.id}
    pub fn generation(&self)->Uuid{self.generation}
    pub fn kind(&self)->SecretKind{self.kind}
    pub fn validate(&self)->Result<()>{
        if self.profile.is_nil()||self.device.is_nil()||self.id.is_nil()||self.generation.is_nil()||
            self.kind==SecretKind::DeviceSigning&&self.id!=self.device{return Err(invalid());}Ok(())
    }
    pub fn account(&self)->Result<String>{
        self.validate()?;let kind=match self.kind{SecretKind::DeviceSigning=>"identity",SecretKind::PairEncryption=>"pair"};
        Ok(format!("{}/{}/{}/{}/{}",self.profile,self.device,kind,self.id,self.generation))
    }
    fn tag(&self)->u8{match self.kind{SecretKind::DeviceSigning=>1,SecretKind::PairEncryption=>2}}
}

/// Trusted native provider only. Missing is explicit; a locked/failed/corrupt
/// vault must return an error, never None. Writes must persist before success.
/// None means no secret material, not necessarily no OS account: a provider may
/// retain an exact public retirement tombstone that blocks late candidate adds.
/// No implementation may fall back to renderer storage or a plaintext file.
pub trait PairingSecretStore:Send+Sync{
    fn read(&self,address:&CredentialAddress)->Result<Option<Zeroizing<Vec<u8>>>>;
    fn write(&self,address:&CredentialAddress,record:&[u8])->Result<()>;
    fn remove(&self,address:&CredentialAddress)->Result<()>;
}

pub struct NativeSigningIdentity{key:SigningKey}
impl NativeSigningIdentity{
    pub fn generate()->Result<Self>{
        let mut seed=Zeroizing::new([0u8;32]);
        getrandom::getrandom(&mut seed[..]).map_err(|_|vault_error())?;
        Ok(Self{key:SigningKey::from_bytes(&seed)})
    }
    pub fn public_key(&self)->String{URL_SAFE_NO_PAD.encode(self.key.verifying_key().as_bytes())}
    /// Native protocol bodies only. No arbitrary signing domain and no secret
    /// export. The product still needs document/session authority checks before
    /// exposing any higher-level operation using this signer.
    pub fn sign(&self,domain:SigningDomain,body:&[u8])->Result<SignedDocument>{
        let(prefix,maximum)=match domain{
            SigningDomain::Request=>(REQUEST_DOMAIN,MAX_REQUEST_BYTES),
            SigningDomain::Invite=>(INVITE_DOMAIN,4096),
            SigningDomain::Hello=>(HELLO_DOMAIN,4096),
            SigningDomain::Grant=>(GRANT_DOMAIN,8192),
        };
        if body.is_empty()||body.len()>maximum{return Err(invalid());}
        // Reject non-object protocol bodies without a serde_json::Value copy of
        // potentially sensitive invitation/request contents.
        validate_object(body)?;
        let mut message=Zeroizing::new(Vec::with_capacity(prefix.len()+body.len()));
        message.extend_from_slice(prefix);message.extend_from_slice(body);
        Ok(SignedDocument{payload:URL_SAFE_NO_PAD.encode(body),signature:URL_SAFE_NO_PAD.encode(self.key.sign(&message).to_bytes())})
    }
}
#[derive(Clone,Copy)]pub enum SigningDomain{Request,Invite,Hello,Grant}

pub(crate) fn validate_object(body:&[u8])->Result<()>{
    struct ObjectOnly;
    impl<'de> Visitor<'de> for ObjectOnly{
        type Value=();
        fn expecting(&self,out:&mut std::fmt::Formatter<'_>)->std::fmt::Result{out.write_str("a bounded protocol object")}
        fn visit_map<A:MapAccess<'de>>(self,mut map:A)->std::result::Result<(),A::Error>{
            while map.next_entry::<IgnoredAny,IgnoredAny>()?.is_some(){}Ok(())
        }
    }
    let mut parser=serde_json::Deserializer::from_slice(body);
    serde::Deserializer::deserialize_map(&mut parser,ObjectOnly).map_err(|_|invalid())?;
    parser.end().map_err(|_|invalid())
}

/// Created by the native signed-transcript exchange or validated vault load. This wrapper
/// cannot serialize, clone, debug-print or export its key to the renderer.
pub struct NativePairKey{bytes:Zeroizing<[u8;32]>}
impl NativePairKey{
    pub(crate) fn from_derived(bytes:Zeroizing<[u8;32]>)->Self{Self{bytes}}
    pub(crate) fn into_signal_key(self)->Result<ring::aead::LessSafeKey>{
        let key=ring::aead::UnboundKey::new(&ring::aead::AES_256_GCM,&self.bytes[..]).map_err(|_|vault_error())?;
        Ok(ring::aead::LessSafeKey::new(key))
    }
}

fn encode(address:&CredentialAddress,binding:&[u8;32],secret:&[u8;32])->Result<Zeroizing<Vec<u8>>>{
    address.validate()?;let mut record=Zeroizing::new(Vec::with_capacity(SECRET_RECORD_BYTES));
    record.extend_from_slice(MAGIC);record.push(address.tag());
    for id in [address.profile,address.device,address.id,address.generation]{record.extend_from_slice(id.as_bytes());}
    record.extend_from_slice(binding);record.extend_from_slice(secret);Ok(record)
}
fn decode_record(address:&CredentialAddress,binding:&[u8;32],record:&[u8])->Result<Zeroizing<[u8;32]>>{
    validate_secret_envelope(address,record)?;let mut offset=73;
    if &record[offset..offset+32]!=binding{return Err(corrupt());}offset+=32;
    let mut secret=Zeroizing::new([0u8;32]);secret.copy_from_slice(&record[offset..offset+32]);Ok(secret)
}
/// Provider-side shape/address admission only; binding/secret correctness still
/// requires the identity or signed-grant load path. No key bytes are returned.
pub fn validate_secret_envelope(address:&CredentialAddress,record:&[u8])->Result<()>{
    address.validate()?;
    if record.len()!=SECRET_RECORD_BYTES||&record[..8]!=MAGIC||record[8]!=address.tag(){return Err(corrupt());}
    let mut offset=9;
    for id in [address.profile,address.device,address.id,address.generation]{
        if &record[offset..offset+16]!=id.as_bytes(){return Err(corrupt());}offset+=16;
    }Ok(())
}
fn insert_candidate(store:&dyn PairingSecretStore,address:&CredentialAddress,record:&[u8])->Result<()>{
    // Caller must serialize lifecycle operations in its durable journal. Vault
    // providers are not CAS stores. Each candidate gets a fresh generation;
    // an existing/unknown address is never silently overwritten or regenerated.
    if store.read(address)?.is_some(){return Err(PlatformError::new(ErrorCode::Conflict,"The pairing credential generation already exists"));}
    store.write(address,record)?;
    let observed=store.read(address)?.ok_or_else(vault_error)?;
    if observed.as_slice()!=record{return Err(corrupt());}Ok(())
}
pub fn store_identity_candidate(store:&dyn PairingSecretStore,address:&CredentialAddress,identity:&NativeSigningIdentity)->Result<()>{
    if address.kind!=SecretKind::DeviceSigning{return Err(invalid());}
    let seed=Zeroizing::new(identity.key.to_bytes());
    let record=encode(address,identity.key.verifying_key().as_bytes(),&seed)?;
    insert_candidate(store,address,&record)
}
pub fn load_identity(store:&dyn PairingSecretStore,address:&CredentialAddress,expected_public:&str)->Result<NativeSigningIdentity>{
    if address.kind!=SecretKind::DeviceSigning{return Err(invalid());}
    address.validate()?;let expected=key(expected_public)?;
    let record=store.read(address)?.ok_or_else(missing)?;
    let seed=decode_record(address,expected.as_bytes(),&record)?;
    let identity=NativeSigningIdentity{key:SigningKey::from_bytes(&seed)};
    if identity.key.verifying_key().as_bytes()!=expected.as_bytes(){return Err(corrupt());}Ok(identity)
}
fn pair_binding(address:&CredentialAddress,document:&SignedDocument,audience:&str,local_public:&str,now:u64)->Result<[u8;32]>{
    address.validate()?;if address.kind!=SecretKind::PairEncryption{return Err(invalid());}
    let grant=verify_grant(document,audience,now)?;key(local_public)?;
    if grant.id!=address.id||(local_public!=grant.host_public_key&&local_public!=grant.client_public_key){return Err(crate::unauthorized());}
    // Bind to the complete signed grant and this device's pinned public key.
    // Not a substitute for deriving the key from the authenticated ECDH exchange.
    let mut hash=Sha256::new();hash.update(b"vlikeon.native.pair-secret.v1\0");
    hash.update(document_hash(document)?.as_bytes());hash.update(decode(local_public,32)?);
    Ok(hash.finalize().into())
}
pub fn store_pair_candidate(store:&dyn PairingSecretStore,address:&CredentialAddress,document:&SignedDocument,audience:&str,local_public:&str,now:u64,key:&NativePairKey)->Result<()>{
    let started=Instant::now();
    let binding=pair_binding(address,document,audience,local_public,now)?;
    let record=encode(address,&binding,&key.bytes)?;insert_candidate(store,address,&record)?;
    pair_binding(address,document,audience,local_public,after_io(now,started)?)?;Ok(())
}
pub fn load_pair(store:&dyn PairingSecretStore,address:&CredentialAddress,document:&SignedDocument,audience:&str,local_public:&str,now:u64)->Result<NativePairKey>{
    let started=Instant::now();
    let binding=pair_binding(address,document,audience,local_public,now)?;
    let record=store.read(address)?.ok_or_else(missing)?;
    pair_binding(address,document,audience,local_public,after_io(now,started)?)?;
    Ok(NativePairKey{bytes:decode_record(address,&binding,&record)?})
}
fn after_io(now:u64,started:Instant)->Result<u64>{
    now.checked_add(u64::try_from(started.elapsed().as_millis()).map_err(|_|invalid())?).ok_or_else(invalid)
}
/// Material cleanup only, after the journal has fenced this generation and all
/// producers. Observed absence does not acknowledge remote/local revocation or
/// prove that an unretired concurrent writer cannot recreate an orphan record.
pub fn delete_material(store:&dyn PairingSecretStore,address:&CredentialAddress)->Result<()>{
    address.validate()?;store.remove(address)?;
    if store.read(address)?.is_some(){return Err(vault_error());}Ok(())
}
fn missing()->PlatformError{PlatformError::new(ErrorCode::NotFound,"The native Remote Play credential is absent").recover("Recover or pair this device again; do not replace its pinned identity automatically")}
fn corrupt()->PlatformError{PlatformError::new(ErrorCode::CorruptData,"The native Remote Play credential does not match its generation and binding")}
fn vault_error()->PlatformError{PlatformError::new(ErrorCode::PermissionDenied,"The native Remote Play credential vault is unavailable").recover("Unlock the device or credential vault and retry")}

#[derive(Default)]pub struct OsPairingSecretStore;
#[cfg(any(target_os="windows",target_os="macos",target_os="linux"))]
fn entry(address:&CredentialAddress)->Result<keyring::Entry>{keyring::Entry::new(SECRET_SERVICE,&address.account()?).map_err(|_|vault_error())}
#[cfg(any(target_os="windows",target_os="macos",target_os="linux"))]
fn retirement_entry(address:&CredentialAddress)->Result<keyring::Entry>{keyring::Entry::new("org.vlikeon.remote-play.retirement.v1",&address.account()?).map_err(|_|vault_error())}
#[cfg(any(target_os="windows",target_os="macos",target_os="linux"))]
fn retirement_marker(address:&CredentialAddress)->[u8;73]{
    let mut marker=[0u8;73];marker[..8].copy_from_slice(b"VLKRF001");marker[8]=address.tag();
    for(index,id)in [address.profile(),address.device(),address.id(),address.generation()].iter().enumerate(){marker[9+index*16..25+index*16].copy_from_slice(id.as_bytes());}marker
}
#[cfg(any(target_os="windows",target_os="macos",target_os="linux"))]
fn retirement_present(address:&CredentialAddress)->Result<bool>{
    match retirement_entry(address)?.get_secret(){
        Ok(bytes)=>{let bytes=Zeroizing::new(bytes);if bytes.as_slice()!=retirement_marker(address).as_slice(){return Err(corrupt());}Ok(true)},
        Err(keyring::Error::NoEntry)=>Ok(false),Err(_)=>Err(vault_error()),
    }
}
#[cfg(any(target_os="windows",target_os="macos",target_os="linux"))]
fn seal_retirement(address:&CredentialAddress)->Result<()>{
    // Serialized by the caller's retained credential lease, not backend CAS.
    if !retirement_present(address)?{retirement_entry(address)?.set_secret(&retirement_marker(address)).map_err(|_|vault_error())?;}
    if !retirement_present(address)?{return Err(vault_error());}Ok(())
}
#[cfg(any(target_os="windows",target_os="macos",target_os="linux"))]
fn retired()->PlatformError{PlatformError::new(ErrorCode::PermissionDenied,"The native credential generation is retired or has unfinished cleanup")}
impl PairingSecretStore for OsPairingSecretStore{
    fn read(&self,address:&CredentialAddress)->Result<Option<Zeroizing<Vec<u8>>>>{
        address.validate()?;
        #[cfg(any(target_os="windows",target_os="macos",target_os="linux"))]{
            let retiring=retirement_present(address)?;
            match entry(address)?.get_secret(){
                Ok(bytes)=>{let bytes=Zeroizing::new(bytes);validate_secret_envelope(address,&bytes)?;
                    if retiring||retirement_present(address)?{return Err(retired());}Ok(Some(bytes))},
                Err(keyring::Error::NoEntry)=>{let _=retirement_present(address)?;Ok(None)},Err(_)=>Err(vault_error()),
            }
        }
        #[cfg(not(any(target_os="windows",target_os="macos",target_os="linux")))]{Err(native_required())}
    }
    fn write(&self,address:&CredentialAddress,record:&[u8])->Result<()>{
        address.validate()?;validate_secret_envelope(address,record)?;
        #[cfg(any(target_os="windows",target_os="macos",target_os="linux"))]{
            if retirement_present(address)?{return Err(retired());}
            let secret=entry(address)?;
            match secret.get_secret(){Ok(bytes)=>{let _bytes=Zeroizing::new(bytes);return Err(retired());},Err(keyring::Error::NoEntry)=>{},Err(_)=>return Err(vault_error())}
            // Native lease serializes check/write; generic keyring set_secret
            // is not an atomic create-only API or a late OS-write barrier.
            if retirement_present(address)?{return Err(retired());}
            secret.set_secret(record).map_err(|_|vault_error())?;
            if retirement_present(address)?{return Err(retired());}Ok(())
        }
        #[cfg(not(any(target_os="windows",target_os="macos",target_os="linux")))]{Err(native_required())}
    }
    fn remove(&self,address:&CredentialAddress)->Result<()>{
        address.validate()?;
        #[cfg(any(target_os="windows",target_os="macos",target_os="linux"))]{
            seal_retirement(address)?;
            let secret=entry(address)?;
            match secret.delete_credential(){Ok(())|Err(keyring::Error::NoEntry)=>{},Err(_)=>return Err(vault_error())}
            match secret.get_secret(){Err(keyring::Error::NoEntry)=>{},Ok(bytes)=>{let _bytes=Zeroizing::new(bytes);return Err(vault_error());},Err(_)=>return Err(vault_error())}
            if !retirement_present(address)?{return Err(vault_error());}Ok(())
        }
        #[cfg(not(any(target_os="windows",target_os="macos",target_os="linux")))]{Err(native_required())}
    }
}
#[cfg(not(any(target_os="windows",target_os="macos",target_os="linux")))]
fn native_required()->PlatformError{PlatformError::new(ErrorCode::UnsupportedCapability,"Native Remote Play requires this application's scoped mobile credential provider")}
