//! Message-authenticated XPC transport. All unsafe operations stay in this
//! module; callers receive owned Files and never raw descriptors or PID rights.
use crate::unavailable;
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use serde_json::{json,Value};
use std::{ffi::{c_char,c_int,CString},fs::File,os::fd::{AsRawFd,FromRawFd},ptr::NonNull,sync::{Arc,Mutex,atomic::{AtomicU64,Ordering}},time::Duration};

#[repr(C)]struct Client{_private:[u8;0]}
#[repr(C)]struct Reply{json:*mut u8,bytes:usize,input:c_int,output:c_int}
unsafe extern "C"{
    fn opmac_open(service:*const c_char,requirement:*const c_char,executable:*const c_char)->*mut Client;
    fn opmac_call(client:*mut Client,json:*const u8,bytes:usize,descriptors:*const c_int,count:usize,timeout_ms:u32,output:*mut Reply)->c_int;
    fn opmac_reply_free(reply:*mut Reply);
    fn opmac_close(client:*mut Client);
}
struct Handle(NonNull<Client>);
// The C handle is only accessed under Connection's non-reentrant mutex. Native
// completion blocks retain independent state; close does not free their memory.
unsafe impl Send for Handle{}
impl Drop for Handle{fn drop(&mut self){unsafe{opmac_close(self.0.as_ptr())}}}
struct Response{value:Value,input:Option<File>,output:Option<File>}
#[derive(Clone)]pub struct MacConnection{handle:Arc<Mutex<Handle>>,profile:String,sequence:Arc<AtomicU64>,execution_available:bool,_profile_lease:Arc<File>}
pub struct StagedAsset{pub name:String,pub sha256:String,pub bytes:u64,pub source:File}
pub struct MacLaunch<'a>{pub session:&'a str,pub runtime:&'a str,pub maximum_width:u32,pub maximum_height:u32,pub volume:u16,pub haptics:bool}
pub struct MacLaunched{pub process:MacPair,pub input:File,pub output:File}
pub struct MacPair{connection:MacConnection,session:String}
fn protocol()->PlatformError{unavailable()}
fn uuid(value:&str)->bool{
    value.len()==36&&value.bytes().enumerate().all(|(i,b)|if matches!(i,8|13|18|23){b==b'-'}else{b.is_ascii_digit()||(b'a'..=b'f').contains(&b)})&&value.bytes().any(|b|matches!(b,b'1'..=b'9'|b'a'..=b'f'))
}
fn identifier(value:&str)->bool{!value.is_empty()&&value.len()<=160&&value.split('.').count()>=3&&value.split('.').all(|part|!part.is_empty()&&part.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'))}
fn runtime_id_valid(value:&str)->bool{!value.is_empty()&&value.len()<=64&&value.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'_'|b'-'))}
fn keys(value:&Value,fields:&[&str])->bool{value.as_object().is_some_and(|object|object.len()==fields.len()&&fields.iter().all(|key|object.contains_key(*key)))}
impl MacConnection{
    /// Identity must come from the authenticated release catalog. It is never
    /// supplied by a renderer, environment variable or on-disk process receipt.
    /// Control/recovery access does not grant execution. launch() and the
    /// product factory independently require the owner's execution capability.
    pub fn connect_control(service:&str,team:&str,profile:&str,executable:&std::path::Path,profile_root:&std::path::Path)->Result<Self>{
        if !identifier(service)||team.len()!=10||!team.bytes().all(|b|b.is_ascii_uppercase()||b.is_ascii_digit())||!uuid(profile){return Err(protocol());}
        let profile_lease=Arc::new(profile_owner_lease(profile_root)?);
        let service=CString::new(service).map_err(|_|protocol())?;
        let requirement=CString::new(format!("anchor apple generic and identifier \"{}\" and certificate leaf[subject.OU] = \"{team}\"",service.to_str().map_err(|_|protocol())?)).map_err(|_|protocol())?;
        let executable=executable.canonicalize().map_err(|_|protocol())?;
        let executable=CString::new(executable.to_str().ok_or_else(protocol)?).map_err(|_|protocol())?;
        let handle=NonNull::new(unsafe{opmac_open(service.as_ptr(),requirement.as_ptr(),executable.as_ptr())}).ok_or_else(protocol)?;
        let mut connection=Self{handle:Arc::new(Mutex::new(Handle(handle))),profile:profile.to_owned(),sequence:Arc::new(AtomicU64::new(1)),execution_available:false,_profile_lease:profile_lease};
        let reply=connection.request("capabilities",json!({}),&[],Duration::from_secs(5))?;
        if !keys(&reply.value,&["protocol","request_id","status","profile_binding","session_stage","session_discard","session_launch","session_recover_previous_boot","maximum_control_bytes"])||reply.input.is_some()||reply.value["profile_binding"]!=true||reply.value["session_stage"]!=true||reply.value["session_discard"]!=true||reply.value["session_recover_previous_boot"]!=true||reply.value["maximum_control_bytes"]!=65536{return Err(protocol());}
        connection.execution_available=reply.value["session_launch"].as_bool().ok_or_else(protocol)?;
        connection.empty("bind_profile",json!({"profile_id":profile}),Duration::from_secs(5))?;
        Ok(connection)
    }
    pub fn execution_available(&self)->bool{self.execution_available}
    pub fn profile(&self)->&str{&self.profile}
    pub fn confirmed_session_ended(&self,session:&str)->Result<bool>{
        if !uuid(session){return Err(protocol());}
        let response=self.request("session_status",json!({"profile_id":self.profile,"session_id":session}),&[],Duration::from_secs(2))?;
        if response.input.is_some()||!keys(&response.value,&["protocol","request_id","status","active"]){return Err(protocol());}Ok(!response.value["active"].as_bool().ok_or_else(protocol)?)
    }
    fn request(&self,operation:&str,mut parameters:Value,files:&[&File],timeout:Duration)->Result<Response>{
        let number=self.sequence.fetch_update(Ordering::AcqRel,Ordering::Acquire,|n|n.checked_add(1)).map_err(|_|protocol())?;
        // A monotonically unique non-nil request UUID within this connection.
        let id=format!("00000000-0000-4000-8000-{number:012x}");
        if number>0xffffffffffff{return Err(protocol());}
        let object=parameters.as_object_mut().ok_or_else(protocol)?;
        object.insert("protocol".into(),json!(1));object.insert("operation".into(),json!(operation));object.insert("request_id".into(),json!(id));
        let raw=serde_json::to_vec(&parameters).map_err(|_|protocol())?;
        if raw.len()>65536||files.len()>144||timeout.is_zero()||timeout>Duration::from_secs(300){return Err(protocol());}
        let descriptors:Vec<c_int>=files.iter().map(|file|file.as_raw_fd()).collect();
        let handle=self.handle.try_lock().map_err(|_|PlatformError::new(ErrorCode::ResourceLimit,"The macOS owner connection is busy"))?;
        let mut reply=Reply{json:std::ptr::null_mut(),bytes:0,input:-1,output:-1};
        let status=unsafe{opmac_call(handle.0.as_ptr(),raw.as_ptr(),raw.len(),descriptors.as_ptr(),descriptors.len(),timeout.as_millis() as u32,&mut reply)};
        // Adopt descriptors even on malformed/error replies, so every path
        // closes them exactly once. The native bridge owns bytes until free.
        let input=if reply.input>=0{let fd=reply.input;reply.input=-1;Some(unsafe{File::from_raw_fd(fd)})}else{None};
        let output=if reply.output>=0{let fd=reply.output;reply.output=-1;Some(unsafe{File::from_raw_fd(fd)})}else{None};
        let decoded=if status==0&&!reply.json.is_null()&&(1..=65536).contains(&reply.bytes){serde_json::from_slice::<Value>(unsafe{std::slice::from_raw_parts(reply.json,reply.bytes)}).ok()}else{None};
        unsafe{opmac_reply_free(&mut reply)};
        let value=decoded.ok_or_else(protocol)?;
        if value["protocol"]!=1||value["request_id"]!=id||value["status"]!="ok"||input.is_some()!=output.is_some(){return Err(protocol());}
        Ok(Response{value,input,output})
    }
    fn empty(&self,operation:&str,parameters:Value,timeout:Duration)->Result<()>{let response=self.request(operation,parameters,&[],timeout)?;if response.input.is_some()||!keys(&response.value,&["protocol","request_id","status"]){return Err(protocol());}Ok(())}
    pub fn stage(&self,session:&str,runtime:&str,assets:&[StagedAsset])->Result<()>{
        if !uuid(session)||!runtime_id_valid(runtime)||assets.is_empty()||assets.len()>144{return Err(protocol());}
        let mut names=std::collections::BTreeSet::new();let mut total=0u64;
        for asset in assets{
            if asset.name.is_empty()||asset.name.len()>128||!asset.name.as_bytes()[0].is_ascii_alphanumeric()||asset.name.ends_with('.')||asset.name=="receipt.json"||!asset.name.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'_'|b'-'|b'.'|b'+'))||!names.insert(&asset.name)||asset.sha256.len()!=64||!asset.sha256.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))||asset.bytes==0||asset.bytes>4*1024*1024*1024{return Err(protocol());}
            total=total.checked_add(asset.bytes).ok_or_else(protocol)?;
        }
        if total>8*1024*1024*1024{return Err(protocol());}
        let list:Vec<Value>=assets.iter().map(|a|json!({"name":a.name,"bytes":a.bytes,"sha256":a.sha256})).collect();
        let files:Vec<&File>=assets.iter().map(|a|&a.source).collect();
        let response=self.request("stage_session",json!({"profile_id":self.profile,"session_id":session,"runtime_id":runtime,"assets":list}),&files,Duration::from_secs(300))?;
        if response.input.is_some()||!keys(&response.value,&["protocol","request_id","status","staged"])||response.value["staged"]!=true{return Err(protocol());}Ok(())
    }
    pub fn discard(&self,session:&str)->Result<()>{if !uuid(session){return Err(protocol());}self.empty("discard_session",json!({"profile_id":self.profile,"session_id":session}),Duration::from_secs(5))}
    /// prior_boot must come from the broker-private durable journal, never from
    /// a child/container receipt or a renderer. The owner checks its own kernel
    /// boot identity too and refuses current-boot/unknown provenance recovery.
    pub fn recover_previous_boot(&self,session:&str,runtime:&str,prior_boot:&str)->Result<()>{
        if !uuid(session)||!runtime_id_valid(runtime)||!uuid(prior_boot)||kernel_boot_session()?==prior_boot{return Err(protocol());}
        self.empty("recover_session",json!({"profile_id":self.profile,"session_id":session,"runtime_id":runtime,"prior_boot":prior_boot}),Duration::from_secs(5))
    }
    pub fn launch(&self,parameters:MacLaunch<'_>)->Result<MacLaunched>{
        if !self.execution_available{return Err(protocol());}
        if !uuid(parameters.session)||!runtime_id_valid(parameters.runtime)||!(1..=4096).contains(&parameters.maximum_width)||!(1..=4096).contains(&parameters.maximum_height)||parameters.volume>100{return Err(protocol());}
        let process=MacPair{connection:self.clone(),session:parameters.session.to_owned()};
        let response=self.request("launch_session",json!({"profile_id":self.profile,"session_id":parameters.session,"runtime_id":parameters.runtime,"maximum_width":parameters.maximum_width,"maximum_height":parameters.maximum_height,"volume":parameters.volume,"haptics":parameters.haptics}),&[],Duration::from_secs(30))?;
        if !keys(&response.value,&["protocol","request_id","status"]){process.terminate();return Err(protocol());}
        match(response.input,response.output){(Some(input),Some(output))=>Ok(MacLaunched{process,input,output}),_=>{process.terminate();Err(protocol())}}
    }
}
impl MacPair{
    pub fn confirmed_ended(&self)->Result<bool>{self.connection.confirmed_session_ended(&self.session)}
    pub fn has_exited(&self)->bool{self.confirmed_ended().unwrap_or(false)}
    pub fn activate(&self)->Result<()>{self.connection.empty("activate_session",json!({"profile_id":self.connection.profile,"session_id":self.session}),Duration::from_secs(2))}
    pub fn terminate(&self){let _=self.connection.request("stop_session",json!({"profile_id":self.connection.profile,"session_id":self.session}),&[],Duration::from_secs(2));}
}
impl Drop for MacPair{fn drop(&mut self){self.terminate();}}

pub fn kernel_boot_session()->Result<String>{
    let mut raw=[0u8;37];let mut bytes=raw.len();
    if unsafe{libc::sysctlbyname(c"kern.bootsessionuuid".as_ptr(),raw.as_mut_ptr().cast(),&mut bytes,std::ptr::null_mut(),0)}!=0||bytes!=37||raw[36]!=0{return Err(protocol());}
    raw[..36].make_ascii_lowercase();let value=std::str::from_utf8(&raw[..36]).map_err(|_|protocol())?;
    if !uuid(value){return Err(protocol());}Ok(value.to_owned())
}
fn profile_owner_lease(profile:&std::path::Path)->Result<File>{
    use std::os::unix::fs::{MetadataExt,OpenOptionsExt};
    if !profile.is_absolute(){return Err(protocol());}
    let shape=std::fs::symlink_metadata(profile).map_err(|_|protocol())?;
    if !shape.is_dir()||shape.uid()!=unsafe{libc::geteuid()}||shape.mode()&0o022!=0{return Err(protocol());}
    let file=std::fs::OpenOptions::new().read(true).write(true).create(true).mode(0o600).custom_flags(libc::O_CLOEXEC|libc::O_NOFOLLOW).open(profile.join("mac-native-owner.lease")).map_err(|_|protocol())?;
    let shape=file.metadata().map_err(|_|protocol())?;
    if !shape.is_file()||shape.nlink()!=1||shape.len()!=0||shape.mode()&0o7777!=0o600||shape.uid()!=unsafe{libc::geteuid()}||unsafe{libc::flock(file.as_raw_fd(),libc::LOCK_EX|libc::LOCK_NB)}!=0{return Err(protocol());}Ok(file)
}

/// Open one hash-named profile asset through held directory descriptors. No
/// renderer path, symlink traversal or ambient owner-container access is used.
pub fn profile_asset(profile:&std::path::Path,hash:&str)->Result<File>{
    use std::os::unix::{ffi::OsStrExt,fs::MetadataExt};
    if !profile.is_absolute()||hash.len()!=64||!hash.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b)){return Err(protocol());}
    let path=CString::new(profile.as_os_str().as_bytes()).map_err(|_|protocol())?;
    let root=unsafe{libc::open(path.as_ptr(),libc::O_RDONLY|libc::O_DIRECTORY|libc::O_CLOEXEC|libc::O_NOFOLLOW)};
    if root<0{return Err(protocol());}let root=unsafe{File::from_raw_fd(root)};
    let metadata=root.metadata().map_err(|_|protocol())?;if !metadata.is_dir()||metadata.uid()!=unsafe{libc::geteuid()}||metadata.mode()&0o022!=0{return Err(protocol());}
    let directory=unsafe{libc::openat(root.as_raw_fd(),c"assets".as_ptr(),libc::O_RDONLY|libc::O_DIRECTORY|libc::O_CLOEXEC|libc::O_NOFOLLOW)};
    if directory<0{return Err(protocol());}let directory=unsafe{File::from_raw_fd(directory)};
    let name=CString::new(hash).map_err(|_|protocol())?;
    let fd=unsafe{libc::openat(directory.as_raw_fd(),name.as_ptr(),libc::O_RDONLY|libc::O_CLOEXEC|libc::O_NOFOLLOW)};
    if fd<0{return Err(protocol());}let file=unsafe{File::from_raw_fd(fd)};
    let metadata=file.metadata().map_err(|_|protocol())?;if !metadata.is_file()||metadata.nlink()!=1||metadata.uid()!=unsafe{libc::geteuid()}{return Err(protocol());}Ok(file)
}
