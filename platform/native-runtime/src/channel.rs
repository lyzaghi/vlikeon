use omniplay_contracts::{ErrorCode,PlatformError,Result,MAX_SAVE_BYTES};
use omniplay_sandbox::ContainedProcess;
use serde::{Deserialize,Serialize};
use serde_json::Value;
use std::{fs::File,io::{Read,Write},sync::{mpsc::{self,SyncSender},atomic::{AtomicBool,Ordering}},thread::JoinHandle,time::{Duration,Instant}};

#[derive(Serialize)]
struct Request {protocol:u16,id:u64,method:String,params:Value}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {protocol:u16,id:u64,status:String,result:Option<Value>,error_code:Option<String>}
pub struct Reply {pub result:Value,pub payload:Vec<u8>}
struct Call {request:Request,payload:Vec<u8>,reply:SyncSender<Result<Reply>>}
pub struct WorkerChannel {sender:Option<SyncSender<Call>>,thread:Option<JoinHandle<()>>,sequence:u64}

fn failed()->PlatformError{PlatformError::new(ErrorCode::RuntimeCrashed,"The contained native runtime stopped responding").recover("Continue from the most recent compatible save")}
pub(crate) trait WorkerProcess{fn has_exited(&self)->bool;fn terminate(&self);}
impl WorkerProcess for ContainedProcess{fn has_exited(&self)->bool{ContainedProcess::has_exited(self)}fn terminate(&self){ContainedProcess::terminate(self)}}
#[cfg(all(target_os="linux",target_arch="x86_64",target_env="gnu"))]
impl WorkerProcess for omniplay_sandbox::linux_launch::LinuxChild{fn has_exited(&self)->bool{omniplay_sandbox::linux_launch::LinuxChild::has_exited(self)}fn terminate(&self){omniplay_sandbox::linux_launch::LinuxChild::terminate(self)}}
impl WorkerChannel {
    pub fn abandon(&mut self){self.sender.take();self.thread.take();}
    pub fn new(mut input:File,mut output:File)->Result<Self> {
        let(sender,receiver)=mpsc::sync_channel::<Call>(4);
        let thread=std::thread::Builder::new().name("omniplay-native-ipc".into()).spawn(move||{
            while let Ok(call)=receiver.recv(){
                let result=exchange(&mut input,&mut output,&call.request,&call.payload);let terminal=result.as_ref().err().is_some_and(terminal_error);let _=call.reply.send(result);if terminal{break;}
            }
        }).map_err(|_|failed())?;
        Ok(Self{sender:Some(sender),thread:Some(thread),sequence:0})
    }
    pub fn call(&mut self,process:&impl WorkerProcess,method:&str,params:Value,payload:Vec<u8>,timeout:Duration)->Result<Reply> {
        self.call_cancellable(process,method,params,payload,timeout,&AtomicBool::new(false))
    }
    pub fn call_cancellable(&mut self,process:&impl WorkerProcess,method:&str,params:Value,payload:Vec<u8>,timeout:Duration,cancel:&AtomicBool)->Result<Reply> {
        if process.has_exited(){return Err(failed());}
        self.sequence=self.sequence.checked_add(1).ok_or_else(failed)?;let(reply,receive)=mpsc::sync_channel(1);
        let call=Call{request:Request{protocol:1,id:self.sequence,method:method.into(),params},payload,reply};
        self.sender.as_ref().ok_or_else(failed)?.try_send(call).map_err(|_|failed())?;
        let started=Instant::now();
        loop{
            if cancel.load(Ordering::Acquire){process.terminate();return Err(PlatformError::new(ErrorCode::Cancelled,"Runtime preparation was cancelled"));}
            let Some(remaining)=timeout.checked_sub(started.elapsed())else{process.terminate();return Err(failed());};
            match receive.recv_timeout(remaining.min(Duration::from_millis(100))){Ok(Ok(value))=>return Ok(value),Ok(Err(error))=>{if terminal_error(&error){process.terminate();}return Err(error)},Err(mpsc::RecvTimeoutError::Disconnected)=>{process.terminate();return Err(failed());},Err(mpsc::RecvTimeoutError::Timeout)=>continue}
        }
    }
}
fn terminal_error(error:&PlatformError)->bool{!matches!(error.code,ErrorCode::UnsupportedCapability|ErrorCode::IncompatibleSave|ErrorCode::InvalidState|ErrorCode::ResourceLimit|ErrorCode::StorageFailure|ErrorCode::PermissionDenied)}
impl Drop for WorkerChannel {fn drop(&mut self){self.sender.take();if let Some(thread)=self.thread.take(){let _=thread.join();}}}

fn exchange(input:&mut File,output:&mut File,request:&Request,payload:&[u8])->Result<Reply> {
    let encoded=serde_json::to_vec(request).map_err(|_|failed())?;
    if encoded.len()>64*1024||payload.len()as u64>MAX_SAVE_BYTES{return Err(failed());}
    input.write_all(&(encoded.len()as u32).to_le_bytes()).and_then(|_|input.write_all(&encoded)).and_then(|_|input.write_all(&(payload.len()as u32).to_le_bytes())).and_then(|_|input.write_all(payload)).and_then(|_|input.flush()).map_err(|_|failed())?;
    let maximum_payload=match request.method.as_str(){"save"=>MAX_SAVE_BYTES as usize,"measurement_read"=>36000*8,_=>0};
    let header=read_part(output,64*1024)?;let binary=read_part(output,maximum_payload)?;
    let response:Response=serde_json::from_slice(&header).map_err(|_|failed())?;
    if response.protocol!=1||response.id!=request.id{return Err(failed());}
    if response.status!="ok" {
        let code=match response.error_code.as_deref(){Some("unsupported_capability")=>ErrorCode::UnsupportedCapability,Some("incompatible_save")=>ErrorCode::IncompatibleSave,Some("invalid_state")=>ErrorCode::InvalidState,Some("resource_limit")=>ErrorCode::ResourceLimit,Some("storage_failure")=>ErrorCode::StorageFailure,Some("permission_denied")=>ErrorCode::PermissionDenied,Some("runtime_unavailable")=>ErrorCode::RuntimeUnavailable,Some("content_mismatch")=>ErrorCode::ContentMismatch,_=>ErrorCode::RuntimeCrashed};
        // Never forward a child's free-form error text, paths or ROM-controlled output.
        if code==ErrorCode::StorageFailure{return Err(PlatformError::new(code,"The game could not finish writing its persistent files; earlier committed saves remain available").recover("Check available storage and retry, or export an earlier saved version"));}
        if code==ErrorCode::PermissionDenied{return Err(PlatformError::new(code,"Focus the OmniPlay game window before resuming").recover("Select the game window and press Escape, or choose Resume again"));}
        return Err(PlatformError::new(code,"The native runtime could not complete this operation"));
    }
    Ok(Reply{result:response.result.ok_or_else(failed)?,payload:binary})
}
fn read_part(source:&mut File,maximum:usize)->Result<Vec<u8>> {
    let mut length=[0u8;4];source.read_exact(&mut length).map_err(|_|failed())?;let length=u32::from_le_bytes(length)as usize;
    if length>maximum{return Err(failed());}
    // An unread response after allocation failure is terminal; a fully framed
    // resource-limit reply above leaves the healthy protocol usable.
    let mut bytes=Vec::new();bytes.try_reserve_exact(length).map_err(|_|failed())?;bytes.resize(length,0);source.read_exact(&mut bytes).map_err(|_|failed())?;Ok(bytes)
}
