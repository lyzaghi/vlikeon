//! Admission of an explicitly provisioned, otherwise empty profile delegation.
//! This never discovers/grants delegation or writes an ancestor/controller knob.
use crate::linux_cgroup::{DelegatedGroup,text,field};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use std::{collections::HashMap,ffi::CString,fs::File,os::{fd::{AsRawFd,FromRawFd},unix::fs::MetadataExt},sync::{Arc,Mutex,OnceLock,Weak},thread,time::Duration};

#[derive(Clone,Copy,PartialEq,Eq)]pub struct DelegationPolicy{pub maximum_sessions:u8,pub maximum_reserved_bytes:u64}
#[derive(Clone,PartialEq,Eq)]pub struct DelegationIdentity{pub profile:String,pub device:u64,pub inode:u64}
#[derive(Clone,PartialEq,Eq)]pub struct GroupIdentity{pub parent:DelegationIdentity,pub session:String,pub presentation:bool,pub device:u64,pub inode:u64,pub charged_memory:u64}
pub struct PairBudget{pub actor_memory:u64,pub root:u64,pub scratch:u64,pub save:u64,pub presentation_memory:u64}
impl PairBudget{
    fn total(&self)->Result<u64>{
        let mib=1024*1024;for(value,minimum,maximum)in[(self.actor_memory,256*mib,8192*mib),(self.root,16*mib,1024*mib),(self.scratch,4*mib,512*mib),(self.save,4*mib,512*mib),(self.presentation_memory,256*mib,2048*mib)]{if !(minimum..=maximum).contains(&value)||value&4095!=0{return Err(unavailable());}}
        [self.actor_memory,self.root,self.scratch,self.save,self.presentation_memory,128*mib].into_iter().try_fold(0u64,|total,value|total.checked_add(value).ok_or_else(unavailable))
    }
}
struct Counters{sessions:u8,bytes:u64}
struct Parent{directory:File,_lease:File,identity:DelegationIdentity,policy:DelegationPolicy,counters:Mutex<Counters>}
#[derive(Clone)]pub struct Delegation{inner:Arc<Parent>}
pub struct RecoveryDelegation{inner:Arc<Parent>,audit:Mutex<()>}
struct Reservation{parent:Arc<Parent>,bytes:u64}
impl Drop for Reservation{fn drop(&mut self){let mut value=self.parent.counters.lock().unwrap_or_else(|error|error.into_inner());value.sessions=value.sessions.saturating_sub(1);value.bytes=value.bytes.saturating_sub(self.bytes);}}
pub(crate) struct OwnedLeaf{parent:Arc<Parent>,reservation:Arc<Reservation>,name:CString,directory:Option<File>,identity:GroupIdentity}
static PARENTS:OnceLock<Mutex<HashMap<(u64,u64),Weak<Parent>>>>=OnceLock::new();
fn unavailable()->PlatformError{PlatformError::new(ErrorCode::RuntimeUnavailable,"The owned profile Linux resource delegation could not be established")}
fn number(raw:&str)->Result<u64>{let value=raw.strip_suffix('\n').ok_or_else(unavailable)?;if value.is_empty()||value.len()>20||!value.bytes().all(|byte|byte.is_ascii_digit()){return Err(unavailable());}value.parse().map_err(|_|unavailable())}
fn no_descendants(directory:&File)->Result<bool>{let raw=text(directory,c"cgroup.stat")?;Ok(field(&raw,"nr_descendants")?==0&&field(&raw,"nr_dying_descendants")?==0)}
fn no_dying(directory:&File)->Result<bool>{Ok(field(&text(directory,c"cgroup.stat")?,"nr_dying_descendants")?==0)}
fn empty(directory:&File)->Result<bool>{Ok(field(&text(directory,c"cgroup.events")?,"populated")?==0&&number(&text(directory,c"pids.current")?)?==0)}
fn metadata(directory:&File)->Result<std::fs::Metadata>{
    let value=directory.metadata().map_err(|_|unavailable())?;let flags=unsafe{libc::fcntl(directory.as_raw_fd(),libc::F_GETFL)};let mut shape=std::mem::MaybeUninit::<libc::statfs>::uninit();
    if unsafe{libc::geteuid()}==0||!value.is_dir()||value.uid()!=unsafe{libc::geteuid()}||value.mode()&0o7777!=0o700||flags<0||flags&libc::O_PATH!=libc::O_PATH||unsafe{libc::fstatfs(directory.as_raw_fd(),shape.as_mut_ptr())}!=0||unsafe{shape.assume_init()}.f_type!=libc::CGROUP2_SUPER_MAGIC{return Err(unavailable());}Ok(value)
}
fn basename(directory:&File)->Result<String>{let link=CString::new(format!("/proc/self/fd/{}",directory.as_raw_fd())).map_err(|_|unavailable())?;let mut raw=[0u8;4096];let count=unsafe{libc::readlink(link.as_ptr(),raw.as_mut_ptr().cast(),raw.len())};if count<=0||count as usize>=raw.len(){return Err(unavailable());}let path=std::str::from_utf8(&raw[..count as usize]).map_err(|_|unavailable())?;Ok(path.rsplit('/').next().ok_or_else(unavailable)?.to_owned())}
impl Parent{
    fn limits(&self)->Result<u64>{
        metadata(&self.directory)?;if basename(&self.directory)?!=format!("omniplay-profile-{}",self.identity.profile)||text(&self.directory,c"cgroup.type")?!="domain\n"||!text(&self.directory,c"cgroup.procs")?.is_empty()||text(&self.directory,c"cgroup.freeze")?!="0\n"||field(&text(&self.directory,c"cgroup.events")?,"frozen")?!=0{return Err(unavailable());}
        let controllers=text(&self.directory,c"cgroup.subtree_control")?;let controllers:Vec<_>=controllers.split_ascii_whitespace().collect();if controllers.len()>32||["cpu","memory","pids"].iter().any(|name|!controllers.contains(name)){return Err(unavailable());}
        let memory=number(&text(&self.directory,c"memory.max")?)?;let tasks=number(&text(&self.directory,c"pids.max")?)?;
        if memory<512*1024*1024||memory>self.policy.maximum_reserved_bytes||text(&self.directory,c"memory.swap.max")?!="0\n"||tasks!=u64::from(self.policy.maximum_sessions)*128||text(&self.directory,c"cgroup.max.depth")?!="1\n"||number(&text(&self.directory,c"cgroup.max.descendants")?)?!=u64::from(self.policy.maximum_sessions)*2{return Err(unavailable());}
        let raw=text(&self.directory,c"cpu.max")?;let parts:Vec<_>=raw.split_ascii_whitespace().collect();if parts.len()!=2||parts.iter().any(|value|value.is_empty()||value.len()>20||!value.bytes().all(|byte|byte.is_ascii_digit())){return Err(unavailable());}let quota:u64=parts[0].parse().map_err(|_|unavailable())?;let period:u64=parts[1].parse().map_err(|_|unavailable())?;if !(1000..=1000000).contains(&period)||quota==0||quota>period*8{return Err(unavailable());}Ok(memory)
    }
}
impl Delegation{
    /// The caller must supply the exact private profile lease and a host/service
    /// provisioned subtree with finite ceilings. Neither is inferred from a path.
    pub fn admit(directory:File,profile:&str,lease:File,policy:DelegationPolicy)->Result<Self>{
        Self::admit_owned(directory,profile,lease,policy,false)
    }
    fn admit_owned(directory:File,profile:&str,lease:File,policy:DelegationPolicy,recovery:bool)->Result<Self>{
        crate::linux_plan::session(profile)?;if !(1..=8).contains(&policy.maximum_sessions)||!(512*1024*1024..=64*1024*1024*1024).contains(&policy.maximum_reserved_bytes)||policy.maximum_reserved_bytes&4095!=0{return Err(unavailable());}
        let value=metadata(&directory)?;let lock=lease.metadata().map_err(|_|unavailable())?;if !lock.is_file()||lock.uid()!=unsafe{libc::geteuid()}||lock.nlink()!=1||lock.mode()&0o7777!=0o600{return Err(unavailable());}lease.try_lock().map_err(|_|unavailable())?;
        if unsafe{libc::fcntl(directory.as_raw_fd(),libc::F_SETFD,libc::FD_CLOEXEC)}!=0||unsafe{libc::fcntl(lease.as_raw_fd(),libc::F_SETFD,libc::FD_CLOEXEC)}!=0{return Err(unavailable());}
        let parent=Arc::new(Parent{directory,_lease:lease,identity:DelegationIdentity{profile:profile.to_owned(),device:value.dev(),inode:value.ino()},policy,counters:Mutex::new(Counters{sessions:0,bytes:0})});parent.limits()?;
        if !recovery&&(!empty(&parent.directory)?||!no_descendants(&parent.directory)?){return Err(unavailable());}
        let mut known=PARENTS.get_or_init(||Mutex::new(HashMap::new())).lock().map_err(|_|unavailable())?;known.retain(|_,parent|parent.strong_count()>0);if known.len()>=64||known.get(&(value.dev(),value.ino())).and_then(Weak::upgrade).is_some(){return Err(unavailable());}known.insert((value.dev(),value.ino()),Arc::downgrade(&parent));Ok(Self{inner:parent})
    }
    pub fn identity(&self)->&DelegationIdentity{&self.inner.identity}
    /// Durable intent precedes either mkdir; each observed inode is acknowledged
    /// before its leaf can be returned for a child launch. Existing names fail.
    pub fn create_pair(&self,session:&str,budget:&PairBudget,before_create:impl FnOnce(&DelegationIdentity,&str)->Result<()>,mut after_create:impl FnMut(&GroupIdentity)->Result<()>)->Result<(DelegatedGroup,DelegatedGroup)>{
        crate::linux_plan::session(session)?;let ceiling=self.inner.limits()?;let bytes=budget.total()?;
        let reservation={let mut value=self.inner.counters.lock().map_err(|_|unavailable())?;let total=value.bytes.checked_add(bytes).ok_or_else(unavailable)?;if value.sessions>=self.inner.policy.maximum_sessions||total>ceiling{return Err(PlatformError::new(ErrorCode::ResourceLimit,"The profile's Linux session resource reservation is full"));}value.sessions+=1;value.bytes=total;Arc::new(Reservation{parent:self.inner.clone(),bytes})};
        before_create(self.identity(),session)?;let actor=self.create_leaf(session,false,bytes-budget.presentation_memory,reservation.clone(),&mut after_create)?;let presentation=self.create_leaf(session,true,budget.presentation_memory,reservation,&mut after_create)?;Ok((actor,presentation))
    }
    fn create_leaf(&self,session:&str,presentation:bool,charged_memory:u64,reservation:Arc<Reservation>,after_create:&mut impl FnMut(&GroupIdentity)->Result<()>)->Result<DelegatedGroup>{
        let name=CString::new(if presentation{format!("omniplay-ui-{session}")}else{format!("omniplay-{session}")}).map_err(|_|unavailable())?;
        if unsafe{libc::mkdirat(self.inner.directory.as_raw_fd(),name.as_ptr(),0o700)}!=0{return Err(unavailable());}
        let raw=unsafe{libc::openat(self.inner.directory.as_raw_fd(),name.as_ptr(),libc::O_PATH|libc::O_DIRECTORY|libc::O_NOFOLLOW|libc::O_CLOEXEC)};
        if raw<0{std::mem::forget(reservation);return Err(unavailable());}let directory=unsafe{File::from_raw_fd(raw)};let value=match metadata(&directory){Ok(value)=>value,Err(error)=>{std::mem::forget((directory,reservation));return Err(error);}};
        let owner=OwnedLeaf{parent:self.inner.clone(),reservation,name,directory:Some(directory),identity:GroupIdentity{parent:self.identity().clone(),session:session.to_owned(),presentation,device:value.dev(),inode:value.ino(),charged_memory}};
        let copied=owner.directory.as_ref().ok_or_else(unavailable)?.try_clone().map_err(|_|unavailable())?;let mut group=if presentation{DelegatedGroup::admit_presentation(copied,session)?}else{DelegatedGroup::admit(copied,session)?};after_create(&owner.identity)?;group.owner=Some(owner);Ok(group)
    }
}
impl RecoveryDelegation{
    /// Trusted provisioning owner only. The exact profile lease is held for the
    /// entire audit; no active Delegation owner can share this parent identity.
    pub fn admit(directory:File,profile:&str,lease:File,policy:DelegationPolicy)->Result<Self>{let admitted=Delegation::admit_owned(directory,profile,lease,policy,true)?;Ok(Self{inner:admitted.inner,audit:Mutex::new(())})}
    pub fn begin_audit(&self)->Result<std::sync::MutexGuard<'_,()>>{self.audit.try_lock().map_err(|_|PlatformError::new(ErrorCode::Busy,"The Linux profile recovery audit is already active"))}
    pub fn identity(&self)->&DelegationIdentity{&self.inner.identity}
    fn leaf(&self,expected:&GroupIdentity)->Result<(CString,Option<File>)>{
        crate::linux_plan::session(&expected.session)?;self.inner.limits()?;if expected.parent!=self.inner.identity||expected.device!=self.inner.identity.device||expected.inode==0||expected.inode==self.inner.identity.inode{return Err(unavailable());}let name=CString::new(if expected.presentation{format!("omniplay-ui-{}",expected.session)}else{format!("omniplay-{}",expected.session)}).map_err(|_|unavailable())?;
        let raw=unsafe{libc::openat(self.inner.directory.as_raw_fd(),name.as_ptr(),libc::O_PATH|libc::O_DIRECTORY|libc::O_NOFOLLOW|libc::O_CLOEXEC)};if raw<0{return if std::io::Error::last_os_error().raw_os_error()==Some(libc::ENOENT){Ok((name,None))}else{Err(unavailable())};}let directory=unsafe{File::from_raw_fd(raw)};let shape=metadata(&directory)?;if shape.dev()!=expected.device||shape.ino()!=expected.inode||text(&directory,c"cgroup.type")?!="domain\n"||!empty(&directory)?||!no_descendants(&directory)?||!text(&directory,c"cgroup.procs")?.is_empty(){return Err(unavailable());}Ok((name,Some(directory)))
    }
    /// Observation alone grants no signalling. The work journal owner must also
    /// prove both recorded processes ended before asking to remove these leaves.
    pub fn confirm_empty(&self,expected:&GroupIdentity)->Result<()>{let(_,directory)=self.leaf(expected)?;drop(directory);Ok(())}
    /// The caller must durably acknowledge retirement before this operation.
    /// Missing names are accepted only under that already recorded ownership
    /// chain. Neither an occupied/mismatched leaf nor an unknown PID is killed.
    pub fn remove_empty(&self,expected:&GroupIdentity)->Result<()>{
        let(name,directory)=self.leaf(expected)?;if let Some(directory)=directory{
            let mut current=std::mem::MaybeUninit::<libc::stat>::uninit();if unsafe{libc::fstatat(self.inner.directory.as_raw_fd(),name.as_ptr(),current.as_mut_ptr(),libc::AT_SYMLINK_NOFOLLOW)}!=0{return Err(unavailable());}let current=unsafe{current.assume_init()};if current.st_dev!=expected.device||current.st_ino!=expected.inode||(current.st_mode&libc::S_IFMT)!=libc::S_IFDIR{return Err(unavailable());}
            if unsafe{libc::unlinkat(self.inner.directory.as_raw_fd(),name.as_ptr(),libc::AT_REMOVEDIR)}!=0{return Err(unavailable());}drop(directory);
        }Ok(())
    }
    pub fn charges_retired(&self)->Result<bool>{self.inner.limits()?;no_dying(&self.inner.directory)}
    /// New launches remain impossible until the whole admitted subtree has no
    /// live, visible or dying descendants. Unknown leftovers prevent conversion.
    pub fn finish(self)->Result<Delegation>{self.inner.limits()?;if !empty(&self.inner.directory)?||!no_descendants(&self.inner.directory)?{return Err(unavailable());}Ok(Delegation{inner:self.inner})}
}
impl OwnedLeaf{
    pub(crate) fn memory_matches(&self,bytes:u64)->bool{self.identity.charged_memory==bytes}
    pub(crate) fn identity(&self)->&GroupIdentity{&self.identity}
    fn same_name(&self)->Result<bool>{let mut value=std::mem::MaybeUninit::<libc::stat>::uninit();if unsafe{libc::fstatat(self.parent.directory.as_raw_fd(),self.name.as_ptr(),value.as_mut_ptr(),libc::AT_SYMLINK_NOFOLLOW)}!=0{return Err(unavailable());}let value=unsafe{value.assume_init()};Ok(value.st_dev==self.identity.device&&value.st_ino==self.identity.inode&&(value.st_mode&libc::S_IFMT)==libc::S_IFDIR&&value.st_uid==unsafe{libc::geteuid()})}
    fn retain_unknown(&mut self){std::mem::forget((self.parent.clone(),self.reservation.clone(),self.directory.take()));}
}
struct Retirement{parent:Arc<Parent>,_reservation:Arc<Reservation>}
impl Drop for OwnedLeaf{fn drop(&mut self){
    let ready=self.directory.as_ref().is_some_and(|directory|empty(directory).unwrap_or(false)&&no_descendants(directory).unwrap_or(false))&&self.same_name().unwrap_or(false);
    if !ready||unsafe{libc::unlinkat(self.parent.directory.as_raw_fd(),self.name.as_ptr(),libc::AT_REMOVEDIR)}!=0{self.retain_unknown();return;}
    // Close this leaf handle before waiting: kernfs references can themselves
    // retain a dying cgroup. The group's other descriptor closes on its drop.
    self.directory.take();let retirement=Arc::new(Retirement{parent:self.parent.clone(),_reservation:self.reservation.clone()});let watcher=retirement.clone();
    if thread::Builder::new().name("omniplay-cgroup-retire".into()).spawn(move||{while !no_dying(&watcher.parent.directory).unwrap_or(false){thread::sleep(Duration::from_millis(100));}}).is_err(){std::mem::forget(retirement);}
}}
