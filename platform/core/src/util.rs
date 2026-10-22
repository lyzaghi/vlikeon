use omniplay_contracts::{ContentHash, ErrorCode, PlatformError, Result};
use sha2::{Digest, Sha256};
use std::{fs::{File, OpenOptions}, io::{Read, Write}, path::{Path, PathBuf}};
use uuid::Uuid;

pub fn storage_error(_: impl std::fmt::Display) -> PlatformError {
    // IO messages commonly contain private filesystem paths. Keep them out of IPC errors.
    PlatformError::new(ErrorCode::StorageFailure, "The local storage operation failed").recover("Check free space and folder permissions, then retry")
}

pub fn digest(bytes: &[u8]) -> ContentHash { ContentHash(format!("{:x}", Sha256::digest(bytes))) }

pub fn bounded_read(reader: &mut impl Read, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(limit.saturating_add(1)).read_to_end(&mut bytes).map_err(storage_error)?;
    if bytes.len() as u64 > limit { return Err(PlatformError::new(ErrorCode::ResourceLimit, "Input exceeds its size limit")); }
    Ok(bytes)
}

pub fn hash_file(file: &mut File, limit: u64) -> Result<(ContentHash, u64)> {
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut length = 0u64;
    loop {
        let read = file.read(&mut buffer).map_err(storage_error)?;
        if read == 0 { break; }
        length = length.checked_add(read as u64).ok_or_else(|| PlatformError::new(ErrorCode::ResourceLimit, "Content is too large"))?;
        if length > limit { return Err(PlatformError::new(ErrorCode::ResourceLimit, "Content exceeds its size limit")); }
        hasher.update(&buffer[..read]);
    }
    Ok((ContentHash(format!("{:x}", hasher.finalize())), length))
}

pub fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    File::open(path).and_then(|file| file.sync_all()).map_err(storage_error)?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

pub struct PendingFile { pub path: PathBuf, pub file: File, committed: bool }

impl PendingFile {
    pub fn create(directory: &Path) -> Result<Self> {
        std::fs::create_dir_all(directory).map_err(storage_error)?;
        let path = directory.join(format!(".pending-{}", Uuid::new_v4()));
        let mut options=OpenOptions::new();options.write(true).read(true).create_new(true);
        #[cfg(unix)]
        {use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
        let file=options.open(&path).map_err(storage_error)?;
        Ok(Self { path, file, committed: false })
    }

    pub fn write_all(&mut self, bytes: &[u8]) -> Result<()> { self.file.write_all(bytes).map_err(storage_error) }

    pub fn commit_new(mut self, target: &Path) -> Result<()> {
        self.file.sync_all().map_err(storage_error)?;
        // A hard link publishes a fully written inode without replacing any existing file.
        // Both paths are generated within the same private data directory and filesystem.
        std::fs::hard_link(&self.path, target).map_err(storage_error)?;
        if let Some(directory) = target.parent() { sync_directory(directory)?; }
        std::fs::remove_file(&self.path).map_err(storage_error)?;
        self.committed = true;
        Ok(())
    }
}

impl Drop for PendingFile {
    fn drop(&mut self) { if !self.committed { let _ = std::fs::remove_file(&self.path); } }
}
