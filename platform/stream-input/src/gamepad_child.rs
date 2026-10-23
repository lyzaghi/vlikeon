//! Normal-user trusted helper launch, not a sandbox for untrusted executables.
//! Admission/hash verification and an immutable executable guard belong to the
//! native broker. No current product path invokes this source candidate.
use crate::{failure,gamepad_process::ProcessWitness,system_directory};
use omniplay_contracts::Result;
use std::{ffi::OsStr,fs::{File,OpenOptions},mem::{size_of,size_of_val},
    os::windows::{ffi::OsStrExt,io::{AsHandle,AsRawHandle,FromRawHandle,OwnedHandle}},
    path::Path,ptr::{null,null_mut},sync::Arc};
use windows_sys::Win32::{Foundation::*,Security::SECURITY_ATTRIBUTES,
    System::{JobObjects::*,Pipes::CreatePipe,Threading::*}};

fn handle(raw:HANDLE)->Result<OwnedHandle>{
    if raw.is_null()||raw==INVALID_HANDLE_VALUE{return Err(failure());}
    // SAFETY: callers pass only a newly acquired sole-owned Windows handle.
    Ok(unsafe{OwnedHandle::from_raw_handle(raw)})
}
fn wide(value:&OsStr)->Result<Vec<u16>>{
    let mut units:Vec<u16>=value.encode_wide().collect();
    if units.is_empty()||units.len()>32766||units.contains(&0){return Err(failure());}
    units.push(0);Ok(units)
}
fn pipe(parent_writes:bool)->Result<(OwnedHandle,OwnedHandle)>{
    let(mut read,mut write)=(null_mut(),null_mut());
    let security=SECURITY_ATTRIBUTES{nLength:size_of::<SECURITY_ATTRIBUTES>()as u32,
        lpSecurityDescriptor:null_mut(),bInheritHandle:1};
    if unsafe{CreatePipe(&mut read,&mut write,&security,65536)}==0{return Err(failure());}
    let(read,write)=(handle(read)?,handle(write)?);
    let parent=if parent_writes{write.as_raw_handle()}else{read.as_raw_handle()};
    if unsafe{SetHandleInformation(parent,HANDLE_FLAG_INHERIT,0)}==0{return Err(failure());}
    Ok((read,write))
}
struct Attributes{_storage:Vec<usize>,pointer:LPPROC_THREAD_ATTRIBUTE_LIST}
impl Attributes{
    fn new()->Result<Self>{
        let mut bytes=0;
        unsafe{InitializeProcThreadAttributeList(null_mut(),2,0,&mut bytes);}
        if bytes==0||bytes>65536{return Err(failure());}
        let mut storage=vec![0usize;bytes.div_ceil(size_of::<usize>())];
        let pointer=storage.as_mut_ptr().cast();
        if unsafe{InitializeProcThreadAttributeList(pointer,2,0,&mut bytes)}==0{return Err(failure());}
        Ok(Self{_storage:storage,pointer})
    }
    fn set<T>(&mut self,key:u32,values:&[T])->Result<()>{
        // Backing values remain alive through CreateProcessW below.
        if unsafe{UpdateProcThreadAttribute(self.pointer,0,key as usize,
            values.as_ptr().cast(),size_of_val(values),null_mut(),null())}==0{return Err(failure());}Ok(())
    }
}
impl Drop for Attributes{fn drop(&mut self){unsafe{DeleteProcThreadAttributeList(self.pointer);}}}

pub(crate) struct ChildProcess{process:OwnedHandle,job:OwnedHandle,witness:ProcessWitness}
impl ChildProcess{
    pub(crate) fn witness(&self)->Result<ProcessWitness>{ProcessWitness::from_child(self.process.as_handle())}
    pub(crate) fn exited(&self)->Result<bool>{self.witness.exited(self.witness.identity())}
    pub(crate) fn stop(&self,wait_ms:u32)->Result<()>{
        if self.exited()?{return Ok(());}
        unsafe{TerminateJobObject(self.job.as_raw_handle(),1);}
        if !self.exited().unwrap_or(false){unsafe{TerminateProcess(self.process.as_raw_handle(),1);}}
        if unsafe{WaitForSingleObject(self.process.as_raw_handle(),wait_ms.min(1000))}==WAIT_OBJECT_0{Ok(())}else{Err(failure())}
    }
}
impl Drop for ChildProcess{fn drop(&mut self){let _=self.stop(0);}}
pub(crate) struct SuspendedChild{
    pub(crate) process:Arc<ChildProcess>,thread:OwnedHandle,
    pub(crate) input:File,pub(crate) output:File,
}
impl SuspendedChild{
    pub(crate) fn launch(executable:&Path)->Result<Self>{
        if !executable.is_absolute(){return Err(failure());}
        let text=executable.to_str().filter(|value|!value.contains('"')).ok_or_else(failure)?;
        let directory=executable.parent().ok_or_else(failure)?;
        let(child_input,parent_input)=pipe(true)?;let(parent_output,child_output)=pipe(false)?;
        let error=OpenOptions::new().write(true).open("NUL").map_err(|_|failure())?;
        if unsafe{SetHandleInformation(error.as_raw_handle(),HANDLE_FLAG_INHERIT,HANDLE_FLAG_INHERIT)}==0{return Err(failure());}
        let inherited=[child_input.as_raw_handle(),child_output.as_raw_handle(),error.as_raw_handle()];
        let job=handle(unsafe{CreateJobObjectW(null(),null())})?;
        let mut limits=JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags=JOB_OBJECT_LIMIT_ACTIVE_PROCESS|
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE|JOB_OBJECT_LIMIT_PROCESS_MEMORY|JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION;
        limits.BasicLimitInformation.ActiveProcessLimit=1;limits.ProcessMemoryLimit=128*1024*1024;
        if unsafe{SetInformationJobObject(job.as_raw_handle(),JobObjectExtendedLimitInformation,
            (&limits as*const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),size_of_val(&limits)as u32)}==0{return Err(failure());}
        let jobs=[job.as_raw_handle()];let mut attributes=Attributes::new()?;
        attributes.set(PROC_THREAD_ATTRIBUTE_HANDLE_LIST,&inherited)?;
        attributes.set(PROC_THREAD_ATTRIBUTE_JOB_LIST,&jobs)?;
        let application=wide(executable.as_os_str())?;let directory=wide(directory.as_os_str())?;
        let mut command=wide(OsStr::new(&format!("\"{text}\" --stdio-gamepad-v2")))?;
        let system=system_directory()?;let system=system.to_str().ok_or_else(failure)?;
        let mut environment=Vec::new();
        for entry in [format!("PATH={system}\\System32"),format!("SystemRoot={system}"),format!("WINDIR={system}")]{
            environment.extend(wide(OsStr::new(&entry))?);
        }
        environment.push(0);
        let mut startup=STARTUPINFOEXW::default();startup.StartupInfo.cb=size_of_val(&startup)as u32;
        startup.StartupInfo.dwFlags=STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput=child_input.as_raw_handle();startup.StartupInfo.hStdOutput=child_output.as_raw_handle();
        startup.StartupInfo.hStdError=error.as_raw_handle();startup.lpAttributeList=attributes.pointer;
        let mut info=PROCESS_INFORMATION::default();
        if unsafe{CreateProcessW(application.as_ptr(),command.as_mut_ptr(),null(),null(),1,
            CREATE_SUSPENDED|CREATE_NO_WINDOW|CREATE_UNICODE_ENVIRONMENT|EXTENDED_STARTUPINFO_PRESENT,
            environment.as_ptr().cast(),directory.as_ptr(),&startup.StartupInfo,&mut info)}==0{return Err(failure());}
        // The private job already owns this suspended process. Closing the job
        // on any setup error kills it before application code can execute.
        let process=handle(info.hProcess)?;let thread=handle(info.hThread)?;
        let mut associated=0;
        if unsafe{IsProcessInJob(process.as_raw_handle(),job.as_raw_handle(),&mut associated)}==0||associated==0{return Err(failure());}
        let witness=ProcessWitness::from_child(process.as_handle())?;
        let process=Arc::new(ChildProcess{process,job,witness});
        Ok(Self{process,thread,input:File::from(parent_input),output:File::from(parent_output)})
    }
    pub(crate) fn into_parts(self)->(Arc<ChildProcess>,OwnedHandle,File,File){
        (self.process,self.thread,self.input,self.output)
    }
    pub(crate) fn resume(thread:&OwnedHandle)->Result<()>{
        if unsafe{ResumeThread(thread.as_raw_handle())}==u32::MAX{return Err(failure());}Ok(())
    }
}
