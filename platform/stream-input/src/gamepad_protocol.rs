//! Versioned private parent/helper pipe. Never expose these commands to a
//! renderer or accept a renderer-created Owner as native authorization.
use crate::gamepad::{Owner,TargetBinding};
use omniplay_contracts::RemotePad;
use serde::{Deserialize,Serialize};
use uuid::Uuid;

pub const VERSION:u16=2;
pub const MAX_ID:u64=10_000_000;
#[derive(Clone,Copy,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct Binding{
    pub grant:Uuid,pub game:Uuid,pub pair:Uuid,pub session:Uuid,pub capture:Uuid,
    pub epoch:u32,pub process_id:u32,pub creation_time:u64,pub window:u64,
}
impl Binding{
    pub fn owner(self)->Owner{Owner{grant:self.grant,game:self.game,pair:self.pair,
        session:self.session,capture:self.capture,epoch:self.epoch,
        target:TargetBinding{process_id:self.process_id,creation_time:self.creation_time,window:self.window}}}
}
impl From<Owner> for Binding{
    fn from(owner:Owner)->Self{Self{grant:owner.grant,game:owner.game,pair:owner.pair,
        session:owner.session,capture:owner.capture,epoch:owner.epoch,
        process_id:owner.target.process_id,creation_time:owner.target.creation_time,window:owner.target.window}}
}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct Identity{pub slot:u8,pub container:Uuid}
#[derive(Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct Pad{pub slot:u8,pub state:RemotePad}
#[derive(Serialize,Deserialize)]#[serde(tag="method",rename_all="snake_case",deny_unknown_fields)]
pub enum Action{
    /// These parameters must come from admitted native package/consent, not RPC
    /// forwarding of a peer-selected provider, identity or device permission.
    Prepare{slots:Vec<u8>,remaining_ms:u32,expires_at_ms:u64,system_visible_consent:bool,vendor:u16,product:u16},
    /// Renewed by the parent reader independently of provider command execution.
    /// Heartbeats have no reply and still consume the global request-id space.
    Heartbeat{document_alive:bool,capture_alive:bool,pair_authorized:bool},
    Activate{identities:Vec<Identity>},
    Input{sequence:u64,pads:Vec<Pad>},Release{sequence:u64},Status,Stop,
}
#[derive(Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct Request{pub protocol:u16,pub id:u64,pub binding:Binding,pub action:Action}
#[derive(Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct Reply{
    pub protocol:u16,pub id:u64,pub binding:Binding,pub status:String,
    /// Prepare first reports waiting with complete=false, then exactly one
    /// final prepared/error receipt for that id. Other commands complete once.
    pub complete:bool,
    pub phase:String,pub reason:Option<String>,
    /// Prepared/active or retained-cleanup set. A parent must validate this set
    /// against its earlier prepared record before accepting removal receipts.
    /// None means unknown, never an empty/fully removed set.
    pub identities:Option<Vec<Identity>>,
}
