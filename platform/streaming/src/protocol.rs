use crate::{invalid,unauthorized,validate_audience};
use base64::{Engine,engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{Signature,VerifyingKey};
use omniplay_contracts::{ContentHash,Result};
use serde::{Deserialize,Serialize,de::DeserializeOwned};
use sha2::{Digest,Sha256};
use uuid::Uuid;

pub const REQUEST_DOMAIN:&[u8]=b"omniplay.streaming.request.v1\0";
pub const INVITE_DOMAIN:&[u8]=b"omniplay.streaming.invite.v1\0";
pub const HELLO_DOMAIN:&[u8]=b"omniplay.streaming.hello.v1\0";
pub const GRANT_DOMAIN:&[u8]=b"omniplay.streaming.grant.v1\0";
pub const MAX_REQUEST_BYTES:usize=96*1024;
pub const MAX_PACKET_BYTES:usize=48*1024;
pub const MAX_PAIR_LIFETIME:u64=90*24*60*60*1000;

#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedDocument {pub payload:String,pub signature:String}
pub fn decode(value:&str,max:usize)->Result<Vec<u8>>{
    if value.len()>max.saturating_mul(4).saturating_add(2)/3||value.is_empty(){return Err(invalid());}
    let data=URL_SAFE_NO_PAD.decode(value).map_err(|_|invalid())?;
    if data.len()>max||URL_SAFE_NO_PAD.encode(&data)!=value{return Err(invalid());}Ok(data)
}
pub fn key(value:&str)->Result<VerifyingKey>{let data=decode(value,32)?;let bytes:[u8;32]=data.try_into().map_err(|_|invalid())?;let key=VerifyingKey::from_bytes(&bytes).map_err(|_|invalid())?;if key.is_weak(){return Err(invalid());}Ok(key)}
pub fn key_id(value:&str)->Result<String>{key(value)?;let bytes=decode(value,32)?;Ok(format!("{:x}",Sha256::digest(bytes)))}
pub fn document_hash(document:&SignedDocument)->Result<String>{let raw=serde_json::to_vec(document).map_err(|_|invalid())?;Ok(format!("{:x}",Sha256::digest(raw)))}
pub fn verify<T:DeserializeOwned>(public_key:&str,domain:&[u8],document:&SignedDocument,max:usize)->Result<T>{
    let data=decode(&document.payload,max)?;let signature=Signature::from_slice(&decode(&document.signature,64)?).map_err(|_|unauthorized())?;
    let mut signed=Vec::with_capacity(domain.len()+data.len());signed.extend_from_slice(domain);signed.extend_from_slice(&data);
    key(public_key)?.verify_strict(&signed,&signature).map_err(|_|unauthorized())?;serde_json::from_slice(&data).map_err(|_|invalid())
}
pub fn inspect<T:DeserializeOwned>(document:&SignedDocument,max:usize)->Result<T>{serde_json::from_slice(&decode(&document.payload,max)?).map_err(|_|invalid())}

#[derive(Clone,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct Invite {pub schema_version:u32,pub id:Uuid,pub audience:String,pub host_public_key:String,pub label:String,pub secret_hash:ContentHash,pub created_at_ms:u64,pub expires_at_ms:u64}
impl Invite {pub fn validate(&self,audience:&str,now:u64)->Result<()>{
    if self.schema_version!=1||self.audience!=audience||self.created_at_ms>now.saturating_add(30_000)||self.expires_at_ms<=now||self.expires_at_ms<=self.created_at_ms||self.expires_at_ms-self.created_at_ms>300_000||self.label.is_empty()||self.label.len()>120||self.label.chars().any(char::is_control){return Err(invalid());}key(&self.host_public_key)?;ContentHash::parse(self.secret_hash.0.clone())?;Ok(())
}}
#[derive(Clone,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct ClientHello {pub schema_version:u32,pub invite_id:Uuid,pub client_public_key:String,pub client_ecdh:String,pub client_nonce:String,pub label:String,pub created_at_ms:u64}
impl ClientHello {pub fn validate(&self,invite_id:Uuid,now:u64)->Result<()>{
    if self.schema_version!=1||self.invite_id!=invite_id||self.created_at_ms.abs_diff(now)>300_000||self.label.is_empty()||self.label.len()>120||self.label.chars().any(char::is_control){return Err(invalid());}key(&self.client_public_key)?;ecdh(&self.client_ecdh)?;if decode(&self.client_nonce,32)?.len()!=32{return Err(invalid());}Ok(())
}}
fn ecdh(value:&str)->Result<()>{let bytes=decode(value,65)?;if bytes.len()!=65||bytes[0]!=4{return Err(invalid());}Ok(())}

#[derive(Clone,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct PairGrant {
    pub schema_version:u32,pub id:Uuid,pub invite_id:Uuid,pub audience:String,pub host_public_key:String,pub client_public_key:String,
    pub client_hello_hash:ContentHash,pub client_ecdh:String,pub host_ecdh:String,pub client_nonce:String,pub host_nonce:String,
    pub input_allowed:bool,pub created_at_ms:u64,pub expires_at_ms:u64,
}
impl PairGrant {pub fn validate(&self,audience:&str,now:u64)->Result<()>{
    if self.schema_version!=1||self.audience!=audience||self.created_at_ms>now.saturating_add(30_000)||self.expires_at_ms<=now||self.expires_at_ms<=self.created_at_ms||self.expires_at_ms-self.created_at_ms>MAX_PAIR_LIFETIME||self.host_public_key==self.client_public_key{return Err(invalid());}
    key(&self.host_public_key)?;key(&self.client_public_key)?;ecdh(&self.host_ecdh)?;ecdh(&self.client_ecdh)?;ContentHash::parse(self.client_hello_hash.0.clone())?;
    if decode(&self.client_nonce,32)?.len()!=32||decode(&self.host_nonce,32)?.len()!=32{return Err(invalid());}Ok(())
}}
pub fn verify_grant(document:&SignedDocument,audience:&str,now:u64)->Result<PairGrant>{let untrusted:PairGrant=inspect(document,8192)?;let grant:PairGrant=verify(&untrusted.host_public_key,GRANT_DOMAIN,document,8192)?;grant.validate(audience,now)?;Ok(grant)}

/// Retirement metadata only. Never use to authorize media, ICE, signal send,
/// receive, approval or activation. Expiry does not invalidate the signature's
/// binding needed to retire an old grant, but future-created grants are refused.
pub fn verify_grant_for_retirement(document:&SignedDocument,audience:&str,now:u64)->Result<PairGrant>{
    validate_audience(audience)?;
    let untrusted:PairGrant=inspect(document,8192)?;
    let grant:PairGrant=verify(&untrusted.host_public_key,GRANT_DOMAIN,document,8192)?;
    grant.validate(audience,grant.created_at_ms)?;
    if grant.id.is_nil()||grant.invite_id.is_nil()||grant.created_at_ms>now.saturating_add(30_000){return Err(unauthorized());}Ok(grant)
}

#[derive(Clone,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct RelayEnvelope {pub public_key:String,pub document:SignedDocument}
#[derive(Clone,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct RelayRequest {pub schema_version:u32,pub audience:String,pub issued_at_ms:u64,pub nonce:Uuid,pub operation:RelayOperation}
#[derive(Clone,Serialize,Deserialize)]#[serde(tag="type",rename_all="snake_case",deny_unknown_fields)]
pub enum RelayOperation {
    CreateInvite {invite:SignedDocument},
    JoinInvite {invite_id:Uuid,secret:String,hello:SignedDocument},
    PollInvite {invite_id:Uuid},
    PairingStatus {invite_id:Uuid},
    ApprovePair {invite_id:Uuid,client_public_key:String,grant:SignedDocument},
    DeclinePair {invite_id:Uuid,client_public_key:String},
    CloseInvite {invite_id:Uuid},
    Send {grant:SignedDocument,message_id:Uuid,session_id:Uuid,sequence:u64,expires_at_ms:u64,iv:String,ciphertext:String},
    Receive {grant:SignedDocument,after:u64,limit:u16},
    IceCredentials {grant:SignedDocument},
    ConfirmPair {grant:SignedDocument},
    RevokePair {grant:SignedDocument},
}
pub fn verify_request(envelope:&RelayEnvelope,audience:&str,now:u64)->Result<RelayRequest>{
    validate_audience(audience)?;let request:RelayRequest=verify(&envelope.public_key,REQUEST_DOMAIN,&envelope.document,MAX_REQUEST_BYTES)?;
    if request.schema_version!=1||request.audience!=audience||request.issued_at_ms.abs_diff(now)>120_000{return Err(unauthorized());}
    match &request.operation {
        RelayOperation::Send{sequence,expires_at_ms,iv,ciphertext,..}=>{if *sequence==0||*sequence>1_000_000||*expires_at_ms<=now||*expires_at_ms>now.saturating_add(60_000)||decode(iv,12)?.len()!=12||decode(ciphertext,MAX_PACKET_BYTES)?.len()<16{return Err(invalid());}},
        RelayOperation::Receive{after,limit,..}=>if *after>9_007_199_254_740_991||*limit==0||*limit>8{return Err(invalid());},
        _=>{},
    }Ok(request)
}

#[derive(Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct RelayPacket {pub cursor:u64,pub sender_public_key:String,pub message_id:Uuid,pub session_id:Uuid,pub sequence:u64,pub expires_at_ms:u64,pub iv:String,pub ciphertext:String}
