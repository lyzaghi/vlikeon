use crate::{ErrorCode,PlatformError,Result};
use serde::{Serialize,Deserialize};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Debug,Clone,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct StreamGrantDocument{pub payload:String,pub signature:String}
#[derive(Debug,Clone,Default,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct RemoteMouse{pub dx:i32,pub dy:i32,pub wheel:i32,pub buttons:u8}
#[derive(Debug,Clone,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct RemotePad{pub axes:[f64;6],pub buttons:u16}
#[derive(Debug,Clone,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct RemoteInputFrame{pub schema_version:u32,pub keys:Vec<String>,pub mouse:RemoteMouse,pub pads:Vec<RemotePad>}
pub fn remote_key(value:&str)->bool{
    matches!(value,"ArrowUp"|"ArrowDown"|"ArrowLeft"|"ArrowRight"|"Space"|"Enter"|"Tab"|"Escape"|"Backspace"|"ShiftLeft"|"ShiftRight")||
    value.strip_prefix("Key").is_some_and(|v|v.len()==1&&v.bytes().all(|b|b.is_ascii_uppercase()))||value.strip_prefix("Digit").is_some_and(|v|v.len()==1&&v.bytes().all(|b|b.is_ascii_digit()))
}
impl RemoteInputFrame{
    pub fn validate(&self)->Result<()>{
        let unique:BTreeSet<_>=self.keys.iter().collect();
        if self.schema_version!=1||self.keys.len()>32||unique.len()!=self.keys.len()||self.keys.iter().any(|v|!remote_key(v))||self.mouse.buttons>7||[self.mouse.dx,self.mouse.dy,self.mouse.wheel].iter().any(|n|!(-2048..=2048).contains(n))||self.pads.len()>4||self.pads.iter().flat_map(|p|p.axes).any(|n|!n.is_finite()||!(-1.0..=1.0).contains(&n)){return Err(PlatformError::new(ErrorCode::InvalidRequest,"Remote input state exceeds its bounded capability"));}Ok(())
    }
}
#[derive(Debug,Clone,Copy,Serialize,Deserialize,PartialEq,Eq)]#[serde(rename_all="snake_case")]
pub enum StreamInputState{WaitingForTarget,Active,Idle,Revoked}
#[derive(Debug,Clone,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct StreamInputStatus{pub grant_id:Uuid,pub game_id:Uuid,pub pair_id:Uuid,pub session_id:Uuid,pub state:StreamInputState,pub reason:Option<String>,pub keyboard_mouse:bool,pub gamepad:bool,pub expires_at_ms:u64}
