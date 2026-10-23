use omniplay_contracts::RemoteInputFrame;
use serde::{Serialize,Deserialize};
use std::io::{Read,Write};
use uuid::Uuid;
pub const MAX_FRAME:usize=16*1024;
#[derive(Clone,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct Target{pub process_id:u32,pub creation_time:u64,#[serde(default)]pub window:Option<u64>}
#[derive(Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct Request{pub protocol:u16,pub id:u64,pub grant_id:Uuid,pub session_id:Uuid,pub action:Action}
#[derive(Serialize,Deserialize)]#[serde(tag="method",rename_all="snake_case",deny_unknown_fields)]
pub enum Action{Prepare{target:Target,expires_at_ms:u64},Input{sequence:u64,frame:RemoteInputFrame},Release,Status,Stop}
#[derive(Clone,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct Observation{pub state:String,pub reason:Option<String>}
#[derive(Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct Reply{pub protocol:u16,pub id:u64,pub status:String,pub observation:Observation}
pub fn read<T:serde::de::DeserializeOwned>(input:&mut impl Read)->std::io::Result<Option<T>>{
    let mut length=[0u8;4];let count=input.read(&mut length[..1])?;if count==0{return Ok(None);}input.read_exact(&mut length[1..])?;let count=u32::from_le_bytes(length)as usize;
    if count==0||count>MAX_FRAME{return Err(std::io::ErrorKind::InvalidData.into());}let mut bytes=vec![0;count];input.read_exact(&mut bytes)?;serde_json::from_slice(&bytes).map(Some).map_err(|_|std::io::ErrorKind::InvalidData.into())
}
pub fn write(value:&impl Serialize,output:&mut impl Write)->std::io::Result<()>{let bytes=serde_json::to_vec(value).map_err(|_|std::io::ErrorKind::InvalidData)?;if bytes.is_empty()||bytes.len()>MAX_FRAME{return Err(std::io::ErrorKind::InvalidData.into());}output.write_all(&(bytes.len()as u32).to_le_bytes())?;output.write_all(&bytes)?;output.flush()}
