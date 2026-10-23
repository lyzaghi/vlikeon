use crate::{registry::RuntimePackage, unavailable};
use omniplay_contracts::{ContentHash, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MacPackage {
    pub owner_signing_identifier: String,
    pub presentation_signing_identifier: String,
    pub team_identifier: String,
    pub owner_entitlements_sha256: ContentHash,
    pub presentation_entitlements_sha256: ContentHash,
    pub hardened_runtime: bool,
    pub app_sandbox: bool,
    pub children_inherit_sandbox: bool,
    pub virtualization: bool,
    pub guest_protocol: u32,
    pub guest_kernel: String,
    pub guest_initrd: String,
    pub guest_cpu_count: u32,
    pub scratch_memory_mb: u32,
    pub network_client: bool,
    pub network_server: bool,
    pub jit: bool,
    pub unsigned_executable_memory: bool,
    pub disable_library_validation: bool,
}
fn identifier(value: &str) -> bool {
    value.len() <= 160 && !value.is_empty() && !value.starts_with('.') && !value.ends_with('.')
        && value.split('.').count() >= 3
        && value.split('.').all(|part| !part.is_empty() && part.len() <= 63
            && part.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'))
}
impl MacPackage {
    pub fn validate(&self, package: &RuntimePackage) -> Result<()> {
        let invalid = || unavailable("The macOS package exceeds the closed diskless guest policy");
        if package.manifest.platform != "macos-aarch64" || !identifier(&self.owner_signing_identifier)
            || !identifier(&self.presentation_signing_identifier)
            || self.owner_signing_identifier == self.presentation_signing_identifier
            || self.team_identifier.len() != 10
            || !self.team_identifier.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
            || !self.hardened_runtime || !self.app_sandbox || !self.children_inherit_sandbox
            || !self.virtualization || self.guest_protocol != 1 || self.network_client || self.network_server
            || self.jit || self.unsigned_executable_memory || self.disable_library_validation
            || !(1..=4).contains(&self.guest_cpu_count)
            || !(16..=512).contains(&self.scratch_memory_mb)
            || self.scratch_memory_mb > package.memory_limit_mb / 2 {
            return Err(invalid());
        }
        ContentHash::parse(self.owner_entitlements_sha256.0.clone()).map_err(|_| invalid())?;
        ContentHash::parse(self.presentation_entitlements_sha256.0.clone()).map_err(|_| invalid())?;
        let helper = package.presentation.as_ref().ok_or_else(invalid)?;
        if package.video_backend != crate::registry::VideoBackend::Software { return Err(invalid()); }
        let paths = [&package.worker, &package.core_library, &helper.executable,
            &self.guest_kernel, &self.guest_initrd];
        let unique: std::collections::BTreeSet<_> = paths.into_iter().collect();
        if unique.len() != 5 { return Err(invalid()); }
        let prefix = format!("native-runtimes/{}/", package.manifest.id);
        for (path, maximum) in [(&self.guest_kernel, 128u64 * 1024 * 1024),
            (&self.guest_initrd, (256u64 * 1024 * 1024).min(u64::from(package.memory_limit_mb) * 1024 * 1024 / 2))] {
            let relative = path.strip_prefix(&prefix).ok_or_else(invalid)?;
            if relative.is_empty() || relative.contains('/') || relative.starts_with('.') { return Err(invalid()); }
            let file = package.files.iter().find(|file| &file.path == path).ok_or_else(invalid)?;
            if file.bytes < 64 || file.bytes > maximum { return Err(invalid()); }
        }
        Ok(())
    }
}
