//! File-backed bootstrap admission source. The expected hash/length must come
//! from the broker's authenticated outer release, not a renderer's declaration.
use omniplay_contracts::{ContentHash,ErrorCode,PlatformError,Result};
use sha2::{Digest,Sha256};
use std::{fs::File,io::ErrorKind,os::{fd::{AsRawFd,RawFd},unix::fs::{FileExt,MetadataExt}},sync::atomic::{AtomicBool,Ordering}};

pub struct BootstrapExecutable{file:File,hash:ContentHash,bytes:u64}
pub struct PresentationExecutable{file:File,hash:ContentHash,bytes:u64}
fn unavailable()->PlatformError{PlatformError::new(ErrorCode::RuntimeUnavailable,"The reviewed Linux bootstrap executable could not be admitted")}
fn cancelled(cancel:&AtomicBool)->Result<()>{if cancel.load(Ordering::Acquire){Err(PlatformError::new(ErrorCode::Cancelled,"Linux executable admission was cancelled"))}else{Ok(())}}
fn read_at(file:&File,output:&mut[u8],offset:u64)->Result<()>{let mut at=0;while at<output.len(){let read=match file.read_at(&mut output[at..],offset+at as u64){Ok(value)=>value,Err(error)if error.kind()==ErrorKind::Interrupted=>continue,Err(_)=>return Err(unavailable())};if read==0{return Err(unavailable());}at+=read;}Ok(())}
fn u16(raw:&[u8],at:usize)->u16{u16::from_le_bytes([raw[at],raw[at+1]])}
fn u32(raw:&[u8],at:usize)->u32{u32::from_le_bytes([raw[at],raw[at+1],raw[at+2],raw[at+3]])}
fn u64(raw:&[u8],at:usize)->u64{u64::from_le_bytes(raw[at..at+8].try_into().expect("fixed ELF field"))}
fn static_pie(file:&File,bytes:u64)->Result<()>{
    let mut header=[0u8;64];read_at(file,&mut header,0)?;
    if &header[..4]!=b"\x7fELF"||header[4]!=2||header[5]!=1||header[6]!=1||!matches!(header[7],0|3)||u16(&header,16)!=3||u16(&header,18)!=62||u32(&header,20)!=1||u16(&header,52)!=64||u16(&header,54)!=56{return Err(unavailable());}
    let offset=u64(&header,32);let count=u16(&header,56)as u64;let entry=u64(&header,24);if !(1..=128).contains(&count)||offset<64||offset.checked_add(count*56).is_none_or(|end|end>bytes)||entry==0{return Err(unavailable());}
    let(mut executable_entry,mut stack,mut dynamic)=(false,false,false);
    for index in 0..count{
        let mut raw=[0u8;56];read_at(file,&mut raw,offset+index*56)?;let kind=u32(&raw,0);let flags=u32(&raw,4);let start=u64(&raw,8);let address=u64(&raw,16);let size=u64(&raw,32);let memory=u64(&raw,40);
        if flags&!7!=0||start.checked_add(size).is_none_or(|end|end>bytes)||memory<size||address.checked_add(memory).is_none(){return Err(unavailable());}
        if kind==3||kind==1&&flags&3==3{return Err(unavailable());}
        if kind==1&&flags&1!=0&&entry>=address&&entry<address+size{executable_entry=true;}
        if kind==0x6474e551{if stack||flags&1!=0{return Err(unavailable());}stack=true;}
        if kind==2{
            if dynamic||size==0||size>16384||size%16!=0{return Err(unavailable());}dynamic=true;let mut ended=false;
            for at in(0..size).step_by(16){let mut item=[0u8;16];read_at(file,&mut item,start+at)?;let tag=u64(&item,0);if tag==0{ended=true;break;}if matches!(tag,1|15|29){return Err(unavailable());}}
            if !ended{return Err(unavailable());}
        }
    }
    if !executable_entry||!stack||!dynamic{return Err(unavailable());}Ok(())
}
fn presentation_pie(file:&File,bytes:u64)->Result<()>{
    let mut header=[0u8;64];read_at(file,&mut header,0)?;
    if &header[..4]!=b"\x7fELF"||header[4]!=2||header[5]!=1||header[6]!=1||!matches!(header[7],0|3)||u16(&header,16)!=3||u16(&header,18)!=62||u32(&header,20)!=1||u16(&header,52)!=64||u16(&header,54)!=56{return Err(unavailable());}
    let offset=u64(&header,32);let count=u16(&header,56)as u64;let entry=u64(&header,24);if !(1..=128).contains(&count)||offset<64||offset.checked_add(count*56).is_none_or(|end|end>bytes)||entry==0{return Err(unavailable());}
    let(mut executable_entry,mut stack,mut dynamic,mut interpreter)=(false,false,false,false);let mut loads=Vec::new();let(mut strings,mut string_bytes)=(None,None);let mut needed=Vec::new();
    for index in 0..count{
        let mut raw=[0u8;56];read_at(file,&mut raw,offset+index*56)?;let kind=u32(&raw,0);let flags=u32(&raw,4);let start=u64(&raw,8);let address=u64(&raw,16);let size=u64(&raw,32);let memory=u64(&raw,40);
        if flags&!7!=0||start.checked_add(size).is_none_or(|end|end>bytes)||memory<size||address.checked_add(memory).is_none()||kind==1&&flags&3==3{return Err(unavailable());}
        if kind==1{loads.push((address,start,size));if flags&1!=0&&entry>=address&&entry<address+size{executable_entry=true;}}
        if kind==0x6474e551{if stack||flags&1!=0{return Err(unavailable());}stack=true;}
        if kind==3{let approved=b"/lib64/ld-linux-x86-64.so.2\0";if interpreter||size!=approved.len()as u64{return Err(unavailable());}let mut actual=vec![0u8;approved.len()];read_at(file,&mut actual,start)?;if actual.as_slice()!=approved{return Err(unavailable());}interpreter=true;}
        if kind==2{
            if dynamic||size==0||size>16384||size%16!=0{return Err(unavailable());}dynamic=true;let mut ended=false;
            for at in(0..size).step_by(16){let mut item=[0u8;16];read_at(file,&mut item,start+at)?;let tag=u64(&item,0);let value=u64(&item,8);if tag==0{ended=true;break;}
                match tag{1=>{if needed.len()>=64{return Err(unavailable());}needed.push(value);},5=>{if strings.replace(value).is_some(){return Err(unavailable());}},10=>{if string_bytes.replace(value).is_some(){return Err(unavailable());}},15|22|29=>return Err(unavailable()),_=>{}}
            }if !ended{return Err(unavailable());}
        }
    }
    if !executable_entry||!stack||!dynamic||!interpreter{return Err(unavailable());}
    if !needed.is_empty(){
        let address=strings.ok_or_else(unavailable)?;let size=string_bytes.filter(|size|*size>0&&*size<=4*1024*1024).ok_or_else(unavailable)?;let end=address.checked_add(size).ok_or_else(unavailable)?;let mut mapped=None;
        for(base,offset,length)in loads{if address>=base&&end<=base+length{if mapped.replace(offset+(address-base)).is_some(){return Err(unavailable());}}}let base=mapped.ok_or_else(unavailable)?;
        for name in needed{if name>=size{return Err(unavailable());}let count=(size-name).min(256)as usize;let mut raw=[0u8;256];read_at(file,&mut raw[..count],base+name)?;let length=raw[..count].iter().position(|byte|*byte==0).filter(|length|*length>0).ok_or_else(unavailable)?;let name=&raw[..length];if !name[0].is_ascii_alphanumeric()||!name.iter().all(|byte|byte.is_ascii_alphanumeric()||matches!(*byte,b'_'|b'+'|b'.'|b'-')){return Err(unavailable());}}
    }Ok(())
}
fn admitted(file:File,expected:&ContentHash,bytes:u64,maximum:u64,cancel:&AtomicBool,inspect:fn(&File,u64)->Result<()>)->Result<File>{
        cancelled(cancel)?;ContentHash::parse(expected.0.clone())?;if !(64..=maximum).contains(&bytes){return Err(unavailable());}
        let before=file.metadata().map_err(|_|unavailable())?;let flags=unsafe{libc::fcntl(file.as_raw_fd(),libc::F_GETFL)};
        if !before.is_file()||before.len()!=bytes||before.mode()&0o222!=0||before.mode()&0o111==0||before.mode()&0o6000!=0||before.uid()!=0&&before.uid()!=unsafe{libc::geteuid()}||flags<0||flags&libc::O_ACCMODE!=libc::O_RDONLY||unsafe{libc::fcntl(file.as_raw_fd(),libc::F_SETFD,libc::FD_CLOEXEC)}!=0{return Err(unavailable());}
        let(mut digest,mut offset)=(Sha256::new(),0u64);let mut buffer=[0u8;65536];while offset<bytes{cancelled(cancel)?;let count=(bytes-offset).min(buffer.len()as u64)as usize;read_at(&file,&mut buffer[..count],offset)?;digest.update(&buffer[..count]);offset+=count as u64;}
        if format!("{:x}",digest.finalize())!=expected.0{return Err(PlatformError::new(ErrorCode::ContentMismatch,"The Linux bootstrap differs from its authenticated inventory"));}
        inspect(&file,bytes)?;let after=file.metadata().map_err(|_|unavailable())?;cancelled(cancel)?;
        if after.dev()!=before.dev()||after.ino()!=before.ino()||after.len()!=before.len()||after.mode()!=before.mode()||after.mtime()!=before.mtime()||after.mtime_nsec()!=before.mtime_nsec()||after.ctime()!=before.ctime()||after.ctime_nsec()!=before.ctime_nsec(){return Err(unavailable());}
        Ok(file)
}
impl BootstrapExecutable{
    pub fn admit(file:File,expected:&ContentHash,bytes:u64,cancel:&AtomicBool)->Result<Self>{let file=admitted(file,expected,bytes,64*1024*1024,cancel,static_pie)?;Ok(Self{file,hash:expected.clone(),bytes})}
    pub fn sha256(&self)->&ContentHash{&self.hash}pub fn bytes(&self)->u64{self.bytes}pub(crate) fn descriptor(&self)->RawFd{self.file.as_raw_fd()}
}
impl PresentationExecutable{
    pub fn admit(file:File,expected:&ContentHash,bytes:u64,cancel:&AtomicBool)->Result<Self>{let file=admitted(file,expected,bytes,256*1024*1024,cancel,presentation_pie)?;Ok(Self{file,hash:expected.clone(),bytes})}
    pub fn sha256(&self)->&ContentHash{&self.hash}pub fn bytes(&self)->u64{self.bytes}pub(crate) fn descriptor(&self)->RawFd{self.file.as_raw_fd()}
}
