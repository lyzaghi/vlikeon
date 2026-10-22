//! Fresh AppContainer filesystem storage is readonly to the low-integrity
//! actor. This is a mandatory-integrity boundary, not a directory disk quota.
use super::{handle,unavailable,wide};
use omniplay_contracts::Result;
use std::{ffi::{OsStr,OsString,c_void},fs::{self,File,OpenOptions},os::windows::{ffi::OsStringExt,fs::{MetadataExt,OpenOptionsExt},io::AsRawHandle},path::{Component,Path,PathBuf,Prefix},ptr::{null,null_mut}};
use windows_sys::Win32::{Foundation::{ERROR_SUCCESS,LocalFree},Security::{PSID,LABEL_SECURITY_INFORMATION,Authorization::{ConvertSidToStringSidW,ConvertStringSecurityDescriptorToSecurityDescriptorW,SetSecurityInfo,SE_FILE_OBJECT},GetSecurityDescriptorSacl},Security::Isolation::GetAppContainerFolderPath,Storage::FileSystem::{FILE_FLAG_BACKUP_SEMANTICS,FILE_FLAG_OPEN_REPARSE_POINT,FILE_READ_ATTRIBUTES},System::Com::CoTaskMemFree};

const READ_CONTROL_ACCESS:u32=0x0002_0000;
const WRITE_OWNER_ACCESS:u32=0x0008_0000;
const REPARSE_ATTRIBUTE:u32=0x400;
const MAX_ENTRIES:usize=512;
const MAX_DEPTH:u8=16;

struct LocalAllocation(*mut c_void);
impl Drop for LocalAllocation{fn drop(&mut self){if !self.0.is_null(){unsafe{LocalFree(self.0);}}}}
struct TaskAllocation(*mut c_void);
impl Drop for TaskAllocation{fn drop(&mut self){if !self.0.is_null(){unsafe{CoTaskMemFree(self.0);}}}}

fn folder(sid:PSID)->Result<PathBuf>{
    let mut sid_string=null_mut();
    if unsafe{ConvertSidToStringSidW(sid,&mut sid_string)}==0{return Err(unavailable());}
    let _sid_string=LocalAllocation(sid_string.cast());let mut raw=null_mut();
    let result=unsafe{GetAppContainerFolderPath(sid_string,&mut raw)};let _path=TaskAllocation(raw.cast());
    if result<0||raw.is_null(){return Err(unavailable());}
    let mut units=Vec::new();
    for index in 0..32768{
        let unit=unsafe{*raw.add(index)};
        if unit==0{
            if units.is_empty(){return Err(unavailable());}
            return Ok(PathBuf::from(OsString::from_wide(&units)));
        }
        units.push(unit);
    }
    Err(unavailable())
}

fn ordinary_directory(path:&Path)->Result<()>{
    let metadata=fs::symlink_metadata(path).map_err(|_|unavailable())?;
    if !metadata.is_dir()||metadata.file_type().is_symlink()||metadata.file_attributes()&REPARSE_ATTRIBUTE!=0{return Err(unavailable());}Ok(())
}

/// The OS path must name only this freshly-created profile's AC directory.
/// Unknown layouts, redirected/reparse ancestors and network profiles fail
/// closed. Never guess a package path or edit the user's AppData/Packages root.
fn admitted_root(folder:&Path,name:&str)->Result<PathBuf>{
    if !super::session_name(name)||!folder.is_absolute(){return Err(unavailable());}
    let mut components=folder.components();
    if !matches!(components.next(),Some(Component::Prefix(value))if matches!(value.kind(),Prefix::Disk(_)|Prefix::VerbatimDisk(_)))||
        components.any(|part|matches!(part,Component::CurDir|Component::ParentDir)){return Err(unavailable());}
    if !folder.file_name().and_then(OsStr::to_str).is_some_and(|part|part.eq_ignore_ascii_case("AC")){return Err(unavailable());}
    let root=folder.parent().ok_or_else(unavailable)?;
    if !root.file_name().and_then(OsStr::to_str).is_some_and(|part|part.eq_ignore_ascii_case(name))||
        !root.parent().and_then(Path::file_name).and_then(OsStr::to_str).is_some_and(|part|part.eq_ignore_ascii_case("Packages")){return Err(unavailable());}
    for ancestor in folder.ancestors(){ordinary_directory(ancestor)?;}
    Ok(root.to_path_buf())
}

pub(super) struct MandatoryLabel{_allocation:LocalAllocation,sacl:*mut windows_sys::Win32::Security::ACL}
impl MandatoryLabel{
    fn new(inherit:bool)->Result<Self>{
        Self::from_sddl(if inherit{"S:(ML;OICI;NW;;;ME)"}else{"S:(ML;;NW;;;ME)"})
    }
    pub(super) fn registry()->Result<Self>{Self::from_sddl("S:(ML;CI;NW;;;ME)")}
    fn from_sddl(value:&str)->Result<Self>{
        let text=wide(OsStr::new(value))?;
        let mut descriptor=null_mut();
        if unsafe{ConvertStringSecurityDescriptorToSecurityDescriptorW(text.as_ptr(),1,&mut descriptor,null_mut())}==0{return Err(unavailable());}
        let allocation=LocalAllocation(descriptor);let mut present=0;let mut defaulted=0;let mut sacl=null_mut();
        if unsafe{GetSecurityDescriptorSacl(descriptor,&mut present,&mut sacl,&mut defaulted)}==0||present==0||sacl.is_null(){return Err(unavailable());}
        Ok(Self{_allocation:allocation,sacl})
    }
    fn apply(&self,file:&File)->Result<()>{
        self.apply_object(file.as_raw_handle(),SE_FILE_OBJECT)
    }
    pub(super) fn apply_object(&self,object:windows_sys::Win32::Foundation::HANDLE,kind:windows_sys::Win32::Security::Authorization::SE_OBJECT_TYPE)->Result<()>{
        if unsafe{SetSecurityInfo(object,kind,LABEL_SECURITY_INFORMATION,null_mut(),null_mut(),null(),self.sacl)}!=ERROR_SUCCESS{return Err(unavailable());}Ok(())
    }
}

fn ordinary_handle(path:&Path,directory:bool)->Result<File>{
    let file=OpenOptions::new().access_mode(READ_CONTROL_ACCESS|WRITE_OWNER_ACCESS|FILE_READ_ATTRIBUTES)
        .share_mode(3).custom_flags(FILE_FLAG_BACKUP_SEMANTICS|FILE_FLAG_OPEN_REPARSE_POINT).open(path).map_err(|_|unavailable())?;
    let metadata=file.metadata().map_err(|_|unavailable())?;
    if metadata.file_attributes()&REPARSE_ATTRIBUTE!=0||directory!=metadata.is_dir()||!directory&&!metadata.is_file(){return Err(unavailable());}Ok(file)
}

pub(super) fn seal_default_files(sid:PSID,name:&str)->Result<()>{
    let root=admitted_root(&folder(sid)?,name)?;
    let directory_label=MandatoryLabel::new(true)?;let file_label=MandatoryLabel::new(false)?;
    let root_handle=ordinary_handle(&root,true)?;
    let mut pending=vec![(root,0u8)];let mut objects=vec![(root_handle,true)];let mut seen=1usize;
    while let Some((path,depth))=pending.pop(){
        for entry in fs::read_dir(&path).map_err(|_|unavailable())?{
            seen+=1;if seen>MAX_ENTRIES{return Err(unavailable());}
            let path=entry.map_err(|_|unavailable())?.path();let metadata=fs::symlink_metadata(&path).map_err(|_|unavailable())?;
            if metadata.file_type().is_symlink()||metadata.file_attributes()&REPARSE_ATTRIBUTE!=0{return Err(unavailable());}
            if metadata.is_dir(){
                if depth>=MAX_DEPTH{return Err(unavailable());}
                let file=ordinary_handle(&path,true)?;objects.push((file,true));pending.push((path,depth+1));
            }else if metadata.is_file(){let file=ordinary_handle(&path,false)?;objects.push((file,false));}
            else{return Err(unavailable());}
        }
    }
    // Validate and pin the entire small fresh tree before any inheritable
    // security update. Install child labels first, and the package root last.
    // No untrusted actor exists yet; a partial update never permits launch.
    for(file,directory)in objects.iter().rev(){if *directory{directory_label.apply(file)?;}else{file_label.apply(file)?;}}
    Ok(())
}

/// The filesystem policy depends on a real low-integrity AppContainer token,
/// not merely on a successful CreateProcess call with requested attributes.
pub(super) fn verify_actor_token(process:&std::os::windows::io::OwnedHandle,expected_sid:PSID)->Result<()>{
    use windows_sys::Win32::Security::*;
    use windows_sys::Win32::System::Threading::OpenProcessToken;
    let mut raw=null_mut();
    if unsafe{OpenProcessToken(process.as_raw_handle(),TOKEN_QUERY,&mut raw)}==0{return Err(unavailable());}
    verify_token(&handle(raw)?,expected_sid)
}

pub(super) fn verify_token(token:&std::os::windows::io::OwnedHandle,expected_sid:windows_sys::Win32::Security::PSID)->Result<()>{
    use windows_sys::Win32::Security::*;
    let mut app_container=0u32;let mut bytes=0u32;
    if unsafe{GetTokenInformation(token.as_raw_handle(),TokenIsAppContainer,(&mut app_container as*mut u32).cast(),4,&mut bytes)}==0||bytes!=4||app_container!=1{return Err(unavailable());}
    let identity_buffer=token_information(token,TokenAppContainerSid)?;
    if identity_buffer.len()*std::mem::size_of::<usize>()<std::mem::size_of::<TOKEN_APPCONTAINER_INFORMATION>(){return Err(unavailable());}
    let identity=unsafe{&*identity_buffer.as_ptr().cast::<TOKEN_APPCONTAINER_INFORMATION>()};
    if identity.TokenAppContainer.is_null()||unsafe{IsValidSid(identity.TokenAppContainer)}==0||unsafe{EqualSid(identity.TokenAppContainer,expected_sid)}==0{return Err(unavailable());}
    let label_buffer=token_information(token,TokenIntegrityLevel)?;
    if label_buffer.len()*std::mem::size_of::<usize>()<std::mem::size_of::<TOKEN_MANDATORY_LABEL>(){return Err(unavailable());}
    let label=unsafe{&*label_buffer.as_ptr().cast::<TOKEN_MANDATORY_LABEL>()};let sid=label.Label.Sid;
    if sid.is_null()||unsafe{IsValidSid(sid)}==0{return Err(unavailable());}
    let count=unsafe{*GetSidSubAuthorityCount(sid)};
    if count!=1||unsafe{(*GetSidIdentifierAuthority(sid)).Value}!=[0,0,0,0,0,16]||unsafe{*GetSidSubAuthority(sid,0)}!=SECURITY_MANDATORY_LOW_RID{return Err(unavailable());}
    let mut policy=TOKEN_MANDATORY_POLICY{Policy:0};bytes=0;
    if unsafe{GetTokenInformation(token.as_raw_handle(),TokenMandatoryPolicy,(&mut policy as*mut TOKEN_MANDATORY_POLICY).cast(),std::mem::size_of::<TOKEN_MANDATORY_POLICY>()as u32,&mut bytes)}==0||bytes as usize!=std::mem::size_of::<TOKEN_MANDATORY_POLICY>()||policy.Policy&TOKEN_MANDATORY_POLICY_NO_WRITE_UP==0{return Err(unavailable());}
    Ok(())
}

fn token_information(token:&std::os::windows::io::OwnedHandle,kind:windows_sys::Win32::Security::TOKEN_INFORMATION_CLASS)->Result<Vec<usize>>{
    use windows_sys::Win32::Security::GetTokenInformation;
    let mut bytes=0u32;unsafe{GetTokenInformation(token.as_raw_handle(),kind,null_mut(),0,&mut bytes);}
    if bytes==0||bytes>4096{return Err(unavailable());}
    let mut buffer=vec![0usize;(bytes as usize+std::mem::size_of::<usize>()-1)/std::mem::size_of::<usize>()];
    let capacity=(buffer.len()*std::mem::size_of::<usize>())as u32;
    if unsafe{GetTokenInformation(token.as_raw_handle(),kind,buffer.as_mut_ptr().cast(),capacity,&mut bytes)}==0||bytes==0||bytes>capacity{return Err(unavailable());}
    Ok(buffer)
}
