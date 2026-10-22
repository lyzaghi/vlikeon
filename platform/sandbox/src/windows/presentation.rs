use super::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use omniplay_contracts::{ErrorCode,PlatformError};
use std::sync::{Arc,atomic::Ordering};

pub struct PresentationHost{process:OwnedHandle,job:OwnedHandle,profile:Arc<ProfileIdentity>}
pub(super) struct Prepared{host:PresentationHost,pub actor_input:OwnedHandle,pub actor_output:OwnedHandle,pub session:String}
fn session(value:&str)->bool{value.len()==36&&value.bytes().enumerate().all(|(at,byte)|if matches!(at,8|13|18|23){byte==b'-'}else{byte.is_ascii_digit()||(b'a'..=b'f').contains(&byte)})&&value.bytes().any(|byte|matches!(byte,b'1'..=b'9'|b'a'..=b'f'))}
impl Prepared{
    pub(super) fn launch(profile:Arc<ProfileIdentity>,spec:&PresentationLaunch<'_>,before_resume:impl FnOnce(crate::ProcessIdentity)->Result<()>)->Result<Self>{
        if !session(spec.session)||!(1..=4096).contains(&spec.maximum_width)||!(1..=4096).contains(&spec.maximum_height)||spec.volume_percent>100||!(256usize*1024*1024..=2usize*1024*1024*1024).contains(&spec.memory_bytes)||u64::from(spec.maximum_width)*u64::from(spec.maximum_height)*16+128*1024*1024>spec.memory_bytes as u64{return Err(unavailable());}
        if !spec.executable.is_absolute()||!spec.working_directory.is_absolute()||!spec.temporary_directory.is_absolute()||!spec.temporary_directory.is_dir(){return Err(unavailable());}
        let executable=spec.executable.to_str().ok_or_else(unavailable)?;if executable.contains('"'){return Err(unavailable());}
        let (helper_input,actor_output)=pipe_pair(true)?;let (actor_input,helper_output)=pipe_pair(false)?;
        for value in[actor_input.as_raw_handle(),actor_output.as_raw_handle()]{if unsafe{SetHandleInformation(value,HANDLE_FLAG_INHERIT,HANDLE_FLAG_INHERIT)}==0{return Err(unavailable());}}
        let error=OpenOptions::new().write(true).open("NUL").map_err(|_|unavailable())?;if unsafe{SetHandleInformation(error.as_raw_handle(),HANDLE_FLAG_INHERIT,HANDLE_FLAG_INHERIT)}==0{return Err(unavailable());}
        let inherited=[helper_input.as_raw_handle(),helper_output.as_raw_handle(),error.as_raw_handle()];let mut attributes=Attributes::new(3)?;attributes.set_slice(PROC_THREAD_ATTRIBUTE_HANDLE_LIST,&inherited)?;
        // This reviewed normal-user UI process loads no core or ROM. It is a
        // trusted desktop/audio boundary, not another AppContainer. The actor
        // receives only private media pipes, never the helper's process token.
        let mut mitigations:u64=(1<<12)|(1<<32)|(1<<36)|(1<<52)|(1<<56)|(1<<60);attributes.set(PROC_THREAD_ATTRIBUTE_MITIGATION_POLICY,&mut mitigations)?;
        let job=handle(unsafe{CreateJobObjectW(null(),null())})?;let mut limits=JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();limits.BasicLimitInformation.LimitFlags=JOB_OBJECT_LIMIT_ACTIVE_PROCESS|JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE|JOB_OBJECT_LIMIT_PROCESS_MEMORY|JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION;limits.BasicLimitInformation.ActiveProcessLimit=1;limits.ProcessMemoryLimit=spec.memory_bytes;
        if unsafe{SetInformationJobObject(job.as_raw_handle(),JobObjectExtendedLimitInformation,(&limits as*const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>()as u32)}==0{return Err(unavailable());}
        let ui=JOBOBJECT_BASIC_UI_RESTRICTIONS{UIRestrictionsClass:JOB_OBJECT_UILIMIT_READCLIPBOARD|JOB_OBJECT_UILIMIT_WRITECLIPBOARD|JOB_OBJECT_UILIMIT_SYSTEMPARAMETERS|JOB_OBJECT_UILIMIT_DISPLAYSETTINGS|JOB_OBJECT_UILIMIT_EXITWINDOWS|JOB_OBJECT_UILIMIT_DESKTOP};
        if unsafe{SetInformationJobObject(job.as_raw_handle(),JobObjectBasicUIRestrictions,(&ui as*const JOBOBJECT_BASIC_UI_RESTRICTIONS).cast(),size_of::<JOBOBJECT_BASIC_UI_RESTRICTIONS>()as u32)}==0{return Err(unavailable());}
        let jobs=[job.as_raw_handle()];attributes.set_slice(PROC_THREAD_ATTRIBUTE_JOB_LIST,&jobs)?;
        let application=wide(spec.executable.as_os_str())?;let directory=wide(spec.working_directory.as_os_str())?;
        let mut command=wide(OsStr::new(&format!("\"{executable}\" --stdio-presentation-v2 {} {} {} {} {}",spec.session,spec.maximum_width,spec.maximum_height,spec.volume_percent,if spec.haptics{"haptics"}else{"no-haptics"})))?;
        let mut startup=STARTUPINFOEXW::default();startup.StartupInfo.cb=size_of::<STARTUPINFOEXW>()as u32;startup.StartupInfo.dwFlags=STARTF_USESTDHANDLES;startup.StartupInfo.hStdInput=helper_input.as_raw_handle();startup.StartupInfo.hStdOutput=helper_output.as_raw_handle();startup.StartupInfo.hStdError=error.as_raw_handle();startup.lpAttributeList=attributes.pointer;
        let mut env=helper_environment(spec.temporary_directory)?;let mut information=PROCESS_INFORMATION::default();
        if unsafe{CreateProcessW(application.as_ptr(),command.as_mut_ptr(),null(),null(),1,CREATE_SUSPENDED|CREATE_NO_WINDOW|CREATE_UNICODE_ENVIRONMENT|EXTENDED_STARTUPINFO_PRESENT,env.as_mut_ptr().cast(),directory.as_ptr(),&startup.StartupInfo,&mut information)}==0{return Err(unavailable());}
        let process=handle(information.hProcess)?;profile.active.fetch_add(1,Ordering::AcqRel);let host=PresentationHost{process,job,profile};let thread=handle(information.hThread)?;
        let mut associated=0;if unsafe{IsProcessInJob(host.process.as_raw_handle(),host.job.as_raw_handle(),&mut associated)}==0||associated==0{return Err(unavailable());}before_resume(process_identity(&host.process)?)?;
        if unsafe{ResumeThread(thread.as_raw_handle())}==u32::MAX{return Err(unavailable());}
        Ok(Self{host,actor_input,actor_output,session:spec.session.to_owned()})
    }
    pub(super) fn finish(self)->PresentationHost{self.host}
}
impl PresentationHost{
    pub fn terminate(&self){unsafe{TerminateJobObject(self.job.as_raw_handle(),1);if WaitForSingleObject(self.process.as_raw_handle(),0)!=WAIT_OBJECT_0{TerminateProcess(self.process.as_raw_handle(),1);}WaitForSingleObject(self.process.as_raw_handle(),5000);}}
    pub fn has_exited(&self)->bool{unsafe{WaitForSingleObject(self.process.as_raw_handle(),0)==WAIT_OBJECT_0}}
    pub fn activate(&self)->Result<()>{
        if self.has_exited(){return Err(unavailable());}let identity=process_identity(&self.process)?;let mut found=Search{process_id:identity.process_id,window:null_mut(),ambiguous:false};
        let enumerated=unsafe{EnumWindows(Some(find_window),(&mut found as*mut Search)as isize)};
        if enumerated==0||found.ambiguous||found.window.is_null(){return Err(unavailable());}
        // Try only the exact live helper's own top-level window. Foreground lock
        // policy is respected; there is no input injection/thread attachment or
        // global AllowSetForegroundWindow grant when the OS refuses this action.
        unsafe{ShowWindowAsync(found.window,SW_RESTORE);}
        if unsafe{GetForegroundWindow()}!=found.window{unsafe{SetForegroundWindow(found.window);}}
        if unsafe{GetForegroundWindow()}!=found.window{return Err(PlatformError::new(ErrorCode::PermissionDenied,"Focus the OmniPlay game window before resuming").recover("Select the game window and press Escape, or choose Resume again"));}Ok(())
    }
}
impl Drop for PresentationHost{fn drop(&mut self){self.terminate();if !self.has_exited(){self.profile.uncertain.store(true,Ordering::Release);}self.profile.active.fetch_sub(1,Ordering::AcqRel);}}
struct Search{process_id:u32,window:HWND,ambiguous:bool}
unsafe extern "system" fn find_window(window:HWND,context:isize)->i32{
    if context==0{return 0;}let found=unsafe{&mut*(context as*mut Search)};let mut process=0;unsafe{GetWindowThreadProcessId(window,&mut process);}
    if process!=found.process_id||unsafe{IsWindowVisible(window)}==0||unsafe{GetAncestor(window,GA_ROOT)}!=window{return 1;}
    let mut class=[0u16;64];let length=unsafe{GetClassNameW(window,class.as_mut_ptr(),class.len()as i32)};let expected=[b'S'as u16,b'D'as u16,b'L'as u16,b'_'as u16,b'a'as u16,b'p'as u16,b'p'as u16];
    if length!=expected.len()as i32||class[..expected.len()]!=expected{return 1;}if !found.window.is_null(){found.ambiguous=true;return 0;}found.window=window;1
}
fn helper_environment(temporary:&Path)->Result<Vec<u16>>{
    let mut system=vec![0u16;32768];let length=unsafe{GetWindowsDirectoryW(system.as_mut_ptr(),system.len()as u32)}as usize;if length==0||length>=system.len(){return Err(unavailable());}let system=String::from_utf16(&system[..length]).map_err(|_|unavailable())?;let temporary=temporary.to_str().ok_or_else(unavailable)?;
    let mut entries=vec![format!("PATH={system}\\System32"),format!("SystemRoot={system}"),format!("TEMP={temporary}"),format!("TMP={temporary}"),format!("WINDIR={system}")];entries.sort_by_key(|value|value.to_ascii_lowercase());let mut result=Vec::new();for entry in entries{result.extend(wide(OsStr::new(&entry))?);}result.push(0);Ok(result)
}
