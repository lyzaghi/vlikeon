//! Files inside a native-owned private transport directory. These helpers reject
//! link/reparse leaves and never accept renderer-selected names. Ancestor-relative
//! kernel containment against a hostile local filesystem owner remains separate.
use std::{fs::{File,OpenOptions},io::{self,Write},path::{Path,PathBuf}};
use uuid::Uuid;

fn invalid()->io::Error{io::Error::new(io::ErrorKind::InvalidData,"Invalid private transfer storage")}
/// Hold this descriptor until the actual operation exits; never unlink a lease
/// file, duplicate its handle, or replace its identity while it may be in use.
pub fn lease(parent:&Path,name:&str)->io::Result<File>{
    if name.is_empty()||name.len()>96||!name.bytes().all(|b|b.is_ascii_lowercase()||b.is_ascii_digit()||b".-".contains(&b))||name=="."||name==".."||!plain(&std::fs::symlink_metadata(parent)?,true){return Err(invalid());}
    let mut options=OpenOptions::new();options.read(true).write(true).create(true);
    #[cfg(unix)]
    {use std::os::unix::fs::OpenOptionsExt;options.mode(0o600).custom_flags(libc::O_NOFOLLOW|libc::O_CLOEXEC);}
    #[cfg(windows)]
    {use std::os::windows::fs::OpenOptionsExt;options.custom_flags(0x00200000).share_mode(3);}
    let file=options.open(parent.join(name))?;
    if !plain(&file.metadata()?,false)||file.metadata()?.len()!=0{return Err(invalid());}
    match file.try_lock(){Ok(())=>Ok(file),Err(std::fs::TryLockError::WouldBlock)=>Err(io::Error::from(io::ErrorKind::WouldBlock)),Err(std::fs::TryLockError::Error(error))=>Err(error)}
}
fn plain(metadata:&std::fs::Metadata,directory:bool)->bool{
    #[cfg(windows)]
    {use std::os::windows::fs::MetadataExt;if metadata.file_attributes()&0x400!=0{return false;}}
    !metadata.file_type().is_symlink()&&if directory{metadata.is_dir()}else{metadata.is_file()}
}
pub fn private_dir(parent:&Path,name:&str)->io::Result<PathBuf>{
    if name.is_empty()||name.len()>96||name=="."||name==".."||!name.bytes().all(|b|b.is_ascii_lowercase()||b.is_ascii_digit()||b".-".contains(&b)){return Err(invalid());}
    if !plain(&std::fs::symlink_metadata(parent)?,true){return Err(invalid());}
    let path=parent.join(name);let mut builder=std::fs::DirBuilder::new();
    #[cfg(unix)]
    {use std::os::unix::fs::DirBuilderExt;builder.mode(0o700);}
    match builder.create(&path){Ok(())=>sync_directory(parent)?,Err(e) if e.kind()==io::ErrorKind::AlreadyExists=>{},Err(e)=>return Err(e)}
    if !plain(&std::fs::symlink_metadata(&path)?,true){return Err(invalid());}Ok(path)
}
pub fn open(path:&Path,write:bool,max_bytes:u64)->io::Result<File>{
    let mut options=OpenOptions::new();options.read(true).write(write);
    #[cfg(unix)]
    {use std::os::unix::fs::OpenOptionsExt;options.custom_flags(libc::O_NOFOLLOW|libc::O_CLOEXEC);}
    #[cfg(windows)]
    {use std::os::windows::fs::OpenOptionsExt;options.custom_flags(0x00200000).share_mode(if write{0}else{1});}
    let file=options.open(path)?;let metadata=file.metadata()?;
    if !plain(&metadata,false)||metadata.len()>max_bytes{return Err(invalid());}Ok(file)
}
pub fn remove(path:&Path)->io::Result<()>{
    match std::fs::symlink_metadata(path){Err(e) if e.kind()==io::ErrorKind::NotFound=>return Ok(()),Err(e)=>return Err(e),Ok(metadata)=>if !plain(&metadata,false){return Err(invalid());}}
    std::fs::remove_file(path)
}
pub fn sync_directory(path:&Path)->io::Result<()>{
    #[cfg(unix)]
    {use std::os::unix::fs::OpenOptionsExt;OpenOptions::new().read(true).custom_flags(libc::O_DIRECTORY|libc::O_NOFOLLOW|libc::O_CLOEXEC).open(path)?.sync_all()?;}
    #[cfg(not(unix))]
    let _=path;
    Ok(())
}
pub struct Pending{path:PathBuf,file:Option<File>}
impl Pending{
    pub fn create(directory:&Path)->io::Result<Self>{
        let path=directory.join(format!(".pending-{}",Uuid::new_v4()));let mut options=OpenOptions::new();options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {use std::os::unix::fs::OpenOptionsExt;options.mode(0o600).custom_flags(libc::O_NOFOLLOW|libc::O_CLOEXEC);}
        #[cfg(windows)]
        {use std::os::windows::fs::OpenOptionsExt;options.custom_flags(0x00200000).share_mode(0);}
        let file=options.open(&path)?;Ok(Self{path,file:Some(file)})
    }
    pub fn file(&mut self)->io::Result<&mut File>{self.file.as_mut().ok_or_else(invalid)}
    pub fn write_all(&mut self,bytes:&[u8])->io::Result<()>{self.file()?.write_all(bytes)}
    pub fn publish(mut self,target:&Path)->io::Result<()>{
        let file=self.file.take().ok_or_else(invalid)?;file.sync_all()?;drop(file);
        std::fs::hard_link(&self.path,target)?;
        if let Some(parent)=target.parent(){sync_directory(parent)?;}
        remove(&self.path)?;Ok(())
    }
}
impl Drop for Pending{fn drop(&mut self){drop(self.file.take());let _=remove(&self.path);}}
