use serde::{Deserialize,Serialize};
use uuid::Uuid;

/// Native account worker input. Secrets are never copied into a status snapshot.
#[derive(Clone,Serialize,Deserialize)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
pub enum SyncAccountAction{
    Create{audience:String},Register{recovery_key:String,registration_token:String},
    Recover{audience:String,recovery_key:String},Renew{recovery_key:String},Devices,
    RevokeDevice{recovery_key:String,device_id:Uuid},DeleteAccount{recovery_key:String},Disconnect,
}
impl std::fmt::Debug for SyncAccountAction{fn fmt(&self,out:&mut std::fmt::Formatter<'_>)->std::fmt::Result{out.debug_struct("SyncAccountAction").field("kind",&self.kind()).finish_non_exhaustive()}}
impl SyncAccountAction{
    pub fn kind(&self)->&'static str{match self{Self::Create{..}=>"create",Self::Register{..}=>"register",Self::Recover{..}=>"recover",Self::Renew{..}=>"renew",Self::Devices=>"devices",Self::RevokeDevice{..}=>"revoke_device",Self::DeleteAccount{..}=>"delete_account",Self::Disconnect=>"disconnect"}}
}
