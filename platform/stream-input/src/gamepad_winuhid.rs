//! Private opt-in Windows provider; source only, not product admission.
//! Requires the pinned, statically linked C++ adapter and an independently
//! admitted signed WinUHid installation. Does not load/install a DLL/driver.
//! Calls may block: the future owned helper needs an external watchdog.
use crate::gamepad::{Owner,Provider,Report};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use uuid::Uuid;

#[repr(C)]
struct NativeReport{sticks:[i16;4],triggers:[u16;2],buttons:u16,reserved:u16}
const _:()=assert!(std::mem::size_of::<NativeReport>()==16);
#[link(name="vlikeon-winpad",kind="static")]
#[link(name="setupapi")]
#[link(name="hid")]
#[link(name="ole32")]
unsafe extern "C"{
    fn vlk_winpad_reserve(slot:u8,vendor:u16,product:u16,token:*mut u64)->u32;
    fn vlk_winpad_container(token:u64,container:*mut u8)->u32;
    fn vlk_winpad_presence(container:*const u8)->u32;
    fn vlk_winpad_write(token:u64,report:*const NativeReport)->u32;
    fn vlk_winpad_retire(token:u64)->u32;
}
fn failure()->PlatformError{PlatformError::new(ErrorCode::RuntimeUnavailable,
    "The admitted Windows virtual gamepad is unavailable or its removal is not confirmed")}

pub struct WinUHidProvider{vendor:u16,product:u16}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum DevicePresence{Absent,Present,Unknown}
impl WinUHidProvider{
    /// Values must come from the installed package's assigned HID identity.
    /// Nonzero values are shape validation, not proof of assignment/admission.
    /// This constructor performs no device operation or OS permission grant.
    pub fn new(vendor:u16,product:u16)->Result<Self>{
        if vendor==0||product==0{return Err(failure());}Ok(Self{vendor,product})
    }
    /// For the independent supervisor, not a substitute for a removal receipt.
    /// A never-observed attempted device cannot be cleared by absence alone.
    pub fn presence(container:Uuid)->DevicePresence{
        if container.is_nil(){return DevicePresence::Unknown;}
        let bytes=container.to_bytes_le();
        // SAFETY: exact 16-byte native GUID representation, borrowed for call.
        match unsafe{vlk_winpad_presence(bytes.as_ptr())}{0=>DevicePresence::Absent,1=>DevicePresence::Present,_=>DevicePresence::Unknown}
    }
}
pub struct WinUHidDevice{token:Option<u64>,initialized:bool,container:Uuid}
impl WinUHidDevice{
    fn retire(&mut self)->Result<()>{
        let Some(token)=self.token else{return Ok(());};
        // SAFETY: an opaque token minted by this statically linked adapter.
        // Native registry retains the record/slot on unconfirmed removal.
        if unsafe{vlk_winpad_retire(token)}!=0{return Err(failure());}
        self.token=None;self.initialized=false;Ok(())
    }
}
impl Drop for WinUHidDevice{
    fn drop(&mut self){
        let _=self.retire();
        // Failed cleanup is not released: the C++ registry retains at most
        // four tombstones and retries a closing slot on a later reservation.
        // A never-observed device needs external helper retirement/review.
    }
}
impl Provider for WinUHidProvider{
    type Device=WinUHidDevice;
    fn create(&mut self,slot:u8,_owner:&Owner)->Result<Self::Device>{
        let mut token=0;
        // SAFETY: a live output pointer; C++ bounds-checks the logical slot.
        // Reservation performs no OS creation, so the manager owns the token
        // before the first neutral write can allocate any driver resources.
        let status=unsafe{vlk_winpad_reserve(slot,self.vendor,self.product,&mut token)};
        if status!=0||token==0{return Err(failure());}
        // Own the reservation immediately, so even a failed identity read drops
        // it through the existing-only cleanup path without creating a device.
        let mut device=WinUHidDevice{token:Some(token),initialized:false,container:Uuid::nil()};
        let mut bytes=[0;16];
        // SAFETY: private live token and exact writable GUID-sized buffer.
        if unsafe{vlk_winpad_container(token,bytes.as_mut_ptr())}!=0{return Err(failure());}
        device.container=Uuid::from_bytes_le(bytes);
        if device.container.is_nil(){return Err(failure());}Ok(device)
    }
    fn write(&mut self,device:&mut Self::Device,report:&Report)->Result<()>{
        let token=device.token.ok_or_else(failure)?;
        if !device.initialized&&*report!=Report::default(){return Err(failure());}
        let native=NativeReport{sticks:report.sticks,triggers:report.triggers,
            buttons:report.buttons,reserved:0};
        // SAFETY: exact repr(C) structure and live private token; native code
        // copies the full report into a fixed-size, zero-initialized HID frame.
        if unsafe{vlk_winpad_write(token,&native)}!=0{return Err(failure());}
        device.initialized=true;Ok(())
    }
    fn neutralize(&mut self,device:&mut Self::Device)->Result<()>{
        // Do not initialize unused reservations during partial-create cleanup.
        // Native retire also attempts neutral on a partly initialized handle.
        if device.initialized{self.write(device,&Report::default())}else{Ok(())}
    }
    fn detach(&mut self,device:&mut Self::Device)->Result<()>{device.retire()}
    fn cleanup_identity(&self,device:&Self::Device)->Option<Uuid>{device.token.map(|_|device.container)}
}
