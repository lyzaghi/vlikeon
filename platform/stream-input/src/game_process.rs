//! Native installed-game lifetime and read-only target admission on Windows.
//! This job is an ownership scope, not an untrusted-code sandbox. It deliberately
//! has no kill-on-close, quotas, breakaway allowance or process termination API.
use crate::gamepad_process::ProcessWitness;
pub use crate::game_windows::GameWindow;
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use std::{ffi::OsStr,fs::OpenOptions,mem::{size_of,size_of_val},
    os::windows::{ffi::OsStrExt,io::{AsHandle,AsRawHandle,BorrowedHandle,FromRawHandle,OwnedHandle}},
    path::Path,ptr::{null,null_mut}};
use windows_sys::Win32::{Foundation::*,System::{JobObjects::*,Threading::*}};

fn unavailable()->PlatformError{
    PlatformError::new(ErrorCode::RuntimeUnavailable,"The installed game's owned process group is unavailable")
        .recover("Select the actual game executable if its external launcher is incompatible")
}
fn wide(value:&OsStr)->Result<Vec<u16>>{
    let mut units:Vec<u16>=value.encode_wide().collect();
    if units.is_empty()||units.len()>32766||units.contains(&0){return Err(unavailable());}
    units.push(0);Ok(units)
}
fn own(raw:HANDLE)->Result<OwnedHandle>{
    if raw.is_null()||raw==INVALID_HANDLE_VALUE{return Err(unavailable());}
    // SAFETY: only fresh owned handles returned by Windows are passed here.
    Ok(unsafe{OwnedHandle::from_raw_handle(raw)})
}
fn readonly(process:BorrowedHandle<'_>)->Result<OwnedHandle>{
    let mut duplicate=null_mut();
    // SAFETY: valid borrowed process, current process and live output pointer.
    let current=unsafe{GetCurrentProcess()};
    if unsafe{DuplicateHandle(current,process.as_raw_handle(),current,&mut duplicate,
        PROCESS_QUERY_LIMITED_INFORMATION|PROCESS_SYNCHRONIZE,0,0)}==0{return Err(unavailable());}
    own(duplicate)
}
struct Attributes{_storage:Vec<usize>,pointer:LPPROC_THREAD_ATTRIBUTE_LIST}
impl Attributes{
    fn new()->Result<Self>{
        let mut bytes=0;
        // SAFETY: documented size query, then aligned, bounded backing storage.
        unsafe{InitializeProcThreadAttributeList(null_mut(),2,0,&mut bytes);}
        if bytes==0||bytes>65536{return Err(unavailable());}
        let mut storage=vec![0usize;bytes.div_ceil(size_of::<usize>())];
        let pointer=storage.as_mut_ptr().cast();
        if unsafe{InitializeProcThreadAttributeList(pointer,2,0,&mut bytes)}==0{return Err(unavailable());}
        Ok(Self{_storage:storage,pointer})
    }
    fn set<T>(&mut self,key:u32,values:&[T])->Result<()>{
        // SAFETY: initialized list and caller-retained values until creation.
        if unsafe{UpdateProcThreadAttribute(self.pointer,0,key as usize,values.as_ptr().cast(),
            size_of_val(values),null_mut(),null())}==0{return Err(unavailable());}Ok(())
    }
}
impl Drop for Attributes{fn drop(&mut self){
    // SAFETY: successfully initialized list; backing allocation is still live.
    unsafe{DeleteProcThreadAttributeList(self.pointer);}
}}

pub struct OwnedGameProcess{root:OwnedHandle,job:OwnedHandle}
impl OwnedGameProcess{
    /// Caller admits and holds the selected executable's immutable file guard.
    /// No shell, user arguments, elevation, injection or detached-launch fallback.
    pub fn launch(executable:&Path,directory:&Path)->Result<Self>{
        if !executable.is_absolute()||!directory.is_absolute()||executable.parent()!=Some(directory){return Err(unavailable());}
        let application=wide(executable.as_os_str())?;let directory=wide(directory.as_os_str())?;
        let mut command=Vec::with_capacity(application.len()+2);command.push(b'"' as u16);
        for &unit in &application[..application.len()-1]{
            if unit==b'"' as u16{return Err(unavailable());}command.push(unit);
        }
        command.push(b'"' as u16);command.push(0);
        if command.len()>32767{return Err(unavailable());}
        // Unnamed, non-inheritable job; a fresh job defaults to no breakaway and
        // no kill-on-close. Do not copy the restrictive helper-job recipe.
        let job=own(unsafe{CreateJobObjectW(null(),null())})?;
        let null_file=OpenOptions::new().read(true).write(true).open("NUL").map_err(|_|unavailable())?;
        // SAFETY: this newly opened private file alone enters the handle list.
        if unsafe{SetHandleInformation(null_file.as_raw_handle(),HANDLE_FLAG_INHERIT,HANDLE_FLAG_INHERIT)}==0{return Err(unavailable());}
        let inherited=[null_file.as_raw_handle()];let jobs=[job.as_raw_handle()];
        let mut attributes=Attributes::new()?;
        attributes.set(PROC_THREAD_ATTRIBUTE_HANDLE_LIST,&inherited)?;
        attributes.set(PROC_THREAD_ATTRIBUTE_JOB_LIST,&jobs)?;
        let mut startup=STARTUPINFOEXW::default();
        startup.StartupInfo.cb=size_of::<STARTUPINFOEXW>()as u32;
        startup.StartupInfo.dwFlags=STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput=null_file.as_raw_handle();
        startup.StartupInfo.hStdOutput=null_file.as_raw_handle();
        startup.StartupInfo.hStdError=null_file.as_raw_handle();
        startup.lpAttributeList=attributes.pointer;
        let mut process=PROCESS_INFORMATION::default();
        // SAFETY: explicit executable, mutable quoted argv[0], bounded live
        // buffers/attributes and exactly one allowed inherited NUL handle.
        // JOB_LIST attaches before the initial thread runs. Inherit the user's
        // environment as with the prior Command path; no injected helper env.
        if unsafe{CreateProcessW(application.as_ptr(),command.as_mut_ptr(),null(),null(),1,
            EXTENDED_STARTUPINFO_PRESENT,null(),directory.as_ptr(),&startup.StartupInfo,&mut process)}==0{
            return Err(unavailable());
        }
        // SAFETY: successful CreateProcessW returns two valid owned handles.
        // Nothing fallible follows creation: do not report a retryable launch
        // failure after a game has already started.
        let root=unsafe{OwnedHandle::from_raw_handle(process.hProcess)};
        let thread=unsafe{OwnedHandle::from_raw_handle(process.hThread)};drop(thread);
        Ok(Self{root,job})
    }

    /// None means the original launcher OR an inherited descendant is active.
    /// Some(None) means ended, but root exit status is not the game's status.
    pub fn poll_exit(&self)->Result<Option<Option<bool>>>{
        let mut accounting=JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();let mut length=0;
        // SAFETY: retained job handle, correctly sized typed output buffers.
        if unsafe{QueryInformationJobObject(self.job.as_raw_handle(),JobObjectBasicAccountingInformation,
            (&mut accounting as*mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
            size_of_val(&accounting)as u32,&mut length)}==0||length!=size_of_val(&accounting)as u32{return Err(unavailable());}
        if accounting.ActiveProcesses!=0{return Ok(None);}
        match unsafe{WaitForSingleObject(self.root.as_raw_handle(),0)}{
            WAIT_TIMEOUT=>return Ok(None),WAIT_OBJECT_0=>{},_=>return Err(unavailable()),
        }
        // More than one process ever assigned: launcher success cannot prove
        // descendant game success, even after the whole job is drained.
        if accounting.TotalProcesses!=1{return Ok(Some(None));}
        let mut code=0;
        if unsafe{GetExitCodeProcess(self.root.as_raw_handle(),&mut code)}==0{return Err(unavailable());}
        Ok(Some(Some(code==0)))
    }

    /// Direct-root compatibility path. A live descendant is never guessed when
    /// the launcher exits; the native chooser must select and admit its handle.
    pub fn root_handle(&self)->Result<OwnedHandle>{self.admit_selected(self.root.as_handle())}

    /// Bounded host-only choices; the caller must present an explicit choice
    /// and then obtain native capture consent. Discovery is not authorization.
    pub fn windows(&self)->Result<Vec<GameWindow>>{crate::game_windows::enumerate(self)}
    pub fn admit_window(&self,selected:&GameWindow)->Result<OwnedHandle>{selected.admit(self)}

    /// Only for a handle from the host's native window selection. Membership
    /// does not itself authorize capture/input or select a window. No PID,
    /// image name, parent-PID snapshot or serialized ownership assertion enters.
    pub fn admit_selected(&self,selected:BorrowedHandle<'_>)->Result<OwnedHandle>{
        let process=readonly(selected)?;
        let witness=ProcessWitness::from_child(process.as_handle())?;
        if witness.exited(witness.identity())?{return Err(unavailable());}
        let mut member=0;
        // SAFETY: retained original job and read-only process-object handles.
        if unsafe{IsProcessInJob(process.as_raw_handle(),self.job.as_raw_handle(),&mut member)}==0||member==0{
            return Err(PlatformError::new(ErrorCode::PermissionDenied,"The selected window does not belong to this launched game session"));
        }
        if witness.exited(witness.identity())?{return Err(unavailable());}
        Ok(process)
    }
}
