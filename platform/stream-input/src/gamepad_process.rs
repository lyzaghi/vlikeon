//! Read-only Windows helper-instance witness. Construct from the actual child
//! process handle at launch, never by opening a renderer-supplied PID. This is
//! not process supervision/termination, a boot identity, or a removal receipt.
use crate::failure;
use omniplay_contracts::Result;
use std::os::windows::io::{AsRawHandle,BorrowedHandle,FromRawHandle,OwnedHandle};
use windows_sys::Win32::{Foundation::*,System::Threading::*};

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct ProcessIdentity{pub process_id:u32,pub creation_time:u64}
pub struct ProcessWitness{handle:OwnedHandle,identity:ProcessIdentity}
impl ProcessWitness{
    pub fn from_child(child:BorrowedHandle<'_>)->Result<Self>{
        let mut duplicate=std::ptr::null_mut();
        let current=unsafe{GetCurrentProcess()};
        // SAFETY: a borrowed live child handle, exact current-process handles,
        // live output pointer, least-privilege non-inheritable duplicate.
        if unsafe{DuplicateHandle(current,child.as_raw_handle(),current,&mut duplicate,
            PROCESS_QUERY_LIMITED_INFORMATION|PROCESS_SYNCHRONIZE,0,0)}==0||
            duplicate.is_null()||duplicate==INVALID_HANDLE_VALUE{return Err(failure());}
        // SAFETY: ownership of one successfully duplicated process handle.
        let handle=unsafe{OwnedHandle::from_raw_handle(duplicate)};
        let pid=unsafe{GetProcessId(handle.as_raw_handle())};
        let mut created=FILETIME::default();let mut exited=FILETIME::default();
        let mut kernel=FILETIME::default();let mut user=FILETIME::default();
        if pid==0||unsafe{GetProcessTimes(handle.as_raw_handle(),&mut created,&mut exited,&mut kernel,&mut user)}==0{
            return Err(failure());
        }
        let identity=ProcessIdentity{process_id:pid,
            creation_time:(u64::from(created.dwHighDateTime)<<32)|u64::from(created.dwLowDateTime)};
        if identity.creation_time==0{return Err(failure());}Ok(Self{handle,identity})
    }
    pub fn identity(&self)->ProcessIdentity{self.identity}
    pub fn exited(&self,expected:ProcessIdentity)->Result<bool>{
        if expected!=self.identity{return Err(failure());}
        // The original kernel process object remains identified after exit,
        // even when Windows recycles its numeric PID for a different process.
        match unsafe{WaitForSingleObject(self.handle.as_raw_handle(),0)}{
            WAIT_OBJECT_0=>Ok(true),WAIT_TIMEOUT=>Ok(false),_=>Err(failure()),
        }
    }
}
