use crate::{gamepad_host::{GamepadHost,ParentLease,Phase,StopReason},
    gamepad_protocol::{self as wire,Action,Identity,Reply,Request},
    gamepad_windows_host::WindowsGamepadGuard,gamepad_winuhid::WinUHidProvider,protocol};
use std::collections::BTreeSet;
use std::sync::{Arc,OnceLock,atomic::{AtomicBool,Ordering},mpsc};
use std::time::{Duration,Instant};

struct ReaderControl{closed:AtomicBool,lease:OnceLock<ParentLease>}
impl ReaderControl{
    fn close(&self){self.closed.store(true,Ordering::Release);if let Some(lease)=self.lease.get(){lease.close();}}
}
fn phase(value:Phase)->&'static str{match value{
    Phase::AwaitingPrepare=>"waiting_for_target",Phase::Prepared=>"prepared",
    Phase::Active=>"active",Phase::CleanupRequired=>"cleanup_required",Phase::Revoked=>"revoked",
}}
fn reason(value:StopReason)->&'static str{match value{
    StopReason::ParentLeaseLost=>"parent_lease_lost",StopReason::HostStopped=>"host_stopped",
    StopReason::TargetLost=>"target_lost",StopReason::LocalOverride=>"local_override",
    StopReason::Deadline=>"deadline",StopReason::InputGap=>"input_gap",
    StopReason::Rejected=>"rejected",StopReason::ProviderFailure=>"provider_failure",
}}

/// Source candidate only. No current broker launches this branch. The future
/// supervisor must admit this exact binary/driver and retain prepared identities
/// before sending Activate. A process exit is not a device-removal receipt.
pub fn run()->i32{
    let control=Arc::new(ReaderControl{closed:AtomicBool::new(false),lease:OnceLock::new()});
    let reader_control=control.clone();let(sender,receiver)=mpsc::sync_channel::<Request>(2);
    let reader=std::thread::Builder::new().name("vlikeon-gamepad-parent".into()).spawn(move||{
        let mut input=std::io::stdin().lock();let mut previous=0;let mut owner=None;
        while let Ok(Some(request))=protocol::read::<Request>(&mut input){
            let actual=request.binding.owner();
            if request.protocol!=wire::VERSION||request.id==0||request.id>wire::MAX_ID||
               request.id<=previous||!actual.valid()||owner.is_some_and(|expected|expected!=actual){break;}
            previous=request.id;
            if owner.is_none(){
                if !matches!(&request.action,Action::Prepare{..}){break;}
                owner=Some(actual);
            }
            if let Action::Heartbeat{document_alive,capture_alive,pair_authorized}=&request.action{
                let Some(lease)=reader_control.lease.get()else{break;};
                if lease.refresh(&actual,request.id,*document_alive,*capture_alive,*pair_authorized).is_err(){break;}
                continue;
            }
            // Never block the independent lease reader behind a busy provider.
            // Bounded command congestion closes authority instead of retaining
            // an old heartbeat indefinitely in a channel send().
            if sender.try_send(request).is_err(){break;}
        }
        reader_control.close();
    });
    if reader.is_err(){return 2;}
    let first=match receiver.recv_timeout(Duration::from_secs(5)){
        Ok(value)=>value,Err(_)=>{control.close();return 2;}
    };
    let owner=first.binding.owner();
    let(slots,remaining_ms,expires_at_ms,consent,vendor,product)=match first.action{
        Action::Prepare{slots,remaining_ms,expires_at_ms,system_visible_consent,vendor,product}=>
            (slots,remaining_ms,expires_at_ms,system_visible_consent,vendor,product),
        _=>{control.close();return 2;}
    };
    if !consent||remaining_ms==0||remaining_ms>30*60*1000||slots.is_empty()||slots.len()>4||
       slots.iter().any(|slot|*slot>3)||slots.iter().collect::<BTreeSet<_>>().len()!=slots.len(){
        control.close();return 2;
    }
    let now=match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH){
        Ok(value)=>value.as_millis().min(u128::from(u64::MAX))as u64,
        Err(_)=>{control.close();return 2;}
    };
    if expires_at_ms<=now{control.close();return 2;}
    // Both the absolute grant/capture expiry and the parent's bounded remaining
    // lifetime constrain the monotonic deadline. Pipe delivery and guard setup
    // cannot silently extend the original native authorization.
    let deadline=Instant::now()+Duration::from_millis((expires_at_ms-now).min(u64::from(remaining_ms)));
    let guard=match WindowsGamepadGuard::open(owner.target){Ok(value)=>value,Err(_)=>{control.close();return 2;}};
    let provider=match WinUHidProvider::new(vendor,product){Ok(value)=>value,Err(_)=>{control.close();return 2;}};
    let(mut host,lease)=match GamepadHost::new(provider,guard,owner,deadline.saturating_duration_since(Instant::now())){
        Ok(value)=>value,Err(_)=>{control.close();return 2;}
    };
    if control.lease.set(lease).is_err(){control.close();return 2;}
    let mut output=std::io::stdout().lock();
    let mut prepared=false;let mut prepare_request=Some(first.id);let mut terminal_since=None;
    let initial=Reply{protocol:wire::VERSION,id:first.id,binding:owner.into(),status:"ok".into(),
        complete:false,phase:"waiting_for_target".into(),reason:None,identities:Some(Vec::new())};
    if protocol::write(&initial,&mut output).is_err(){control.close();let _=host.revoke();return 2;}
    loop{
        if control.closed.load(Ordering::Acquire){let _=host.revoke();break;}
        let _=host.poll();
        // Preparation gets a separate receipt (same initial request id) after
        // focus/quiet readiness. Parent must keep reading that bounded response
        // while sending independent heartbeats; it cannot treat waiting as ready.
        if !prepared&&host.phase()==Phase::AwaitingPrepare&&host.ready_for_prepare().unwrap_or(false){
            prepared=true;
            let result=host.prepare(&slots,consent);
            if result.is_err(){let _=host.revoke();}
            let identities=host.cleanup_identities().ok().map(|ids|ids.into_iter().map(|(slot,container)|
                Identity{slot,container}).collect());
            let reply=Reply{protocol:wire::VERSION,id:prepare_request.take().unwrap_or(first.id),
                binding:owner.into(),status:if result.is_ok(){"ok"}else{"error"}.into(),
                complete:true,phase:phase(host.phase()).into(),reason:host.stop_reason().map(|value|reason(value).into()),identities};
            if protocol::write(&reply,&mut output).is_err(){control.close();let _=host.revoke();break;}
        }
        if matches!(host.phase(),Phase::Revoked|Phase::CleanupRequired){
            if let Some(id)=prepare_request.take(){
                let identities=host.cleanup_identities().ok().map(|ids|ids.into_iter().map(|(slot,container)|Identity{slot,container}).collect());
                let reply=Reply{protocol:wire::VERSION,id,binding:owner.into(),status:"error".into(),complete:true,
                    phase:phase(host.phase()).into(),reason:host.stop_reason().map(|value|reason(value).into()),identities};
                if protocol::write(&reply,&mut output).is_err(){control.close();let _=host.revoke();break;}
            }
            let started=*terminal_since.get_or_insert_with(Instant::now);
            if started.elapsed()>=Duration::from_secs(5){control.close();break;}
        }
        let request=match receiver.recv_timeout(Duration::from_millis(8)){
            Ok(value)=>value,Err(mpsc::RecvTimeoutError::Timeout)=>continue,Err(_)=>{control.close();let _=host.revoke();break;}
        };
        let mut stop=false;
        let result=match request.action{
            Action::Activate{identities}=>{
                if identities.len()>4{let _=host.revoke();Err(crate::failure())}else{
                    let ids:Vec<_>=identities.into_iter().map(|id|(id.slot,id.container)).collect();host.activate(&ids)
                }
            },
            Action::Input{sequence,pads}=>{
                if pads.len()>4{let _=host.revoke();Err(crate::failure())}else{
                    let frames:Vec<_>=pads.into_iter().map(|pad|(pad.slot,pad.state)).collect();host.input(sequence,&frames)
                }
            },
            Action::Release{sequence}=>host.release(sequence),
            Action::Status=>Ok(()),
            Action::Stop=>{stop=true;host.revoke()},
            _=>{let _=host.revoke();Err(crate::failure())},
        };
        let identities=host.cleanup_identities().ok().map(|ids|ids.into_iter().map(|(slot,container)|Identity{slot,container}).collect());
        let reply=Reply{protocol:wire::VERSION,id:request.id,binding:owner.into(),
            status:if result.is_ok(){"ok"}else{"error"}.into(),complete:true,phase:phase(host.phase()).into(),
            reason:host.stop_reason().map(|value|reason(value).into()),identities};
        if protocol::write(&reply,&mut output).is_err(){control.close();let _=host.revoke();break;}
        if stop{control.close();break;}
    }
    let _=host.revoke();
    if host.phase()==Phase::CleanupRequired{3}else{0}
}
