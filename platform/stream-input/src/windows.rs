use crate::{failure,protocol::{Target,Observation}};
use omniplay_contracts::{RemoteInputFrame,Result};
use std::{collections::BTreeSet,mem::size_of,os::windows::io::{AsRawHandle,BorrowedHandle,FromRawHandle,OwnedHandle},path::PathBuf,time::{Duration,Instant}};
use windows_sys::Win32::{Foundation::*,Graphics::Gdi::ClientToScreen,System::{Threading::*,SystemInformation::GetWindowsDirectoryW,LibraryLoader::{SetDefaultDllDirectories,LOAD_LIBRARY_SEARCH_SYSTEM32}},UI::{WindowsAndMessaging::*,Input::KeyboardAndMouse::*,HiDpi::*}};

pub fn system_directory()->Result<PathBuf>{let mut data=vec![0u16;32768];let n=unsafe{GetWindowsDirectoryW(data.as_mut_ptr(),data.len()as u32)}as usize;if n==0||n>=data.len(){return Err(failure());}Ok(PathBuf::from(String::from_utf16(&data[..n]).map_err(|_|failure())?))}
pub fn target_identity(process:BorrowedHandle<'_>)->Result<Target>{identity(process.as_raw_handle())}
fn identity(handle:HANDLE)->Result<Target>{
    let process_id=unsafe{GetProcessId(handle)};let mut created=FILETIME::default();let mut exited=FILETIME::default();let mut kernel=FILETIME::default();let mut user=FILETIME::default();
    if process_id==0||unsafe{WaitForSingleObject(handle,0)}!=WAIT_TIMEOUT||unsafe{GetProcessTimes(handle,&mut created,&mut exited,&mut kernel,&mut user)}==0{return Err(failure());}
    Ok(Target{process_id,creation_time:(u64::from(created.dwHighDateTime)<<32)|u64::from(created.dwLowDateTime)})
}
#[derive(Clone,Copy,PartialEq,Eq,PartialOrd,Ord)]struct Key{scan:u16,extended:bool,vk:u16}
fn key(value:&str)->Option<Key>{
    let(scan,extended,vk)=match value{
        "ArrowUp"=>(0x48,true,VK_UP),"ArrowDown"=>(0x50,true,VK_DOWN),"ArrowLeft"=>(0x4b,true,VK_LEFT),"ArrowRight"=>(0x4d,true,VK_RIGHT),
        "Space"=>(0x39,false,VK_SPACE),"Enter"=>(0x1c,false,VK_RETURN),"Tab"=>(0x0f,false,VK_TAB),"Escape"=>(0x01,false,VK_ESCAPE),"Backspace"=>(0x0e,false,VK_BACK),"ShiftLeft"=>(0x2a,false,VK_LSHIFT),"ShiftRight"=>(0x36,false,VK_RSHIFT),
        _=>{
            if let Some(letter)=value.strip_prefix("Key").filter(|v|v.len()==1){let index=(letter.as_bytes()[0].checked_sub(b'A')?)as usize;let scans=[0x1e,0x30,0x2e,0x20,0x12,0x21,0x22,0x23,0x17,0x24,0x25,0x26,0x32,0x31,0x18,0x19,0x10,0x13,0x1f,0x14,0x16,0x2f,0x11,0x2d,0x15,0x2c];if index>=scans.len(){return None;}(scans[index],false,u16::from(letter.as_bytes()[0]))}
            else if let Some(digit)=value.strip_prefix("Digit").filter(|v|v.len()==1){let n=digit.as_bytes()[0].checked_sub(b'0')?;if n>9{return None;}(if n==0{0x0b}else{u16::from(n)+1},false,u16::from(digit.as_bytes()[0]))}else{return None;}
        }
    };Some(Key{scan,extended,vk})
}
fn keyboard(key:Key,up:bool)->INPUT{INPUT{r#type:INPUT_KEYBOARD,Anonymous:INPUT_0{ki:KEYBDINPUT{wVk:0,wScan:key.scan,dwFlags:KEYEVENTF_SCANCODE|(if key.extended{KEYEVENTF_EXTENDEDKEY}else{0})|(if up{KEYEVENTF_KEYUP}else{0}),time:0,dwExtraInfo:crate::activity::tag()}}}}
fn mouse(flags:u32,data:u32,x:i32,y:i32)->INPUT{INPUT{r#type:INPUT_MOUSE,Anonymous:INPUT_0{mi:MOUSEINPUT{dx:x,dy:y,mouseData:data,dwFlags:flags,time:0,dwExtraInfo:crate::activity::tag()}}}}
fn button_flags(bit:u8,up:bool)->u32{match(bit,up){(1,false)=>MOUSEEVENTF_LEFTDOWN,(1,true)=>MOUSEEVENTF_LEFTUP,(2,false)=>MOUSEEVENTF_RIGHTDOWN,(2,true)=>MOUSEEVENTF_RIGHTUP,(4,false)=>MOUSEEVENTF_MIDDLEDOWN,(4,true)=>MOUSEEVENTF_MIDDLEUP,_=>0}}
fn pressed(vk:u16)->bool{(unsafe{GetAsyncKeyState(i32::from(vk))}as u16&0x8000)!=0}
fn send(inputs:&[INPUT])->bool{inputs.is_empty()||unsafe{SendInput(inputs.len()as u32,inputs.as_ptr(),size_of::<INPUT>()as i32)}==inputs.len()as u32}

pub(crate) struct TargetInput{
    process:OwnedHandle,target:Target,window:HWND,start:Instant,deadline:Instant,last_input:Instant,sequence:u64,
    keys:BTreeSet<Key>,buttons:u8,activated:bool,state:&'static str,reason:Option<&'static str>,
    activity:crate::activity::ActivityMonitor,activity_baseline:u64,
}
impl TargetInput{
    pub fn open(target:Target,expires_at_ms:u64)->Result<Self>{
        let start=Instant::now();
        let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|_|failure())?.as_millis()as u64;
        if target.process_id==0||target.creation_time==0||expires_at_ms<=now||expires_at_ms-now>30*60*1000{return Err(failure());}
        let remaining_ms=expires_at_ms-now;
        if unsafe{SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)}==0&&unsafe{AreDpiAwarenessContextsEqual(GetThreadDpiAwarenessContext(),DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)}==0{return Err(failure());}
        if unsafe{SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32)}==0{return Err(failure());}
        let handle=unsafe{OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION|PROCESS_SYNCHRONIZE,0,target.process_id)};if handle.is_null()||handle==INVALID_HANDLE_VALUE{return Err(failure());}let process=unsafe{OwnedHandle::from_raw_handle(handle)};
        let actual=identity(process.as_raw_handle())?;if actual.process_id!=target.process_id||actual.creation_time!=target.creation_time{return Err(failure());}
        let activity=crate::activity::ActivityMonitor::start()?;let activity_baseline=activity.count();
        Ok(Self{process,target,window:std::ptr::null_mut(),start,deadline:start+Duration::from_millis(remaining_ms),last_input:start,sequence:0,keys:BTreeSet::new(),buttons:0,activated:false,state:"waiting_for_target",reason:None,activity,activity_baseline})
    }
    fn foreground(&self)->Option<HWND>{
        if unsafe{WaitForSingleObject(self.process.as_raw_handle(),0)}!=WAIT_TIMEOUT{return None;}
        let window=unsafe{GetForegroundWindow()};if window.is_null()||unsafe{IsIconic(window)}!=0||unsafe{IsWindowVisible(window)}==0{return None;}let mut pid=0;if unsafe{GetWindowThreadProcessId(window,&mut pid)}==0||pid!=self.target.process_id{return None;}Some(window)
    }
    fn override_active()->bool{[VK_LWIN,VK_RWIN,VK_LCONTROL,VK_RCONTROL,VK_LMENU,VK_RMENU,VK_F12].into_iter().any(pressed)}
    pub fn tick(&mut self){
        if self.state=="revoked"{if self.reason==Some("release_failed"){self.release();}return;}let now=Instant::now();
        if now>=self.deadline{self.revoke("grant_expired");return;}
        if unsafe{WaitForSingleObject(self.process.as_raw_handle(),0)}!=WAIT_TIMEOUT{self.revoke("game_exited");return;}
        if Self::override_active(){self.revoke("local_override");return;}
        if !self.activity.healthy(){self.revoke("local_watchdog_unavailable");return;}
        let foreground=self.foreground();
        if !self.activated{
            if now.duration_since(self.start)>Duration::from_secs(30){self.revoke("target_focus_timeout");return;}
            if let Some(window)=foreground{if self.activity.quiet(){self.window=window;self.activated=true;self.state="idle";self.last_input=now;self.activity_baseline=self.activity.count();}}
            return;
        }
        if foreground!=Some(self.window){self.revoke("target_focus_lost");return;}
        if self.activity.count()!=self.activity_baseline{self.revoke("local_input_active");return;}
        if now.duration_since(self.last_input)>Duration::from_millis(250){self.release();}
    }
    pub fn apply(&mut self,sequence:u64,frame:&RemoteInputFrame)->Result<()>{
        frame.validate()?;if sequence==0||sequence>10000000||sequence<=self.sequence{return Err(failure());}self.sequence=sequence;self.tick();
        if self.state=="revoked"||!self.activated{return Ok(());}
        // Virtual gamepads require a separately qualified device adapter. They are
        // not silently translated to arbitrary keyboard shortcuts here.
        if frame.pads.iter().any(|p|p.buttons!=0||p.axes.iter().any(|a|a.abs()>0.05)){self.revoke("gamepad_unavailable");return Ok(());}
        let wanted:BTreeSet<Key>=frame.keys.iter().map(|v|key(v).ok_or_else(failure)).collect::<Result<_>>()?;
        let thread=unsafe{GetWindowThreadProcessId(self.window,std::ptr::null_mut())};let layout=unsafe{GetKeyboardLayout(thread)};
        if thread==0||layout.is_null(){self.revoke("input_unmapped");return Ok(());}
        for new in wanted.difference(&self.keys){let scan=u32::from(new.scan)|(if new.extended{0xe000}else{0});let vk=unsafe{MapVirtualKeyExW(scan,MAPVK_VSC_TO_VK_EX,layout)};if vk==0||vk>u32::from(u16::MAX){self.revoke("input_unmapped");return Ok(());}if pressed(vk as u16){self.revoke("local_key_active");return Ok(());}}
        let mut inputs=Vec::with_capacity(80);
        for old in self.keys.difference(&wanted){inputs.push(keyboard(*old,true));}
        for new in wanted.difference(&self.keys){inputs.push(keyboard(*new,false));}
        let mouse_changed=frame.mouse.dx!=0||frame.mouse.dy!=0||frame.mouse.wheel!=0||frame.mouse.buttons!=self.buttons;
        if mouse_changed{
            let mut rect=RECT::default();let mut point=POINT::default();let window=self.window;
            if unsafe{GetClientRect(window,&mut rect)}==0||rect.right<=rect.left||rect.bottom<=rect.top||unsafe{GetCursorPos(&mut point)}==0{self.revoke("pointer_unavailable");return Ok(());}
            let mut top=POINT{x:rect.left,y:rect.top};let mut bottom=POINT{x:rect.right,y:rect.bottom};
            if unsafe{ClientToScreen(window,&mut top)}==0||unsafe{ClientToScreen(window,&mut bottom)}==0||point.x<top.x||point.x>=bottom.x||point.y<top.y||point.y>=bottom.y||unsafe{GetAncestor(WindowFromPoint(point),GA_ROOT)}!=unsafe{GetAncestor(window,GA_ROOT)}{self.revoke("pointer_outside_target");return Ok(());}
            let x=(i64::from(point.x)+i64::from(frame.mouse.dx)).clamp(i64::from(top.x),i64::from(bottom.x)-1);let y=(i64::from(point.y)+i64::from(frame.mouse.dy)).clamp(i64::from(top.y),i64::from(bottom.y)-1);
            let vx=unsafe{GetSystemMetrics(SM_XVIRTUALSCREEN)};let vy=unsafe{GetSystemMetrics(SM_YVIRTUALSCREEN)};let vw=unsafe{GetSystemMetrics(SM_CXVIRTUALSCREEN)};let vh=unsafe{GetSystemMetrics(SM_CYVIRTUALSCREEN)};if vw<=1||vh<=1{self.revoke("pointer_unavailable");return Ok(());}
            if frame.mouse.dx!=0||frame.mouse.dy!=0{let absolute_x=((x-i64::from(vx))*65535/i64::from(vw-1)).clamp(0,65535)as i32;let absolute_y=((y-i64::from(vy))*65535/i64::from(vh-1)).clamp(0,65535)as i32;inputs.push(mouse(MOUSEEVENTF_MOVE|MOUSEEVENTF_ABSOLUTE|MOUSEEVENTF_VIRTUALDESK,0,absolute_x,absolute_y));}
            for bit in [1,2,4]{if frame.mouse.buttons&bit!=self.buttons&bit{inputs.push(mouse(button_flags(bit,frame.mouse.buttons&bit==0),0,0,0));}}
            if frame.mouse.wheel!=0{inputs.push(mouse(MOUSEEVENTF_WHEEL,frame.mouse.wheel as u32,0,0));}
        }
        if self.foreground()!=Some(self.window)||Self::override_active(){self.revoke("target_focus_lost");return Ok(());}
        if !self.activity.healthy()||self.activity.count()!=self.activity_baseline{self.revoke("local_input_active");return Ok(());}
        // Track every possibly inserted down before the batch. On a partial OS
        // insertion we release the union, including the previous held state.
        let previous=self.keys.clone();let previous_buttons=self.buttons;self.keys=wanted;self.buttons=frame.mouse.buttons;
        if !send(&inputs){self.keys.extend(previous);self.buttons|=previous_buttons;self.revoke("input_blocked");return Ok(());}
        self.state="active";self.reason=None;self.last_input=Instant::now();Ok(())
    }
    pub fn release(&mut self){
        let mut inputs=Vec::new();for key in &self.keys{inputs.push(keyboard(*key,true));}for bit in [1,2,4]{if self.buttons&bit!=0{inputs.push(mouse(button_flags(bit,true),0,0,0));}}
        if send(&inputs){self.keys.clear();self.buttons=0;if self.state!="revoked"&&self.activated{self.state="idle";}}
        else{self.state="revoked";self.reason=Some("release_failed");}
    }
    pub fn revoke(&mut self,reason:&'static str){self.release();self.state="revoked";if self.reason!=Some("release_failed"){self.reason=Some(reason);}}
    pub fn observation(&self)->Observation{Observation{state:self.state.into(),reason:self.reason.map(str::to_owned)}}
}
impl Drop for TargetInput{fn drop(&mut self){self.release();}}
