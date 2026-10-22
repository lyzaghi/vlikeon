//! Gated bootstrap launch source for the future Linux broker adapter. This owns
//! live pidfds and a fresh delegated tree; it does not enable registry support.
use crate::{linux_cgroup::DelegatedGroup,linux_elf::{BootstrapExecutable,PresentationExecutable},linux_plan::{BootstrapPlan,RootFile,RootFileKind},linux_sealed::{SealedBudget,SealedObject},linux_presentation::{PresentationSpawn,LinuxPresentationHost}};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use std::{ffi::{CStr,CString},fs::{File,OpenOptions},io::Read,mem::MaybeUninit,os::{fd::{AsRawFd,FromRawFd,OwnedFd,RawFd},unix::{fs::{MetadataExt,OpenOptionsExt,FileTypeExt},ffi::OsStrExt}},ptr::{null,null_mut},sync::{Arc,Mutex,OnceLock,mpsc::{self,SyncSender},atomic::{AtomicBool,Ordering}},thread,time::{Duration,Instant}};

#[derive(Clone)]pub struct LinuxProcessIdentity{pub process_id:u32,pub start_ticks:u64,pub boot_id:String}
pub enum LaunchFile{Image{path:String,executable:bool,object:SealedObject},GameInput{path:String,bytes:u64,file:File}}
impl LaunchFile{
    fn descriptor(&self)->RawFd{match self{Self::Image{object,..}=>object.as_raw_fd(),Self::GameInput{file,..}=>file.as_raw_fd()}}
    fn declaration(&self,descriptor:u16)->RootFile{match self{Self::Image{path,executable,object}=>RootFile{descriptor,path:path.clone(),bytes:object.bytes(),kind:RootFileKind::Image{executable:*executable}},Self::GameInput{path,bytes,..}=>RootFile{descriptor,path:path.clone(),bytes:*bytes,kind:RootFileKind::GameInput}}}
}
pub struct BootstrapSpawn{
    pub executable:BootstrapExecutable,pub session:String,pub root_directory:File,pub resource_group:DelegatedGroup,pub work_lease:File,
    pub files:Vec<LaunchFile>,pub media_input:File,pub media_output:File,
    pub memory_bytes:u64,pub maximum_file_bytes:u64,pub root_bytes:u64,pub scratch_bytes:u64,pub save_bytes:u64,pub cpu_cores:u8,
}
pub struct BootstrapLaunched{pub process:LinuxChild,pub input:File,pub output:File}
enum Retained{Bootstrap{_root:File,_lease:File,_files:Vec<LaunchFile>,_executable:BootstrapExecutable,_plan:SealedObject},Presentation{_directory:File,_temporary:File,_lease:File,_executable:PresentationExecutable}}
struct Resources{group:DelegatedGroup,_retained:Retained}
struct State{pidfd:OwnedFd,resources:Resources,start_gate:Mutex<Option<OwnedFd>>,stop:AtomicBool,ended:AtomicBool}
pub struct LinuxChild{state:Arc<State>,identity:Option<LinuxProcessIdentity>}
fn unavailable()->PlatformError{PlatformError::new(ErrorCode::RuntimeUnavailable,"The private Linux runtime process could not be established")}
fn cancelled(cancel:&AtomicBool)->Result<()>{if cancel.load(Ordering::Acquire){Err(PlatformError::new(ErrorCode::Cancelled,"Linux runtime launch was cancelled"))}else{Ok(())}}
fn owned(fd:RawFd)->Result<OwnedFd>{if fd<0{Err(unavailable())}else{Ok(unsafe{OwnedFd::from_raw_fd(fd)})}}
fn duplicated(fd:RawFd)->Result<OwnedFd>{owned(unsafe{libc::fcntl(fd,libc::F_DUPFD_CLOEXEC,512)})}
pub fn private_pipe()->Result<(File,File)>{let mut descriptors=[-1;2];if unsafe{libc::pipe2(descriptors.as_mut_ptr(),libc::O_CLOEXEC)}!=0{return Err(unavailable());}Ok((File::from(owned(descriptors[0])?),File::from(owned(descriptors[1])?)))}
pub fn private_directory(path:&std::path::Path)->Result<File>{let directory=OpenOptions::new().read(true).custom_flags(libc::O_PATH|libc::O_DIRECTORY|libc::O_CLOEXEC|libc::O_NOFOLLOW).open(path).map_err(|_|unavailable())?;let value=directory.metadata().map_err(|_|unavailable())?;if !value.is_dir()||value.uid()!=unsafe{libc::geteuid()}||value.mode()&0o7777!=0o700{return Err(unavailable());}Ok(directory)}
pub fn private_record(path:&std::path::Path)->Result<File>{let file=OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK|libc::O_CLOEXEC|libc::O_NOFOLLOW).open(path).map_err(|_|unavailable())?;let value=file.metadata().map_err(|_|unavailable())?;if !value.is_file()||value.uid()!=unsafe{libc::geteuid()}||value.mode()&0o7777!=0o600||value.nlink()!=1{return Err(unavailable());}Ok(file)}
pub fn private_input(path:&std::path::Path,bytes:u64)->Result<File>{let file=OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK|libc::O_CLOEXEC|libc::O_NOFOLLOW).open(path).map_err(|_|unavailable())?;let value=file.metadata().map_err(|_|unavailable())?;if bytes==0||bytes>4*1024*1024*1024||!value.is_file()||value.uid()!=unsafe{libc::geteuid()}||value.mode()&0o7777!=0o400||value.nlink()!=1||value.len()!=bytes{return Err(unavailable());}Ok(file)}
fn gate()->Result<(OwnedFd,OwnedFd)>{let mut descriptors=[-1;2];if unsafe{libc::socketpair(libc::AF_UNIX,libc::SOCK_SEQPACKET|libc::SOCK_CLOEXEC,0,descriptors.as_mut_ptr())}!=0{return Err(unavailable());}Ok((owned(descriptors[0])?,owned(descriptors[1])?))}
fn bounded(path:&str,maximum:usize)->Result<Vec<u8>>{let file=File::open(path).map_err(|_|unavailable())?;let mut raw=Vec::new();file.take(maximum as u64+1).read_to_end(&mut raw).map_err(|_|unavailable())?;if raw.len()>maximum{return Err(unavailable());}Ok(raw)}
fn dead(pidfd:RawFd,timeout:i32)->Result<bool>{let mut item=libc::pollfd{fd:pidfd,events:libc::POLLIN,revents:0};let result=unsafe{libc::poll(&mut item,1,timeout)};if result<0{return Err(unavailable());}if item.revents&(libc::POLLERR|libc::POLLNVAL)!=0{return Err(unavailable());}Ok(result==1&&item.revents&(libc::POLLIN|libc::POLLHUP)!=0)}
fn boot_id()->Result<String>{let raw=bounded("/proc/sys/kernel/random/boot_id",64)?;let value=std::str::from_utf8(&raw).map_err(|_|unavailable())?;if value.len()!=37||!value.ends_with('\n'){return Err(unavailable());}let value=&value[..36];crate::linux_plan::session(value)?;Ok(value.to_owned())}
fn process_id(pidfd:RawFd)->Result<u32>{
    let raw=bounded(&format!("/proc/self/fdinfo/{pidfd}"),4096)?;let text=std::str::from_utf8(&raw).map_err(|_|unavailable())?;let mut found=None;
    for line in text.lines(){if let Some(value)=line.strip_prefix("Pid:"){if found.is_some(){return Err(unavailable());}let value=value.trim();if value.is_empty()||value.len()>10||!value.bytes().all(|byte|byte.is_ascii_digit()){return Err(unavailable());}let value:u32=value.parse().map_err(|_|unavailable())?;if !(2..=i32::MAX as u32).contains(&value){return Err(unavailable());}found=Some(value);}}
    found.ok_or_else(unavailable)
}
fn start_ticks(process:u32)->Result<u64>{
    let raw=bounded(&format!("/proc/{process}/stat"),8192)?;let text=std::str::from_utf8(&raw).map_err(|_|unavailable())?;let close=text.rfind(')').ok_or_else(unavailable)?;let open=text.find('(').ok_or_else(unavailable)?;
    if open>=close||text[..open].trim()!=process.to_string(){return Err(unavailable());}let fields:Vec<_>=text[close+1..].split_ascii_whitespace().collect();if fields.len()<20||fields.len()>64{return Err(unavailable());}let value=fields[19];if value.len()>20||!value.bytes().all(|byte|byte.is_ascii_digit()){return Err(unavailable());}let ticks=value.parse().map_err(|_|unavailable())?;if ticks==0{return Err(unavailable());}Ok(ticks)
}
fn identity(pidfd:RawFd)->Result<LinuxProcessIdentity>{if dead(pidfd,0)?{return Err(unavailable());}let identity=LinuxProcessIdentity{process_id:process_id(pidfd)?,start_ticks:0,boot_id:boot_id()?};let ticks=start_ticks(identity.process_id)?;if dead(pidfd,0)?{return Err(unavailable());}Ok(LinuxProcessIdentity{start_ticks:ticks,..identity})}
/// Observe the live broker before it publishes a new work ownership intent.
/// The returned identity is recovery evidence, never authority for signalling.
pub fn current_broker_identity()->Result<LinuxProcessIdentity>{let raw=unsafe{libc::syscall(libc::SYS_pidfd_open,std::process::id(),0u32)};if raw<0{return Err(unavailable());}let descriptor=owned(raw as RawFd)?;let result=identity(descriptor.as_raw_fd())?;if result.process_id!=std::process::id(){return Err(unavailable());}Ok(result)}
type Spawn=unsafe extern "C" fn(*mut libc::c_int,*const libc::c_char,*const libc::posix_spawn_file_actions_t,*const libc::posix_spawnattr_t,*const *mut libc::c_char,*const *mut libc::c_char)->libc::c_int;
type Fchdir=unsafe extern "C" fn(*mut libc::posix_spawn_file_actions_t,libc::c_int)->libc::c_int;
type CloseFrom=unsafe extern "C" fn(*mut libc::posix_spawn_file_actions_t,libc::c_int)->libc::c_int;
type SetCgroup=unsafe extern "C" fn(*mut libc::posix_spawnattr_t,libc::c_int)->libc::c_int;
struct Functions{spawn:Spawn,fchdir:Fchdir,close_from:CloseFrom,set_cgroup:SetCgroup}
fn symbol(name:&CStr)->Result<*mut libc::c_void>{let value=unsafe{libc::dlsym(libc::RTLD_DEFAULT,name.as_ptr())};if value.is_null(){Err(unavailable())}else{Ok(value)}}
fn functions()->Result<Functions>{
    // Resolve optional GNU APIs in the already loaded libc. Older hosts retain
    // their browser/Web path; there is no fork/PID-number launch fallback.
    let version:unsafe extern "C" fn()->*const libc::c_char=unsafe{std::mem::transmute(symbol(c"gnu_get_libc_version")?)};let pointer=unsafe{version()};if pointer.is_null(){return Err(unavailable());}let(mut raw,mut terminated)=(Vec::new(),false);for at in 0..32{let byte=unsafe{*pointer.add(at)}as u8;if byte==0{terminated=true;break;}raw.push(byte);}if !terminated||!raw.iter().all(|byte|byte.is_ascii_digit()||*byte==b'.'){return Err(unavailable());}
    let version=std::str::from_utf8(&raw).map_err(|_|unavailable())?;let mut parts=version.split('.');let major=parts.next().ok_or_else(unavailable)?;let minor=parts.next().ok_or_else(unavailable)?;if major!="2"||minor.is_empty()||!minor.bytes().all(|byte|byte.is_ascii_digit())||minor.parse::<u32>().map_err(|_|unavailable())?<40{return Err(unavailable());}
    Ok(Functions{spawn:unsafe{std::mem::transmute(symbol(c"pidfd_spawn")?)},fchdir:unsafe{std::mem::transmute(symbol(c"posix_spawn_file_actions_addfchdir_np")?)},close_from:unsafe{std::mem::transmute(symbol(c"posix_spawn_file_actions_addclosefrom_np")?)},set_cgroup:unsafe{std::mem::transmute(symbol(c"posix_spawnattr_setcgroup_np")?)}})
}
struct Actions(libc::posix_spawn_file_actions_t);
impl Actions{fn new()->Result<Self>{let mut value=MaybeUninit::uninit();if unsafe{libc::posix_spawn_file_actions_init(value.as_mut_ptr())}!=0{return Err(unavailable());}Ok(Self(unsafe{value.assume_init()}))}fn duplicate(&mut self,source:RawFd,target:RawFd)->Result<()>{if unsafe{libc::posix_spawn_file_actions_adddup2(&mut self.0,source,target)}!=0{return Err(unavailable());}Ok(())}}
impl Drop for Actions{fn drop(&mut self){unsafe{libc::posix_spawn_file_actions_destroy(&mut self.0);}}}
struct Attributes(libc::posix_spawnattr_t);
impl Attributes{fn new()->Result<Self>{let mut value=MaybeUninit::uninit();if unsafe{libc::posix_spawnattr_init(value.as_mut_ptr())}!=0{return Err(unavailable());}let mut value=Self(unsafe{value.assume_init()});let(mut empty,mut defaults)=(MaybeUninit::<libc::sigset_t>::uninit(),MaybeUninit::<libc::sigset_t>::uninit());
    if unsafe{libc::sigemptyset(empty.as_mut_ptr())}!=0||unsafe{libc::sigfillset(defaults.as_mut_ptr())}!=0{return Err(unavailable());}let(empty,defaults)=unsafe{(empty.assume_init(),defaults.assume_init())};
    let flags=(libc::POSIX_SPAWN_SETSIGDEF|libc::POSIX_SPAWN_SETSIGMASK|libc::POSIX_SPAWN_SETPGROUP)as libc::c_short;
    if unsafe{libc::posix_spawnattr_setsigmask(&mut value.0,&empty)}!=0||unsafe{libc::posix_spawnattr_setsigdefault(&mut value.0,&defaults)}!=0||unsafe{libc::posix_spawnattr_setpgroup(&mut value.0,0)}!=0||unsafe{libc::posix_spawnattr_setflags(&mut value.0,flags)}!=0{return Err(unavailable());}Ok(value)
}}
impl Drop for Attributes{fn drop(&mut self){unsafe{libc::posix_spawnattr_destroy(&mut self.0);}}}
impl Attributes{fn cgroup(&mut self,functions:&Functions,descriptor:RawFd)->Result<()>{
    // GNU spawn.h POSIX_SPAWN_SETCGROUP, with no legacy clone fallback when
    // atomic CLONE_INTO_CGROUP cannot be established by the actual host.
    let flags=(libc::POSIX_SPAWN_SETSIGDEF|libc::POSIX_SPAWN_SETSIGMASK|libc::POSIX_SPAWN_SETPGROUP|0x100)as libc::c_short;
    if unsafe{(functions.set_cgroup)(&mut self.0,descriptor)}!=0||unsafe{libc::posix_spawnattr_setflags(&mut self.0,flags)}!=0{return Err(unavailable());}Ok(())
}}
impl State{
    fn request_stop(&self){
        self.stop.store(true,Ordering::Release);match self.start_gate.lock(){Ok(mut value)=>{value.take();},Err(value)=>{value.into_inner().take();}}
        let _=self.resources.group.terminate_tree();unsafe{libc::syscall(libc::SYS_pidfd_send_signal,self.pidfd.as_raw_fd(),libc::SIGKILL,null::<libc::siginfo_t>(),0u32);}
    }
    fn completed(&self)->Result<bool>{
        if self.ended.load(Ordering::Acquire){return Ok(true);}if !dead(self.pidfd.as_raw_fd(),0)?||!self.resources.group.empty()?{return Ok(false);}
        let mut status=MaybeUninit::<libc::siginfo_t>::zeroed();let result=unsafe{libc::waitid(libc::P_PIDFD,self.pidfd.as_raw_fd()as libc::id_t,status.as_mut_ptr(),libc::WEXITED|libc::WNOHANG)};
        if result<0{match std::io::Error::last_os_error().raw_os_error(){Some(libc::ECHILD)=>{},Some(libc::EINTR)=>return Ok(false),_=>return Err(unavailable())}}
        else if unsafe{status.assume_init().si_pid()}==0{return Ok(false);}
        // pidfd death plus an empty owned cgroup tree is required even when a
        // caller's SIGCHLD policy already reaped the exact process (ECHILD).
        self.ended.store(true,Ordering::Release);Ok(true)
    }
    fn reap(self:Arc<Self>){
        loop{if self.stop.load(Ordering::Acquire){self.request_stop();}if self.completed().unwrap_or(false){return;}let _=dead(self.pidfd.as_raw_fd(),500);thread::sleep(Duration::from_millis(100));}
    }
}
impl LinuxChild{
    pub fn identity(&self)->Result<&LinuxProcessIdentity>{self.identity.as_ref().ok_or_else(unavailable)}
    pub fn has_exited(&self)->bool{self.state.ended.load(Ordering::Acquire)}
    pub fn terminate(&self){let first=!self.state.stop.swap(true,Ordering::AcqRel);self.state.request_stop();if first{let until=Instant::now()+Duration::from_secs(5);while !self.has_exited()&&Instant::now()<until{thread::sleep(Duration::from_millis(10));}}}
}
impl Drop for LinuxChild{fn drop(&mut self){self.terminate();/* The reaper owns all resources until real process/tree exit. */}}
fn observed(pidfd:OwnedFd,resources:Resources,gate_parent:OwnedFd,cancel:&AtomicBool)->Result<LinuxChild>{
    let state=Arc::new(State{pidfd,resources,start_gate:Mutex::new(Some(gate_parent)),stop:AtomicBool::new(false),ended:AtomicBool::new(false)});let mut process=LinuxChild{state:state.clone(),identity:None};let watcher=state.clone();
    if thread::Builder::new().name("omniplay-linux-reaper".into()).spawn(move||watcher.reap()).is_err(){
        state.request_stop();let until=Instant::now()+Duration::from_secs(5);while Instant::now()<until&&!state.completed().unwrap_or(false){thread::sleep(Duration::from_millis(25));}
        if !state.ended.load(Ordering::Acquire){std::mem::forget(process);}return Err(unavailable());
    }
    process.identity=Some(identity(state.pidfd.as_raw_fd())?);cancelled(cancel)?;Ok(process)
}

fn spawn_owned(spec:BootstrapSpawn,budget:&SealedBudget,cancel:&AtomicBool)->Result<BootstrapLaunched>{
    cancelled(cancel)?;let charged=[spec.memory_bytes,spec.root_bytes,spec.scratch_bytes,spec.save_bytes,128*1024*1024].into_iter().try_fold(0u64,|total,value|total.checked_add(value).ok_or_else(unavailable))?;if spec.resource_group.session()!=spec.session||spec.resource_group.is_presentation()||!spec.resource_group.memory_matches(charged)||!(2..=256).contains(&spec.files.len()){return Err(unavailable());}let functions=functions()?;
    for file in &spec.files{let path=match file{LaunchFile::Image{path,..}|LaunchFile::GameInput{path,..}=>path};if path.is_empty()||path.len()>=512{return Err(unavailable());}}
    let lease=spec.work_lease.metadata().map_err(|_|unavailable())?;if !lease.is_file()||lease.uid()!=unsafe{libc::geteuid()}||lease.nlink()!=1||lease.mode()&0o077!=0{return Err(unavailable());}spec.work_lease.try_lock().map_err(|_|unavailable())?;
    let plan=BootstrapPlan{session:spec.session.clone(),expected_parent:std::process::id(),empty_directory_descriptor:8,cgroup_descriptor:9,memory_bytes:spec.memory_bytes,maximum_file_bytes:spec.maximum_file_bytes,root_bytes:spec.root_bytes,scratch_bytes:spec.scratch_bytes,save_bytes:spec.save_bytes,cpu_cores:spec.cpu_cores,files:spec.files.iter().enumerate().map(|(index,file)|file.declaration((10+index)as u16)).collect()};
    let plan=budget.plan(&plan,cancel)?;let(child_input,input)=private_pipe()?;let(output,child_output)=private_pipe()?;let(gate_child,gate_parent)=gate()?;
    let null_file=OpenOptions::new().write(true).custom_flags(libc::O_CLOEXEC|libc::O_NOFOLLOW).open("/dev/null").map_err(|_|unavailable())?;
    let cwd=OpenOptions::new().read(true).custom_flags(libc::O_PATH|libc::O_DIRECTORY|libc::O_CLOEXEC).open("/").map_err(|_|unavailable())?;let cwd=duplicated(cwd.as_raw_fd())?;
    let mut originals=vec![child_input.as_raw_fd(),child_output.as_raw_fd(),null_file.as_raw_fd(),spec.media_input.as_raw_fd(),spec.media_output.as_raw_fd(),plan.as_raw_fd(),gate_child.as_raw_fd(),spec.executable.descriptor(),spec.root_directory.as_raw_fd(),spec.resource_group.descriptor()];
    originals.extend(spec.files.iter().map(LaunchFile::descriptor));let mut sources=Vec::with_capacity(originals.len());for descriptor in originals{sources.push(duplicated(descriptor)?);}
    let mut actions=Actions::new()?;for(index,source)in sources.iter().enumerate(){actions.duplicate(source.as_raw_fd(),index as RawFd)?;}
    if unsafe{(functions.fchdir)(&mut actions.0,cwd.as_raw_fd())}!=0||unsafe{(functions.close_from)(&mut actions.0,sources.len()as RawFd)}!=0{return Err(unavailable());}let attributes=Attributes::new()?;
    let arguments=[CString::new("omniplay-linux-bootstrap").map_err(|_|unavailable())?,CString::new("--stdio-isolation-v3").map_err(|_|unavailable())?,CString::new(spec.session.as_str()).map_err(|_|unavailable())?];let mut argv:Vec<_>=arguments.iter().map(|value|value.as_ptr()as *mut libc::c_char).collect();argv.push(null_mut());
    let environment=[c"PATH=/usr/bin:/bin",c"LANG=C",c"LC_ALL=C"];let mut envp:Vec<_>=environment.iter().map(|value|value.as_ptr()as *mut libc::c_char).collect();envp.push(null_mut());cancelled(cancel)?;
    let mut pidfd=-1;let result=unsafe{(functions.spawn)(&mut pidfd,c"/proc/self/fd/7".as_ptr(),&actions.0,&attributes.0,argv.as_ptr(),envp.as_ptr())};if result!=0{return Err(unavailable());}let pidfd=owned(pidfd)?;
    // These temporary duplicates must not keep the child's gate/media/control
    // endpoints open after spawn. All unselected inherited descriptors closed
    // in the libc file actions before the reviewed static bootstrap executed.
    drop(sources);drop(cwd);drop(actions);drop(attributes);drop(child_input);drop(child_output);drop(gate_child);drop(null_file);
    let resources=Resources{group:spec.resource_group,_retained:Retained::Bootstrap{_root:spec.root_directory,_lease:spec.work_lease,_files:spec.files,_executable:spec.executable,_plan:plan}};
    let process=observed(pidfd,resources,gate_parent,cancel)?;
    // The media handles supplied by the peer belong only to the spawned actor
    // now. The remaining spec fields drop here; Resources retain admitted inputs.
    Ok(BootstrapLaunched{process,input,output})
}

fn pipe_direction(file:&File,mode:i32)->Result<(u64,u64)>{let value=file.metadata().map_err(|_|unavailable())?;let flags=unsafe{libc::fcntl(file.as_raw_fd(),libc::F_GETFL)};if !value.file_type().is_fifo()||flags<0||flags&libc::O_ACCMODE!=mode||flags&libc::O_NONBLOCK!=0{return Err(unavailable());}Ok((value.dev(),value.ino()))}
fn spawn_presentation_owned(spec:PresentationSpawn,cancel:&AtomicBool)->Result<LinuxPresentationHost>{
    cancelled(cancel)?;if !spec.resource_group.is_presentation()||spec.resource_group.session()!=spec.session||!spec.resource_group.memory_matches(spec.memory_bytes)||!(1..=4096).contains(&spec.maximum_width)||!(1..=4096).contains(&spec.maximum_height)||spec.volume_percent>100||u64::from(spec.maximum_width)*u64::from(spec.maximum_height)*16+128*1024*1024>spec.memory_bytes{return Err(unavailable());}
    crate::linux_plan::session(&spec.session)?;if pipe_direction(&spec.input,libc::O_RDONLY)?==pipe_direction(&spec.output,libc::O_WRONLY)?{return Err(unavailable());}let functions=functions()?;
    let directory=spec.directory.metadata().map_err(|_|unavailable())?;let directory_flags=unsafe{libc::fcntl(spec.directory.as_raw_fd(),libc::F_GETFL)};
    if !directory.is_dir()||directory.mode()&0o022!=0||directory.uid()!=0&&directory.uid()!=unsafe{libc::geteuid()}||directory_flags<0||directory_flags&libc::O_PATH!=libc::O_PATH{return Err(unavailable());}
    if !spec.temporary_path.is_absolute()||std::fs::symlink_metadata(&spec.temporary_path).map_err(|_|unavailable())?.file_type().is_symlink(){return Err(unavailable());}let temporary=spec.temporary_path.canonicalize().map_err(|_|unavailable())?;let selected=spec.temporary_directory.metadata().map_err(|_|unavailable())?;let actual=std::fs::metadata(&temporary).map_err(|_|unavailable())?;
    if !selected.is_dir()||selected.uid()!=unsafe{libc::geteuid()}||selected.mode()&0o7777!=0o700||actual.dev()!=selected.dev()||actual.ino()!=selected.ino()||temporary.as_os_str().as_bytes().len()>4096{return Err(unavailable());}
    let lease=spec.work_lease.metadata().map_err(|_|unavailable())?;if !lease.is_file()||lease.uid()!=unsafe{libc::geteuid()}||lease.nlink()!=1||lease.mode()&0o077!=0{return Err(unavailable());}spec.work_lease.try_lock().map_err(|_|unavailable())?;
    spec.resource_group.configure_presentation(spec.memory_bytes,spec.cpu_cores)?;
    let(gate_child,gate_parent)=gate()?;let(owner_child,owner_parent)=gate()?;let null_file=OpenOptions::new().write(true).custom_flags(libc::O_CLOEXEC|libc::O_NOFOLLOW).open("/dev/null").map_err(|_|unavailable())?;let cwd=duplicated(spec.directory.as_raw_fd())?;
    let originals=[spec.input.as_raw_fd(),spec.output.as_raw_fd(),null_file.as_raw_fd(),gate_child.as_raw_fd(),spec.executable.descriptor(),owner_child.as_raw_fd()];let mut sources=Vec::with_capacity(originals.len());for descriptor in originals{sources.push(duplicated(descriptor)?);}
    let mut actions=Actions::new()?;for(index,source)in sources.iter().enumerate(){actions.duplicate(source.as_raw_fd(),index as RawFd)?;}
    if unsafe{(functions.fchdir)(&mut actions.0,cwd.as_raw_fd())}!=0||unsafe{(functions.close_from)(&mut actions.0,6)}!=0{return Err(unavailable());}let mut attributes=Attributes::new()?;attributes.cgroup(&functions,spec.resource_group.descriptor())?;
    let values=["omniplay-presentation-host".to_owned(),"--stdio-presentation-v2-gated".to_owned(),spec.session.clone(),spec.maximum_width.to_string(),spec.maximum_height.to_string(),spec.volume_percent.to_string(),if spec.haptics{"haptics"}else{"no-haptics"}.into(),std::process::id().to_string()];let arguments:Vec<_>=values.into_iter().map(|value|CString::new(value).map_err(|_|unavailable())).collect::<Result<_>>()?;let mut argv:Vec<_>=arguments.iter().map(|value|value.as_ptr()as *mut libc::c_char).collect();argv.push(null_mut());
    let mut environment=spec.desktop.entries().to_vec();for key in["TMPDIR","TMP","TEMP"]{let mut raw=key.as_bytes().to_vec();raw.push(b'=');raw.extend_from_slice(temporary.as_os_str().as_bytes());environment.push(CString::new(raw).map_err(|_|unavailable())?);}let mut envp:Vec<_>=environment.iter().map(|value|value.as_ptr()as *mut libc::c_char).collect();envp.push(null_mut());cancelled(cancel)?;
    let mut pidfd=-1;let result=unsafe{(functions.spawn)(&mut pidfd,c"/proc/self/fd/4".as_ptr(),&actions.0,&attributes.0,argv.as_ptr(),envp.as_ptr())};if result!=0{return Err(unavailable());}let pidfd=owned(pidfd)?;drop(sources);drop(cwd);drop(actions);drop(attributes);drop(gate_child);drop(owner_child);drop(null_file);
    let resources=Resources{group:spec.resource_group,_retained:Retained::Presentation{_directory:spec.directory,_temporary:spec.temporary_directory,_lease:spec.work_lease,_executable:spec.executable}};let process=observed(pidfd,resources,gate_parent,cancel)?;LinuxPresentationHost::new(process,owner_parent,&spec.session)
}

enum LaunchJob{Bootstrap{spec:BootstrapSpawn,budget:SealedBudget,cancel:Arc<AtomicBool>,reply:SyncSender<Result<BootstrapLaunched>>},Presentation{spec:PresentationSpawn,cancel:Arc<AtomicBool>,reply:SyncSender<Result<LinuxPresentationHost>>}}
static LAUNCH_LANE:OnceLock<std::result::Result<SyncSender<LaunchJob>,()>>=OnceLock::new();
fn launch_lane()->Result<&'static SyncSender<LaunchJob>>{
    LAUNCH_LANE.get_or_init(||{
        let(sender,receiver)=mpsc::sync_channel::<LaunchJob>(8);
        let(ready_send,ready_receive)=mpsc::sync_channel(1);
        thread::Builder::new().name("omniplay-linux-launch".into()).spawn(move||{
            // Set only the permanent spawning task's inherited restriction.
            // No setuid/file-capability gain may precede bootstrap main().
            let ready=unsafe{libc::prctl(libc::PR_SET_NO_NEW_PRIVS,1 as libc::c_ulong,0 as libc::c_ulong,0 as libc::c_ulong,0 as libc::c_ulong)}==0&&unsafe{libc::prctl(libc::PR_GET_NO_NEW_PRIVS,0 as libc::c_ulong,0 as libc::c_ulong,0 as libc::c_ulong,0 as libc::c_ulong)}==1;
            if ready_send.send(ready).is_err()||!ready{return;}
            while let Ok(job)=receiver.recv(){
                // PR_SET_PDEATHSIG follows the creating thread. Every child is
                // created by this process-lifetime lane, not a short-lived
                // preparation worker which may end after returning its adapter.
                match job{
                    LaunchJob::Bootstrap{spec,budget,cancel,reply}=>{let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||spawn_owned(spec,&budget,&cancel))).unwrap_or_else(|_|Err(unavailable()));let _=reply.send(result);},
                    LaunchJob::Presentation{spec,cancel,reply}=>{let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||spawn_presentation_owned(spec,&cancel))).unwrap_or_else(|_|Err(unavailable()));let _=reply.send(result);}
                }
            }
        }).map_err(|_|())?;if ready_receive.recv_timeout(Duration::from_secs(2)).map_err(|_|())?!=true{return Err(());}Ok(sender)
    }).as_ref().map_err(|_|unavailable())
}

/// Sources are owned by the returned process/reaper. The exact work lease,
/// signed package/root/cgroup provisioning and durable callback are caller duties.
/// Cancellation joins the queued operation; it never abandons an unobserved job.
pub fn spawn_bootstrap(spec:BootstrapSpawn,budget:&SealedBudget,cancel:Arc<AtomicBool>,before_release:impl FnOnce(&LinuxProcessIdentity)->Result<()>)->Result<BootstrapLaunched>{
    cancelled(&cancel)?;let(reply,receive)=mpsc::sync_channel(1);let job=LaunchJob::Bootstrap{spec,budget:budget.clone(),cancel:cancel.clone(),reply};
    launch_lane()?.try_send(job).map_err(|_|PlatformError::new(ErrorCode::Busy,"The private Linux launch lane is busy or unavailable"))?;
    let launched=loop{match receive.recv_timeout(Duration::from_millis(100)){Ok(result)=>break result?,Err(mpsc::RecvTimeoutError::Timeout)=>continue,Err(mpsc::RecvTimeoutError::Disconnected)=>return Err(unavailable())}};
    cancelled(&cancel)?;before_release(launched.process.identity()?)?;cancelled(&cancel)?;
    authorize(&launched.process)?;
    Ok(launched)
}
fn authorize(process:&LinuxChild)->Result<()>{let authorization=process.state.start_gate.lock().map_err(|_|unavailable())?.take().ok_or_else(unavailable)?;let grant=[1u8];let sent=unsafe{libc::send(authorization.as_raw_fd(),grant.as_ptr().cast(),grant.len(),libc::MSG_NOSIGNAL|libc::MSG_DONTWAIT)};drop(authorization);if sent!=1{return Err(unavailable());}Ok(())}
pub fn spawn_presentation(spec:PresentationSpawn,cancel:Arc<AtomicBool>,before_release:impl FnOnce(&LinuxProcessIdentity)->Result<()>)->Result<LinuxPresentationHost>{
    cancelled(&cancel)?;let(reply,receive)=mpsc::sync_channel(1);launch_lane()?.try_send(LaunchJob::Presentation{spec,cancel:cancel.clone(),reply}).map_err(|_|PlatformError::new(ErrorCode::Busy,"The private Linux launch lane is busy or unavailable"))?;
    let host=loop{match receive.recv_timeout(Duration::from_millis(100)){Ok(result)=>break result?,Err(mpsc::RecvTimeoutError::Timeout)=>continue,Err(mpsc::RecvTimeoutError::Disconnected)=>return Err(unavailable())}};cancelled(&cancel)?;before_release(host.identity()?)?;cancelled(&cancel)?;authorize(&host.process)?;Ok(host)
}
pub struct PresentationChannels{pub actor_input:File,pub actor_output:File,pub helper_input:File,pub helper_output:File}
pub fn presentation_channels()->Result<PresentationChannels>{let(helper_input,actor_output)=private_pipe()?;let(actor_input,helper_output)=private_pipe()?;Ok(PresentationChannels{actor_input,actor_output,helper_input,helper_output})}
pub struct PresentedBootstrap{pub actor:BootstrapLaunched,pub presentation:LinuxPresentationHost}
pub fn spawn_presented(actor:BootstrapSpawn,helper:PresentationSpawn,budget:&SealedBudget,cancel:Arc<AtomicBool>,before_actor:impl FnOnce(&LinuxProcessIdentity)->Result<()>,before_helper:impl FnOnce(&LinuxProcessIdentity)->Result<()>)->Result<PresentedBootstrap>{
    if actor.session!=helper.session||actor.resource_group.identity()==helper.resource_group.identity()||pipe_direction(&actor.media_input,libc::O_RDONLY)?==pipe_direction(&actor.media_output,libc::O_WRONLY)?||pipe_direction(&actor.media_input,libc::O_RDONLY)?!=pipe_direction(&helper.output,libc::O_WRONLY)?||pipe_direction(&actor.media_output,libc::O_WRONLY)?!=pipe_direction(&helper.input,libc::O_RDONLY)?{return Err(unavailable());}
    let presentation=spawn_presentation(helper,cancel.clone(),before_helper)?;let actor=spawn_bootstrap(actor,budget,cancel,before_actor)?;Ok(PresentedBootstrap{actor,presentation})
}

/// Read-only abandoned-work check. Disk records never authorize signalling.
pub fn confirmed_process_ended(expected:&LinuxProcessIdentity)->Result<()>{
    if !(2..=i32::MAX as u32).contains(&expected.process_id)||expected.start_ticks==0{return Err(unavailable());}crate::linux_plan::session(&expected.boot_id)?;
    // A failed boot-id read is uncertainty; a proven different boot retires the
    // old process identity without interpreting its numeric PID.
    if boot_id()?!=expected.boot_id{return Ok(());}
    let raw=unsafe{libc::syscall(libc::SYS_pidfd_open,expected.process_id,0u32)};if raw<0{return if std::io::Error::last_os_error().raw_os_error()==Some(libc::ESRCH){Ok(())}else{Err(unavailable())};}let pidfd=owned(raw as RawFd)?;
    if dead(pidfd.as_raw_fd(),0)?{return Ok(());}let current=start_ticks(expected.process_id)?;if dead(pidfd.as_raw_fd(),0)?||current!=expected.start_ticks{return Ok(());}Err(unavailable())
}
