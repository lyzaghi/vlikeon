//! Seal only the suspended actor's own AppContainer registry namespace.
use super::{handle,storage,unavailable};
use omniplay_contracts::Result;
use windows_sys::Win32::System::SystemServices::ACCESS_ALLOWED_ACE_TYPE;
use std::{ffi::c_void,os::windows::io::{AsRawHandle,OwnedHandle},ptr::{null,null_mut}};
use windows_sys::Win32::{Foundation::{ERROR_FILE_NOT_FOUND,ERROR_MORE_DATA,ERROR_NO_MORE_ITEMS,ERROR_SUCCESS,LocalFree},Security::{ACCESS_ALLOWED_ACE,ACE_HEADER,CopySid,DACL_SECURITY_INFORMATION,GetAce,GetLengthSid,ImpersonateLoggedOnUser,INHERIT_ONLY_ACE,IsValidAcl,IsValidSid,PSID,RevertToSelf,TOKEN_DUPLICATE,TOKEN_QUERY,Authorization::{GetSecurityInfo,SetSecurityInfo,SetEntriesInAclW,EXPLICIT_ACCESS_W,TRUSTEE_W,TRUSTEE_IS_SID,TRUSTEE_IS_UNKNOWN,NO_MULTIPLE_TRUSTEE,DENY_ACCESS,SE_REGISTRY_KEY},Isolation::GetAppContainerRegistryLocation},System::{Registry::*,Threading::{GetCurrentThread,OpenProcessToken,OpenThreadToken}}};

const READ_CONTROL_ACCESS:u32=0x0002_0000;
const WRITE_OWNER_ACCESS:u32=0x0008_0000;
const WRITE_DACL_ACCESS:u32=0x0004_0000;
const DELETE_ACCESS:u32=0x0001_0000;
const GENERIC_READ_OR_ALL_ACCESS:u32=0x9000_0000;
const SEAL_ACCESS:u32=KEY_QUERY_VALUE|KEY_ENUMERATE_SUB_KEYS|READ_CONTROL_ACCESS|WRITE_OWNER_ACCESS|WRITE_DACL_ACCESS;
const MAX_KEYS:usize=256;
const MAX_DEPTH:u8=16;

struct Descriptor(*mut c_void);
impl Drop for Descriptor{fn drop(&mut self){if !self.0.is_null(){unsafe{LocalFree(self.0);}}}}

struct RegistryKey(HKEY);
// A real opened registry handle is thread-independent. Ownership is moved once
// out of a dedicated lookup thread; RegCloseKey runs exactly once. No Sync or
// clone implementation exposes concurrent or predefined-hive ownership.
unsafe impl Send for RegistryKey {}
impl RegistryKey {
    fn deny_actor_mutation(&self,sid:PSID)->Result<()>{
        let mut old=null_mut();let mut descriptor=null_mut();
        if unsafe{GetSecurityInfo(self.0,SE_REGISTRY_KEY,DACL_SECURITY_INFORMATION,null_mut(),null_mut(),&mut old,null_mut(),&mut descriptor)}!=ERROR_SUCCESS{return Err(unavailable());}
        let _descriptor=Descriptor(descriptor);
        if old.is_null()||unsafe{IsValidAcl(old)}==0||unsafe{(*old).AceCount}>256{return Err(unavailable());}
        // Do not use KEY_WRITE: it also includes READ_CONTROL. Preserve readonly
        // lookup and broker/user access, denying mutation only to this fresh SID.
        let rights=KEY_SET_VALUE|KEY_CREATE_SUB_KEY|KEY_CREATE_LINK|DELETE_ACCESS|WRITE_DACL_ACCESS|WRITE_OWNER_ACCESS;
        let entry=EXPLICIT_ACCESS_W{grfAccessPermissions:rights,grfAccessMode:DENY_ACCESS,grfInheritance:2,Trustee:TRUSTEE_W{pMultipleTrustee:null_mut(),MultipleTrusteeOperation:NO_MULTIPLE_TRUSTEE,TrusteeForm:TRUSTEE_IS_SID,TrusteeType:TRUSTEE_IS_UNKNOWN,ptstrName:sid.cast()}};
        let mut updated=null_mut();
        if unsafe{SetEntriesInAclW(1,&entry,old,&mut updated)}!=ERROR_SUCCESS{return Err(unavailable());}
        let _updated=Descriptor(updated.cast());
        if updated.is_null()||unsafe{SetSecurityInfo(self.0,SE_REGISTRY_KEY,DACL_SECURITY_INFORMATION,null_mut(),null_mut(),updated,null())}!=ERROR_SUCCESS{return Err(unavailable());}
        Ok(())
    }
    fn own(raw:HKEY)->Result<Self>{
        // Predefined hive pseudo-handles are negative values on the supported
        // Windows x64 target. Never accept one as a private storage root.
        if raw.is_null()||(raw as isize)<0{return Err(unavailable());}Ok(Self(raw))
    }
    fn reopen(&self,access:u32)->Result<Self>{
        let mut raw=null_mut();
        if (unsafe{RegOpenKeyExW(self.0,null(),REG_OPTION_OPEN_LINK,access,&mut raw)}as u32)!=ERROR_SUCCESS{return Err(unavailable());}
        Self::own(raw)
    }
    fn reject_link(&self)->Result<()>{
        let name=[83u16,121,109,98,111,108,105,99,76,105,110,107,86,97,108,117,101,0]; // SymbolicLinkValue
        let mut kind=0u32;let mut bytes=0u32;
        let result=unsafe{RegQueryValueExW(self.0,name.as_ptr(),null_mut(),&mut kind,null_mut(),&mut bytes)}as u32;
        match result{
            ERROR_FILE_NOT_FOUND=>Ok(()),
            ERROR_SUCCESS|ERROR_MORE_DATA if kind!=REG_LINK=>Ok(()),
            _=>Err(unavailable()),
        }
    }
    fn require_private_grant(&self,expected:PSID)->Result<()>{
        let mut acl=null_mut();let mut descriptor=null_mut();
        if unsafe{GetSecurityInfo(self.0,SE_REGISTRY_KEY,DACL_SECURITY_INFORMATION,null_mut(),null_mut(),&mut acl,null_mut(),&mut descriptor)}!=ERROR_SUCCESS{return Err(unavailable());}
        let _descriptor=Descriptor(descriptor);
        if descriptor.is_null()||acl.is_null()||unsafe{IsValidAcl(acl)}==0||unsafe{(*acl).AceCount}>256{return Err(unavailable());}
        for index in 0..u32::from(unsafe{(*acl).AceCount}){
            let mut raw=null_mut();if unsafe{GetAce(acl,index,&mut raw)}==0||raw.is_null(){return Err(unavailable());}
            let header=unsafe{&*raw.cast::<ACE_HEADER>()};
            if header.AceType!=ACCESS_ALLOWED_ACE_TYPE as u8||header.AceFlags&(INHERIT_ONLY_ACE as u8)!=0{continue;}
            let offset=std::mem::offset_of!(ACCESS_ALLOWED_ACE,SidStart);
            if usize::from(header.AceSize)<offset+8{return Err(unavailable());}
            let ace=unsafe{&*raw.cast::<ACCESS_ALLOWED_ACE>()};
            let subauthorities=unsafe{*raw.cast::<u8>().add(offset+1)};
            if subauthorities>15||8+usize::from(subauthorities)*4>usize::from(header.AceSize)-offset{return Err(unavailable());}
            let sid=unsafe{raw.cast::<u8>().add(offset)}.cast();
            if unsafe{IsValidSid(sid)}==0||(unsafe{GetLengthSid(sid)}as usize)>usize::from(header.AceSize)-offset{return Err(unavailable());}
            let read=KEY_QUERY_VALUE|KEY_ENUMERATE_SUB_KEYS;
            let readable=ace.Mask&read==read||ace.Mask&GENERIC_READ_OR_ALL_ACCESS!=0;
            if readable&&unsafe{windows_sys::Win32::Security::EqualSid(sid,expected)}!=0{return Ok(());}
        }
        Err(unavailable())
    }
}
impl Drop for RegistryKey{fn drop(&mut self){unsafe{RegCloseKey(self.0);}}}

fn copied_sid(sid:PSID)->Result<Vec<usize>>{
    if sid.is_null()||unsafe{IsValidSid(sid)}==0{return Err(unavailable());}
    let bytes=unsafe{GetLengthSid(sid)};
    if bytes==0||bytes>68{return Err(unavailable());}
    let mut result=vec![0usize;(bytes as usize+std::mem::size_of::<usize>()-1)/std::mem::size_of::<usize>()];
    if unsafe{CopySid(bytes,result.as_mut_ptr().cast(),sid)}==0{return Err(unavailable());}Ok(result)
}

fn actor_root(process:&OwnedHandle,expected_sid:PSID)->Result<RegistryKey>{
    let mut raw=null_mut();
    if unsafe{OpenProcessToken(process.as_raw_handle(),TOKEN_QUERY|TOKEN_DUPLICATE,&mut raw)}==0{return Err(unavailable());}
    let token=handle(raw)?;let mut expected=copied_sid(expected_sid)?;
    // Never impersonate on the broker/preparation thread or a reusable pool.
    // This new thread performs one lookup and is joined before any registry
    // mutation. A panic or failed revert ends this thread without reuse.
    let thread=std::thread::Builder::new().name("omniplay-registry-scope".into()).spawn(move||{
        if unsafe{ImpersonateLoggedOnUser(token.as_raw_handle())}==0{return Err(unavailable());}
        let opened=(||{
            let mut actual=null_mut();
            if unsafe{OpenThreadToken(GetCurrentThread(),TOKEN_QUERY,1,&mut actual)}==0{return Err(unavailable());}
            let actual=handle(actual)?;storage::verify_token(&actual,expected.as_mut_ptr().cast())?;
            let mut key=null_mut();let result=unsafe{GetAppContainerRegistryLocation(KEY_READ,&mut key)};
            if result<0{if !key.is_null()&&(key as isize)>0{unsafe{RegCloseKey(key);}}return Err(unavailable());}
            RegistryKey::own(key)
        })();
        // Return no key on revert failure. Only local handle cleanup and thread
        // exit follow; the parent was never impersonating and joins this thread.
        if unsafe{RevertToSelf()}==0{drop(opened);return Err(unavailable());}
        opened
    }).map_err(|_|unavailable())?;
    thread.join().map_err(|_|unavailable())?
}

pub(super) fn seal(process:&OwnedHandle,expected_sid:PSID)->Result<()>{
    let lookup=actor_root(process,expected_sid)?;
    // A real kernel key with an effective grant naming the exact fresh SID is
    // required before requesting broker security-write access. An opened user
    // hive or another profile's otherwise-readable root cannot be substituted.
    lookup.require_private_grant(expected_sid)?;
    // Reopen this exact kernel key in the broker context. The low token needs
    // only readonly lookup access, never WRITE_OWNER or a guessed HKCU path.
    let root=lookup.reopen(SEAL_ACCESS)?;drop(lookup);root.reject_link()?;
    let mut keys=vec![root];let mut pending=vec![(0usize,0u8)];
    while let Some((parent,depth))=pending.pop(){
        let parent=keys[parent].0;let mut index=0u32;
        loop{
            let mut name=[0u16;256];let mut units=name.len()as u32;
            let result=unsafe{RegEnumKeyExW(parent,index,name.as_mut_ptr(),&mut units,null_mut(),null_mut(),null_mut(),null_mut())}as u32;
            if result==ERROR_NO_MORE_ITEMS{break;}
            if result!=ERROR_SUCCESS||units==0||units as usize>=name.len()||depth>=MAX_DEPTH||keys.len()>=MAX_KEYS{return Err(unavailable());}
            if name[..units as usize].iter().any(|unit|matches!(*unit,0|92)){return Err(unavailable());}
            name[units as usize]=0;let mut raw=null_mut();
            if (unsafe{RegOpenKeyExW(parent,name.as_ptr(),REG_OPTION_OPEN_LINK,SEAL_ACCESS,&mut raw)}as u32)!=ERROR_SUCCESS{return Err(unavailable());}
            let child=RegistryKey::own(raw)?;child.reject_link()?;
            let position=keys.len();keys.push(child);pending.push((position,depth+1));index+=1;
        }
    }
    // Full bounded/link-refusing preflight precedes inheritable label changes.
    // The actor has never resumed and inherits no registry handles.
    let label=storage::MandatoryLabel::registry()?;
    for key in keys.iter().rev(){
        // A mandatory label alone did not deny RegSetValueEx in the actual
        // actor (batch024). Enforce the exact SID's access mask as well.
        key.deny_actor_mutation(expected_sid)?;
        label.apply_object(key.0,SE_REGISTRY_KEY)?;
    }
    Ok(())
}
