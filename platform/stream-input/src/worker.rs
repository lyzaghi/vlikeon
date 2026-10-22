use crate::protocol::{self,Request,Reply,Action,Observation};
use std::sync::{Arc,atomic::{AtomicBool,Ordering},mpsc};
use std::time::{Duration,Instant};

#[cfg(not(windows))]pub fn run_worker()->i32{2}
#[cfg(windows)]pub fn run_worker()->i32{
    use crate::TargetInput;
    let closed=Arc::new(AtomicBool::new(false));let reader_closed=closed.clone();let(sender,receiver)=mpsc::sync_channel::<Request>(2);
    let reader=std::thread::Builder::new().name("omniplay-input-commands".into()).spawn(move||{
        let mut input=std::io::stdin().lock();while let Ok(Some(request))=protocol::read::<Request>(&mut input){if sender.send(request).is_err(){break;}}reader_closed.store(true,Ordering::Release);
    });if reader.is_err(){return 2;}
    let mut output=std::io::stdout().lock();let mut target:Option<TargetInput>=None;let mut owner=None;let mut previous=0;let start=Instant::now();
    loop{
        if closed.load(Ordering::Acquire){break;}
        if let Some(target)=target.as_mut(){target.tick();}
        let request=match receiver.recv_timeout(Duration::from_millis(8)){Ok(value)=>value,Err(mpsc::RecvTimeoutError::Timeout)=>{if target.is_none()&&start.elapsed()>Duration::from_secs(5){break;}continue;},Err(_)=>break};
        if request.protocol!=1||request.id<=previous||owner.is_some_and(|(grant,session)|(grant,session)!=(request.grant_id,request.session_id)){break;}previous=request.id;
        let mut stop=false;
        let result=match request.action{
            Action::Prepare{target:identity,expires_at_ms}=>{
                if target.is_some()||owner.is_some(){Err(())}else{
                    match TargetInput::open(identity,expires_at_ms){Ok(value)=>{target=Some(value);owner=Some((request.grant_id,request.session_id));Ok(())},Err(_)=>Err(())}
                }
            }
            Action::Input{sequence,frame}=>target.as_mut().ok_or(()).and_then(|target|target.apply(sequence,&frame).map_err(|_|())),
            Action::Release=>target.as_mut().ok_or(()).map(|target|target.release()),
            Action::Status=>if target.is_some(){Ok(())}else{Err(())},
            Action::Stop=>{stop=true;if let Some(target)=target.as_mut(){target.revoke("host_stopped");}Ok(())},
        };
        let observation=target.as_ref().map(|target|target.observation()).unwrap_or(Observation{state:"revoked".into(),reason:Some("target_unavailable".into())});
        if protocol::write(&Reply{protocol:1,id:request.id,status:if result.is_ok(){"ok"}else{"error"}.into(),observation},&mut output).is_err()||stop||result.is_err(){break;}
    }
    if let Some(mut target)=target{target.revoke("owner_disconnected");}0
}
