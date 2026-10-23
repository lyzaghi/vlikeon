//! Windows observations for the private gamepad helper. Process identity and
//! foreground/local activity are read here, not accepted over parent/peer IPC.
use crate::{activity::ActivityMonitor,failure,gamepad::TargetBinding,
    gamepad_host::{LocalGuard,LocalObservation}};
use omniplay_contracts::Result;
use std::os::windows::io::{AsRawHandle,FromRawHandle,OwnedHandle};
use windows_sys::Win32::{Foundation::*,System::{Threading::*,LibraryLoader::{SetDefaultDllDirectories,LOAD_LIBRARY_SEARCH_SYSTEM32}},
    UI::{WindowsAndMessaging::*,Input::KeyboardAndMouse::*}};

pub struct WindowsGamepadGuard{
    process:OwnedHandle,target:TargetBinding,activity:ActivityMonitor,
    baseline:u64,armed:bool,local_override:bool,
}
impl WindowsGamepadGuard{
    pub fn open(target:TargetBinding)->Result<Self>{
        if target.process_id==0||target.creation_time==0||target.window==0||
           target.window>usize::MAX as u64{return Err(failure());}
        if unsafe{SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32)}==0{return Err(failure());}
        // Narrow rights on the real process. Keeping this handle prevents PID
        // reuse from becoming a new authorization, even after the game exits.
        let handle=unsafe{OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION|PROCESS_SYNCHRONIZE,0,target.process_id)};
        if handle.is_null()||handle==INVALID_HANDLE_VALUE{return Err(failure());}
        // SAFETY: one newly acquired, non-null process handle with sole owner.
        let process=unsafe{OwnedHandle::from_raw_handle(handle)};
        let mut created=FILETIME::default();let mut exited=FILETIME::default();
        let mut kernel=FILETIME::default();let mut user=FILETIME::default();
        // SAFETY: live process handle and exact writable OS structures.
        if unsafe{WaitForSingleObject(process.as_raw_handle(),0)}!=WAIT_TIMEOUT||
           unsafe{GetProcessTimes(process.as_raw_handle(),&mut created,&mut exited,&mut kernel,&mut user)}==0||
           ((u64::from(created.dwHighDateTime)<<32)|u64::from(created.dwLowDateTime))!=target.creation_time{
            return Err(failure());
        }
        let activity=ActivityMonitor::start()?;let baseline=activity.count();
        Ok(Self{process,target,activity,baseline,armed:false,local_override:false})
    }
}
impl LocalGuard for WindowsGamepadGuard{
    fn observe(&mut self)->LocalObservation{
        let alive=unsafe{WaitForSingleObject(self.process.as_raw_handle(),0)}==WAIT_TIMEOUT;
        let window=self.target.window as usize as HWND;let mut pid=0;
        let foreground=alive&&unsafe{GetForegroundWindow()}==window&&
            unsafe{IsWindowVisible(window)}!=0&&unsafe{IsIconic(window)}==0&&
            unsafe{GetWindowThreadProcessId(window,&mut pid)}!=0&&pid==self.target.process_id;
        let override_key=[VK_LWIN,VK_RWIN,VK_LCONTROL,VK_RCONTROL,VK_LMENU,VK_RMENU,VK_F12,
            VK_LBUTTON,VK_RBUTTON,VK_MBUTTON]
            .into_iter().any(|key|(unsafe{GetAsyncKeyState(i32::from(key))}as u16&0x8000)!=0);
        let healthy=self.activity.healthy();
        let count=self.activity.count();
        if !self.armed&&foreground&&healthy&&self.activity.quiet()&&self.activity.count()==count{
            self.armed=true;self.baseline=count;
        }
        if !healthy||override_key||(self.armed&&self.activity.count()!=self.baseline){self.local_override=true;}
        LocalObservation{target:alive.then_some(self.target),
            foreground:foreground&&self.armed,local_override:self.local_override}
    }
}
