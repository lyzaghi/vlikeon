use omniplay_contracts::{ErrorCode,PlatformError,Result,MAX_SAVE_BYTES};
use omniplay_sandbox::ContainedProcess;
use serde::{Deserialize,Serialize};
use serde_json::Value;
use std::{fs::File,io::{Read,Write},sync::{mpsc::{self,SyncSender},atomic::{AtomicBool,Ordering}},thread::JoinHandle,time::{Duration,Instant}};

#[derive(Serialize)]
struct Request {protocol:u16,id:u64,method:String,params:Value}
#[derive(Deserialize)]
#[serde(tag="status",deny_unknown_fields)]
enum Response {
    #[serde(rename="ok")]Success{protocol:u16,id:u64,result:Value},
    #[serde(rename="error")]Error{protocol:u16,id:u64,error_code:ChildError},
}
#[derive(Deserialize)]
#[serde(rename_all="snake_case")]
enum ChildError{UnsupportedCapability,IncompatibleSave,InvalidState,ResourceLimit,StorageFailure,PermissionDenied,RuntimeUnavailable,ContentMismatch,RuntimeCrashed}
pub struct Reply {pub result:Value,pub payload:Vec<u8>}
struct Call {id:u64,method:String,encoded:Vec<u8>,payload:Vec<u8>,reply:SyncSender<Result<Reply>>}
pub struct WorkerChannel {sender:Option<SyncSender<Call>>,thread:Option<JoinHandle<()>>,sequence:u64}

fn failed()->PlatformError{PlatformError::new(ErrorCode::RuntimeCrashed,"The contained native runtime stopped responding").recover("Continue from the most recent compatible save")}
const MAX_HEADER:usize=64*1024;
const MAX_SEQUENCE:u64=i64::MAX as u64;
fn invalid()->PlatformError{PlatformError::new(ErrorCode::InvalidRequest,"The native control request is outside its protocol")}
fn request_payload_limit(method:&str)->Option<usize>{match method{
    "load"=>Some(MAX_SAVE_BYTES as usize),
    "hello"|"prepare"|"start"|"resume"|"pause"|"cancel_close"|"status"|"metrics"|"measurement_start"|"measurement_status"|"measurement_cancel"|"measurement_read"|"disc_status"|"disc_select"|"save"|"stop"=>Some(0),_=>None,
}}
fn valid_depth(value:&Value,depth:usize)->bool{depth<=32&&match value{Value::Array(values)=>values.iter().all(|value|valid_depth(value,depth+1)),Value::Object(values)=>values.values().all(|value|valid_depth(value,depth+1)),_=>true}}
// Bound allocation during serialization, rather than checking an unbounded
// to_vec allocation afterward. Invalid local requests never enter the pipe.
struct HeaderBuffer(Vec<u8>);
impl Write for HeaderBuffer{
    fn write(&mut self,bytes:&[u8])->std::io::Result<usize>{
        if bytes.len()>MAX_HEADER-self.0.len(){return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput,"native_header_bound"));}
        let required=self.0.len()+bytes.len();
        if required>self.0.capacity(){
            let capacity=self.0.capacity().saturating_mul(2).max(required).max(256).min(MAX_HEADER);
            self.0.try_reserve_exact(capacity-self.0.len()).map_err(|_|std::io::Error::new(std::io::ErrorKind::Other,"native_header_allocation"))?;
        }
        self.0.extend_from_slice(bytes);Ok(bytes.len())
    }
    fn flush(&mut self)->std::io::Result<()>{Ok(())}
}
fn encode_request(request:&Request,payload_bytes:usize)->Result<Vec<u8>>{
    let limit=request_payload_limit(&request.method).ok_or_else(invalid)?;
    if request.protocol!=1||request.id==0||request.id>MAX_SEQUENCE||!request.params.is_object()||!valid_depth(&request.params,1){return Err(invalid());}
    if payload_bytes>limit{return Err(PlatformError::new(ErrorCode::ResourceLimit,"The native control payload exceeds its method bound"));}
    let mut buffer=HeaderBuffer(Vec::new());serde_json::to_writer(&mut buffer,request).map_err(|_|PlatformError::new(ErrorCode::ResourceLimit,"The native control header exceeds its memory bound"))?;Ok(buffer.0)
}
pub(crate) trait WorkerProcess{fn has_exited(&self)->bool;fn terminate(&self);}
impl WorkerProcess for ContainedProcess{fn has_exited(&self)->bool{ContainedProcess::has_exited(self)}fn terminate(&self){ContainedProcess::terminate(self)}}
#[cfg(all(target_os="linux",target_arch="x86_64",target_env="gnu"))]
impl WorkerProcess for omniplay_sandbox::linux_launch::LinuxChild{fn has_exited(&self)->bool{omniplay_sandbox::linux_launch::LinuxChild::has_exited(self)}fn terminate(&self){omniplay_sandbox::linux_launch::LinuxChild::terminate(self)}}
impl WorkerChannel {
    pub fn abandon(&mut self){self.sender.take();self.thread.take();}
    fn invalidate(&mut self,process:&impl WorkerProcess){self.sender.take();process.terminate();}
    pub fn new(mut input:File,mut output:File)->Result<Self> {
        let(sender,receiver)=mpsc::sync_channel::<Call>(4);
        let thread=std::thread::Builder::new().name("omniplay-native-ipc".into()).spawn(move||{
            while let Ok(call)=receiver.recv(){
                let result=exchange(&mut input,&mut output,call.id,&call.method,&call.encoded,&call.payload);let terminal=result.as_ref().err().is_some_and(terminal_error);let _=call.reply.send(result);if terminal{break;}
            }
        }).map_err(|_|failed())?;
        Ok(Self{sender:Some(sender),thread:Some(thread),sequence:0})
    }
    pub fn call(&mut self,process:&impl WorkerProcess,method:&str,params:Value,payload:Vec<u8>,timeout:Duration)->Result<Reply> {
        self.call_cancellable(process,method,params,payload,timeout,&AtomicBool::new(false))
    }
    pub fn call_cancellable(&mut self,process:&impl WorkerProcess,method:&str,params:Value,payload:Vec<u8>,timeout:Duration,cancel:&AtomicBool)->Result<Reply> {
        if self.sender.is_none(){return Err(failed());}
        if process.has_exited(){self.sender.take();return Err(failed());}
        if timeout.is_zero()||timeout>Duration::from_secs(300)||request_payload_limit(method).is_none(){return Err(invalid());}
        if cancel.load(Ordering::Acquire){self.invalidate(process);return Err(PlatformError::new(ErrorCode::Cancelled,"Runtime preparation was cancelled"));}
        let Some(sequence)=self.sequence.checked_add(1).filter(|value|*value<=MAX_SEQUENCE)else{self.invalidate(process);return Err(failed());};
        let request=Request{protocol:1,id:sequence,method:method.into(),params};let encoded=encode_request(&request,payload.len())?;
        let(reply,receive)=mpsc::sync_channel(1);let call=Call{id:request.id,method:request.method,encoded,payload,reply};
        let sent=self.sender.as_ref().ok_or_else(failed)?.try_send(call);
        if sent.is_err(){self.invalidate(process);return Err(failed());}self.sequence=sequence;
        let started=Instant::now();
        loop{
            if cancel.load(Ordering::Acquire){self.invalidate(process);return Err(PlatformError::new(ErrorCode::Cancelled,"Runtime preparation was cancelled"));}
            let Some(remaining)=timeout.checked_sub(started.elapsed())else{self.invalidate(process);return Err(failed());};
            match receive.recv_timeout(remaining.min(Duration::from_millis(100))){Ok(Ok(value))=>return Ok(value),Ok(Err(error))=>{if terminal_error(&error){self.invalidate(process);}return Err(error)},Err(mpsc::RecvTimeoutError::Disconnected)=>{self.invalidate(process);return Err(failed());},Err(mpsc::RecvTimeoutError::Timeout)=>continue}
        }
    }
}
fn terminal_error(error:&PlatformError)->bool{!matches!(error.code,ErrorCode::UnsupportedCapability|ErrorCode::IncompatibleSave|ErrorCode::InvalidState|ErrorCode::ResourceLimit|ErrorCode::StorageFailure|ErrorCode::PermissionDenied)}
impl Drop for WorkerChannel {fn drop(&mut self){self.sender.take();if let Some(thread)=self.thread.take(){let _=thread.join();}}}

fn exchange(input:&mut impl Write,output:&mut impl Read,id:u64,method:&str,encoded:&[u8],payload:&[u8])->Result<Reply> {
    input.write_all(&(encoded.len()as u32).to_le_bytes()).and_then(|_|input.write_all(encoded)).and_then(|_|input.write_all(&(payload.len()as u32).to_le_bytes())).and_then(|_|input.write_all(payload)).and_then(|_|input.flush()).map_err(|_|failed())?;
    decode_response(output,id,method)
}
fn decode_response(output:&mut impl Read,id:u64,method:&str)->Result<Reply>{
    // No payload length/body is read until the closed envelope and identity
    // have passed. Error envelopes always require an empty binary frame.
    let header=read_part(output,MAX_HEADER)?;let response:Response=serde_json::from_slice(&header).map_err(|_|failed())?;
    match response{
        Response::Success{protocol,id:received,result}=>{
            if protocol!=1||received!=id||!result.is_object(){return Err(failed());}
            let maximum=match method{"save"=>MAX_SAVE_BYTES as usize,"measurement_read"=>36000*8,_=>0};
            let payload=read_part(output,maximum)?;
            if matches!(method,"save"|"measurement_read")&&payload.is_empty(){return Err(failed());}
            Ok(Reply{result,payload})
        },
        Response::Error{protocol,id:received,error_code}=>{
            if protocol!=1||received!=id{return Err(failed());}read_part(output,0)?;Err(child_error(error_code))
        },
    }
}
fn child_error(error:ChildError)->PlatformError{
    let code=match error{ChildError::UnsupportedCapability=>ErrorCode::UnsupportedCapability,ChildError::IncompatibleSave=>ErrorCode::IncompatibleSave,ChildError::InvalidState=>ErrorCode::InvalidState,ChildError::ResourceLimit=>ErrorCode::ResourceLimit,ChildError::StorageFailure=>ErrorCode::StorageFailure,ChildError::PermissionDenied=>ErrorCode::PermissionDenied,ChildError::RuntimeUnavailable=>ErrorCode::RuntimeUnavailable,ChildError::ContentMismatch=>ErrorCode::ContentMismatch,ChildError::RuntimeCrashed=>ErrorCode::RuntimeCrashed};
    // Never forward a child's free-form error text, paths or game output.
    if code==ErrorCode::StorageFailure{return PlatformError::new(code,"The game could not finish writing its persistent files; earlier committed saves remain available").recover("Check available storage and retry, or export an earlier saved version");}
    if code==ErrorCode::PermissionDenied{return PlatformError::new(code,"Focus the Vlikeon game window before resuming").recover("Select the game window and press Escape, or choose Resume again");}
    PlatformError::new(code,"The native runtime could not complete this operation")
}
fn read_part(source:&mut impl Read,maximum:usize)->Result<Vec<u8>> {
    let mut length=[0u8;4];source.read_exact(&mut length).map_err(|_|failed())?;let length=u32::from_le_bytes(length)as usize;
    if length>maximum{return Err(failed());}
    // An unread response after allocation failure is terminal; a fully framed
    // resource-limit reply above leaves the healthy protocol usable.
    let mut bytes=Vec::new();bytes.try_reserve_exact(length).map_err(|_|failed())?;bytes.resize(length,0);source.read_exact(&mut bytes).map_err(|_|failed())?;Ok(bytes)
}

#[cfg(test)]
#[path="channel_tests.rs"]
mod tests;
