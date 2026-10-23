//! Read-only Windows helper-instance and startup-recovery witnesses. A launch
//! witness uses the actual child handle; recovery uses only a validated native
//! journal identity. Neither path accepts renderer PID authority, terminates a
//! process, establishes a boot identity or acknowledges device removal.
use crate::failure;
use omniplay_contracts::Result;
use std::{mem::size_of,os::windows::io::{AsHandle,AsRawHandle,BorrowedHandle,FromRawHandle,OwnedHandle}};
use windows_sys::Win32::{Foundation::*,System::{Diagnostics::ToolHelp::*,Threading::*}};

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

/// Read-only recovery after losing the original launch handle. This is only
/// retirement evidence for a validated durable helper record, never authority
/// to control/terminate a process found by numeric PID or to resume old input.
pub struct RecoveryProcessWitness{expected:ProcessIdentity,observed:Option<ProcessWitness>}
impl RecoveryProcessWitness{
    pub fn from_saved(expected:ProcessIdentity)->Result<Self>{
        if expected.process_id==0||expected.creation_time==0{return Err(failure());}
        if !snapshot_contains(expected.process_id)?{
            return Ok(Self{expected,observed:None});
        }
        // No escalation, debug privilege, image lookup or process mutation.
        // Open refusal or a race with exit is unknown, not proof of retirement.
        let raw=unsafe{OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION|PROCESS_SYNCHRONIZE,0,expected.process_id)};
        if raw.is_null()||raw==INVALID_HANDLE_VALUE{return Err(failure());}
        // SAFETY: newly returned, privately owned process handle.
        let handle=unsafe{OwnedHandle::from_raw_handle(raw)};
        let observed=ProcessWitness::from_child(handle.as_handle())?;
        if observed.identity.process_id!=expected.process_id{return Err(failure());}
        if observed.identity.creation_time!=expected.creation_time{
            // A positive observation of another creation identity at this PID
            // proves the saved instance is no longer live. Never inspect its
            // exit status as the old helper's, or terminate the replacement.
            return Ok(Self{expected,observed:None});
        }
        // A matching timestamp is conservative, even across a restart/clock
        // change: retain the real handle and wait for this object to signal.
        Ok(Self{expected,observed:Some(observed)})
    }
    pub fn exited(&self,expected:ProcessIdentity)->Result<bool>{
        if expected!=self.expected{return Err(failure());}
        match &self.observed{None=>Ok(true),Some(process)=>process.exited(expected)}
    }
}
fn snapshot_contains(pid:u32)->Result<bool>{
    let raw=unsafe{CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS,0)};
    if raw.is_null()||raw==INVALID_HANDLE_VALUE{return Err(failure());}
    // SAFETY: newly created read-only, non-inheritable snapshot handle.
    let snapshot=unsafe{OwnedHandle::from_raw_handle(raw)};
    let mut entry=PROCESSENTRY32W::default();entry.dwSize=size_of::<PROCESSENTRY32W>()as u32;
    if unsafe{Process32FirstW(snapshot.as_raw_handle(),&mut entry)}==0{return Err(failure());}
    let current=unsafe{GetCurrentProcessId()};let mut own=false;let mut found=false;let mut count=0;
    loop{
        count+=1;if count>65536{return Err(failure());}
        own|=entry.th32ProcessID==current;found|=entry.th32ProcessID==pid;
        if unsafe{Process32NextW(snapshot.as_raw_handle(),&mut entry)}==0{
            if unsafe{GetLastError()}!=ERROR_NO_MORE_FILES||!own{return Err(failure());}
            return Ok(found);
        }
    }
}
