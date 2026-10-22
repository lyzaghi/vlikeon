use crate::{BlobHeader,error};
use base64::{Engine,engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{Signature,Signer,SigningKey,VerifyingKey};
use omniplay_contracts::Result;
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
use uuid::Uuid;

pub const CERTIFICATE_DOMAIN:&[u8]=b"OmniPlay device certificate v1\0";
pub const REQUEST_DOMAIN:&[u8]=b"OmniPlay device request v1\0";
pub const ROOT_COMMAND_DOMAIN:&[u8]=b"OmniPlay account command v1\0";
pub const MAX_SIGNED_BYTES:usize=16*1024;

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedDocument {pub payload_base64:String,pub signature_base64:String}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceCertificate {pub schema_version:u32,pub account_id:String,pub device_id:Uuid,pub device_public_key:String,pub audience:String,pub issued_at_ms:u64,pub expires_at_ms:u64,pub serial:Uuid}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthenticatedRequest {pub certificate:SignedDocument,pub request:SignedDocument}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncRequest {pub schema_version:u32,pub account_id:String,pub device_id:Uuid,pub audience:String,pub nonce:Uuid,pub issued_at_ms:u64,pub operation:SyncOperation}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(tag="method",content="params",rename_all="snake_case",deny_unknown_fields)]
pub enum SyncOperation {
    Put{header:BlobHeader},Get{object_id:Uuid},List{after_sequence:u64,limit:u16},Devices,
    TransferCapabilities,
    UploadBegin{transfer_id:Uuid,header:BlobHeader},
    UploadChunk{transfer_id:Uuid,offset:u64,bytes:u32,sha256:String},
    UploadCommit{transfer_id:Uuid,header:BlobHeader},
    UploadAbort{transfer_id:Uuid},
    ObjectHeader{object_id:Uuid},
    ObjectChunk{object_id:Uuid,offset:u64,bytes:u32},
}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootCommand {pub schema_version:u32,pub account_id:String,pub audience:String,pub nonce:Uuid,pub issued_at_ms:u64,pub operation:RootOperation}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(tag="method",content="params",rename_all="snake_case",deny_unknown_fields)]
pub enum RootOperation {Create{root_public_key:String},RevokeDevice{device_id:Uuid},DeleteAccount}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootRequest {pub root_public_key:String,pub command:SignedDocument}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectSummary {pub object_id:Uuid,pub sequence:u64,pub encrypted_bytes:u64,pub uploaded_at_ms:u64}

pub fn valid_account_id(value:&str)->bool {value.len()==64&&value.bytes().all(|byte|byte.is_ascii_digit()||(b'a'..=b'f').contains(&byte))}

pub fn validate_audience(audience:&str)->Result<()> {
    if audience.len()>512||!audience.starts_with("https://")||audience.contains(['@','?','#','\\'])||audience.chars().any(char::is_control)||audience.ends_with('/') {return Err(error("Sync audience must be a canonical HTTPS service address without credentials or trailing slash"));}
    let authority=audience[8..].split('/').next().unwrap_or("");
    if authority.is_empty()||authority.contains(' ')||authority.starts_with('.') {return Err(error("Sync service address is invalid"));}
    Ok(())
}

pub fn parse_key(encoded:&str)->Result<VerifyingKey> {
    if encoded.len()!=43{return Err(error("Signing public key length is invalid"));}
    let bytes:[u8;32]=URL_SAFE_NO_PAD.decode(encoded).map_err(|_|error("Signing public key encoding is invalid"))?.try_into().map_err(|_|error("Signing public key length is invalid"))?;
    VerifyingKey::from_bytes(&bytes).map_err(|_|error("Signing public key is invalid"))
}

pub fn account_id_for_key(encoded:&str)->Result<String> {Ok(format!("{:x}",Sha256::digest(parse_key(encoded)?.as_bytes())))}

pub fn verify_document(key:&VerifyingKey,domain:&[u8],document:&SignedDocument,limit:usize)->Result<Vec<u8>> {
    if limit>MAX_SIGNED_BYTES||document.payload_base64.len()>limit.saturating_mul(4)/3+8||document.signature_base64.len()!=86{return Err(error("Signed sync document exceeds policy"));}
    let payload=URL_SAFE_NO_PAD.decode(&document.payload_base64).map_err(|_|error("Signed sync payload encoding is invalid"))?;
    if payload.is_empty()||payload.len()>limit{return Err(error("Signed sync payload length is invalid"));}
    let signature=URL_SAFE_NO_PAD.decode(&document.signature_base64).map_err(|_|error("Sync signature encoding is invalid"))?;
    let signature=Signature::from_slice(&signature).map_err(|_|error("Sync signature length is invalid"))?;
    let mut message=Vec::with_capacity(domain.len()+payload.len());message.extend_from_slice(domain);message.extend_from_slice(&payload);
    key.verify_strict(&message,&signature).map_err(|_|error("Sync request signature is not trusted"))?;
    Ok(payload)
}

pub(crate) fn sign_document(key:&SigningKey,domain:&[u8],payload:&[u8])->Result<SignedDocument> {
    if payload.is_empty()||payload.len()>MAX_SIGNED_BYTES{return Err(error("Sync document exceeds its size limit"));}
    let mut message=Vec::with_capacity(domain.len()+payload.len());message.extend_from_slice(domain);message.extend_from_slice(payload);
    Ok(SignedDocument{payload_base64:URL_SAFE_NO_PAD.encode(payload),signature_base64:URL_SAFE_NO_PAD.encode(key.sign(&message).to_bytes())})
}

pub fn verify_device_request(root_public_key:&str,request:&AuthenticatedRequest,audience:&str,now:u64)->Result<(DeviceCertificate,SyncRequest)> {
    validate_audience(audience)?;
    let certificate_bytes=verify_document(&parse_key(root_public_key)?,CERTIFICATE_DOMAIN,&request.certificate,8192)?;
    let certificate:DeviceCertificate=serde_json::from_slice(&certificate_bytes).map_err(|_|error("Device certificate is invalid"))?;
    let account_id=account_id_for_key(root_public_key)?;
    if certificate.schema_version!=1||certificate.account_id!=account_id||certificate.audience!=audience||certificate.expires_at_ms<=now||certificate.issued_at_ms>now.saturating_add(120_000)||certificate.expires_at_ms<=certificate.issued_at_ms||certificate.expires_at_ms-certificate.issued_at_ms>90*24*60*60*1000{return Err(error("Device certificate expired or belongs to another account/service"));}
    let body=verify_document(&parse_key(&certificate.device_public_key)?,REQUEST_DOMAIN,&request.request,MAX_SIGNED_BYTES)?;
    let body:SyncRequest=serde_json::from_slice(&body).map_err(|_|error("Sync request is invalid"))?;
    if body.schema_version!=1||body.account_id!=account_id||body.device_id!=certificate.device_id||body.audience!=audience||body.issued_at_ms.abs_diff(now)>120_000{return Err(error("Sync request identity, service or timestamp is invalid"));}
    match &body.operation {
        SyncOperation::Put{header}|SyncOperation::UploadBegin{header,..}|SyncOperation::UploadCommit{header,..}=>{header.validate()?;if header.binding.account_id!=account_id{return Err(error("Save belongs to another account"));}},
        SyncOperation::UploadChunk{offset,bytes,sha256,..}=>{if *bytes==0||*bytes>crate::TRANSFER_CHUNK_BYTES||*offset>=crate::MAX_CIPHERTEXT_BYTES as u64||*offset%u64::from(crate::TRANSFER_CHUNK_BYTES)!=0{return Err(error("Upload chunk exceeds policy"));}omniplay_contracts::ContentHash::parse(sha256.clone())?;},
        SyncOperation::ObjectChunk{offset,bytes,..} if *bytes==0||*bytes>crate::TRANSFER_CHUNK_BYTES||*offset>=crate::MAX_CIPHERTEXT_BYTES as u64||*offset%u64::from(crate::TRANSFER_CHUNK_BYTES)!=0=>return Err(error("Download chunk exceeds policy")),
        SyncOperation::List{limit,..} if *limit==0||*limit>200=>return Err(error("Sync list page size is invalid")),
        _=>{},
    }
    Ok((certificate,body))
}

pub fn verify_root_request(request:&RootRequest,audience:&str,now:u64)->Result<RootCommand> {
    validate_audience(audience)?;
    let payload=verify_document(&parse_key(&request.root_public_key)?,ROOT_COMMAND_DOMAIN,&request.command,8192)?;
    let body:RootCommand=serde_json::from_slice(&payload).map_err(|_|error("Account command is invalid"))?;
    if body.schema_version!=1||body.account_id!=account_id_for_key(&request.root_public_key)?||body.audience!=audience||body.issued_at_ms.abs_diff(now)>120_000{return Err(error("Account command service, identity or timestamp is invalid"));}
    if let RootOperation::Create{root_public_key}=&body.operation {if root_public_key!=&request.root_public_key{return Err(error("Account creation key is inconsistent"));}}
    Ok(body)
}
