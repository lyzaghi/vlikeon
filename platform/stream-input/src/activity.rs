use crate::failure;
use omniplay_contracts::Result;
use std::sync::{Arc,OnceLock,atomic::{AtomicBool,AtomicU64,AtomicUsize,Ordering},mpsc};
use std::time::Duration;
use windows_sys::Win32::{Foundation::*,System::{LibraryLoader::GetModuleHandleW,SystemInformation::GetTickCount64,Threading::GetCurrentProcessId},UI::WindowsAndMessaging::*};

struct Signals{count:AtomicU64,last_activity:AtomicU64,heartbeat:AtomicU64,stop:AtomicBool,fault:AtomicBool}
static SIGNALS:OnceLock<Arc<Signals>>=OnceLock::new();
static TAG:AtomicUsize=AtomicUsize::new(0);
pub(crate) fn tag()->usize{TAG.load(Ordering::Relaxed)}
fn activity(){if let Some(signals)=SIGNALS.get(){signals.last_activity.store(unsafe{GetTickCount64()},Ordering::Release);signals.count.fetch_add(1,Ordering::AcqRel);}}
unsafe extern "system" fn keyboard(code:i32,wparam:WPARAM,lparam:LPARAM)->LRESULT{
    if code==HC_ACTION as i32&&lparam!=0{
        // Windows supplies this structure only for the documented hook action.
        // Record an activity counter, never key values, text or a history log.
        let event=unsafe{&*(lparam as *const KBDLLHOOKSTRUCT)};
        if event.flags&LLKHF_INJECTED==0||event.dwExtraInfo!=tag(){activity();}
    }
    unsafe{CallNextHookEx(std::ptr::null_mut(),code,wparam,lparam)}
}
unsafe extern "system" fn mouse(code:i32,wparam:WPARAM,lparam:LPARAM)->LRESULT{
    if code==HC_ACTION as i32&&lparam!=0{let event=unsafe{&*(lparam as *const MSLLHOOKSTRUCT)};if event.flags&LLMHF_INJECTED==0||event.dwExtraInfo!=tag(){activity();}}
    unsafe{CallNextHookEx(std::ptr::null_mut(),code,wparam,lparam)}
}
struct Hook(HHOOK);
impl Drop for Hook{fn drop(&mut self){if !self.0.is_null(){unsafe{UnhookWindowsHookEx(self.0);}}}}
pub(crate) struct ActivityMonitor{signals:Arc<Signals>}
impl ActivityMonitor{
    pub fn start()->Result<Self>{
        let now=unsafe{GetTickCount64()};let signals=Arc::new(Signals{count:AtomicU64::new(0),last_activity:AtomicU64::new(now),heartbeat:AtomicU64::new(now),stop:AtomicBool::new(false),fault:AtomicBool::new(false)});
        if SIGNALS.set(signals.clone()).is_err(){return Err(failure());}
        // A tag separates this helper's own injected events from another helper
        // or input producer. It is diagnostic attribution, not a secret token.
        TAG.store((0x4f50_0000_0000_0000u64|u64::from(unsafe{GetCurrentProcessId()}))as usize,Ordering::Relaxed);
        let thread_signals=signals.clone();let(sender,ready)=mpsc::sync_channel(1);
        std::thread::Builder::new().name("omniplay-local-input-watch".into()).spawn(move||{
            let module=unsafe{GetModuleHandleW(std::ptr::null())};if module.is_null(){thread_signals.fault.store(true,Ordering::Release);let _=sender.send(false);return;}
            let keyboard_hook=Hook(unsafe{SetWindowsHookExW(WH_KEYBOARD_LL,Some(keyboard),module,0)});if keyboard_hook.0.is_null(){thread_signals.fault.store(true,Ordering::Release);let _=sender.send(false);return;}
            let mouse_hook=Hook(unsafe{SetWindowsHookExW(WH_MOUSE_LL,Some(mouse),module,0)});if mouse_hook.0.is_null(){thread_signals.fault.store(true,Ordering::Release);let _=sender.send(false);return;}
            if sender.send(true).is_err(){return;}
            while !thread_signals.stop.load(Ordering::Acquire){
                let mut message=MSG::default();let mut count=0;
                while count<64&&unsafe{PeekMessageW(&mut message,std::ptr::null_mut(),0,0,PM_REMOVE)}!=0{
                    if message.message==WM_QUIT{thread_signals.fault.store(true,Ordering::Release);return;}
                    unsafe{TranslateMessage(&message);DispatchMessageW(&message);}count+=1;
                }
                thread_signals.heartbeat.store(unsafe{GetTickCount64()},Ordering::Release);std::thread::sleep(Duration::from_millis(4));
            }
            drop(mouse_hook);drop(keyboard_hook);
        }).map_err(|_|failure())?;
        match ready.recv_timeout(Duration::from_secs(2)){Ok(true)=>Ok(Self{signals}),_=>{signals.stop.store(true,Ordering::Release);Err(failure())}}
    }
    pub fn healthy(&self)->bool{!self.signals.fault.load(Ordering::Acquire)&&unsafe{GetTickCount64()}.saturating_sub(self.signals.heartbeat.load(Ordering::Acquire))<=200}
    pub fn count(&self)->u64{self.signals.count.load(Ordering::Acquire)}
    pub fn quiet(&self)->bool{unsafe{GetTickCount64()}.saturating_sub(self.signals.last_activity.load(Ordering::Acquire))>=250}
}
impl Drop for ActivityMonitor{fn drop(&mut self){self.signals.stop.store(true,Ordering::Release);}}
