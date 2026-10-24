//! Broker-owned native receive slot. Exact native scope only; no renderer RPC.
//! Failed retirement retains its session for retry instead of replacing it.
use crate::{stream_video_session::{WindowsReceiveSession,WindowsReceiveConfiguration},
    stream_video_authority::NativeVideoAuthority,stream_video_receive::ReceiveState,stream_video::UnpresentedVideoFrame};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_stream_input::video_client::{VideoOwner,DecoderReceipt,VideoAuthorityObservation};
use std::{time::Instant,sync::{Arc,Weak,atomic::{AtomicBool,Ordering}}};
use uuid::Uuid;

fn denied()->PlatformError{PlatformError::new(ErrorCode::PermissionDenied,"The native receive slot belongs to another document or media generation")}
trait ReceiveOwner{
    fn push(&mut self,bytes:&[u8],receipt:DecoderReceipt)->Result<()>;
    fn tick(&mut self)->Result<()>;
    fn take(&mut self)->Result<Option<UnpresentedVideoFrame>>;
    fn close(&mut self,deadline:Instant)->Result<()>;
    fn state(&self)->ReceiveState;
}
impl<S:NativeVideoAuthority> ReceiveOwner for WindowsReceiveSession<S>{
    fn push(&mut self,bytes:&[u8],receipt:DecoderReceipt)->Result<()>{self.push_authenticated(bytes,receipt)}
    fn tick(&mut self)->Result<()>{WindowsReceiveSession::tick(self)}
    fn take(&mut self)->Result<Option<UnpresentedVideoFrame>>{self.take_unpresented()}
    fn close(&mut self,deadline:Instant)->Result<()>{self.close_until(deadline)}
    fn state(&self)->ReceiveState{WindowsReceiveSession::state(self)}
}
/// Owned by the trusted native document lifecycle. Closing or dropping the
/// final owner fences delivery immediately without waiting for broker cleanup.
pub struct ReceiveDocument{state:Arc<DocumentState>}
struct DocumentState{id:Uuid,alive:AtomicBool}
impl ReceiveDocument{
    pub fn new()->Self{Self{state:Arc::new(DocumentState{id:Uuid::new_v4(),alive:AtomicBool::new(true)})}}
    pub fn close(&self){self.state.alive.store(false,Ordering::Release);}
    pub fn id(&self)->Uuid{self.state.id}
}
impl Drop for ReceiveDocument{fn drop(&mut self){self.close();}}
struct DocumentAuthority<S:NativeVideoAuthority>{document:Weak<DocumentState>,source:S}
impl<S:NativeVideoAuthority> NativeVideoAuthority for DocumentAuthority<S>{
    fn observe(&mut self,owner:VideoOwner)->Result<VideoAuthorityObservation>{
        let document=self.document.upgrade().ok_or_else(denied)?;
        if !document.alive.load(Ordering::Acquire){return Err(denied());}
        // Preserve the source's actual timestamp; a slow observe cannot refresh
        // admission, and closing the document during it invalidates the result.
        let observation=self.source.observe(owner)?;
        if !document.alive.load(Ordering::Acquire){return Err(denied());}
        Ok(observation)
    }
}
struct Entry{document:Weak<DocumentState>,document_id:Uuid,owner:VideoOwner,sealed:bool,session:Box<dyn ReceiveOwner>}
#[derive(Default)]
pub struct ReceiveRegistry{entry:Option<Entry>,closed:bool}
impl ReceiveRegistry{
    pub fn open(&mut self,document:&ReceiveDocument,configuration:WindowsReceiveConfiguration,source:impl NativeVideoAuthority,deadline:Instant)->Result<()>{
        if self.closed||!document.state.alive.load(Ordering::Acquire){return Err(denied());}
        if self.entry.is_some(){return Err(PlatformError::new(ErrorCode::Busy,"The previous native receive session must retire before replacement"));}
        let owner=configuration.owner;
        let source=DocumentAuthority{document:Arc::downgrade(&document.state),source};
        let session=WindowsReceiveSession::open(configuration,source,deadline)?;
        self.entry=Some(Entry{document:Arc::downgrade(&document.state),document_id:document.state.id,owner,sealed:false,session:Box::new(session)});
        if !document.state.alive.load(Ordering::Acquire){
            if let Some(entry)=self.entry.as_mut(){entry.sealed=true;let _=entry.session.close(deadline);}return Err(denied());
        }Ok(())
    }
    fn exact(&mut self,document:&ReceiveDocument,owner:VideoOwner)->Result<&mut Entry>{
        if self.closed{return Err(denied());}
        self.entry.as_mut().filter(|entry|entry.document_id==document.state.id&&entry.owner==owner&&!entry.sealed&&
            entry.document.upgrade().is_some_and(|state|Arc::ptr_eq(&state,&document.state)&&state.alive.load(Ordering::Acquire))).ok_or_else(denied)
    }
    pub fn push_authenticated(&mut self,document:&ReceiveDocument,owner:VideoOwner,bytes:&[u8],receipt:DecoderReceipt)->Result<()>{self.exact(document,owner)?.session.push(bytes,receipt)}
    pub fn tick(&mut self,document:&ReceiveDocument,owner:VideoOwner)->Result<()>{self.exact(document,owner)?.session.tick()}
    pub fn take_unpresented(&mut self,document:&ReceiveDocument,owner:VideoOwner)->Result<Option<UnpresentedVideoFrame>>{
        let result=self.exact(document,owner)?.session.take()?;
        self.exact(document,owner)?;Ok(result)
    }
    pub fn state(&self,document:Uuid,owner:VideoOwner)->Result<ReceiveState>{
        self.entry.as_ref().filter(|entry|entry.document_id==document&&entry.owner==owner).map(|entry|
            if (entry.sealed||entry.document.upgrade().is_none_or(|state|!state.alive.load(Ordering::Acquire)))&&entry.session.state()==ReceiveState::Active{ReceiveState::CleanupRequired}else{entry.session.state()}).ok_or_else(denied)
    }
    pub fn retire(&mut self,document:Uuid,owner:VideoOwner,deadline:Instant)->Result<()>{
        let entry=self.entry.as_mut().filter(|entry|entry.document_id==document&&entry.owner==owner).ok_or_else(denied)?;
        entry.sealed=true;
        entry.session.close(deadline)?;
        if entry.session.state()!=ReceiveState::Revoked{return Err(denied());}
        self.entry.take();Ok(())
    }
    /// Permanently closes admission, but keeps uncertain cleanup reachable for
    /// subsequent native retire/close calls. No TTL or owner liveness shortcut.
    pub fn close_until(&mut self,deadline:Instant)->Result<()>{
        self.closed=true;
        if let Some(entry)=self.entry.as_mut(){
            entry.sealed=true;
            entry.session.close(deadline)?;
            if entry.session.state()!=ReceiveState::Revoked{return Err(denied());}
            self.entry.take();
        }Ok(())
    }
    /// Called independently of packet arrival to reap closed native documents.
    pub fn maintain(&mut self,deadline:Instant)->Result<()>{
        if self.closed{return self.close_until(deadline);}
        if let Some(entry)=self.entry.as_mut(){
            if !entry.sealed&&entry.document.upgrade().is_some_and(|state|state.alive.load(Ordering::Acquire)){
                // Run receive-gap/assembly/age checks even without an RTP push.
                if entry.session.tick().is_err(){entry.sealed=true;}
            }
        }
        let lost=self.entry.as_ref().is_some_and(|entry|entry.sealed||entry.document.upgrade().is_none_or(|state|!state.alive.load(Ordering::Acquire)));
        if lost{
            let entry=self.entry.as_ref().ok_or_else(denied)?;
            let(document,owner)=(entry.document_id,entry.owner);self.retire(document,owner,deadline)?;
        }Ok(())
    }
}
