use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_sync_client::{CredentialStore,MAX_CREDENTIAL_BYTES};
use std::{ffi::c_void,sync::{Arc,OnceLock}};
use uuid::Uuid;
use zeroize::{Zeroize,Zeroizing};

type Read=unsafe extern "C" fn(*mut c_void,*const u8,*mut u8,usize)->i64;
type Write=unsafe extern "C" fn(*mut c_void,*const u8,*const u8,usize)->i32;
type Remove=unsafe extern "C" fn(*mut c_void,*const u8)->i32;
type Release=unsafe extern "C" fn(*mut c_void);
type Check=unsafe extern "C" fn(*mut c_void,*const u8)->i32;
#[repr(C)]
pub struct OmniCredentialCallbacks{pub abi_version:u32,pub context:*mut c_void,pub read:Option<Read>,pub write:Option<Write>,pub remove:Option<Remove>,pub release:Option<Release>,pub check:Option<Check>}

/// Created only from a trusted native table. Storing the context as an opaque
/// integer does not make a provider thread-safe: take() requires that guarantee.
pub(crate) struct NativeCredentialStore{context:usize,read:Option<Read>,write:Option<Write>,remove:Option<Remove>,release:Option<Release>,access:Option<Check>,profile:OnceLock<Uuid>}
impl NativeCredentialStore{
    /// # Safety
    /// Every supplied callback must be callable for the context, including the
    /// release callback on rejection. Calls may come from different native worker
    /// threads. Buffers are borrowed synchronously; no pointer may be retained.
    /// Ownership of the context is consumed even when validation fails.
    pub(crate) unsafe fn take(table:OmniCredentialCallbacks)->Result<Arc<Self>>{
        let store=Self{context:table.context as usize,read:table.read,write:table.write,remove:table.remove,release:table.release,access:table.check,profile:OnceLock::new()};
        if table.abi_version!=2||store.context==0||store.read.is_none()||store.write.is_none()||store.remove.is_none()||store.release.is_none()||store.access.is_none(){return Err(PlatformError::new(ErrorCode::InvalidRequest,"Native credential provider ABI is invalid"));}Ok(Arc::new(store))
    }
    pub(crate) fn bind(&self,profile:Uuid)->Result<()>{self.profile.set(profile).map_err(|_|denied())}
    fn check(&self,profile:Uuid)->Result<()>{if self.profile.get()!=Some(&profile){return Err(denied());}Ok(())}
    fn context(&self)->*mut c_void{self.context as *mut c_void}
}
impl CredentialStore for NativeCredentialStore{
    fn check_access(&self)->Result<()>{let profile=self.profile.get().ok_or_else(denied)?;let check=self.access.ok_or_else(denied)?;let status=unsafe{check(self.context(),profile.as_bytes().as_ptr())};if status!=0{return Err(provider_error(status.into()));}Ok(())}
    fn read(&self,profile:Uuid)->Result<Zeroizing<Vec<u8>>>{
        self.check(profile)?;let read=self.read.ok_or_else(denied)?;let mut output=Zeroizing::new(vec![0;MAX_CREDENTIAL_BYTES]);
        let count=unsafe{read(self.context(),profile.as_bytes().as_ptr(),output.as_mut_ptr(),output.len())};if count<0{return Err(provider_error(count));}if count==0||count as u64>MAX_CREDENTIAL_BYTES as u64{return Err(PlatformError::new(ErrorCode::CorruptData,"Native credential response exceeds its fixed bound"));}
        let count=count as usize;output[count..].zeroize();output.truncate(count);Ok(output)
    }
    fn write(&self,profile:Uuid,secret:&[u8])->Result<()>{
        self.check(profile)?;if secret.is_empty()||secret.len()>MAX_CREDENTIAL_BYTES{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Native credential input exceeds its fixed bound"));}let write=self.write.ok_or_else(denied)?;
        let status=unsafe{write(self.context(),profile.as_bytes().as_ptr(),secret.as_ptr(),secret.len())};if status!=0{return Err(provider_error(i64::from(status)));}Ok(())
    }
    fn remove(&self,profile:Uuid)->Result<()>{self.check(profile)?;let remove=self.remove.ok_or_else(denied)?;let status=unsafe{remove(self.context(),profile.as_bytes().as_ptr())};if status!=0&&status!= -1{return Err(provider_error(i64::from(status)));}Ok(())}
}
impl Drop for NativeCredentialStore{fn drop(&mut self){if self.context!=0{if let Some(release)=self.release{unsafe{release(self.context())};}}}}
fn denied()->PlatformError{PlatformError::new(ErrorCode::PermissionDenied,"Native credential provider does not own this profile")}
fn provider_error(status:i64)->PlatformError{match status{-1=>PlatformError::new(ErrorCode::NotFound,"The native secure credential is absent").recover("Recover the synchronization account using its retained recovery key"),-2=>PlatformError::new(ErrorCode::PermissionDenied,"The native secure credential is locked").recover("Unlock the device and retry"),_=>PlatformError::new(ErrorCode::PermissionDenied,"The native secure credential operation failed").recover("Unlock the device and retry the native operation")}}
