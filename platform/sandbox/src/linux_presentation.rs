//! Trusted normal-user GUI host source. Desktop/audio environment and owner
//! control are separate from the actor's closed media pipes and private root.
use crate::{linux_cgroup::DelegatedGroup,linux_elf::PresentationExecutable,linux_launch::{LinuxChild,LinuxProcessIdentity}};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use std::{ffi::CString,fs::File,os::{fd::{AsRawFd,OwnedFd},unix::ffi::OsStrExt},path::PathBuf,sync::Mutex};

pub struct DesktopEnvironment{entries:Vec<CString>}
impl DesktopEnvironment{
    pub fn capture()->Result<Self>{
        let mut entries=vec![CString::new("PATH=/usr/bin:/bin").map_err(|_|unavailable())?,CString::new("LANG=C.UTF-8").map_err(|_|unavailable())?,CString::new("LC_ALL=C.UTF-8").map_err(|_|unavailable())?];let mut total=0;
        for key in["HOME","USER","LOGNAME","DISPLAY","WAYLAND_DISPLAY","XAUTHORITY","XDG_RUNTIME_DIR","DBUS_SESSION_BUS_ADDRESS","PULSE_SERVER","PIPEWIRE_REMOTE","XDG_SESSION_TYPE"]{
            if let Some(value)=std::env::var_os(key){let value=value.as_bytes();total+=value.len();if value.is_empty()||value.len()>4096||total>16384||value.iter().any(|byte|*byte<32||*byte==127){return Err(unavailable());}let mut raw=key.as_bytes().to_vec();raw.push(b'=');raw.extend_from_slice(value);entries.push(CString::new(raw).map_err(|_|unavailable())?);}
        }Ok(Self{entries})
    }
    pub(crate) fn entries(&self)->&[CString]{&self.entries}
}
pub struct PresentationSpawn{
    pub executable:PresentationExecutable,pub session:String,pub resource_group:DelegatedGroup,
    pub directory:File,pub temporary_directory:File,pub temporary_path:PathBuf,pub work_lease:File,
    pub input:File,pub output:File,pub desktop:DesktopEnvironment,
    pub maximum_width:u32,pub maximum_height:u32,pub volume_percent:u16,pub haptics:bool,pub memory_bytes:u64,pub cpu_cores:u8,
}
struct OwnerSender{descriptor:OwnedFd,session:[u8;16],sequence:u64}
pub struct LinuxPresentationHost{pub(crate) process:LinuxChild,owner:Mutex<Option<OwnerSender>>}
fn unavailable()->PlatformError{PlatformError::new(ErrorCode::RuntimeUnavailable,"The owned Linux presentation window could not be established")}
impl LinuxPresentationHost{
    pub(crate) fn new(process:LinuxChild,descriptor:OwnedFd,session:&str)->Result<Self>{Ok(Self{process,owner:Mutex::new(Some(OwnerSender{descriptor,session:crate::linux_plan::session(session)?,sequence:0}))})}
    pub fn identity(&self)->Result<&LinuxProcessIdentity>{self.process.identity()}
    pub fn has_exited(&self)->bool{self.process.has_exited()}
    pub fn terminate(&self){match self.owner.lock(){Ok(mut value)=>{value.take();},Err(value)=>{value.into_inner().take();}}self.process.terminate();}
    /// Requests only this helper's own SDL window. Actual compositor focus must
    /// still be observed through fresh input; a queued request is not activation.
    pub fn activate(&self)->Result<()>{
        if self.process.has_exited(){return Err(unavailable());}let mut guard=self.owner.lock().map_err(|_|unavailable())?;let owner=guard.as_mut().ok_or_else(unavailable)?;let sequence=owner.sequence.checked_add(1).filter(|value|*value<=i64::MAX as u64).ok_or_else(unavailable)?;
        let mut packet=[0u8;40];packet[..8].copy_from_slice(b"OPOWNER1");packet[8..24].copy_from_slice(&owner.session);packet[24..32].copy_from_slice(&sequence.to_le_bytes());packet[32..36].copy_from_slice(&1u32.to_le_bytes());let sent=unsafe{libc::send(owner.descriptor.as_raw_fd(),packet.as_ptr().cast(),packet.len(),libc::MSG_NOSIGNAL|libc::MSG_DONTWAIT)};
        if sent!=packet.len()as isize{return Err(PlatformError::new(ErrorCode::PermissionDenied,"Focus the OmniPlay game window before resuming").recover("Select the game window and press Escape, or choose Resume again"));}owner.sequence=sequence;Ok(())
    }
}
impl Drop for LinuxPresentationHost{fn drop(&mut self){self.terminate();}}
