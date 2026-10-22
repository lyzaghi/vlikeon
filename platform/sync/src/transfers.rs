use crate::BlobHeader;
use serde::{Deserialize,Serialize};
use uuid::Uuid;

pub const TRANSFER_PROTOCOL:u32=1;
pub const TRANSFER_CHUNK_BYTES:u32=256*1024;
pub const TRANSFER_TTL_MS:u64=24*60*60*1000;
pub const MAX_ACCOUNT_TRANSFERS:u32=4;
pub const MAX_SERVICE_TRANSFERS:u32=32;

#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransferCapabilities{pub protocol:u32,pub chunk_bytes:u32,pub max_object_bytes:u64,pub expiry_ms:u64}
impl Default for TransferCapabilities{fn default()->Self{Self{protocol:TRANSFER_PROTOCOL,chunk_bytes:TRANSFER_CHUNK_BYTES,max_object_bytes:crate::MAX_CIPHERTEXT_BYTES as u64,expiry_ms:TRANSFER_TTL_MS}}}

#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UploadProgress{pub protocol:u32,pub transfer_id:Uuid,pub header:BlobHeader,pub received_bytes:u64,pub stored:bool,pub expires_at_ms:u64}

pub fn chunk_length(total:u64,offset:u64)->Option<u32>{
    if offset>=total||offset%u64::from(TRANSFER_CHUNK_BYTES)!=0{return None;}
    Some((total-offset).min(u64::from(TRANSFER_CHUNK_BYTES)) as u32)
}
