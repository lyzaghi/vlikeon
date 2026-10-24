//! Dedicated native Remote Play provider ABI, separate from save-sync ABI 2.
//! No renderer callback or secret export. Product/mobile startup wiring is due.
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_streaming::native_credentials::{CredentialAddress,SecretKind,PairingSecretStore,SECRET_RECORD_BYTES,validate_secret_envelope};
use std::{ffi::c_void,sync::{Arc,Mutex,atomic::{AtomicBool,AtomicU64,Ordering}}};
use uuid::Uuid;
use zeroize::Zeroizing;

#[repr(C)]
pub struct OmniPairingAddress{pub profile:[u8;16],pub device:[u8;16],pub item:[u8;16],pub generation:[u8;16],pub kind:u32}
const _: [();68]=[();std::mem::size_of::<OmniPairingAddress>()];
type Read=unsafe extern "C" fn(*mut c_void,*const OmniPairingAddress,*mut u8,usize)->i64;
type Write=unsafe extern "C" fn(*mut c_void,*const OmniPairingAddress,*const u8,usize)->i32;
type Remove=unsafe extern "C" fn(*mut c_void,*const OmniPairingAddress)->i32;
type Check=unsafe extern "C" fn(*mut c_void,*const OmniPairingAddress)->i32;
type Release=unsafe extern "C" fn(*mut c_void);
#[repr(C)]
pub struct OmniPairingCredentialCallbacks{pub abi_version:u32,pub context:*mut c_void,pub read:Option<Read>,pub write:Option<Write>,pub remove:Option<Remove>,pub check:Option<Check>,pub release:Option<Release>}
pub struct NativePairingCredentialStore{profile:Uuid,device:Uuid,callbacks:OmniPairingCallbacksOwned,gate:Mutex<()>,available:AtomicBool,closed:AtomicBool,epoch:AtomicU64}
struct OmniPairingCallbacksOwned{context:usize,read:Option<Read>,write:Option<Write>,remove:Option<Remove>,check:Option<Check>,release:Option<Release>}
impl Drop for OmniPairingCallbacksOwned{fn drop(&mut self){
    if self.context!=0{if let Some(release)=self.release{
        // SAFETY: take() transfers the trusted native context including failures;
        // last owner Drop runs only after all synchronous provider calls finish.
        unsafe{release(self.context as*mut c_void)};
    }}
}}
impl NativePairingCredentialStore{
    /// # Safety
    /// The native table/context is consumed even on rejection. All callbacks and
    /// release must remain valid, support worker threads and never reenter Rust.
    /// Buffers/address are synchronous borrows: no retained/overrun pointers.
    /// Bind profile/device from the actual native profile database, not renderer
    /// IDs. Callbacks must use a separate Remote Play OS-vault namespace and
    /// distinguish missing (-1), locked (-2), corrupt/failed (-3), never fallback.
    pub unsafe fn take(table:OmniPairingCredentialCallbacks,profile:Uuid,device:Uuid)->Result<Arc<Self>>{
        let callbacks=OmniPairingCallbacksOwned{context:table.context as usize,read:table.read,write:table.write,remove:table.remove,check:table.check,release:table.release};
        if table.abi_version!=1||profile.is_nil()||device.is_nil()||callbacks.context==0||callbacks.read.is_none()||callbacks.write.is_none()||callbacks.remove.is_none()||callbacks.check.is_none()||callbacks.release.is_none(){return Err(denied());}
        Ok(Arc::new(Self{profile,device,callbacks,gate:Mutex::new(()),available:AtomicBool::new(false),closed:AtomicBool::new(false),epoch:AtomicU64::new(1)}))
    }
    pub(crate) fn set_available(&self,value:bool){
        self.available.store(false,Ordering::Release);
        if self.epoch.fetch_update(Ordering::AcqRel,Ordering::Acquire,|epoch|epoch.checked_add(1)).is_err(){self.close();return;}
        self.available.store(value&&!self.closed.load(Ordering::Acquire),Ordering::Release);
    }
    pub(crate) fn close(&self){self.closed.store(true,Ordering::Release);self.available.store(false,Ordering::Release);}
    pub(crate) fn operation_epoch(&self)->Result<u64>{
        let epoch=self.epoch.load(Ordering::Acquire);
        if self.closed.load(Ordering::Acquire)||!self.available.load(Ordering::Acquire){return Err(denied());}Ok(epoch)
    }
    pub(crate) fn require_operation_epoch(&self,expected:u64)->Result<()>{
        if self.operation_epoch()?!=expected{return Err(denied());}Ok(())
    }
    pub(crate) fn confirm_access(&self,address:&CredentialAddress,expected:u64)->Result<()>{
        self.require_operation_epoch(expected)?;
        let native=self.scoped(address)?;
        if self.access(&native)?!=expected{return Err(denied());}Ok(())
    }
    fn scoped(&self,address:&CredentialAddress)->Result<OmniPairingAddress>{
        address.validate()?;
        if address.profile()!=self.profile||address.device()!=self.device{return Err(denied());}
        Ok(OmniPairingAddress{profile:*address.profile().as_bytes(),device:*address.device().as_bytes(),item:*address.id().as_bytes(),generation:*address.generation().as_bytes(),
            kind:match address.kind(){SecretKind::DeviceSigning=>1,SecretKind::PairEncryption=>2}})
    }
    fn access(&self,address:&OmniPairingAddress)->Result<u64>{
        let epoch=self.epoch.load(Ordering::Acquire);
        if self.closed.load(Ordering::Acquire)||!self.available.load(Ordering::Acquire){return Err(denied());}
        let check=self.callbacks.check.ok_or_else(denied)?;
        // SAFETY: consumed valid native table and live stack-owned address.
        let status=unsafe{check(self.callbacks.context as*mut c_void,address)};
        if status!=0{return Err(provider_error(status.into()));}
        if self.closed.load(Ordering::Acquire)||!self.available.load(Ordering::Acquire)||self.epoch.load(Ordering::Acquire)!=epoch{return Err(denied());}Ok(epoch)
    }
}
impl PairingSecretStore for NativePairingCredentialStore{
    fn read(&self,address:&CredentialAddress)->Result<Option<Zeroizing<Vec<u8>>>>{
        let _guard=self.gate.lock().map_err(|_|denied())?;let native=self.scoped(address)?;let epoch=self.access(&native)?;
        let mut bytes=Zeroizing::new(vec![0u8;SECRET_RECORD_BYTES]);let read=self.callbacks.read.ok_or_else(denied)?;
        // SAFETY: scoped immutable address and exact bounded output allocation.
        let count=unsafe{read(self.callbacks.context as*mut c_void,&native,bytes.as_mut_ptr(),bytes.len())};
        if self.access(&native)?!=epoch{return Err(denied());}
        if count== -1{return Ok(None);}if count<0{return Err(provider_error(count));}
        if count!=SECRET_RECORD_BYTES as i64{return Err(corrupt());}
        validate_secret_envelope(address,&bytes)?;Ok(Some(bytes))
    }
    fn write(&self,address:&CredentialAddress,record:&[u8])->Result<()>{
        let _guard=self.gate.lock().map_err(|_|denied())?;let native=self.scoped(address)?;
        validate_secret_envelope(address,record)?;let epoch=self.access(&native)?;
        let write=self.callbacks.write.ok_or_else(denied)?;
        // SAFETY: exact scoped record/address borrowed only during the callback.
        let status=unsafe{write(self.callbacks.context as*mut c_void,&native,record.as_ptr(),record.len())};
        if self.access(&native)?!=epoch{return Err(denied());}if status!=0{return Err(provider_error(status.into()));}Ok(())
    }
    fn remove(&self,address:&CredentialAddress)->Result<()>{
        let _guard=self.gate.lock().map_err(|_|denied())?;let native=self.scoped(address)?;let epoch=self.access(&native)?;
        let remove=self.callbacks.remove.ok_or_else(denied)?;
        // SAFETY: exact generation only, no vault-wide/profile-prefix deletion.
        let status=unsafe{remove(self.callbacks.context as*mut c_void,&native)};
        if self.access(&native)?!=epoch{return Err(denied());}if status!=0&&status!= -1{return Err(provider_error(status.into()));}Ok(())
    }
}
fn denied()->PlatformError{PlatformError::new(ErrorCode::PermissionDenied,"Native pairing credential provider scope or callbacks are unavailable")}
fn corrupt()->PlatformError{PlatformError::new(ErrorCode::CorruptData,"Native pairing vault record does not match its fixed envelope")}
fn provider_error(status:i64)->PlatformError{
    match status{-2=>PlatformError::new(ErrorCode::PermissionDenied,"Native pairing vault is locked").recover("Unlock the device and retry"),
        _=>PlatformError::new(ErrorCode::PermissionDenied,"Native pairing vault operation failed; retain lifecycle cleanup state")}
}
