//! Existing installed-package admission only. No download, build, repair or
//! arbitrary executable path is supplied by a renderer. Product wiring is due.
use omniplay_contracts::{ContentHash,ErrorCode,PlatformError,Result};
use omniplay_core::util::{hash_file,storage_error};
use omniplay_updater::{channel::Channel,package::{installed_file,verify_entry},store::Store};
use std::{fs::{File,OpenOptions},os::windows::fs::{MetadataExt,OpenOptionsExt},path::{Path,PathBuf}};
const ENTRY:&str="helpers/omniplay-video-worker.exe";
fn denied()->PlatformError{PlatformError::new(ErrorCode::UnsupportedCapability,"The installed native video worker is not admitted for this broker version")}
pub struct InstalledVideoWorker{pub path:PathBuf,pub artifact:File,pub directories:Vec<File>,pub sha256:ContentHash}
fn directory(path:&Path)->Result<File>{
    // Open the reparse object itself; do not traverse a replacement junction.
    // Read/write sharing permits ordinary access but excludes deletion/rename.
    let guard=OpenOptions::new().read(true).share_mode(3).custom_flags(0x02000000|0x00200000).open(path).map_err(storage_error)?;
    let metadata=guard.metadata().map_err(storage_error)?;
    if !metadata.is_dir()||metadata.file_attributes()&0x400!=0{return Err(denied());}Ok(guard)
}
pub fn admit()->Result<InstalledVideoWorker>{
    let store=Store::open(&omniplay_updater::install_root()?,Channel::embedded()?)?;
    let active=store.active()?;let root=store.version_directory(active.id);
    // Keep every ancestor stable while file-path checks/hash and CreateProcess
    // occur. Failure is unavailable, never a weakened inheritance/admission path.
    let mut directories=Vec::new();
    for ancestor in root.ancestors(){directories.push(directory(ancestor)?);}
    directories.push(directory(&root.join("helpers"))?);
    let broker=verify_entry(&root,&active.manifest,&active.manifest.broker_entry)?;
    if std::env::current_exe().and_then(|path|path.canonicalize()).map_err(storage_error)?!=broker{return Err(denied());}
    let entry=active.manifest.files.iter().find(|file|file.path==ENTRY&&file.executable).ok_or_else(denied)?;
    if entry.bytes==0||entry.bytes>128*1024*1024{return Err(denied());}
    let path=installed_file(&root,ENTRY)?;
    let mut artifact=OpenOptions::new().read(true).share_mode(1).custom_flags(0x00200000).open(&path).map_err(storage_error)?;
    let metadata=artifact.metadata().map_err(storage_error)?;
    if !metadata.is_file()||metadata.file_attributes()&0x400!=0||metadata.len()!=entry.bytes{return Err(denied());}
    // Hash the SAME immutable handle that survives the transport lifetime.
    let(hash,bytes)=hash_file(&mut artifact,entry.bytes)?;
    if hash!=entry.sha256||bytes!=entry.bytes||installed_file(&root,ENTRY)?!=path{return Err(denied());}
    Ok(InstalledVideoWorker{path,artifact,directories,sha256:hash})
}
