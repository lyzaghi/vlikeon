//! Native transport ingress into the broker-owned decoder session. Cryptographic
//! authentication/replay decisions belong to the installed SRTP engine, never
//! packet fields, renderer flags, this queue or an ordinary UDP source address.
use crate::stream_video_registry::{ReceiveRegistry,ReceiveDocument};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_stream_input::video_client::{DecoderReceipt,VideoOwner};
use std::{time::Instant,net::{UdpSocket,SocketAddr},io::ErrorKind};

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

/// Native OS receive adapter must return each datagram's actual ingress receipt
/// (including kernel queue residence), not capture a new clock after dequeue.
/// Windows ancillary timestamp/correlation implementation is still required.
pub trait NativeDatagramReceipt{
    fn received(&mut self,socket:&UdpSocket,datagram:&[u8])->Result<DecoderReceipt>;
}
/// Owns an already connected ICE-selected socket, never binds/connects to a
/// renderer-provided address. Fixed peer checks are not SRTP authentication.
pub struct UdpVideoIngress<T:NativeSrtpReceiver,C:NativeDatagramReceipt>{socket:Option<UdpSocket>,peer:SocketAddr,ingress:VideoIngress<T>,clock:C,closed:bool}
impl<T:NativeSrtpReceiver,C:NativeDatagramReceipt> UdpVideoIngress<T,C>{
    pub fn from_selected_socket(socket:UdpSocket,expected_peer:SocketAddr,owner:VideoOwner,transport:T,clock:C)->Result<Self>{
        if socket.peer_addr().map_err(|_|denied())?!=expected_peer||expected_peer.port()==0||expected_peer.ip().is_unspecified()||expected_peer.ip().is_multicast(){return Err(denied());}
        socket.set_nonblocking(true).map_err(|_|denied())?;
        let ingress=VideoIngress::new(owner,transport)?;
        Ok(Self{socket:Some(socket),peer:expected_peer,ingress,clock,closed:false})
    }
    /// Cooperative poll: no blocking receive, at most 32 datagrams per caller
    /// tick, no heap queue of encrypted packets. Caller retains original deadline.
    pub fn poll(&mut self,registry:&mut ReceiveRegistry,document:&ReceiveDocument,deadline:Instant)->Result<usize>{
        let result=(||{
            if self.closed||Instant::now()>=deadline{return Err(denied());}
            let socket=self.socket.as_ref().ok_or_else(denied)?;
            if socket.peer_addr().map_err(|_|denied())?!=self.peer{return Err(denied());}
            // Full UDP length detects oversized datagrams without accepting a
            // truncated prefix as a different authenticated SRTP packet.
            let mut buffer=[0u8;65_536];let mut count=0;
            while count<32&&Instant::now()<deadline{
                let length=match socket.recv(&mut buffer){
                    Ok(length)=>length,
                    Err(error)if error.kind()==ErrorKind::WouldBlock=>break,
                    Err(error)if error.kind()==ErrorKind::Interrupted=>continue,
                    Err(_)=>return Err(denied()),
                };
                if length==0||length>2048{return Err(denied());}
                let receipt=self.clock.received(socket,&buffer[..length])?;
                if Instant::now()>=deadline{return Err(denied());}
                self.ingress.receive(registry,document,&buffer[..length],receipt)?;count+=1;
            }Ok(count)
        })();
        if result.is_err(){self.closed=true;self.socket.take();}result
    }
    pub fn retire_until(&mut self,registry:&mut ReceiveRegistry,document:&ReceiveDocument,deadline:Instant)->Result<()>{
        self.closed=true;self.socket.take();self.ingress.retire_until(registry,document,deadline)
    }
}
