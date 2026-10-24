//! Native transport ingress into the broker-owned decoder session. Cryptographic
//! authentication/replay decisions belong to the installed SRTP engine, never
//! packet fields, renderer flags, this queue or an ordinary UDP source address.
use crate::stream_video_registry::{ReceiveRegistry,ReceiveDocument};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_stream_input::video_client::{DecoderReceipt,VideoOwner};
use std::time::Instant;

pub trait NativeSrtpReceiver{
    /// Exact authorized negotiated media generation retained by this engine.
    fn owner(&self)->VideoOwner;
    /// Authenticate tag/peer binding, decrypt and reject replay BEFORE returning
    /// RTP. None means an authenticated duplicate/control packet with no media.
    /// A failed tag must be an error, not a plaintext fallback. Input receipt is
    /// the original ingress stamp, never captured after cryptographic work.
    fn receive(&mut self,datagram:&[u8],receipt:DecoderReceipt)->Result<Option<Vec<u8>>>;
    /// Cancel/join receive, ICE/DTLS work and retire retained key/allocations.
    /// Success is actual retirement, not a request flag. No default backend.
    fn retire_until(&mut self,deadline:Instant)->Result<()>;
}
pub struct VideoIngress<T:NativeSrtpReceiver>{owner:VideoOwner,transport:T,closed:bool,retired:bool,video_retired:bool}
impl<T:NativeSrtpReceiver> VideoIngress<T>{
    pub fn new(owner:VideoOwner,transport:T)->Result<Self>{
        if transport.owner()!=owner||[owner.capture,owner.game,owner.pair,owner.session].iter().any(|id|id.is_nil())||!(1..=1_000_000).contains(&owner.generation){return Err(denied());}
        Ok(Self{owner,transport,closed:false,retired:false,video_retired:false})
    }
    /// Call directly at native datagram ingress; authentication and every later
    /// receive stage share this stamp. No raw datagram enters decoder APIs.
    pub fn receive(&mut self,registry:&mut ReceiveRegistry,document:&ReceiveDocument,datagram:&[u8],receipt:DecoderReceipt)->Result<()>{
        let result=(||{
            if self.closed||self.transport.owner()!=self.owner||datagram.is_empty()||datagram.len()>2048||!receipt.within_receive_age(){return Err(denied());}
            let media=self.transport.receive(datagram,receipt)?;
            if !receipt.within_receive_age()||self.transport.owner()!=self.owner{return Err(denied());}
            if let Some(rtp)=media{
                if !(13..=1400).contains(&rtp.len()){return Err(denied());}
                registry.push_authenticated(document,self.owner,&rtp,receipt)?;
            }Ok(())
        })();
        if result.is_err(){self.closed=true;}
        result
    }
    /// Fence ingress first and attempt both cleanup domains with one deadline.
    /// One failure never skips the other or releases its retained owner.
    pub fn retire_until(&mut self,registry:&mut ReceiveRegistry,document:&ReceiveDocument,deadline:Instant)->Result<()>{
        self.closed=true;
        let transport=if self.retired{Ok(())}else{self.transport.retire_until(deadline)};
        if transport.is_ok()&&Instant::now()<deadline{self.retired=true;}
        let video=if self.video_retired{Ok(())}else{registry.retire(document.id(),self.owner,deadline)};
        if video.is_ok(){self.video_retired=true;}
        transport?;video?;
        if !self.retired||Instant::now()>=deadline{return Err(denied());}Ok(())
    }
    pub fn transport_retired(&self)->bool{self.retired}
}
fn denied()->PlatformError{PlatformError::new(ErrorCode::PermissionDenied,"The authenticated native video ingress is unavailable or requires retirement")}
