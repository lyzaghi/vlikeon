//! Linux x86-64/ARM64 uinput provider. Source only; not admitted by the broker.
//! Requires existing OS permission for the fixed /dev/uinput node. No root,
//! chmod, udev rule, module load, driver install or device-path request exists.
//! uinput devices are system-visible, NOT confined to the selected game.
use crate::gamepad::{Owner,Provider,Report};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use std::{fs::{File,OpenOptions},io::Write,mem::size_of,
    os::{fd::AsRawFd,unix::fs::{FileTypeExt,MetadataExt,OpenOptionsExt}},
    sync::Mutex};
use uuid::Uuid;

const EV_KEY:u16=1;
const EV_ABS:u16=3;
const ABS:[u16;8]=[0x00,0x01,0x03,0x04,0x02,0x05,0x10,0x11];
const BUTTONS:[u16;12]=[0x130,0x131,0x134,0x133,0x136,0x137,0x138,0x139,0x13a,0x13b,0x13d,0x13e];
// Process-local arbitration only; trusted service packaging must prevent
// independent service processes from allocating duplicate player slots.
static SLOTS:Mutex<[bool;4]>=Mutex::new([false;4]);
fn failure()->PlatformError{PlatformError::new(ErrorCode::RuntimeUnavailable,"The authorized Linux virtual gamepad could not be created, updated or removed")}
struct SlotLease{number:u8}
impl SlotLease{
    fn reserve(number:u8)->Result<Self>{
        let mut slots=SLOTS.lock().map_err(|_|failure())?;
        let slot=slots.get_mut(usize::from(number)).ok_or_else(failure)?;
        if *slot{return Err(PlatformError::new(ErrorCode::Busy,"This virtual gamepad slot is already reserved"));}
        *slot=true;Ok(Self{number})
    }
}
impl Drop for SlotLease{fn drop(&mut self){if let Ok(mut slots)=SLOTS.lock(){slots[usize::from(self.number)]=false;}}}

pub struct UinputProvider{_private:()}
impl UinputProvider{
    /// Construction performs no device operation. Use only behind native host
    /// consent and the installed service's signed-provider admission.
    pub fn new()->Self{Self{_private:()}}
}
impl Default for UinputProvider{fn default()->Self{Self::new()}}
pub struct UinputDevice{file:Option<File>,created:bool,lease:Option<SlotLease>,container:Uuid}
impl UinputDevice{
    fn file(&self)->Result<&File>{self.file.as_ref().filter(|_|self.created).ok_or_else(failure)}
    fn destroy(&mut self)->Result<()>{
        if self.created{
            let file=self.file.as_ref().ok_or_else(failure)?;
            // SAFETY: exact live, privately owned uinput descriptor; no argument.
            if unsafe{libc::ioctl(file.as_raw_fd(),libc::_IO(b'U'.into(),2))}<0{return Err(failure());}
            self.created=false;
        }
        self.file.take();self.lease.take();Ok(())
    }
}
impl Drop for UinputDevice{
    fn drop(&mut self){
        let _=self.destroy();
        // Closing the final private fd also tears down uinput on process loss.
        // Drop cannot report an OS failure; service runtime must exercise this.
        self.file.take();self.lease.take();
    }
}
fn bit(file:&File,command:u32,value:u16)->Result<()>{
    // SAFETY: these private callers supply only UI_SET_EVBIT/KEYBIT/ABSBIT;
    // their ABI takes the integer argument itself, not an int pointer.
    if unsafe{libc::ioctl(file.as_raw_fd(),libc::_IOW::<libc::c_int>(b'U'.into(),command),libc::c_int::from(value))}<0{return Err(failure());}Ok(())
}
fn axis(file:&File,code:u16,minimum:i32,maximum:i32)->Result<()>{
    // Zero the UAPI struct including padding; libc owns architecture layouts.
    let mut setup:libc::uinput_abs_setup=unsafe{std::mem::zeroed()};
    setup.code=code;setup.absinfo.minimum=minimum;setup.absinfo.maximum=maximum;
    // SAFETY: valid pointer to exact UAPI structure, copied during this ioctl.
    if unsafe{libc::ioctl(file.as_raw_fd(),libc::_IOW::<libc::uinput_abs_setup>(b'U'.into(),4),&setup)}<0{return Err(failure());}Ok(())
}
fn event(bytes:&mut Vec<u8>,kind:u16,code:u16,value:i32){
    // On the only compiled targets (Linux x86-64/ARM64), input_event is a
    // 16-byte timeval followed by u16/u16/i32. Explicit zero bytes avoid
    // exposing struct padding/uninitialized timestamps through write().
    let start=bytes.len();bytes.resize(start+24,0);
    bytes[start+16..start+18].copy_from_slice(&kind.to_ne_bytes());
    bytes[start+18..start+20].copy_from_slice(&code.to_ne_bytes());
    bytes[start+20..start+24].copy_from_slice(&value.to_ne_bytes());
}
impl Provider for UinputProvider{
    type Device=UinputDevice;
    fn create(&mut self,slot:u8,_owner:&Owner)->Result<UinputDevice>{
        if size_of::<libc::input_event>()!=24{return Err(failure());}
        let lease=SlotLease::reserve(slot)?;
        let file=OpenOptions::new().write(true)
            .custom_flags(libc::O_NONBLOCK|libc::O_CLOEXEC|libc::O_NOFOLLOW)
            .open("/dev/uinput").map_err(|_|failure())?;
        let metadata=file.metadata().map_err(|_|failure())?;
        if !metadata.file_type().is_char_device()||libc::major(metadata.rdev())!=10||libc::minor(metadata.rdev())!=223{return Err(failure());}
        let mut version:libc::c_uint=0;
        // SAFETY: live exact device fd and initialized writable integer.
        if unsafe{libc::ioctl(file.as_raw_fd(),libc::_IOR::<libc::c_uint>(b'U'.into(),45),&mut version)}<0||version<5{return Err(failure());}
        let mut device=UinputDevice{file:Some(file),created:false,lease:Some(lease),container:Uuid::new_v4()};
        let file=device.file.as_ref().ok_or_else(failure)?;
        bit(file,100,EV_KEY)?;bit(file,100,EV_ABS)?;
        for button in BUTTONS{bit(file,101,button)?;}
        for(index,code)in ABS.into_iter().enumerate(){
            bit(file,103,code)?;
            let(minimum,maximum)=match index{0..=3=>(-32767,32767),4..=5=>(0,65535),_=>(-1,1)};
            axis(file,code,minimum,maximum)?;
        }
        // No impersonated vendor/product ID and no force-feedback capability.
        let mut setup:libc::uinput_setup=unsafe{std::mem::zeroed()};
        setup.id.bustype=0x06;setup.id.version=1;
        let name=format!("Vlikeon Remote Gamepad {}",slot+1);
        for(destination,byte)in setup.name.iter_mut().zip(name.bytes()){*destination=byte as libc::c_char;}
        // SAFETY: fixed requests and valid owned UAPI structure/descriptor.
        if unsafe{libc::ioctl(file.as_raw_fd(),libc::_IOW::<libc::uinput_setup>(b'U'.into(),3),&setup)}<0{return Err(failure());}
        if unsafe{libc::ioctl(file.as_raw_fd(),libc::_IO(b'U'.into(),1))}<0{return Err(failure());}
        device.created=true;Ok(device)
    }
    fn write(&mut self,device:&mut UinputDevice,report:&Report)->Result<()>{
        if report.sticks.contains(&i16::MIN){return Err(failure());}
        let mut bytes=Vec::with_capacity(21*24);
        for(index,code)in ABS[..4].iter().enumerate(){event(&mut bytes,EV_ABS,*code,i32::from(report.sticks[index]));}
        for(index,code)in ABS[4..6].iter().enumerate(){event(&mut bytes,EV_ABS,*code,i32::from(report.triggers[index]));}
        // Opposite D-pad buttons cancel on each hat axis.
        let held=|index:u8|i32::from((report.buttons&(1u16<<index))!=0);
        event(&mut bytes,EV_ABS,ABS[6],held(15)-held(14));
        event(&mut bytes,EV_ABS,ABS[7],held(13)-held(12));
        for(index,code)in BUTTONS.iter().enumerate(){event(&mut bytes,EV_KEY,*code,held(index as u8));}
        event(&mut bytes,0,0,0); // SYN_REPORT publishes the full snapshot.
        let mut file=device.file()?;
        // A partial/nonblocking write is a failure, not a successful frame.
        // The owner then neutralizes/removes every device; no unbounded retry.
        if file.write(&bytes).map_err(|_|failure())?!=bytes.len(){return Err(failure());}Ok(())
    }
    fn detach(&mut self,device:&mut UinputDevice)->Result<()>{device.destroy()}
    fn cleanup_identity(&self,device:&UinputDevice)->Option<Uuid>{Some(device.container)}
}
