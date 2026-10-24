//! Cooperative native credential exclusion. Windows source candidate only.
//! Every journal writer and side-effect owner must hold this same profile lock.
use crate::Database;
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use std::{fs::File,path::PathBuf};
use uuid::Uuid;
#[cfg(windows)]use std::{fs::{self,OpenOptions},path::{Component,Path,Prefix},os::windows::fs::{MetadataExt,OpenOptionsExt}};
#[cfg(windows)]use crate::util::storage_error;

/// No Clone, file-handle export, explicit unlock or serialized constructor.
/// Ownership lasts through the final vault/journal operation, including errors.
pub struct CredentialLease{profile:Uuid,device:Uuid,root:PathBuf,_file:File,_directories:Vec<File>}
impl CredentialLease{
    pub fn acquire(database:&Database,profile:Uuid)->Result<Self>{
        #[cfg(not(windows))]{let _=(database,profile);Err(PlatformError::new(ErrorCode::UnsupportedCapability,"Native credential writer exclusion needs this platform's admitted lease provider"))}
        #[cfg(windows)]{
            if profile.is_nil()||database.device_id.is_nil()||!database.root.is_absolute(){return Err(invalid());}
            let root=&database.root;
            // Local drive-shaped paths only. UNC/network locking semantics and
            // remote/mapped storage still require explicit platform admission.
            match root.components().next(){Some(Component::Prefix(prefix))=>match prefix.kind(){Prefix::Disk(_)|Prefix::VerbatimDisk(_)=>{},_=>return Err(invalid())},_=>return Err(invalid())}
            if root.components().any(|part|matches!(part,Component::CurDir|Component::ParentDir)){return Err(invalid());}
            let ancestors:Vec<_>=root.ancestors().collect();
            if ancestors.len()>64{return Err(invalid());}
            let mut directories=Vec::with_capacity(ancestors.len());
            // No FILE_SHARE_DELETE, including every traversed parent: keep the
            // admitted namespace from being renamed/replaced while holding it.
            for path in ancestors.into_iter().rev(){
                ordinary(path,true)?;
                let file=OpenOptions::new().read(true).share_mode(3)
                    .custom_flags(0x0200_0000|0x0020_0000).open(path).map_err(storage_error)?;
                plain(&file.metadata().map_err(storage_error)?,true)?;directories.push(file);
            }
            let path=root.join("native-remote-play-credentials-v1.lock");
            match fs::symlink_metadata(&path){Ok(metadata)=>plain(&metadata,false)?,Err(error)if error.kind()==std::io::ErrorKind::NotFound=>{},Err(error)=>return Err(storage_error(error))}
            let file=OpenOptions::new().read(true).write(true).create(true).truncate(false)
                .share_mode(3).custom_flags(0x0020_0000).open(&path).map_err(storage_error)?;
            let metadata=file.metadata().map_err(storage_error)?;plain(&metadata,false)?;
            if metadata.len()!=0{return Err(invalid());}
            match file.try_lock(){
                Ok(())=>{},Err(std::fs::TryLockError::WouldBlock)=>return Err(PlatformError::new(ErrorCode::Busy,"Another native pairing writer or recovery step is still running")),
                Err(std::fs::TryLockError::Error(error))=>return Err(storage_error(error)),
            }
            let lease=Self{profile,device:database.device_id,root:root.clone(),_file:file,_directories:directories};
            lease.check(database,profile)?;Ok(lease)
        }
    }
    pub(crate) fn check(&self,database:&Database,profile:Uuid)->Result<()>{
        if self.profile!=profile||self.device!=database.device_id||self.root!=database.root{return Err(invalid());}
        #[cfg(windows)]{
            let metadata=self._file.metadata().map_err(storage_error)?;plain(&metadata,false)?;
            if metadata.len()!=0{return Err(invalid());}
            for directory in &self._directories{plain(&directory.metadata().map_err(storage_error)?,true)?;}
        }
        Ok(())
    }
}
fn invalid()->PlatformError{PlatformError::new(ErrorCode::PermissionDenied,"The native credential writer lease does not match an admitted profile namespace")}
#[cfg(windows)]fn plain(metadata:&fs::Metadata,directory:bool)->Result<()>{
    if metadata.file_type().is_symlink()||metadata.file_attributes()&0x400!=0||
        (if directory{!metadata.is_dir()}else{!metadata.is_file()}){return Err(invalid());}Ok(())
}
#[cfg(windows)]fn ordinary(path:&Path,directory:bool)->Result<()>{plain(&fs::symlink_metadata(path).map_err(storage_error)?,directory)}
