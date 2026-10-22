//! Broker-side immutable descriptor preparation source. No runtime availability
//! follows from this module. Actual signed package selection, spawn inheritance,
//! profile cache sharing and process-lifetime ownership belong to the launcher.
use crate::linux_plan::BootstrapPlan;
use omniplay_contracts::{ContentHash,ErrorCode,PlatformError,Result};
use sha2::{Digest,Sha256};
use std::{fs::{File,OpenOptions},io::{ErrorKind,Write},os::{fd::{AsRawFd,FromRawFd,RawFd},unix::fs::{FileExt,MetadataExt,OpenOptionsExt}},sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}}};

const MIB:u64=1024*1024;
struct Usage{bytes:u64,files:usize}
struct Budget{usage:Mutex<Usage>,maximum_bytes:u64,maximum_files:usize}
#[derive(Clone)]pub struct SealedBudget{inner:Arc<Budget>}
struct Reservation{budget:Arc<Budget>,bytes:u64}
impl Drop for Reservation{fn drop(&mut self){let mut used=match self.budget.usage.lock(){Ok(value)=>value,Err(value)=>value.into_inner()};used.bytes=used.bytes.saturating_sub(self.bytes);used.files=used.files.saturating_sub(1);}}
struct Object{file:File,bytes:u64,hash:ContentHash,_reservation:Reservation}
#[derive(Clone)]pub struct SealedObject{inner:Arc<Object>,_owner:Option<Arc<dyn Send+Sync>>}
impl AsRawFd for SealedObject{fn as_raw_fd(&self)->RawFd{self.inner.file.as_raw_fd()}}
impl SealedObject{pub fn bytes(&self)->u64{self.inner.bytes}pub fn sha256(&self)->&ContentHash{&self.inner.hash}pub fn cache_unused(&self)->bool{Arc::strong_count(&self.inner)==1}pub fn retain_owner(&self,owner:Arc<dyn Send+Sync>)->Self{Self{inner:self.inner.clone(),_owner:Some(owner)}}pub(crate) fn file(&self)->&File{&self.inner.file}}
fn unavailable()->PlatformError{PlatformError::new(ErrorCode::RuntimeUnavailable,"The immutable Linux runtime descriptor could not be prepared")}
fn limit()->PlatformError{PlatformError::new(ErrorCode::ResourceLimit,"The immutable Linux runtime descriptor budget is exhausted")}
fn cancelled(cancel:&AtomicBool)->Result<()>{if cancel.load(Ordering::Acquire){Err(PlatformError::new(ErrorCode::Cancelled,"Linux runtime preparation was cancelled"))}else{Ok(())}}
fn created()->Result<File>{
    let flags=libc::MFD_CLOEXEC|libc::MFD_ALLOW_SEALING|libc::MFD_NOEXEC_SEAL;
    let fd=unsafe{libc::memfd_create(c"OmniPlay.Immutable".as_ptr(),flags)};
    if fd<0{return Err(unavailable());}Ok(unsafe{File::from_raw_fd(fd)})
}
fn readonly(output:File,bytes:u64)->Result<File>{
    let fd=output.as_raw_fd();let required=libc::F_SEAL_WRITE|libc::F_SEAL_GROW|libc::F_SEAL_SHRINK|libc::F_SEAL_SEAL|libc::F_SEAL_EXEC;
    if unsafe{libc::fchmod(fd,0o400)}!=0||unsafe{libc::fcntl(fd,libc::F_ADD_SEALS,required)}!=0{return Err(unavailable());}
    // Follow only this process's still-owned fd number. This intentionally
    // reopens the same sealed inode readonly; no external path is accepted.
    let result=OpenOptions::new().read(true).custom_flags(libc::O_CLOEXEC).open(format!("/proc/self/fd/{fd}")).map_err(|_|unavailable())?;
    let before=output.metadata().map_err(|_|unavailable())?;let after=result.metadata().map_err(|_|unavailable())?;
    let seals=unsafe{libc::fcntl(result.as_raw_fd(),libc::F_GET_SEALS)};let flags=unsafe{libc::fcntl(result.as_raw_fd(),libc::F_GETFL)};let descriptor_flags=unsafe{libc::fcntl(result.as_raw_fd(),libc::F_GETFD)};
    if !after.is_file()||after.len()!=bytes||after.dev()!=before.dev()||after.ino()!=before.ino()||after.mode()&0o111!=0||seals<0||seals&required!=required||flags<0||flags&libc::O_ACCMODE!=libc::O_RDONLY||descriptor_flags<0||descriptor_flags&libc::FD_CLOEXEC==0{return Err(unavailable());}drop(output);Ok(result)
}
impl SealedBudget{
    pub fn new(maximum_bytes:u64,maximum_files:usize)->Result<Self>{
        if !(16*MIB..=1024*MIB).contains(&maximum_bytes)||!(1..=256).contains(&maximum_files){return Err(limit());}
        Ok(Self{inner:Arc::new(Budget{usage:Mutex::new(Usage{bytes:0,files:0}),maximum_bytes,maximum_files})})
    }
    fn reserve(&self,bytes:u64)->Result<Reservation>{
        if bytes==0{return Err(limit());}let mut used=self.inner.usage.lock().map_err(|_|limit())?;
        let total=used.bytes.checked_add(bytes).filter(|value|*value<=self.inner.maximum_bytes).ok_or_else(limit)?;
        if used.files>=self.inner.maximum_files{return Err(limit());}used.bytes=total;used.files+=1;Ok(Reservation{budget:self.inner.clone(),bytes})
    }
    /// `expected` and `bytes` must come from the authenticated outer inventory,
    /// never from a renderer's approval claim. The actual copied bytes are hashed
    /// before publication. A clone shares the one immutable inode/reservation.
    pub fn image(&self,source:File,expected:&ContentHash,bytes:u64,cancel:&AtomicBool)->Result<SealedObject>{
        cancelled(cancel)?;ContentHash::parse(expected.0.clone())?;
        if bytes==0||bytes>256*MIB{return Err(limit());}let metadata=source.metadata().map_err(|_|unavailable())?;
        if !metadata.is_file()||metadata.len()!=bytes{return Err(unavailable());}let reservation=self.reserve(bytes)?;let mut output=created()?;let mut digest=Sha256::new();let mut buffer=[0u8;65536];let mut at=0u64;
        while at<bytes{
            cancelled(cancel)?;let count=(bytes-at).min(buffer.len()as u64)as usize;
            let read=match source.read_at(&mut buffer[..count],at){Ok(value)=>value,Err(error)if error.kind()==ErrorKind::Interrupted=>continue,Err(_)=>return Err(unavailable())};
            if read==0{return Err(unavailable());}digest.update(&buffer[..read]);output.write_all(&buffer[..read]).map_err(|_|unavailable())?;at+=read as u64;
        }
        cancelled(cancel)?;if source.metadata().map_err(|_|unavailable())?.len()!=bytes||format!("{:x}",digest.finalize())!=expected.0{return Err(PlatformError::new(ErrorCode::ContentMismatch,"The Linux runtime image differs from its authenticated inventory"));}
        let file=readonly(output,bytes)?;cancelled(cancel)?;Ok(SealedObject{inner:Arc::new(Object{file,bytes,hash:expected.clone(),_reservation:reservation}),_owner:None})
    }
    pub fn plan(&self,plan:&BootstrapPlan,cancel:&AtomicBool)->Result<SealedObject>{
        cancelled(cancel)?;let encoded=plan.encode()?;let bytes=encoded.len()as u64;let reservation=self.reserve(bytes)?;let mut output=created()?;output.write_all(&encoded).map_err(|_|unavailable())?;
        let hash=ContentHash(format!("{:x}",Sha256::digest(&encoded)));let file=readonly(output,bytes)?;cancelled(cancel)?;Ok(SealedObject{inner:Arc::new(Object{file,bytes,hash,_reservation:reservation}),_owner:None})
    }
}
