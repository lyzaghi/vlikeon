use crate::{Launched,Limits,PresentationLaunch,PresentedLaunched,unavailable};
use omniplay_contracts::Result;
use std::{ffi::{OsStr,c_void},fs::{File,OpenOptions},mem::size_of,os::windows::{ffi::OsStrExt,io::{AsRawHandle,FromRawHandle,OwnedHandle}},path::Path,ptr::{null,null_mut}};
use windows_sys::Win32::{Foundation::*,Security::{*,Authorization::*,Isolation::*},Storage::FileSystem::{FILE_GENERIC_EXECUTE,FILE_GENERIC_READ,FILE_GENERIC_WRITE},System::{Pipes::*,Threading::*,JobObjects::*,SystemInformation::GetWindowsDirectoryW}};
mod presentation;
mod storage;
mod registry_storage;
#[cfg(test)]mod storage_tests;
#[cfg(test)]mod device_free_tests;
pub use presentation::PresentationHost;
pub(crate) fn owned_thread_ended(thread:&std::thread::JoinHandle<()>)->bool{
    // Poll the held OS thread object, including its TLS/destructor lifetime;
    // JoinHandle::is_finished alone may precede actual thread termination.
    unsafe{WaitForSingleObject(thread.as_raw_handle(),0)==WAIT_OBJECT_0}
}

fn wide(value:&OsStr)->Result<Vec<u16>> {let mut out=value.encode_wide().collect::<Vec<_>>();if out.contains(&0){return Err(unavailable());}out.push(0);Ok(out)}
fn handle(value:HANDLE)->Result<OwnedHandle>{if value.is_null()||value==INVALID_HANDLE_VALUE{return Err(unavailable());}Ok(unsafe{OwnedHandle::from_raw_handle(value)})}

// Local test diagnostics name only the failed boundary. Production errors keep
// their closed public contract and never disclose profile/path/token details.
fn launch_checkpoint<T>(_stage:&'static str,result:Result<T>)->Result<T>{
    #[cfg(test)]if result.is_err(){eprintln!("Windows launch checkpoint: {_stage}");}
    result
}

struct ProfileIdentity{sid:PSID,name:Vec<u16>,lifecycle:std::sync::Mutex<()>,active:std::sync::atomic::AtomicU32,uncertain:std::sync::atomic::AtomicBool,deleted:std::sync::atomic::AtomicBool,launched:std::sync::atomic::AtomicBool}
pub struct ContainerIdentity {profile:std::sync::Arc<ProfileIdentity>}
// The SID is immutable, heap-owned and only inspected by Windows APIs. Its allocation
// is released exactly once after the identity is no longer used by a launch call.
unsafe impl Send for ProfileIdentity {}
unsafe impl Sync for ProfileIdentity {}
impl ProfileIdentity{
    fn retire(&self)->Result<()>{use std::sync::atomic::Ordering;let _lifecycle=self.lifecycle.lock().map_err(|_|unavailable())?;if self.deleted.load(Ordering::Acquire){return Ok(());}if self.active.load(Ordering::Acquire)!=0||self.uncertain.load(Ordering::Acquire){return Err(unavailable());}if unsafe{DeleteAppContainerProfile(self.name.as_ptr())}<0{return Err(unavailable());}self.deleted.store(true,Ordering::Release);Ok(())}
}
impl Drop for ProfileIdentity {fn drop(&mut self){let _=self.retire();unsafe{FreeSid(self.sid);}}}
fn session_name(name:&str)->bool{name.strip_prefix("OmniPlay.Session.").is_some_and(|suffix|suffix.len()==32&&suffix.bytes().all(|c|c.is_ascii_digit()||(b'a'..=b'f').contains(&c)))}

impl ContainerIdentity {
    pub fn create_for_session(name:&str)->Result<Self>{
        if !session_name(name){return Err(unavailable());}
        let moniker=wide(OsStr::new(name))?;let mut sid=null_mut();
        let result=unsafe{CreateAppContainerProfile(moniker.as_ptr(),moniker.as_ptr(),moniker.as_ptr(),null(),0,&mut sid)};
        // Never adopt an existing OS profile, even under our random namespace.
        if result<0{if !sid.is_null(){unsafe{FreeSid(sid);}}return Err(unavailable());}
        if sid.is_null(){return Err(unavailable());}
        Ok(Self{profile:std::sync::Arc::new(ProfileIdentity{sid,name:moniker,lifecycle:std::sync::Mutex::new(()),active:0.into(),uncertain:false.into(),deleted:false.into(),launched:false.into()})})
    }
    pub fn retire(&self)->Result<()>{self.profile.retire()}

    pub fn grant_game_read(&self,directory:&Path)->Result<()> {use std::sync::atomic::Ordering;let _lifecycle=self.profile.lifecycle.lock().map_err(|_|unavailable())?;if self.profile.deleted.load(Ordering::Acquire)||self.profile.launched.load(Ordering::Acquire){return Err(unavailable());}grant_access(directory,self.profile.sid,false,false)}
    pub fn grant_private_write(&self,directory:&Path)->Result<()> {
        use std::sync::atomic::Ordering;let _lifecycle=self.profile.lifecycle.lock().map_err(|_|unavailable())?;if self.profile.deleted.load(Ordering::Acquire)||self.profile.launched.load(Ordering::Acquire){return Err(unavailable());}
        grant_access(directory,self.profile.sid,true,false)?;
        // An AppContainer token runs at low integrity. A DACL grant alone cannot
        // authorize writes to a medium-integrity object. Lower only the explicit
        // private writable subtree, never ROM/firmware or installed program files.
        let sddl=wide(OsStr::new("S:(ML;OICI;NW;;;LW)"))?;let mut descriptor=null_mut();
        if unsafe{ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(),1,&mut descriptor,null_mut())}==0{return Err(unavailable());}
        let mut present=0;let mut defaulted=0;let mut sacl=null_mut();
        let obtained=unsafe{GetSecurityDescriptorSacl(descriptor,&mut present,&mut sacl,&mut defaulted)};
        let path=wide(directory.as_os_str())?;
        let result=if obtained!=0&&present!=0{unsafe{SetNamedSecurityInfoW(path.as_ptr(),SE_FILE_OBJECT,LABEL_SECURITY_INFORMATION,null_mut(),null_mut(),null(),sacl)}}else{ERROR_INVALID_SECURITY_DESCR};
        unsafe{LocalFree(descriptor);}
        if result!=ERROR_SUCCESS{return Err(unavailable());}Ok(())
    }

    pub fn launch(&self,executable:&Path,working_directory:&Path,limits:&Limits,before_resume:impl FnOnce(crate::ProcessIdentity)->Result<()>)->Result<Launched>{
        let _lifecycle=self.profile.lifecycle.lock().map_err(|_|unavailable())?;
        self.claim_launch(limits)?;self.launch_claimed(executable,working_directory,limits,None,before_resume)
    }
    fn claim_launch(&self,limits:&Limits)->Result<()>{
        use std::sync::atomic::Ordering;
        if self.profile.deleted.load(Ordering::Acquire)||self.profile.launched.swap(true,Ordering::AcqRel){return Err(unavailable());}
        if !(256*1024*1024..=8usize*1024*1024*1024).contains(&limits.memory_bytes){return Err(unavailable());}
        Ok(())
    }
    pub fn launch_presented(&self,executable:&Path,working_directory:&Path,limits:&Limits,spec:&PresentationLaunch<'_>,before_actor:impl FnOnce(crate::ProcessIdentity)->Result<()>,before_helper:impl FnOnce(crate::ProcessIdentity)->Result<()>)->Result<PresentedLaunched>{
        let _lifecycle=self.profile.lifecycle.lock().map_err(|_|unavailable())?;
        self.claim_launch(limits)?;let prepared=presentation::Prepared::launch(self.profile.clone(),spec,before_helper)?;let actor=self.launch_claimed(executable,working_directory,limits,Some(&prepared),before_actor)?;Ok(PresentedLaunched{actor,presentation:prepared.finish()})
    }
    fn launch_claimed(&self,executable:&Path,working_directory:&Path,limits:&Limits,presentation:Option<&presentation::Prepared>,before_resume:impl FnOnce(crate::ProcessIdentity)->Result<()>)->Result<Launched>{
        use std::sync::atomic::Ordering;
        let application=wide(executable.as_os_str())?;let directory=wide(working_directory.as_os_str())?;
        let path=executable.to_str().ok_or_else(unavailable)?;
        if path.contains('"'){return Err(unavailable());}
        let suffix=presentation.map(|value|format!(" --presentation-client-v2 {} {} {}",value.session,value.actor_input.as_raw_handle()as usize,value.actor_output.as_raw_handle()as usize)).unwrap_or_default();
        let mut command=wide(OsStr::new(&format!("\"{path}\" --stdio-runtime-v1{suffix}")))?;
        let (child_input,parent_input)=pipe_pair(true)?;
        let (parent_output,child_output)=pipe_pair(false)?;
        let error_file=OpenOptions::new().write(true).open("NUL").map_err(|_|unavailable())?;
        if unsafe{SetHandleInformation(error_file.as_raw_handle(),HANDLE_FLAG_INHERIT,HANDLE_FLAG_INHERIT)}==0{return Err(unavailable());}
        let mut inherited=vec![child_input.as_raw_handle(),child_output.as_raw_handle(),error_file.as_raw_handle()];if let Some(value)=presentation{inherited.push(value.actor_input.as_raw_handle());inherited.push(value.actor_output.as_raw_handle());}
        let mut capabilities=SECURITY_CAPABILITIES{AppContainerSid:self.profile.sid,Capabilities:null_mut(),CapabilityCount:0,Reserved:0};
        let mut attributes=launch_checkpoint("attribute-list",Attributes::new(4))?;
        launch_checkpoint("security-capabilities",attributes.set(PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,&mut capabilities))?;
        launch_checkpoint("inherited-handles",attributes.set_slice(PROC_THREAD_ATTRIBUTE_HANDLE_LIST,&inherited))?;
        // SDK WinBase.h policy bits: heap termination, disabled extension points,
        // prohibited dynamic code, local medium-integrity images, System32 preference.
        // The software-core build uses Windows fibers, never executable heap trampolines.
        let mut mitigations:u64=(1<<12)|(1<<32)|(1<<36)|(1<<52)|(1<<56)|(1<<60);
        // The private software actor has no GUI/device role. Enforce that at
        // the syscall boundary; a cooperating SDL_Init(0) branch alone is not
        // confinement. Never retry this mode with Win32k lockdown removed.
        if presentation.is_some(){mitigations|=1<<28;}
        launch_checkpoint("mitigation-policy",attributes.set(PROC_THREAD_ATTRIBUTE_MITIGATION_POLICY,&mut mitigations))?;
        let job=handle(unsafe{CreateJobObjectW(null(),null())})?;
        let mut job_limits=JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        job_limits.BasicLimitInformation.LimitFlags=JOB_OBJECT_LIMIT_ACTIVE_PROCESS|JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE|JOB_OBJECT_LIMIT_PROCESS_MEMORY|JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION;
        job_limits.BasicLimitInformation.ActiveProcessLimit=1;job_limits.ProcessMemoryLimit=limits.memory_bytes;
        if unsafe{SetInformationJobObject(job.as_raw_handle(),JobObjectExtendedLimitInformation,(&job_limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>()as u32)}==0{return Err(unavailable());}
        let cpu=JOBOBJECT_CPU_RATE_CONTROL_INFORMATION{ControlFlags:JOB_OBJECT_CPU_RATE_CONTROL_ENABLE|JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP,Anonymous:JOBOBJECT_CPU_RATE_CONTROL_INFORMATION_0{CpuRate:5000}};
        if unsafe{SetInformationJobObject(job.as_raw_handle(),JobObjectCpuRateControlInformation,(&cpu as *const JOBOBJECT_CPU_RATE_CONTROL_INFORMATION).cast(),size_of::<JOBOBJECT_CPU_RATE_CONTROL_INFORMATION>()as u32)}==0{return Err(unavailable());}
        let ui=JOBOBJECT_BASIC_UI_RESTRICTIONS{UIRestrictionsClass:JOB_OBJECT_UILIMIT_READCLIPBOARD|JOB_OBJECT_UILIMIT_WRITECLIPBOARD|JOB_OBJECT_UILIMIT_SYSTEMPARAMETERS|JOB_OBJECT_UILIMIT_DISPLAYSETTINGS|JOB_OBJECT_UILIMIT_EXITWINDOWS|JOB_OBJECT_UILIMIT_DESKTOP};
        if unsafe{SetInformationJobObject(job.as_raw_handle(),JobObjectBasicUIRestrictions,(&ui as *const JOBOBJECT_BASIC_UI_RESTRICTIONS).cast(),size_of::<JOBOBJECT_BASIC_UI_RESTRICTIONS>()as u32)}==0{return Err(unavailable());}
        // Associate the kill-on-close job inside CreateProcess itself. Creating
        // suspended and assigning afterwards leaves an orphan window if the
        // broker dies between those two calls. No assign-after-create fallback.
        let jobs=[job.as_raw_handle()];launch_checkpoint("creation-time-job",attributes.set_slice(PROC_THREAD_ATTRIBUTE_JOB_LIST,&jobs))?;
        let mut startup=STARTUPINFOEXW::default();startup.StartupInfo.cb=size_of::<STARTUPINFOEXW>()as u32;startup.StartupInfo.dwFlags=STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput=child_input.as_raw_handle();startup.StartupInfo.hStdOutput=child_output.as_raw_handle();startup.StartupInfo.hStdError=error_file.as_raw_handle();startup.lpAttributeList=attributes.pointer;
        let mut environment=environment(working_directory)?;let mut information=PROCESS_INFORMATION::default();
        let created=unsafe{CreateProcessW(application.as_ptr(),command.as_mut_ptr(),null(),null(),1,CREATE_SUSPENDED|CREATE_NO_WINDOW|CREATE_UNICODE_ENVIRONMENT|EXTENDED_STARTUPINFO_PRESENT,environment.as_mut_ptr().cast(),directory.as_ptr(),&startup.StartupInfo,&mut information)};
        if created==0{
            #[cfg(test)]eprintln!("Windows launch checkpoint: CreateProcessW error {}",unsafe{GetLastError()});
            return Err(unavailable());
        }
        let process=handle(information.hProcess)?;
        self.profile.active.fetch_add(1,Ordering::AcqRel);let guard=ContainedProcess{process,job,profile:self.profile.clone()};
        let thread=handle(information.hThread)?;
        // Confirm creation-time membership before recording identity or allowing
        // the first worker instruction to run.
        let mut associated=0;
        if unsafe{IsProcessInJob(guard.process.as_raw_handle(),guard.job.as_raw_handle(),&mut associated)}==0||associated==0{return Err(unavailable());}
        launch_checkpoint("verify-actor-token",storage::verify_actor_token(&guard.process,self.profile.sid))?;
        launch_checkpoint("seal-private-registry",registry_storage::seal(&guard.process,self.profile.sid))?;
        // Resolve/materialize the registry namespace before sealing its backing
        // filesystem tree. Both fallible policies remain inside the suspended,
        // job-owned interval; no actor instruction or process record precedes
        // them, and failure never retries with a weaker policy.
        let name=self.profile.name.strip_suffix(&[0u16][..]).ok_or_else(unavailable)?;
        let name=String::from_utf16(name).map_err(|_|unavailable())?;
        launch_checkpoint("seal-default-files",storage::seal_default_files(self.profile.sid,&name))?;
        launch_checkpoint("pre-resume-admission",before_resume(process_identity(&guard.process)?))?;
        if unsafe{ResumeThread(thread.as_raw_handle())}==u32::MAX{return Err(unavailable());}
        drop(thread);drop(child_input);drop(child_output);drop(error_file);
        Ok(Launched{process:guard,input:File::from(parent_input),output:File::from(parent_output)})
    }
}

pub struct ContainedProcess {process:OwnedHandle,job:OwnedHandle,profile:std::sync::Arc<ProfileIdentity>}
impl ContainedProcess {
    pub fn terminate(&self){unsafe{TerminateJobObject(self.job.as_raw_handle(),1);if WaitForSingleObject(self.process.as_raw_handle(),0)!=WAIT_OBJECT_0{TerminateProcess(self.process.as_raw_handle(),1);}WaitForSingleObject(self.process.as_raw_handle(),5000);}}
    pub fn has_exited(&self)->bool{unsafe{WaitForSingleObject(self.process.as_raw_handle(),0)==WAIT_OBJECT_0}}
}
impl Drop for ContainedProcess {fn drop(&mut self){use std::sync::atomic::Ordering;self.terminate();if !self.has_exited(){self.profile.uncertain.store(true,Ordering::Release);}self.profile.active.fetch_sub(1,Ordering::AcqRel);}}
fn process_identity(process:&OwnedHandle)->Result<crate::ProcessIdentity>{
    let mut created=FILETIME::default();let mut exited=FILETIME::default();let mut kernel=FILETIME::default();let mut user=FILETIME::default();
    if unsafe{GetProcessTimes(process.as_raw_handle(),&mut created,&mut exited,&mut kernel,&mut user)}==0{return Err(unavailable());}
    let id=unsafe{GetProcessId(process.as_raw_handle())};let time=(u64::from(created.dwHighDateTime)<<32)|u64::from(created.dwLowDateTime);if id==0||time==0{return Err(unavailable());}Ok(crate::ProcessIdentity{process_id:id,creation_time:time})
}
pub fn retire_abandoned_profile(name:&str,process:Option<crate::ProcessIdentity>)->Result<()>{
    if !session_name(name){return Err(unavailable());}
    if let Some(expected)=process{
        confirmed_process_ended(expected)?;
    }
    let moniker=wide(OsStr::new(name))?;if unsafe{DeleteAppContainerProfile(moniker.as_ptr())}<0{return Err(unavailable());}Ok(())
}
pub fn confirmed_process_ended(expected:crate::ProcessIdentity)->Result<()>{
    if expected.process_id==0||expected.creation_time==0{return Err(unavailable());}let raw=unsafe{OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION|PROCESS_SYNCHRONIZE,0,expected.process_id)};
    if raw.is_null(){if unsafe{GetLastError()}!=ERROR_INVALID_PARAMETER{return Err(unavailable());}}
    else{let process=handle(raw)?;let actual=process_identity(&process)?;if actual.creation_time==expected.creation_time&&unsafe{WaitForSingleObject(process.as_raw_handle(),0)}!=WAIT_OBJECT_0{return Err(unavailable());}}Ok(())
}

fn pipe_pair(parent_writes:bool)->Result<(OwnedHandle,OwnedHandle)>{
    let mut read=null_mut();let mut write=null_mut();let security=SECURITY_ATTRIBUTES{nLength:size_of::<SECURITY_ATTRIBUTES>()as u32,lpSecurityDescriptor:null_mut(),bInheritHandle:1};
    if unsafe{CreatePipe(&mut read,&mut write,&security,65536)}==0{return Err(unavailable());}
    let read=handle(read)?;let write=handle(write)?;let parent=if parent_writes{write.as_raw_handle()}else{read.as_raw_handle()};
    if unsafe{SetHandleInformation(parent,HANDLE_FLAG_INHERIT,0)}==0{return Err(unavailable());}Ok((read,write))
}

struct Attributes {storage:Vec<usize>,pointer:LPPROC_THREAD_ATTRIBUTE_LIST}
impl Attributes {
    fn new(count:u32)->Result<Self>{
        let mut bytes=0usize;unsafe{InitializeProcThreadAttributeList(null_mut(),count,0,&mut bytes);}
        if bytes==0||bytes>65536{return Err(unavailable());}
        let mut storage=vec![0usize;(bytes+size_of::<usize>()-1)/size_of::<usize>()];let pointer=storage.as_mut_ptr().cast();
        if unsafe{InitializeProcThreadAttributeList(pointer,count,0,&mut bytes)}==0{return Err(unavailable());}Ok(Self{storage,pointer})
    }
    fn set<T>(&mut self,attribute:u32,value:&mut T)->Result<()>{if unsafe{UpdateProcThreadAttribute(self.pointer,0,attribute as usize,(value as *mut T).cast(),size_of::<T>(),null_mut(),null())}==0{return Err(unavailable());}Ok(())}
    fn set_slice<T>(&mut self,attribute:u32,value:&[T])->Result<()>{if unsafe{UpdateProcThreadAttribute(self.pointer,0,attribute as usize,value.as_ptr().cast(),std::mem::size_of_val(value),null_mut(),null())}==0{return Err(unavailable());}Ok(())}
}
impl Drop for Attributes {fn drop(&mut self){unsafe{DeleteProcThreadAttributeList(self.pointer);}let _=&self.storage;}}

fn local_app_data()->Result<String>{
    use windows_sys::Win32::{UI::Shell::{SHGetKnownFolderPath,FOLDERID_LocalAppData},System::Com::CoTaskMemFree};
    let mut raw=null_mut();
    let status=unsafe{SHGetKnownFolderPath(&FOLDERID_LocalAppData,0,null_mut(),&mut raw)};
    let value=(||{
        if status<0||raw.is_null(){return Err(unavailable());}
        let mut units=Vec::new();
        for index in 0..32768{
            let unit=unsafe{*raw.add(index)};
            if unit==0{
                let value=String::from_utf16(&units).map_err(|_|unavailable())?;
                if !Path::new(&value).is_absolute(){return Err(unavailable());}
                return Ok(value);
            }
            units.push(unit);
        }
        Err(unavailable())
    })();
    if !raw.is_null(){unsafe{CoTaskMemFree(raw.cast());}}
    value
}

fn environment(working_directory:&Path)->Result<Vec<u16>> {
    let mut system=vec![0u16;32768];let length=unsafe{GetWindowsDirectoryW(system.as_mut_ptr(),system.len()as u32)}as usize;
    if length==0||length>=system.len(){return Err(unavailable());}
    let system=String::from_utf16(&system[..length]).map_err(|_|unavailable())?;let work=working_directory.to_str().ok_or_else(unavailable)?;
    // Windows resolves fresh AppContainer profile storage during CreateProcess.
    // Supply its required known-folder location without inheriting arbitrary
    // parent variables/credentials or granting any additional filesystem access.
    let local=local_app_data()?;
    let mut entries=vec![format!("LOCALAPPDATA={local}"),format!("OMNIPLAY_PRIVATE_SCRATCH={work}\\scratch"),format!("PATH={system}\\System32"),format!("SystemRoot={system}"),format!("TEMP={work}\\scratch"),format!("TMP={work}\\scratch"),format!("WINDIR={system}")];entries.sort_by_key(|v|v.to_ascii_lowercase());
    let mut block=Vec::new();for entry in entries{block.extend(wide(OsStr::new(&entry))?);}block.push(0);Ok(block)
}

pub fn grant_public_runtime_read(directory:&Path)->Result<()> {
    let value=wide(OsStr::new("S-1-15-2-1"))?;let mut sid=null_mut();
    if unsafe{ConvertStringSidToSidW(value.as_ptr(),&mut sid)}==0{return Err(unavailable());}
    let result=grant_access(directory,sid,false,true);unsafe{LocalFree(sid);};result
}
fn grant_access(directory:&Path,sid:PSID,writable:bool,executable_files:bool)->Result<()> {
    if !directory.is_absolute()||!directory.is_dir()||std::fs::symlink_metadata(directory).map_err(|_|unavailable())?.file_type().is_symlink(){return Err(unavailable());}
    let path=wide(directory.as_os_str())?;let mut old=null_mut();let mut descriptor=null_mut();
    if unsafe{GetNamedSecurityInfoW(path.as_ptr(),SE_FILE_OBJECT,DACL_SECURITY_INFORMATION,null_mut(),null_mut(),&mut old,null_mut(),&mut descriptor)}!=ERROR_SUCCESS{return Err(unavailable());}
    // FILE_TRAVERSE shares its bit with FILE_EXECUTE. Inherit traversal only to
    // directories; ordinary game/save files do not receive execute permission.
    let trustee=TRUSTEE_W{pMultipleTrustee:null_mut(),MultipleTrusteeOperation:NO_MULTIPLE_TRUSTEE,TrusteeForm:TRUSTEE_IS_SID,TrusteeType:TRUSTEE_IS_WELL_KNOWN_GROUP,ptstrName:sid.cast()};
    let access=FILE_GENERIC_READ|(if executable_files{FILE_GENERIC_EXECUTE}else{0})|(if writable{FILE_GENERIC_WRITE|0x00010000|0x40}else{0}); // DELETE and FILE_DELETE_CHILD
    let entries=[EXPLICIT_ACCESS_W{grfAccessPermissions:access,grfAccessMode:GRANT_ACCESS,grfInheritance:SUB_CONTAINERS_AND_OBJECTS_INHERIT,Trustee:trustee},EXPLICIT_ACCESS_W{grfAccessPermissions:0x20,grfAccessMode:GRANT_ACCESS,grfInheritance:SUB_CONTAINERS_ONLY_INHERIT,Trustee:trustee}];
    let mut updated=null_mut();let built=unsafe{SetEntriesInAclW(entries.len()as u32,entries.as_ptr(),old,&mut updated)};
    let result=if built==ERROR_SUCCESS{unsafe{SetNamedSecurityInfoW(path.as_ptr(),SE_FILE_OBJECT,DACL_SECURITY_INFORMATION,null_mut(),null_mut(),updated,null())}}else{built};
    unsafe{if !updated.is_null(){LocalFree(updated.cast::<c_void>());}LocalFree(descriptor);}
    if result!=ERROR_SUCCESS{return Err(unavailable());}Ok(())
}
