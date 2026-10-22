use omniplay_contracts::{ErrorCode,PlatformError,Result,MAX_ROM_BYTES};
use omniplay_core::media_import::{DiscProvider,ProvidedDiscFile};
use std::{collections::BTreeSet,ffi::c_void,io::{self,Read},path::{Component,Path,PathBuf},sync::{Arc,Mutex,OnceLock}};
use uuid::Uuid;

#[repr(C)]#[derive(Default)]pub struct OmniMediaStream{pub token:u64,pub byte_length:i64}
type Open=unsafe extern "C" fn(*mut c_void,*const u8,*const u8,usize,*mut OmniMediaStream)->i32;
type ReadChunk=unsafe extern "C" fn(*mut c_void,u64,*mut u8,usize)->i64;
type Close=unsafe extern "C" fn(*mut c_void,u64);
type Cancel=unsafe extern "C" fn(*mut c_void);
type Release=unsafe extern "C" fn(*mut c_void);
#[repr(C)]pub struct OmniMediaCallbacks{pub abi_version:u32,pub context:*mut c_void,pub open:Option<Open>,pub read:Option<ReadChunk>,pub close:Option<Close>,pub cancel:Option<Cancel>,pub release:Option<Release>}
pub(crate) struct NativeDiscProvider{context:usize,open:Option<Open>,read:Option<ReadChunk>,close:Option<Close>,cancel:Option<Cancel>,release:Option<Release>,profile:OnceLock<Uuid>,streams:Mutex<BTreeSet<u64>>}
impl NativeDiscProvider{
    /// # Safety
    /// Trusted native callbacks must be valid for the context, thread-safe,
    /// synchronously borrow buffers without overrunning/retaining them, stay in
    /// the chosen document tree, and never reenter the Rust host. open may return
    /// a lazy stream token. close must release that exact token; release consumes
    /// the context once, including validation failure. Context is always consumed.
    pub(crate) unsafe fn take(table:OmniMediaCallbacks)->Result<Arc<Self>>{
        let value=Self{context:table.context as usize,open:table.open,read:table.read,close:table.close,cancel:table.cancel,release:table.release,profile:OnceLock::new(),streams:Mutex::new(BTreeSet::new())};
        if table.abi_version!=1||value.context==0||value.open.is_none()||value.read.is_none()||value.close.is_none()||value.cancel.is_none()||value.release.is_none(){return Err(PlatformError::new(ErrorCode::InvalidRequest,"Native media provider ABI is invalid"));}Ok(Arc::new(value))
    }
    pub fn bind(&self,profile:Uuid)->Result<()>{self.profile.set(profile).map_err(|_|denied())}
    fn context(&self)->*mut c_void{self.context as *mut c_void}
    fn close_stream(&self,token:u64){let present=self.streams.lock().unwrap_or_else(|error|error.into_inner()).remove(&token);if present{if let Some(close)=self.close{unsafe{close(self.context(),token)};}}}
    fn reader(self:&Arc<Self>,relative:&Path)->Result<ProvidedDiscFile>{
        let profile=self.profile.get().ok_or_else(denied)?;if relative.is_absolute()||relative.components().count()>8{return Err(denied());}
        let mut parts=Vec::new();for part in relative.components(){let Component::Normal(part)=part else{return Err(denied());};let value=part.to_str().ok_or_else(denied)?;if value.is_empty()||value.len()>512||value.ends_with('.')||value.ends_with(' ')||value.chars().any(|character|matches!(character,':'|'/'|'\\')||character.is_control()){return Err(denied());}parts.push(value);}
        let path=parts.join("/");if path.is_empty()||path.len()>2048{return Err(denied());}
        if self.streams.lock().map_err(|_|denied())?.len()>=129{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Too many native disc streams are open"));}
        let open=self.open.ok_or_else(denied)?;let mut stream=OmniMediaStream::default();let status=unsafe{open(self.context(),profile.as_bytes().as_ptr(),path.as_ptr(),path.len(),&mut stream)};
        if status!=0{return Err(provider_error(status as i64));}
        if stream.token==0{return Err(PlatformError::new(ErrorCode::CorruptData,"Native disc stream token is invalid"));}
        {let mut streams=self.streams.lock().map_err(|_|denied())?;if streams.contains(&stream.token){return Err(PlatformError::new(ErrorCode::CorruptData,"Native disc stream token was reused"));}streams.insert(stream.token);}
        if stream.byte_length< -1||stream.byte_length==0||stream.byte_length>MAX_ROM_BYTES as i64||self.streams.lock().map_err(|_|denied())?.len()>129{self.close_stream(stream.token);return Err(PlatformError::new(ErrorCode::ResourceLimit,"Native disc length or stream count exceeds its bound"));}
        let length=if stream.byte_length<0{None}else{Some(stream.byte_length as u64)};
        Ok(ProvidedDiscFile{identity:PathBuf::from(path),byte_length:length,reader:Box::new(NativeDiscReader{provider:self.clone(),token:stream.token,length,seen:0,closed:false})})
    }
}
/// The small Arc facade preserves the callback context through every planned
/// reader even when a renderer's one-use folder grant has already been consumed.
pub(crate) struct NativeDiscSource(pub Arc<NativeDiscProvider>);
impl DiscProvider for NativeDiscSource{fn open(&self,relative:&Path)->Result<ProvidedDiscFile>{self.0.reader(relative)}fn cancel(&self){if let Some(cancel)=self.0.cancel{unsafe{cancel(self.0.context())};}}}
impl Drop for NativeDiscProvider{fn drop(&mut self){if self.context!=0{if let Some(release)=self.release{unsafe{release(self.context())};}}}}
struct NativeDiscReader{provider:Arc<NativeDiscProvider>,token:u64,length:Option<u64>,seen:u64,closed:bool}
impl NativeDiscReader{fn finish(&mut self){if !self.closed{self.closed=true;self.provider.close_stream(self.token);}}fn failed(&mut self,error:PlatformError)->io::Error{self.finish();io::Error::other(error)}}
impl Read for NativeDiscReader{
    fn read(&mut self,output:&mut[u8])->io::Result<usize>{
        if output.is_empty()||self.closed{return Ok(0);}let capacity=output.len().min(64*1024);let Some(read)=self.provider.read else{return Err(self.failed(denied()));};
        let count=unsafe{read(self.provider.context(),self.token,output.as_mut_ptr(),capacity)};
        if count<0{return Err(self.failed(provider_error(count)));}if count as u64>capacity as u64{return Err(self.failed(PlatformError::new(ErrorCode::CorruptData,"Native disc read count exceeds its borrowed buffer")));}
        if count==0{let mismatch=self.length.is_some_and(|expected|self.seen!=expected);self.finish();if mismatch{return Err(io::Error::other(PlatformError::new(ErrorCode::ContentMismatch,"Native disc ended before its declared length")));}return Ok(0);}
        let Some(next)=self.seen.checked_add(count as u64).filter(|value|*value<=MAX_ROM_BYTES&&!self.length.is_some_and(|expected|*value>expected))else{return Err(self.failed(PlatformError::new(ErrorCode::ContentMismatch,"Native disc bytes exceed their declared or observed bound")));};self.seen=next;Ok(count as usize)
    }
}
impl Drop for NativeDiscReader{fn drop(&mut self){self.finish();}}
fn denied()->PlatformError{PlatformError::new(ErrorCode::PermissionDenied,"Native disc access is not authorized for this selected folder")}
fn provider_error(status:i64)->PlatformError{match status{-1=>PlatformError::new(ErrorCode::NotFound,"A referenced document was not found in the selected folder"),-2=>PlatformError::new(ErrorCode::Cancelled,"Native document reading was cancelled"),-3=>denied(),-4=>PlatformError::new(ErrorCode::DeadlineExceeded,"The document provider read deadline expired"),_=>PlatformError::new(ErrorCode::StorageFailure,"The selected document provider could not read this disc asset")}}
