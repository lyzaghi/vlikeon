//! A live, newly owned delegated leaf. No host/ancestor controller or permission
//! is changed here. The profile owner must create this unique leaf legitimately,
//! retain its parent/journal and remove it only after actual process/tree exit.
use omniplay_contracts::{PlatformError,ErrorCode,Result};
use std::{ffi::CString,fs::File,io::{Read,Write},os::{fd::{AsRawFd,FromRawFd,RawFd},unix::fs::MetadataExt}};

pub struct DelegatedGroup{directory:File,session:String,presentation:bool,device:u64,inode:u64,pub(crate) owner:Option<crate::linux_delegation::OwnedLeaf>}
fn unavailable()->PlatformError{PlatformError::new(ErrorCode::RuntimeUnavailable,"The private delegated Linux resource group could not be established")}
pub(crate) fn entry(directory:&File,name:&std::ffi::CStr,write:bool)->Result<File>{
    let fd=unsafe{libc::openat(directory.as_raw_fd(),name.as_ptr(),(if write{libc::O_WRONLY}else{libc::O_RDONLY})|libc::O_CLOEXEC|libc::O_NOFOLLOW)};
    if fd<0{return Err(unavailable());}Ok(unsafe{File::from_raw_fd(fd)})
}
fn bounded(mut file:File,maximum:usize)->Result<Vec<u8>>{let mut raw=Vec::new();Read::by_ref(&mut file).take(maximum as u64+1).read_to_end(&mut raw).map_err(|_|unavailable())?;if raw.len()>maximum{return Err(unavailable());}Ok(raw)}
pub(crate) fn text(directory:&File,name:&std::ffi::CStr)->Result<String>{String::from_utf8(bounded(entry(directory,name,false)?,4096)?).map_err(|_|unavailable())}
pub(crate) fn field(raw:&str,key:&str)->Result<u64>{
    let mut found=None;for line in raw.lines(){let mut parts=line.split_ascii_whitespace();let name=parts.next().ok_or_else(unavailable)?;let value=parts.next().ok_or_else(unavailable)?;
        if name.len()>64||value.len()>20||!name.bytes().all(|byte|byte.is_ascii_lowercase()||byte==b'_')||!value.bytes().all(|byte|byte.is_ascii_digit())||parts.next().is_some(){return Err(unavailable());}
        if name==key{if found.is_some(){return Err(unavailable());}found=Some(value.parse().map_err(|_|unavailable())?);}
    }found.ok_or_else(unavailable)
}
impl DelegatedGroup{
    pub fn admit(directory:File,session:&str)->Result<Self>{Self::admit_role(directory,session,false)}
    pub fn admit_presentation(directory:File,session:&str)->Result<Self>{Self::admit_role(directory,session,true)}
    fn admit_role(directory:File,session:&str,presentation:bool)->Result<Self>{
        crate::linux_plan::session(session)?;let metadata=directory.metadata().map_err(|_|unavailable())?;let mut shape=std::mem::MaybeUninit::<libc::statfs>::uninit();
        let flags=unsafe{libc::fcntl(directory.as_raw_fd(),libc::F_GETFL)};
        if !metadata.is_dir()||metadata.uid()!=unsafe{libc::geteuid()}||metadata.mode()&0o7777!=0o700||flags<0||flags&libc::O_PATH!=libc::O_PATH||unsafe{libc::fstatfs(directory.as_raw_fd(),shape.as_mut_ptr())}!=0{return Err(unavailable());}
        if unsafe{shape.assume_init()}.f_type!=libc::CGROUP2_SUPER_MAGIC{return Err(unavailable());}
        let link=CString::new(format!("/proc/self/fd/{}",directory.as_raw_fd())).map_err(|_|unavailable())?;let mut path=[0u8;4096];let count=unsafe{libc::readlink(link.as_ptr(),path.as_mut_ptr().cast(),path.len())};
        if count<=0||count as usize>=path.len(){return Err(unavailable());}let path=std::str::from_utf8(&path[..count as usize]).map_err(|_|unavailable())?;
        let name=if presentation{format!("omniplay-ui-{session}")}else{format!("omniplay-{session}")};
        if path.rsplit('/').next()!=Some(name.as_str())||text(&directory,c"cgroup.type")?!="domain\n"||!text(&directory,c"cgroup.procs")?.is_empty()||text(&directory,c"pids.current")?!="0\n"||text(&directory,c"cgroup.freeze")?!="0\n"||!text(&directory,c"cgroup.subtree_control")?.trim().is_empty(){return Err(unavailable());}
        let stat=text(&directory,c"cgroup.stat")?;if field(&stat,"nr_descendants")?!=0||field(&stat,"nr_dying_descendants")?!=0{return Err(unavailable());}
        if unsafe{libc::fcntl(directory.as_raw_fd(),libc::F_SETFD,libc::FD_CLOEXEC)}!=0{return Err(unavailable());}Ok(Self{directory,session:session.to_owned(),presentation,device:metadata.dev(),inode:metadata.ino(),owner:None})
    }
    pub(crate) fn descriptor(&self)->RawFd{self.directory.as_raw_fd()}
    pub(crate) fn session(&self)->&str{&self.session}
    pub(crate) fn is_presentation(&self)->bool{self.presentation}
    pub(crate) fn identity(&self)->(u64,u64){(self.device,self.inode)}
    pub(crate) fn memory_matches(&self,bytes:u64)->bool{self.owner.as_ref().is_none_or(|owner|owner.memory_matches(bytes))}
    pub fn managed_identity(&self)->Option<&crate::linux_delegation::GroupIdentity>{self.owner.as_ref().map(|owner|owner.identity())}
    pub(crate) fn configure_presentation(&self,memory:u64,cpu:u8)->Result<()>{
        if !self.presentation||!(256*1024*1024..=2*1024*1024*1024).contains(&memory)||memory&4095!=0||!(1..=8).contains(&cpu)||!self.empty()?{return Err(unavailable());}
        for(name,value)in[(c"memory.max",memory.to_string()),(c"memory.swap.max","0".into()),(c"memory.oom.group","1".into()),(c"pids.max","64".into()),(c"cpu.max",format!("{} 100000",u32::from(cpu)*100000)),(c"cgroup.max.depth","0".into()),(c"cgroup.max.descendants","0".into())]{entry(&self.directory,name,true)?.write_all(value.as_bytes()).map_err(|_|unavailable())?;if text(&self.directory,name)?!=format!("{value}\n"){return Err(unavailable());}}
        Ok(())
    }
    pub(crate) fn terminate_tree(&self)->Result<()>{entry(&self.directory,c"cgroup.kill",true)?.write_all(b"1").map_err(|_|unavailable())}
    pub(crate) fn empty(&self)->Result<bool>{
        let events=text(&self.directory,c"cgroup.events")?;let populated=field(&events,"populated")?;if populated>1{return Err(unavailable());}
        let tasks=text(&self.directory,c"pids.current")?;let number=tasks.strip_suffix('\n').ok_or_else(unavailable)?;if number.is_empty()||number.len()>20||!number.bytes().all(|byte|byte.is_ascii_digit()){return Err(unavailable());}
        let tasks:u64=number.parse().map_err(|_|unavailable())?;Ok(populated==0&&tasks==0)
    }
}
