use crate::{failure,protocol::{self,Action,Observation,Reply,Request,Target}};
use omniplay_contracts::{RemoteInputFrame,Result};
use std::{path::Path,process::{Child,Command,Stdio},sync::mpsc::{self,SyncSender},thread::JoinHandle,time::{Duration,Instant}};
use uuid::Uuid;

struct Call{request:Request,reply:SyncSender<Result<Observation>>}
pub struct InputWorker{child:Child,sender:Option<SyncSender<Call>>,thread:Option<JoinHandle<()>>,sequence:u64,grant_id:Uuid,session_id:Uuid,closed:bool}
impl InputWorker{
    pub fn spawn(executable:&Path,target:Target,grant_id:Uuid,session_id:Uuid,expires_at_ms:u64)->Result<Self>{
        let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|_|failure())?.as_millis()as u64;
        if !crate::supported()||!executable.is_absolute()||expires_at_ms<=now||expires_at_ms-now>30*60*1000{return Err(failure());}
        let mut command=Command::new(executable);command.arg("--stdio-input-v1").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).env_clear();
        #[cfg(windows)]{
            use std::os::windows::process::CommandExt;
            let system=crate::system_directory()?;command.env("SystemRoot",&system).env("WINDIR",&system).env("PATH",system.join("System32")).creation_flags(0x08000000);
        }
        let mut child=command.spawn().map_err(|_|failure())?;let(mut input,mut output)=match(child.stdin.take(),child.stdout.take()){(Some(a),Some(b))=>(a,b),_=>{let _=child.kill();return Err(failure());}};
        let(sender,receive)=mpsc::sync_channel::<Call>(1);
        let thread=std::thread::Builder::new().name("omniplay-input-ipc".into()).spawn(move||{
            while let Ok(call)=receive.recv(){
                let result=(||{
                    protocol::write(&call.request,&mut input).map_err(|_|failure())?;let reply:Reply=protocol::read(&mut output).map_err(|_|failure())?.ok_or_else(failure)?;
                    if reply.protocol!=1||reply.id!=call.request.id||reply.status!="ok"||!["waiting_for_target","active","idle","revoked"].contains(&reply.observation.state.as_str())||reply.observation.reason.as_ref().is_some_and(|r|r.is_empty()||r.len()>64||!r.bytes().all(|c|c.is_ascii_lowercase()||c==b'_')){return Err(failure());}Ok(reply.observation)
                })();let terminal=result.is_err();let _=call.reply.send(result);if terminal{break;}
            }
            // Closing this private stdin pipe is the helper's independent signal
            // to release held input, even if the owning broker died unexpectedly.
            drop(input);
        }).map_err(|_|{let _=child.kill();failure()})?;
        let mut worker=Self{child,sender:Some(sender),thread:Some(thread),sequence:0,grant_id,session_id,closed:false};worker.call(Action::Prepare{target,expires_at_ms},Duration::from_secs(5))?;Ok(worker)
    }
    fn call(&mut self,action:Action,timeout:Duration)->Result<Observation>{
        if self.closed||self.child.try_wait().map_err(|_|failure())?.is_some(){return Err(failure());}
        self.sequence=self.sequence.checked_add(1).ok_or_else(failure)?;let(reply,receive)=mpsc::sync_channel(1);let request=Request{protocol:1,id:self.sequence,grant_id:self.grant_id,session_id:self.session_id,action};
        self.sender.as_ref().ok_or_else(failure)?.try_send(Call{request,reply}).map_err(|_|failure())?;
        receive.recv_timeout(timeout).map_err(|_|failure())?
    }
    pub fn input(&mut self,sequence:u64,frame:RemoteInputFrame)->Result<Observation>{frame.validate()?;self.call(Action::Input{sequence,frame},Duration::from_millis(750))}
    pub fn status(&mut self)->Result<Observation>{self.call(Action::Status,Duration::from_millis(750))}
    pub fn release(&mut self)->Result<Observation>{self.call(Action::Release,Duration::from_millis(750))}
    /// A process exit or a successful Stop RPC without a released-state
    /// observation is not proof that tracked keyboard/mouse input was released.
    pub fn stop_confirmed(&mut self)->Result<Observation>{
        if self.closed{return Err(failure());}
        let observed=self.call(Action::Stop,Duration::from_millis(750));self.closed=true;self.sender.take();
        let start=Instant::now();while start.elapsed()<Duration::from_secs(2){if self.child.try_wait().ok().flatten().is_some(){break;}std::thread::sleep(Duration::from_millis(20));}
        if self.child.try_wait().ok().flatten().is_none(){let _=self.child.kill();}let _=self.child.wait();if let Some(thread)=self.thread.take(){let _=thread.join();}
        observed.and_then(|value|if value.state=="revoked"&&value.reason.as_deref()!=Some("release_failed"){Ok(value)}else{Err(failure())})
    }
    pub fn close(&mut self){let _=self.stop_confirmed();}
}
impl Drop for InputWorker{fn drop(&mut self){self.close();}}
