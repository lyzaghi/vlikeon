//! Closed source contract for the optional Linux bootstrap. Encoding is not OS
//! admission: the future broker must authenticate the package and actual FDs,
//! seal the plan/images, arrange the exact spawn map and own process/cgroup exit.
//! This module does not enable native Linux availability.
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use std::collections::BTreeSet;

pub const HEADER_BYTES:usize=96;
pub const ENTRY_BYTES:usize=536;
pub const MAXIMUM_FILES:usize=256;
const MIB:u64=1024*1024;

#[derive(Clone,Copy)]
pub enum RootFileKind{Image{executable:bool},GameInput}
pub struct RootFile{pub descriptor:u16,pub path:String,pub bytes:u64,pub kind:RootFileKind}
pub struct BootstrapPlan{
    pub session:String,pub expected_parent:u32,pub empty_directory_descriptor:u16,pub cgroup_descriptor:u16,
    pub memory_bytes:u64,pub maximum_file_bytes:u64,pub root_bytes:u64,pub scratch_bytes:u64,pub save_bytes:u64,
    pub cpu_cores:u8,pub files:Vec<RootFile>,
}
fn invalid()->PlatformError{PlatformError::new(ErrorCode::InvalidRequest,"The private Linux bootstrap plan is outside its closed policy")}
pub(crate) fn session(value:&str)->Result<[u8;16]>{
    let raw=value.as_bytes();if raw.len()!=36{return Err(invalid());}let mut identity=[0u8;16];let(mut source,mut target)=(0,0);
    fn digit(value:u8)->Result<u8>{match value{b'0'..=b'9'=>Ok(value-b'0'),b'a'..=b'f'=>Ok(value-b'a'+10),_=>Err(invalid())}}
    while source<36{if matches!(source,8|13|18|23){if raw[source]!=b'-'{return Err(invalid());}source+=1;continue;}if source+1>=36||target>=16{return Err(invalid());}identity[target]=digit(raw[source])?*16+digit(raw[source+1])?;target+=1;source+=2;}
    if target!=16||identity.iter().all(|byte|*byte==0){return Err(invalid());}Ok(identity)
}
pub fn root_path(value:&str)->bool{
    if value.is_empty()||value.len()>=512||!value.is_ascii(){return false;}let mut depth=0;
    for part in value.split('/'){
        depth+=1;if depth>8||part.is_empty()||part.len()>127{return false;}
        if !part.bytes().enumerate().all(|(at,value)|value.is_ascii_alphanumeric()||matches!(value,b'_'|b'-')||matches!(value,b'.'|b'+')&&at>0){return false;}
    }true
}
fn put32(output:&mut[u8],offset:usize,value:u32){output[offset..offset+4].copy_from_slice(&value.to_le_bytes());}
fn put64(output:&mut[u8],offset:usize,value:u64){output[offset..offset+8].copy_from_slice(&value.to_le_bytes());}
impl BootstrapPlan{
    pub fn encode(&self)->Result<Vec<u8>>{
        let identity=session(&self.session)?;
        if !(2..=i32::MAX as u32).contains(&self.expected_parent)||!(8..1024).contains(&self.empty_directory_descriptor)||!(8..1024).contains(&self.cgroup_descriptor)||self.empty_directory_descriptor==self.cgroup_descriptor||!(1..=8).contains(&self.cpu_cores)||!(2..=MAXIMUM_FILES).contains(&self.files.len())||!(256*MIB..=8192*MIB).contains(&self.memory_bytes)||self.maximum_file_bytes==0||self.maximum_file_bytes>256*MIB||!(16*MIB..=1024*MIB).contains(&self.root_bytes)||!(4*MIB..=512*MIB).contains(&self.scratch_bytes)||!(4*MIB..=512*MIB).contains(&self.save_bytes)||(self.memory_bytes|self.root_bytes|self.scratch_bytes|self.save_bytes)&4095!=0{return Err(invalid());}
        let mut descriptors=BTreeSet::from([self.empty_directory_descriptor,self.cgroup_descriptor]);let mut paths=BTreeSet::new();let(mut worker,mut loader,mut images,mut inputs)=(false,false,0u64,0u64);
        for file in &self.files{
            if !(8..1024).contains(&file.descriptor)||!descriptors.insert(file.descriptor)||!root_path(&file.path)||!paths.insert(&file.path)||file.bytes==0{return Err(invalid());}
            match file.kind{
                RootFileKind::Image{executable}=>{
                    let is_worker=file.path=="runtime/omniplay-native-worker";let is_loader=file.path=="lib64/ld-linux-x86-64.so.2";
                    if (!file.path.starts_with("runtime/")&&!is_loader)||executable!=(is_worker||is_loader)||file.bytes>256*MIB{return Err(invalid());}
                    images=images.checked_add(file.bytes).ok_or_else(invalid)?;if images>self.root_bytes-2*MIB{return Err(invalid());}worker|=is_worker;loader|=is_loader;
                }
                RootFileKind::GameInput=>{
                    let name=file.path.strip_prefix("work/").ok_or_else(invalid)?;
                    if name.is_empty()||name.contains('/')||matches!(name,"scratch"|"save")||file.bytes>4096*MIB{return Err(invalid());}
                    inputs=inputs.checked_add(file.bytes).ok_or_else(invalid)?;if inputs>32768*MIB{return Err(invalid());}
                }
            }
        }
        if !worker||!loader{return Err(invalid());}
        let count=HEADER_BYTES+self.files.len()*ENTRY_BYTES;let mut output=Vec::new();output.try_reserve_exact(count).map_err(|_|PlatformError::new(ErrorCode::ResourceLimit,"The private Linux bootstrap plan could not be allocated"))?;output.resize(count,0);
        output[..8].copy_from_slice(b"OPNSBX03");put32(&mut output,8,3);put32(&mut output,12,self.files.len()as u32);put32(&mut output,16,self.expected_parent);put32(&mut output,20,u32::from(self.empty_directory_descriptor));
        for(offset,value)in[(24,self.memory_bytes),(32,self.maximum_file_bytes),(40,self.root_bytes),(48,self.scratch_bytes),(56,self.save_bytes)]{put64(&mut output,offset,value);}output[64..80].copy_from_slice(&identity);put32(&mut output,80,u32::from(self.cgroup_descriptor));put32(&mut output,84,64);put32(&mut output,88,u32::from(self.cpu_cores));
        for(index,file)in self.files.iter().enumerate(){
            let at=HEADER_BYTES+index*ENTRY_BYTES;let(role,executable)=match file.kind{RootFileKind::Image{executable}=>(1,if executable{1}else{0}),RootFileKind::GameInput=>(2,0)};
            put32(&mut output,at,role);put32(&mut output,at+4,u32::from(file.descriptor));put64(&mut output,at+8,file.bytes);put32(&mut output,at+16,executable);output[at+20..at+22].copy_from_slice(&(file.path.len()as u16).to_le_bytes());output[at+24..at+24+file.path.len()].copy_from_slice(file.path.as_bytes());
        }Ok(output)
    }
}
