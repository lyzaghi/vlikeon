// Unsafe FFI is confined to target-specific launch code. Callers own safe Files and
// a process guard; dropping the guard terminates its entire contained process tree.
#![deny(unsafe_op_in_unsafe_fn)]
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use std::{fs::File,path::Path};
pub mod linux_plan;
#[cfg(all(target_os="linux",target_arch="x86_64"))]pub mod linux_sealed;
#[cfg(all(target_os="linux",target_arch="x86_64"))]pub mod linux_cgroup;
#[cfg(all(target_os="linux",target_arch="x86_64"))]pub mod linux_delegation;
#[cfg(all(target_os="linux",target_arch="x86_64"))]pub mod linux_host;
#[cfg(all(target_os="linux",target_arch="x86_64"))]pub mod linux_host_channel;
#[cfg(all(target_os="linux",target_arch="x86_64"))]pub mod linux_elf;
#[cfg(all(target_os="linux",target_arch="x86_64"))]pub mod linux_images;
#[cfg(all(target_os="linux",target_arch="x86_64",target_env="gnu"))]pub mod linux_launch;
#[cfg(all(target_os="linux",target_arch="x86_64",target_env="gnu"))]pub mod linux_presentation;

#[cfg(windows)]mod windows;
#[cfg(windows)]pub use windows::{ContainedProcess,ContainerIdentity,PresentationHost};

pub struct Launched {pub process:ContainedProcess,pub input:File,pub output:File}
pub struct Limits {pub memory_bytes:usize}
pub struct PresentationLaunch<'a>{pub executable:&'a Path,pub working_directory:&'a Path,pub temporary_directory:&'a Path,pub session:&'a str,pub maximum_width:u32,pub maximum_height:u32,pub volume_percent:u16,pub haptics:bool,pub memory_bytes:usize}
pub struct PresentedLaunched{pub actor:Launched,pub presentation:PresentationHost}
#[derive(Clone,Copy)]
pub struct ProcessIdentity{pub process_id:u32,pub creation_time:u64}

pub fn supported()->bool {cfg!(windows)}
pub fn presentation_supported()->bool{cfg!(windows)}
pub(crate) fn unavailable()->PlatformError {PlatformError::new(ErrorCode::RuntimeUnavailable,"The required native runtime isolation could not be established").recover("Use an approved web runtime while native isolation is unavailable")}

#[cfg(not(windows))]
pub struct ContainerIdentity;
#[cfg(not(windows))]
pub struct ContainedProcess;
#[cfg(not(windows))]
pub struct PresentationHost;
#[cfg(not(windows))]
impl ContainerIdentity {
    pub fn create_for_session(_: &str)->Result<Self>{Err(unavailable())}
    pub fn retire(&self)->Result<()>{Err(unavailable())}
    pub fn grant_game_read(&self,_:&Path)->Result<()>{Err(unavailable())}
    pub fn grant_private_write(&self,_:&Path)->Result<()>{Err(unavailable())}
    pub fn launch(&self,_:&Path,_:&Path,_:&Limits,_:impl FnOnce(ProcessIdentity)->Result<()>)->Result<Launched>{Err(unavailable())}
    pub fn launch_presented(&self,_:&Path,_:&Path,_:&Limits,_:&PresentationLaunch<'_>,_:impl FnOnce(ProcessIdentity)->Result<()>,_:impl FnOnce(ProcessIdentity)->Result<()>)->Result<PresentedLaunched>{Err(unavailable())}
}
#[cfg(not(windows))]
impl ContainedProcess {pub fn terminate(&self){}pub fn has_exited(&self)->bool{true}}
#[cfg(not(windows))]
impl PresentationHost{pub fn terminate(&self){}pub fn has_exited(&self)->bool{true}pub fn activate(&self)->Result<()>{Err(unavailable())}}
pub fn confirmed_process_ended(process:ProcessIdentity)->Result<()>{
    #[cfg(windows)]{windows::confirmed_process_ended(process)}
    #[cfg(not(windows))]{let _=process;Err(unavailable())}
}
pub fn retire_abandoned_profile(name:&str,process:Option<ProcessIdentity>)->Result<()>{
    #[cfg(windows)]{windows::retire_abandoned_profile(name,process)}
    #[cfg(not(windows))]{let _=(name,process);Err(unavailable())}
}

pub fn grant_public_runtime_read(directory:&Path)->Result<()> {
    #[cfg(windows)]{windows::grant_public_runtime_read(directory)}
    #[cfg(not(windows))]{let _=directory;Err(unavailable())}
}
