use crate::{registry::RuntimePackage,unavailable};
use omniplay_contracts::{ContentHash,Result};
use serde::{Deserialize,Serialize};

#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MacPackage {
    pub owner_signing_identifier:String,
    pub actor_signing_identifier:String,
    pub presentation_signing_identifier:String,
    pub team_identifier:String,
    pub owner_entitlements_sha256:ContentHash,
    pub actor_entitlements_sha256:ContentHash,
    pub presentation_entitlements_sha256:ContentHash,
    pub hardened_runtime:bool,
    pub app_sandbox:bool,
    pub children_inherit_sandbox:bool,
    pub network_client:bool,
    pub network_server:bool,
    pub jit:bool,
    pub unsigned_executable_memory:bool,
    pub disable_library_validation:bool,
}

fn identifier(value:&str)->bool{
    value.len()<=160&&!value.is_empty()&&!value.starts_with('.')&&!value.ends_with('.')&&value.split('.').count()>=3&&value.split('.').all(|part|!part.is_empty()&&part.len()<=63&&part.bytes().all(|byte|byte.is_ascii_alphanumeric()||byte==b'-'))
}
impl MacPackage {
    pub fn validate(&self,package:&RuntimePackage)->Result<()> {
        if package.manifest.platform!="macos-aarch64"||!identifier(&self.owner_signing_identifier)||!identifier(&self.actor_signing_identifier)||!identifier(&self.presentation_signing_identifier)||self.owner_signing_identifier==self.actor_signing_identifier||self.owner_signing_identifier==self.presentation_signing_identifier||self.actor_signing_identifier==self.presentation_signing_identifier||self.team_identifier.len()!=10||!self.team_identifier.bytes().all(|byte|byte.is_ascii_uppercase()||byte.is_ascii_digit())||!self.hardened_runtime||!self.app_sandbox||!self.children_inherit_sandbox||self.network_client||self.network_server||self.jit||self.unsigned_executable_memory||self.disable_library_validation{return Err(unavailable("The macOS native package exceeds the closed App Sandbox policy"));}
        ContentHash::parse(self.owner_entitlements_sha256.0.clone()).map_err(|_|unavailable("The macOS owner entitlement identity is invalid"))?;
        ContentHash::parse(self.actor_entitlements_sha256.0.clone()).map_err(|_|unavailable("The macOS actor entitlement identity is invalid"))?;
        ContentHash::parse(self.presentation_entitlements_sha256.0.clone()).map_err(|_|unavailable("The macOS presentation entitlement identity is invalid"))?;
        let presentation=package.presentation.as_ref().ok_or_else(||unavailable("macOS native execution requires the separate presentation process"))?;
        if package.worker==presentation.executable||package.video_backend!=crate::registry::VideoBackend::Software{return Err(unavailable("The macOS actor must use the private software presentation boundary"));}
        Ok(())
    }
}
