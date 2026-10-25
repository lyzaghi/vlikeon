//! Trusted host-side provisioning within an already granted service delegation.
//! No root privilege, ancestor traversal, implicit cgroup discovery or renderer
//! path is accepted. This is not a service installer or a permission grant.
use crate::linux_cgroup::{entry,text,field};
use crate::linux_delegation::{DelegationPolicy,RecoveryDelegation};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use std::{ffi::{CStr,CString},fs::{File,OpenOptions},io::{Read,Write},os::{fd::{AsRawFd,FromRawFd},unix::fs::{MetadataExt,OpenOptionsExt}},sync::Mutex};

#[derive(Clone,Copy)]
pub struct HostPolicy{pub maximum_profiles:u8,pub profile:DelegationPolicy,pub cpu_cores:u8}
pub struct NativeHostRoot{directory:File,_lease:File,policy:HostPolicy,serial:Mutex<()>}
fn unavailable()->PlatformError{PlatformError::new(ErrorCode::RuntimeUnavailable,"The explicitly delegated Linux native host could not provision its bounded profile tree")}
fn metadata(directory:&File,private:bool)->Result<std::fs::Metadata>{
    let value=directory.metadata().map_err(|_|unavailable())?;let flags=unsafe{libc::fcntl(directory.as_raw_fd(),libc::F_GETFL)};let mut shape=std::mem::MaybeUninit::<libc::statfs>::uninit();
    if unsafe{libc::geteuid()}==0||!value.is_dir()||value.uid()!=unsafe{libc::geteuid()}||value.mode()&0o022!=0||private&&value.mode()&0o7777!=0o700||flags<0||flags&libc::O_PATH!=libc::O_PATH||unsafe{libc::fstatfs(directory.as_raw_fd(),shape.as_mut_ptr())}!=0||unsafe{shape.assume_init()}.f_type!=libc::CGROUP2_SUPER_MAGIC{return Err(unavailable());}Ok(value)
}
fn traversal_metadata(directory:&File)->Result<std::fs::Metadata>{
    let value=directory.metadata().map_err(|_|unavailable())?;let flags=unsafe{libc::fcntl(directory.as_raw_fd(),libc::F_GETFL)};let mut shape=std::mem::MaybeUninit::<libc::statfs>::uninit();
    let uid=value.uid();if !value.is_dir()||!(uid==0||uid==unsafe{libc::geteuid()})||value.mode()&0o022!=0||flags<0||flags&libc::O_PATH!=libc::O_PATH||unsafe{libc::fstatfs(directory.as_raw_fd(),shape.as_mut_ptr())}!=0||unsafe{shape.assume_init()}.f_type!=libc::CGROUP2_SUPER_MAGIC{return Err(unavailable());}Ok(value)
}
fn opened_child(parent:&File,name:&CStr,private:bool)->Result<File>{
    let raw=unsafe{libc::openat(parent.as_raw_fd(),name.as_ptr(),libc::O_PATH|libc::O_DIRECTORY|libc::O_NOFOLLOW|libc::O_CLOEXEC)};if raw<0{return Err(unavailable());}let value=unsafe{File::from_raw_fd(raw)};metadata(&value,private)?;Ok(value)
}
fn traversed_child(parent:&File,name:&CStr)->Result<File>{let value=opened_child_unchecked(parent,name)?;traversal_metadata(&value)?;Ok(value)}
fn opened_child_unchecked(parent:&File,name:&CStr)->Result<File>{let raw=unsafe{libc::openat(parent.as_raw_fd(),name.as_ptr(),libc::O_PATH|libc::O_DIRECTORY|libc::O_NOFOLLOW|libc::O_CLOEXEC)};if raw<0{return Err(unavailable());}Ok(unsafe{File::from_raw_fd(raw)})}
fn child(parent:&File,name:&CStr)->Result<File>{opened_child(parent,name,true)}
fn created(parent:&File,name:&CStr)->Result<(File,bool)>{
    let fresh=if unsafe{libc::mkdirat(parent.as_raw_fd(),name.as_ptr(),0o700)}==0{true}else if std::io::Error::last_os_error().raw_os_error()==Some(libc::EEXIST){false}else{return Err(unavailable());};Ok((child(parent,name)?,fresh))
}
fn basename(directory:&File)->Result<String>{
    let link=CString::new(format!("/proc/self/fd/{}",directory.as_raw_fd())).map_err(|_|unavailable())?;let mut raw=[0u8;4096];let count=unsafe{libc::readlink(link.as_ptr(),raw.as_mut_ptr().cast(),raw.len())};if count<=0||count as usize>=raw.len(){return Err(unavailable());}let path=std::str::from_utf8(&raw[..count as usize]).map_err(|_|unavailable())?;Ok(path.rsplit('/').next().ok_or_else(unavailable)?.to_owned())
}
fn service_directory()->Result<File>{
    let mut membership=OpenOptions::new().read(true).custom_flags(libc::O_CLOEXEC|libc::O_NOFOLLOW|libc::O_NONBLOCK).open("/proc/self/cgroup").map_err(|_|unavailable())?;let mut raw=Vec::new();Read::by_ref(&mut membership).take(4097).read_to_end(&mut raw).map_err(|_|unavailable())?;if raw.len()>4096{return Err(unavailable());}let raw=std::str::from_utf8(&raw).map_err(|_|unavailable())?;
    let mut found=None;for line in raw.lines(){let path=line.strip_prefix("0::").filter(|value|value.starts_with('/')).ok_or_else(unavailable)?;if found.replace(path).is_some(){return Err(unavailable());}}let path=found.ok_or_else(unavailable)?;
    let parts:Vec<_>=path.split('/').filter(|value|!value.is_empty()).collect();
    let delegated=parts.last()==Some(&"omniplay-native-host.service");
    let subgroup=parts.len()>=2&&parts[parts.len()-2]=="omniplay-native-host.service"&&parts.last()==Some(&"manager");if parts.is_empty()||parts.len()>64||!(delegated||subgroup)||parts.iter().any(|part|part.len()>255||!part.bytes().all(|byte|byte.is_ascii_alphanumeric()||matches!(byte,b'_'|b'-'|b'.'|b'@'|b':'))){return Err(unavailable());}
    let mut current=OpenOptions::new().read(true).custom_flags(libc::O_PATH|libc::O_DIRECTORY|libc::O_NOFOLLOW|libc::O_CLOEXEC).open("/sys/fs/cgroup").map_err(|_|unavailable())?;let root=current.metadata().map_err(|_|unavailable())?;let mut shape=std::mem::MaybeUninit::<libc::statfs>::uninit();if !root.is_dir()||unsafe{libc::fstatfs(current.as_raw_fd(),shape.as_mut_ptr())}!=0||unsafe{shape.assume_init()}.f_type!=libc::CGROUP2_SUPER_MAGIC{return Err(unavailable());}
    for part in &parts{let name=CString::new(*part).map_err(|_|unavailable())?;current=traversed_child(&current,&name)?;}
    let process=std::process::id().to_string();if delegated{
        let service=current;metadata(&service,false)?;let(manager,fresh)=created(&service,c"manager")?;if !fresh{empty_new(&manager)?;}entry(&manager,c"cgroup.procs",true)?.write_all(process.as_bytes()).map_err(|_|unavailable())?;
        let processes=text(&manager,c"cgroup.procs")?;
        let mut members=processes.lines();if members.next()!=Some(process.as_str())||members.next().is_some()||!text(&service,c"cgroup.procs")?.is_empty(){return Err(unavailable());}drop(manager);return Ok(service);
    }
    let manager=current;metadata(&manager,false)?;let processes=text(&manager,c"cgroup.procs")?;let mut members=processes.lines();if members.next()!=Some(process.as_str())||members.next().is_some(){return Err(unavailable());}
    let parent_path=CString::new(format!("/proc/self/fd/{}/..",manager.as_raw_fd())).map_err(|_|unavailable())?;let raw=unsafe{libc::open(parent_path.as_ptr(),libc::O_PATH|libc::O_DIRECTORY|libc::O_NOFOLLOW|libc::O_CLOEXEC)};if raw<0{return Err(unavailable());}let service=unsafe{File::from_raw_fd(raw)};metadata(&service,false)?;if basename(&manager)?!="manager"||basename(&service)?!="omniplay-native-host.service"{return Err(unavailable());}Ok(service)
}
fn controllers(directory:&File,name:&CStr)->Result<()>{let raw=text(directory,name)?;let list:Vec<_>=raw.split_ascii_whitespace().collect();if list.len()>32||["cpu","memory","pids"].iter().any(|value|!list.contains(value)){return Err(unavailable());}Ok(())}
fn set(directory:&File,name:&CStr,value:&str)->Result<()>{entry(directory,name,true)?.write_all(value.as_bytes()).map_err(|_|unavailable())?;if text(directory,name)?!=format!("{value}\n"){return Err(unavailable());}Ok(())}
fn empty_new(directory:&File)->Result<()>{
    if text(directory,c"cgroup.type")?!="domain\n"||!text(directory,c"cgroup.procs")?.is_empty()||field(&text(directory,c"cgroup.events")?,"populated")?!=0||text(directory,c"pids.current")?!="0\n"||field(&text(directory,c"cgroup.stat")?,"nr_descendants")?!=0||field(&text(directory,c"cgroup.stat")?,"nr_dying_descendants")?!=0{return Err(unavailable());}Ok(())
}
fn host_values(policy:HostPolicy)->Result<Vec<(&'static CStr,String)>>{
    let total=policy.profile.maximum_reserved_bytes.checked_mul(u64::from(policy.maximum_profiles)).and_then(|value|value.checked_add(128*1024*1024)).filter(|value|*value<=64*1024*1024*1024).ok_or_else(unavailable)?;
    let descendants=u64::from(policy.maximum_profiles)*(1+2*u64::from(policy.profile.maximum_sessions));
    Ok(vec![(c"memory.max",total.to_string()),(c"memory.swap.max","0".into()),(c"memory.oom.group","1".into()),(c"pids.max",(u64::from(policy.maximum_profiles)*u64::from(policy.profile.maximum_sessions)*128).to_string()),(c"cpu.max",format!("{} 100000",u32::from(policy.cpu_cores)*100000)),(c"cgroup.max.depth","2".into()),(c"cgroup.max.descendants",descendants.to_string())])
}
fn profile_active(directory:&File)->Result<bool>{
    Ok(!text(directory,c"cgroup.procs")?.is_empty()||field(&text(directory,c"cgroup.events")?,"populated")?!=0||text(directory,c"pids.current")?!="0\n"||field(&text(directory,c"cgroup.stat")?,"nr_descendants")?!=0||field(&text(directory,c"cgroup.stat")?,"nr_dying_descendants")?!=0)
}
fn active_profile_admissible(directory:&File,policy:DelegationPolicy)->Result<()>{
    // Never mutate or reset cgroup knobs around live/dying children. Verify that
    // the active profile tree still obeys fundamental containment bounds.
    controllers(directory,c"cgroup.subtree_control")?;
    if text(directory,c"memory.swap.max")?!="0\n"||text(directory,c"memory.oom.group")?!="1\n"||text(directory,c"cgroup.max.depth")?!="1\n"{return Err(unavailable());}
    let mem_raw=text(directory,c"memory.max")?;let mem:u64=mem_raw.strip_suffix('\n').ok_or_else(unavailable)?.parse().map_err(|_|unavailable())?;
    if mem<512*1024*1024||mem>policy.maximum_reserved_bytes{return Err(unavailable());}
    Ok(())
}
fn profile_limits(directory:&File,policy:DelegationPolicy,cpu:u8,fresh:bool)->Result<()>{
    metadata(directory,true)?;
    if text(directory,c"cgroup.type")?!="domain\n"||!text(directory,c"cgroup.procs")?.is_empty()||text(directory,c"cgroup.freeze")?!="0\n"||field(&text(directory,c"cgroup.events")?,"frozen")?!=0{return Err(unavailable());}
    let values=[(c"memory.max",policy.maximum_reserved_bytes.to_string()),(c"memory.swap.max","0".into()),(c"memory.oom.group","1".into()),(c"pids.max",(u64::from(policy.maximum_sessions)*128).to_string()),(c"cpu.max",format!("{} 100000",u32::from(cpu)*100000)),(c"cgroup.max.depth","1".into()),(c"cgroup.max.descendants",(u64::from(policy.maximum_sessions)*2).to_string())];
    let exact=values.iter().all(|(name,value)|text(directory,name).is_ok_and(|raw|raw==format!("{value}\n")))&&controllers(directory,c"cgroup.subtree_control").is_ok();
    if !fresh&&!exact&&profile_active(directory)?{
        return active_profile_admissible(directory,policy);
    }
    if fresh||!exact{empty_new(directory)?;for(name,value)in &values{set(directory,name,value)?;}controllers(directory,c"cgroup.controllers")?;entry(directory,c"cgroup.subtree_control",true)?.write_all(b"+cpu +memory +pids").map_err(|_|unavailable())?;}
    for(name,value)in values{if text(directory,name)?!=format!("{value}\n"){return Err(unavailable());}}controllers(directory,c"cgroup.subtree_control")
}
impl NativeHostRoot{
    /// Discover only the calling trusted service's own unified-cgroup membership.
    /// The exact service/manager suffix and current sole manager PID are required.
    pub fn discover(lease:File,policy:HostPolicy)->Result<Self>{Self::create(service_directory()?,lease,policy)}
    /// A trusted service owner supplies its actual O_PATH service cgroup and a
    /// separately opened private 0600 single-link host lease. The service must
    /// already have been legitimately delegated and moved into its manager leaf.
    pub fn create(service:File,lease:File,policy:HostPolicy)->Result<Self>{
        if !(1..=8).contains(&policy.maximum_profiles)||!(1..=8).contains(&policy.profile.maximum_sessions)||!(512*1024*1024..=16*1024*1024*1024).contains(&policy.profile.maximum_reserved_bytes)||policy.profile.maximum_reserved_bytes&4095!=0||!(1..=8).contains(&policy.cpu_cores){return Err(unavailable());}
        let values=host_values(policy)?;
        metadata(&service,false)?;if basename(&service)?!="omniplay-native-host.service"||text(&service,c"cgroup.type")?!="domain\n"||!text(&service,c"cgroup.procs")?.is_empty(){return Err(unavailable());}
        let lock=lease.metadata().map_err(|_|unavailable())?;if !lock.is_file()||lock.uid()!=unsafe{libc::geteuid()}||lock.mode()&0o7777!=0o600||lock.nlink()!=1{return Err(unavailable());}lease.try_lock().map_err(|_|unavailable())?;
        if unsafe{libc::fcntl(service.as_raw_fd(),libc::F_SETFD,libc::FD_CLOEXEC)}!=0||unsafe{libc::fcntl(lease.as_raw_fd(),libc::F_SETFD,libc::FD_CLOEXEC)}!=0{return Err(unavailable());}
        controllers(&service,c"cgroup.controllers")?;entry(&service,c"cgroup.subtree_control",true)?.write_all(b"+cpu +memory +pids").map_err(|_|unavailable())?;controllers(&service,c"cgroup.subtree_control")?;
        let(directory,fresh)=created(&service,c"omniplay-native-profiles")?;
        let exact=values.iter().all(|(name,value)|text(&directory,name).is_ok_and(|raw|raw==format!("{value}\n")))&&controllers(&directory,c"cgroup.subtree_control").is_ok();
        if fresh||!exact{empty_new(&directory)?;for(name,value)in &values{set(&directory,name,value)?;}controllers(&directory,c"cgroup.controllers")?;entry(&directory,c"cgroup.subtree_control",true)?.write_all(b"+cpu +memory +pids").map_err(|_|unavailable())?;}
        for(name,value)in values{if text(&directory,name)?!=format!("{value}\n"){return Err(unavailable());}}
        if text(&directory,c"cgroup.type")?!="domain\n"||!text(&directory,c"cgroup.procs")?.is_empty()||text(&directory,c"cgroup.freeze")?!="0\n"||field(&text(&directory,c"cgroup.events")?,"frozen")?!=0{return Err(unavailable());}controllers(&directory,c"cgroup.subtree_control")?;
        let host=Self{directory,_lease:lease,policy,serial:Mutex::new(())};host.inventory()?;Ok(host)
    }
    fn inventory(&self)->Result<usize>{
        metadata(&self.directory,true)?;for(name,value)in host_values(self.policy)?{if text(&self.directory,name)?!=format!("{value}\n"){return Err(unavailable());}}controllers(&self.directory,c"cgroup.subtree_control")?;
        if text(&self.directory,c"cgroup.type")?!="domain\n"||!text(&self.directory,c"cgroup.procs")?.is_empty()||text(&self.directory,c"cgroup.freeze")?!="0\n"||field(&text(&self.directory,c"cgroup.events")?,"frozen")?!=0{return Err(unavailable());}
        let entries=std::fs::read_dir(format!("/proc/self/fd/{}",self.directory.as_raw_fd())).map_err(|_|unavailable())?;let mut profiles=0;let mut count=0;
        for entry in entries{count+=1;if count>256{return Err(unavailable());}let entry=entry.map_err(|_|unavailable())?;let name=entry.file_name();let name=name.to_str().ok_or_else(unavailable)?;let encoded=CString::new(name).map_err(|_|unavailable())?;let mut shape=std::mem::MaybeUninit::<libc::stat>::uninit();if unsafe{libc::fstatat(self.directory.as_raw_fd(),encoded.as_ptr(),shape.as_mut_ptr(),libc::AT_SYMLINK_NOFOLLOW)}!=0{return Err(unavailable());}let shape=unsafe{shape.assume_init()};if shape.st_mode&libc::S_IFMT==libc::S_IFREG{continue;}if shape.st_mode&libc::S_IFMT!=libc::S_IFDIR{return Err(unavailable());}
            let profile=name.strip_prefix("omniplay-profile-").ok_or_else(unavailable)?;crate::linux_plan::session(profile)?;profiles+=1;if profiles>usize::from(self.policy.maximum_profiles){return Err(unavailable());}let directory=child(&self.directory,&encoded)?;profile_limits(&directory,self.policy.profile,self.policy.cpu_cores,false)?;
        }Ok(profiles)
    }
    pub fn quiescent(&self)->Result<bool>{
        let _serial=self.serial.lock().map_err(|_|unavailable())?;self.inventory()?;let entries=std::fs::read_dir(format!("/proc/self/fd/{}",self.directory.as_raw_fd())).map_err(|_|unavailable())?;let mut count=0;
        for entry in entries{count+=1;if count>256{return Err(unavailable());}let entry=entry.map_err(|_|unavailable())?;let name=entry.file_name();let Some(name)=name.to_str()else{return Err(unavailable());};let Some(profile)=name.strip_prefix("omniplay-profile-")else{continue;};crate::linux_plan::session(profile)?;let encoded=CString::new(name).map_err(|_|unavailable())?;let directory=child(&self.directory,&encoded)?;
            if field(&text(&directory,c"cgroup.events")?,"populated")?!=0||field(&text(&directory,c"cgroup.stat")?,"nr_dying_descendants")?!=0{return Ok(false);}
        }Ok(true)
    }
    /// Existing profile limits are compared, never reset around active children.
    /// The returned recovery owner still cannot launch or signal any process.
    pub fn provision_profile(&self,profile:&str)->Result<File>{
        crate::linux_plan::session(profile)?;let _serial=self.serial.lock().map_err(|_|unavailable())?;let count=self.inventory()?;let name=CString::new(format!("omniplay-profile-{profile}")).map_err(|_|unavailable())?;
        let mut shape=std::mem::MaybeUninit::<libc::stat>::uninit();let exists=unsafe{libc::fstatat(self.directory.as_raw_fd(),name.as_ptr(),shape.as_mut_ptr(),libc::AT_SYMLINK_NOFOLLOW)}==0;
        if !exists{if std::io::Error::last_os_error().raw_os_error()!=Some(libc::ENOENT){return Err(unavailable());}if count>=usize::from(self.policy.maximum_profiles){return Err(PlatformError::new(ErrorCode::ResourceLimit,"The Linux native host profile resource limit is full"));}}
        let(directory,fresh)=created(&self.directory,&name)?;profile_limits(&directory,self.policy.profile,self.policy.cpu_cores,fresh)?;
        Ok(directory)
    }
    pub fn profile(&self,profile:&str,lease:File)->Result<RecoveryDelegation>{
        RecoveryDelegation::admit(self.provision_profile(profile)?,profile,lease,self.policy.profile)
    }
}
