//! Finite ELF identity/DT_NEEDED inspection of already authenticated immutable
//! bytes. This never loads a library, invokes a loader or executes guest code.
use crate::linux_sealed::SealedObject;
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use std::{collections::BTreeSet,ffi::CString,fs::{File,OpenOptions},io::ErrorKind,os::{fd::{AsRawFd,FromRawFd},unix::fs::{FileExt,MetadataExt,OpenOptionsExt}},path::Path};

#[derive(Clone,Copy,PartialEq,Eq)]pub enum ImageRole{Worker,Loader,Core,Dependency,Asset}
pub struct ElfDependencies{pub needed:Vec<String>,pub soname:Option<String>}
pub struct ReleaseDirectory{directory:File}
fn invalid()->PlatformError{PlatformError::new(ErrorCode::RuntimeUnavailable,"The authenticated Linux image closure is outside its loader policy")}
pub fn basename(value:&str)->bool{!value.is_empty()&&value.len()<=127&&value.as_bytes()[0].is_ascii_alphanumeric()&&!value.ends_with('.')&&value.bytes().all(|byte|byte.is_ascii_alphanumeric()||matches!(byte,b'_'|b'-'|b'+'|b'.'))}
fn directory(value:&File)->Result<()>{let shape=value.metadata().map_err(|_|invalid())?;if !shape.is_dir()||shape.mode()&0o022!=0||shape.uid()!=0&&shape.uid()!=unsafe{libc::geteuid()}{return Err(invalid());}Ok(())}
impl ReleaseDirectory{
    pub fn open(path:&Path)->Result<Self>{let selected=OpenOptions::new().read(true).custom_flags(libc::O_PATH|libc::O_DIRECTORY|libc::O_NOFOLLOW|libc::O_CLOEXEC).open(path).map_err(|_|invalid())?;directory(&selected)?;Ok(Self{directory:selected})}
    /// Relative ordinary files only. Every component is opened against the
    /// previous held directory; no symlink or mutable group/other directory.
    pub fn file(&self,path:&str)->Result<File>{
        self.entry(path,false)
    }
    pub fn subdirectory(&self,path:&str)->Result<File>{self.entry(path,true)}
    fn entry(&self,path:&str,leaf_directory:bool)->Result<File>{
        if path.is_empty()||path.len()>240||!path.is_ascii(){return Err(invalid());}let parts:Vec<_>=path.split('/').collect();if parts.len()>16||parts.iter().any(|part|part.is_empty()||*part=="."||*part==".."||part.len()>127||!part.bytes().all(|byte|byte.is_ascii_alphanumeric()||matches!(byte,b'_'|b'-'|b'+'|b'.'|b' '))){return Err(invalid());}
        let mut current=self.directory.try_clone().map_err(|_|invalid())?;
        for(index,part)in parts.iter().enumerate(){let leaf=index+1==parts.len();let name=CString::new(*part).map_err(|_|invalid())?;let flags=libc::O_CLOEXEC|libc::O_NOFOLLOW|if leaf&&!leaf_directory{libc::O_RDONLY|libc::O_NONBLOCK}else{libc::O_PATH|libc::O_DIRECTORY};let raw=unsafe{libc::openat(current.as_raw_fd(),name.as_ptr(),flags)};if raw<0{return Err(invalid());}let selected=unsafe{File::from_raw_fd(raw)};
            if leaf{if leaf_directory{directory(&selected)?;return Ok(selected);}let shape=selected.metadata().map_err(|_|invalid())?;if !shape.is_file()||shape.nlink()!=1||shape.mode()&0o022!=0||shape.uid()!=0&&shape.uid()!=unsafe{libc::geteuid()}{return Err(invalid());}return Ok(selected);}directory(&selected)?;current=selected;
        }Err(invalid())
    }
}
fn read(file:&File,raw:&mut[u8],offset:u64)->Result<()>{let mut at=0;while at<raw.len(){let count=match file.read_at(&mut raw[at..],offset+at as u64){Ok(value)=>value,Err(error)if error.kind()==ErrorKind::Interrupted=>continue,Err(_)=>return Err(invalid())};if count==0{return Err(invalid());}at+=count;}Ok(())}
fn u16(raw:&[u8],at:usize)->u16{u16::from_le_bytes([raw[at],raw[at+1]])}
fn u32(raw:&[u8],at:usize)->u32{u32::from_le_bytes(raw[at..at+4].try_into().expect("fixed ELF u32"))}
fn u64(raw:&[u8],at:usize)->u64{u64::from_le_bytes(raw[at..at+8].try_into().expect("fixed ELF u64"))}
#[derive(Clone,Copy)]struct Load{offset:u64,address:u64,bytes:u64,memory:u64,flags:u32}
fn mapped(loads:&[Load],address:u64,bytes:u64)->Result<u64>{let end=address.checked_add(bytes).ok_or_else(invalid)?;let mut result=None;for load in loads{if address>=load.address&&end<=load.address+load.bytes{if result.replace(load.offset+(address-load.address)).is_some(){return Err(invalid());}}}result.ok_or_else(invalid)}
fn name(file:&File,base:u64,bytes:u64,offset:u64)->Result<String>{if offset>=bytes{return Err(invalid());}let count=(bytes-offset).min(128)as usize;let mut raw=[0u8;128];read(file,&mut raw[..count],base+offset)?;let count=raw[..count].iter().position(|byte|*byte==0).ok_or_else(invalid)?;let value=std::str::from_utf8(&raw[..count]).map_err(|_|invalid())?;if !basename(value){return Err(invalid());}Ok(value.to_owned())}
pub fn inspect(object:&SealedObject,role:ImageRole)->Result<Option<ElfDependencies>>{
    let file=object.file();let bytes=object.bytes();let mut magic=[0u8;4];if bytes>=4{read(file,&mut magic,0)?;}if role==ImageRole::Asset{if magic==*b"\x7fELF"{return Err(invalid());}return Ok(None);}
    if bytes<64{return Err(invalid());}let mut header=[0u8;64];read(file,&mut header,0)?;if &header[..4]!=b"\x7fELF"||header[4]!=2||header[5]!=1||header[6]!=1||!matches!(header[7],0|3)||u16(&header,16)!=3||u16(&header,18)!=62||u32(&header,20)!=1||u32(&header,48)!=0||u16(&header,52)!=64||u16(&header,54)!=56{return Err(invalid());}
    let offset=u64(&header,32);let count=u16(&header,56)as u64;let entry=u64(&header,24);if !(1..=128).contains(&count)||offset<64||offset.checked_add(count*56).is_none_or(|end|end>bytes){return Err(invalid());}
    let mut loads=Vec::new();let(mut stack,mut interpreter)=(false,false);let(mut dynamic,mut relro)=(None,None);
    for index in 0..count{let mut raw=[0u8;56];read(file,&mut raw,offset+index*56)?;let kind=u32(&raw,0);let flags=u32(&raw,4);let start=u64(&raw,8);let address=u64(&raw,16);let length=u64(&raw,32);let memory=u64(&raw,40);let align=u64(&raw,48);
        if flags&!7!=0||memory<length||start.checked_add(length).is_none_or(|end|end>bytes)||address.checked_add(memory).is_none(){return Err(invalid());}
        match kind{
            1=>{if loads.len()>=32||flags&3==3||flags&1!=0&&flags&4==0||!(4096..=2*1024*1024).contains(&align)||!align.is_power_of_two()||address%align!=start%align{return Err(invalid());}let load=Load{offset:start,address,bytes:length,memory,flags};if loads.iter().any(|other:&Load|memory>0&&other.memory>0&&address<other.address+other.memory&&other.address<address+memory){return Err(invalid());}loads.push(load);},
            2=>{if dynamic.replace((start,length,address)).is_some()||length==0||length>16384||length%16!=0{return Err(invalid());}},
            3=>{let approved=b"/lib64/ld-linux-x86-64.so.2\0";if role==ImageRole::Loader||interpreter||length!=approved.len()as u64{return Err(invalid());}let mut actual=vec![0u8;approved.len()];read(file,&mut actual,start)?;if actual.as_slice()!=approved{return Err(invalid());}interpreter=true;},
            0x6474e551=>{if stack||flags&1!=0{return Err(invalid());}stack=true;},
            0x6474e552=>{if relro.replace((address,memory)).is_some()||memory==0{return Err(invalid());}},_=>{}
        }
    }
    let(relro_address,relro_memory)=relro.ok_or_else(invalid)?;if !loads.iter().any(|load|relro_address>=load.address&&relro_address+relro_memory<=load.address+load.memory)||loads.is_empty()||!stack||role==ImageRole::Worker&&!interpreter||matches!(role,ImageRole::Worker|ImageRole::Loader)&&(entry==0||!loads.iter().any(|load|load.flags&1!=0&&entry>=load.address&&entry<load.address+load.bytes)){return Err(invalid());}
    let(start,length,address)=dynamic.ok_or_else(invalid)?;if mapped(&loads,address,length)?!=start{return Err(invalid());}let(mut strings,mut string_bytes,mut soname,mut flags,mut flags1)=(None,None,None,None,None);let mut needed=Vec::new();let mut ended=false;
    for at in(0..length).step_by(16){let mut item=[0u8;16];read(file,&mut item,start+at)?;let tag=u64(&item,0);let value=u64(&item,8);if tag==0{ended=true;break;}match tag{
        1=>{if needed.len()>=64{return Err(invalid());}needed.push(value);},5=>{if strings.replace(value).is_some(){return Err(invalid());}},10=>{if string_bytes.replace(value).is_some(){return Err(invalid());}},14=>{if soname.replace(value).is_some(){return Err(invalid());}},30=>{if flags.replace(value).is_some()||value&4!=0{return Err(invalid());}},0x6ffffffb=>{if flags1.replace(value).is_some()||value&(0x10|0x01000000|0x20000000)!=0{return Err(invalid());}},
        // RPATH/RUNPATH/TEXTREL, audit/configuration and auxiliary/filter loader
        // extensions cannot introduce an unlisted dependency/search authority.
        15|22|29|0x6ffffefa|0x6ffffefb|0x6ffffefc|0x7ffffffd|0x7fffffff=>return Err(invalid()),_=>{}
    }}if !ended||role==ImageRole::Worker&&(flags1.unwrap_or(0)&0x08000000==0||flags.unwrap_or(0)&8==0&&flags1.unwrap_or(0)&1==0){return Err(invalid());}
    let length=string_bytes.filter(|value|*value>0&&*value<=4*1024*1024).ok_or_else(invalid)?;let base=mapped(&loads,strings.ok_or_else(invalid)?,length)?;let soname=soname.map(|offset|name(file,base,length,offset)).transpose()?;let mut names=Vec::new();let mut distinct=BTreeSet::new();for offset in needed{let value=name(file,base,length,offset)?;if !distinct.insert(value.clone()){return Err(invalid());}names.push(value);}Ok(Some(ElfDependencies{needed:names,soname}))
}
