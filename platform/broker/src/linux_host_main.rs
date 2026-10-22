#![cfg_attr(not(all(target_os="linux",target_arch="x86_64",target_env="gnu")),allow(dead_code))]

#[cfg(all(target_os="linux",target_arch="x86_64",target_env="gnu"))]
fn run()->omniplay_contracts::Result<()> {
    use omniplay_sandbox::{linux_delegation::DelegationPolicy,linux_host::{HostPolicy,NativeHostRoot},linux_host_channel};
    use std::{fs::OpenOptions,os::unix::fs::{MetadataExt,OpenOptionsExt},path::PathBuf};
    let runtime=std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).filter(|path|path.is_absolute()).ok_or_else(||omniplay_contracts::PlatformError::new(omniplay_contracts::ErrorCode::RuntimeUnavailable,"The Linux user runtime directory is unavailable"))?;
    let directory=runtime.join("omniplay"),socket=directory.join("native-host.sock"),lock=directory.join("native-host.lock");
    let lease=OpenOptions::new().read(true).write(true).create(true).mode(0o600).custom_flags(libc::O_CLOEXEC|libc::O_NOFOLLOW).open(lock).map_err(|_|omniplay_contracts::PlatformError::new(omniplay_contracts::ErrorCode::RuntimeUnavailable,"The Linux native-host lease is unavailable"))?;
    let shape=lease.metadata().map_err(|_|omniplay_contracts::PlatformError::new(omniplay_contracts::ErrorCode::RuntimeUnavailable,"The Linux native-host lease is unavailable"))?;
    if !shape.is_file()||shape.uid()!=unsafe{libc::geteuid()}||shape.mode()&0o7777!=0o600||shape.nlink()!=1{return Err(omniplay_contracts::PlatformError::new(omniplay_contracts::ErrorCode::RuntimeUnavailable,"The Linux native-host lease is not private"));}
    let policy=HostPolicy{maximum_profiles:4,profile:DelegationPolicy{maximum_sessions:4,maximum_reserved_bytes:14*1024*1024*1024},cpu_cores:8};
    let host=NativeHostRoot::discover(lease,policy)?;let listener=linux_host_channel::systemd_listener(&socket)?;let mut idle=0u8;
    loop{if linux_host_channel::wait(&listener,std::time::Duration::from_secs(60))?{linux_host_channel::serve_one(&listener,&host)?;idle=0;}else if host.quiescent()?{idle+=1;if idle>=5{return Ok(());}}else{idle=0;}}
}

#[cfg(all(target_os="linux",target_arch="x86_64",target_env="gnu"))]
fn main(){if run().is_err(){std::process::exit(78);}}

#[cfg(not(all(target_os="linux",target_arch="x86_64",target_env="gnu")))]
fn main(){std::process::exit(78);}
