//! Bounded native-host window choices. These are not capture consent, renderer
//! capabilities or a complete inventory of every process/window on the system.
use crate::{game_process::OwnedGameProcess,gamepad_process::{ProcessIdentity,ProcessWitness}};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use std::{os::windows::io::{AsHandle,FromRawHandle,OwnedHandle},time::{Duration,Instant}};
use windows_sys::Win32::{Foundation::*,System::Threading::*,UI::WindowsAndMessaging::*};

const MAX_VISITED:usize=8192;
const MAX_CHOICES:usize=32;
const ENUMERATION_BUDGET:Duration=Duration::from_millis(250);
const CHOICE_LIFETIME:Duration=Duration::from_secs(30);

fn stale()->PlatformError{PlatformError::new(ErrorCode::InvalidState,"Refresh the installed game's window choices before selecting a window")}

/// Constructed only from a live window whose retained process object was
/// admitted to the exact game job. No Deserialize/Clone or raw-handle setter.
pub struct GameWindow{
    window:usize,title:String,process:OwnedHandle,identity:ProcessIdentity,observed:Instant,
}
impl GameWindow{
    /// Local display text only. Never use a title as ownership or executable ID.
    pub fn title(&self)->&str{&self.title}
    /// Native-host HWND only; not a transferable capture token.
    pub fn window(&self)->u64{self.window as u64}
    pub(crate) fn admit(&self,owner:&OwnedGameProcess)->Result<OwnedHandle>{
        if self.observed.elapsed()>=CHOICE_LIFETIME{return Err(stale());}
        let window=self.window as HWND;
        if !usable_window(window,self.identity.process_id){return Err(stale());}
        let process=owner.admit_selected(self.process.as_handle())?;
        let witness=ProcessWitness::from_child(process.as_handle())?;
        if witness.identity()!=self.identity||witness.exited(self.identity)?||
            !usable_window(window,self.identity.process_id)||self.observed.elapsed()>=CHOICE_LIFETIME{return Err(stale());}
        Ok(process)
    }
}

fn usable_window(window:HWND,expected_pid:u32)->bool{
    let mut pid=0;
    // SAFETY: Windows validates the HWND, live PID output and zero-time queries.
    unsafe{!window.is_null()&&IsWindowVisible(window)!=0&&IsIconic(window)==0&&
        GetAncestor(window,GA_ROOT)==window&&GetWindowThreadProcessId(window,&mut pid)!=0&&pid==expected_pid}
}
struct Probe<'a>{owner:&'a OwnedGameProcess,started:Instant,visited:usize,windows:Vec<GameWindow>,error:Option<PlatformError>}
impl Probe<'_>{
    fn visit(&mut self,window:HWND)->Result<()>{
        self.visited+=1;
        if self.visited>MAX_VISITED||self.started.elapsed()>=ENUMERATION_BUDGET{
            return Err(PlatformError::new(ErrorCode::ResourceLimit,"Game window discovery exceeded its bounded scan; refresh to try again"));
        }
        let mut pid=0;
        // SAFETY: OS-supplied HWND and valid PID output. Do not read titles of
        // unrelated applications, or send WM_GETTEXT to this broker itself.
        if unsafe{GetWindowThreadProcessId(window,&mut pid)}==0||pid==0||
            pid==unsafe{GetCurrentProcessId()}||!usable_window(window,pid){return Ok(());}
        let raw=unsafe{OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION|PROCESS_SYNCHRONIZE,0,pid)};
        if raw.is_null()||raw==INVALID_HANDLE_VALUE{return Ok(());}
        // SAFETY: newly acquired sole-owned read-only process handle.
        let opened=unsafe{OwnedHandle::from_raw_handle(raw)};
        // PID lookup is only discovery. Ownership is established by this exact
        // process object's live membership in the retained job. Refused/unknown
        // objects are omitted, never inferred to be ours from a title or parent.
        let process=match self.owner.admit_selected(opened.as_handle()){Ok(value)=>value,Err(_)=>return Ok(())};
        let witness=match ProcessWitness::from_child(process.as_handle()){Ok(value)=>value,Err(_)=>return Ok(())};
        let identity=witness.identity();
        if identity.process_id!=pid||witness.exited(identity)?||!usable_window(window,pid){return Ok(());}
        if self.windows.len()>=MAX_CHOICES{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Too many game windows are open to select safely; close unused game windows"));}
        let mut text=[0u16;513];
        // SAFETY: bounded UTF-16 buffer; only a foreign top-level job member's
        // cached caption is requested, not arbitrary child control contents.
        let length=unsafe{GetWindowTextW(window,text.as_mut_ptr(),text.len()as i32)};
        let title=if length>0&&length<text.len()as i32{
            String::from_utf16_lossy(&text[..length as usize]).chars()
                .filter(|c|!c.is_control()&&!matches!(*c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}')).collect::<String>()
        }else{String::new()};
        if !usable_window(window,pid)||witness.exited(identity)?{return Ok(());}
        if self.windows.iter().any(|entry|entry.window==window as usize){return Ok(());}
        self.windows.push(GameWindow{window:window as usize,title:if title.trim().is_empty(){"Game window".into()}else{title},
            process,identity,observed:self.started});Ok(())
    }
}
unsafe extern "system" fn visit(window:HWND,context:LPARAM)->windows_sys::core::BOOL{
    // SAFETY: enumerate supplies its exclusive stack-owned Probe pointer only
    // for the duration of synchronous EnumWindows. It is never retained.
    let probe=unsafe{&mut*(context as*mut Probe<'_>)};
    match probe.visit(window){Ok(())=>1,Err(error)=>{probe.error=Some(error);0}}
}

pub(crate) fn enumerate(owner:&OwnedGameProcess)->Result<Vec<GameWindow>>{
    let mut probe=Probe{owner,started:Instant::now(),visited:0,windows:Vec::with_capacity(MAX_CHOICES),error:None};
    // SAFETY: callback/context lifetime is exactly this synchronous call; no
    // other reference mutates Probe until enumeration returns.
    let complete=unsafe{EnumWindows(Some(visit),(&mut probe as*mut Probe<'_>)as LPARAM)};
    if let Some(error)=probe.error{return Err(error);}
    if complete==0||probe.started.elapsed()>=ENUMERATION_BUDGET{
        return Err(PlatformError::new(ErrorCode::RuntimeUnavailable,"Game window discovery could not complete; refresh the window choices"));
    }
    Ok(probe.windows)
}
