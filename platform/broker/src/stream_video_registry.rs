//! Broker-owned native receive slot. Exact native scope only; no renderer RPC.
//! Failed retirement retains its session for retry instead of replacing it.
use crate::{stream_video_session::{WindowsReceiveSession,WindowsReceiveConfiguration},
    stream_video_authority::NativeVideoAuthority,stream_video_receive::ReceiveState,stream_video::UnpresentedVideoFrame};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_stream_input::video_client::{VideoOwner,DecoderReceipt};
use std::time::Instant;
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
struct Entry{document:Uuid,owner:VideoOwner,session:Box<dyn ReceiveOwner>}
#[derive(Default)]
pub struct ReceiveRegistry{entry:Option<Entry>,closed:bool}
impl ReceiveRegistry{
    pub fn open(&mut self,document:Uuid,configuration:WindowsReceiveConfiguration,source:impl NativeVideoAuthority,deadline:Instant)->Result<()>{
        if self.closed||document.is_nil(){return Err(denied());}
        if self.entry.is_some(){return Err(PlatformError::new(ErrorCode::Busy,"The previous native receive session must retire before replacement"));}
        let owner=configuration.owner;
        let session=WindowsReceiveSession::open(configuration,source,deadline)?;
        self.entry=Some(Entry{document,owner,session:Box::new(session)});Ok(())
    }
    fn exact(&mut self,document:Uuid,owner:VideoOwner)->Result<&mut Entry>{
        if self.closed{return Err(denied());}
        self.entry.as_mut().filter(|entry|entry.document==document&&entry.owner==owner).ok_or_else(denied)
    }
    pub fn push_authenticated(&mut self,document:Uuid,owner:VideoOwner,bytes:&[u8],receipt:DecoderReceipt)->Result<()>{self.exact(document,owner)?.session.push(bytes,receipt)}
    pub fn tick(&mut self,document:Uuid,owner:VideoOwner)->Result<()>{self.exact(document,owner)?.session.tick()}
    pub fn take_unpresented(&mut self,document:Uuid,owner:VideoOwner)->Result<Option<UnpresentedVideoFrame>>{self.exact(document,owner)?.session.take()}
    pub fn state(&self,document:Uuid,owner:VideoOwner)->Result<ReceiveState>{
        self.entry.as_ref().filter(|entry|entry.document==document&&entry.owner==owner).map(|entry|entry.session.state()).ok_or_else(denied)
    }
    pub fn retire(&mut self,document:Uuid,owner:VideoOwner,deadline:Instant)->Result<()>{
        let entry=self.entry.as_mut().filter(|entry|entry.document==document&&entry.owner==owner).ok_or_else(denied)?;
        entry.session.close(deadline)?;
        if entry.session.state()!=ReceiveState::Revoked{return Err(denied());}
        self.entry.take();Ok(())
    }
    /// Permanently closes admission, but keeps uncertain cleanup reachable for
    /// subsequent native retire/close calls. No TTL or owner liveness shortcut.
    pub fn close_until(&mut self,deadline:Instant)->Result<()>{
        self.closed=true;
        if let Some(entry)=self.entry.as_mut(){
            entry.session.close(deadline)?;
            if entry.session.state()!=ReceiveState::Revoked{return Err(denied());}
            self.entry.take();
        }Ok(())
    }
}
