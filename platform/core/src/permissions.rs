use crate::{now_ms, util::storage_error};
use omniplay_contracts::{ErrorCode, PlatformError, Result};
use std::{collections::HashMap, fs::{File, OpenOptions}, path::Path};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileAccess { ImportRead, ExportWrite }

struct Grant { owner: Uuid, access: FileAccess, file: File, expires_at_ms: u64, single_use: bool }

#[derive(Default)]
pub struct PermissionBroker { grants: HashMap<Uuid, Grant> }

impl PermissionBroker {
    /// Called only by the authenticated host after its native file picker returns.
    /// Paths are never accepted in renderer protocol commands.
    pub fn grant_selected_file(&mut self, owner: Uuid, path: &Path, access: FileAccess, ttl_ms: u64) -> Result<Uuid> {
        self.expire();
        if self.grants.len() >= 128 || ttl_ms == 0 || ttl_ms > 300_000 {
            return Err(PlatformError::new(ErrorCode::ResourceLimit, "File permission limit exceeded"));
        }
        if path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
            return Err(PlatformError::new(ErrorCode::PermissionDenied, "Relative parent paths are not accepted"));
        }
        let mut options = OpenOptions::new();
        #[cfg(unix)]
        {use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
        match access { FileAccess::ImportRead => { options.read(true); }, FileAccess::ExportWrite => { options.write(true).create_new(true); } }
        let file = options.open(path).map_err(storage_error)?;
        if !file.metadata().map_err(storage_error)?.is_file() { return Err(PlatformError::new(ErrorCode::PermissionDenied, "Select a regular file")); }
        let id = Uuid::new_v4();
        self.grants.insert(id, Grant { owner, access, file, expires_at_ms: now_ms().saturating_add(ttl_ms), single_use: true });
        Ok(id)
    }

    pub fn take(&mut self, owner: Uuid, id: Uuid, access: FileAccess) -> Result<File> {
        self.expire();
        let grant = self.grants.get(&id).ok_or_else(|| PlatformError::new(ErrorCode::PermissionExpired, "File permission is missing or expired").recover("Choose the file again"))?;
        if grant.owner != owner || grant.access != access { return Err(PlatformError::new(ErrorCode::PermissionDenied, "File permission does not cover this operation")); }
        if grant.single_use { return Ok(self.grants.remove(&id).expect("grant was just found").file); }
        grant.file.try_clone().map_err(storage_error)
    }

    pub fn revoke(&mut self, owner: Uuid, id: Uuid) -> Result<()> {
        if self.grants.get(&id).is_some_and(|g| g.owner != owner) { return Err(PlatformError::new(ErrorCode::PermissionDenied, "Permission belongs to another client")); }
        self.grants.remove(&id);
        Ok(())
    }

    pub fn revoke_owner(&mut self, owner: Uuid) { self.grants.retain(|_, grant| grant.owner != owner); }
    pub fn expire(&mut self) { let now = now_ms(); self.grants.retain(|_, grant| grant.expires_at_ms > now); }
}
