//! Native transport ingress into the broker-owned decoder session. Cryptographic
//! authentication/replay decisions belong to the installed SRTP engine, never
//! packet fields, renderer flags, this queue or an ordinary UDP source address.
use crate::stream_video_registry::{ReceiveRegistry,ReceiveDocument};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_stream_input::video_client::{DecoderReceipt,VideoOwner};
use std::{time::Instant,net::{UdpSocket,SocketAddr}};

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
/// Windows implements this through the packet-info/software-QPC Winsock adapter.
pub trait NativeDatagramReceipt{
    /// One nonblocking recvmsg operation supplies BOTH bytes and ancillary
    /// receipt. None means WouldBlock. Reject truncation/missing timestamps;
    /// do not perform recv() followed by a separate timestamp lookup.
    fn receive(&mut self,socket:&UdpSocket,buffer:&mut[u8])->Result<Option<(usize,DecoderReceipt)>>;
}
/// Owns an already connected ICE-selected socket, never binds/connects to a
/// renderer-provided address. Fixed peer checks are not SRTP authentication.
pub struct UdpVideoIngress<T:NativeSrtpReceiver,C:NativeDatagramReceipt>{socket:Option<UdpSocket>,peer:SocketAddr,ingress:VideoIngress<T>,clock:C,closed:bool}
impl<T:NativeSrtpReceiver> UdpVideoIngress<T,crate::stream_video_winsock::SelectedWinsockDatagrams>{
    /// Native ICE caller supplies its selected, already connected socket. Keep
    /// the engine in the caller's slot on any setup error so keys/workers can
    /// still be explicitly retired; never consume it before socket admission.
    pub fn from_windows_selected_socket(socket:UdpSocket,expected_peer:SocketAddr,index:u32,owner:VideoOwner,transport:&mut Option<T>)->Result<Self>{
        if socket.peer_addr().map_err(|_|denied())?!=expected_peer||expected_peer.port()==0
            ||expected_peer.ip().is_unspecified()||expected_peer.ip().is_multicast()
            ||[owner.capture,owner.game,owner.pair,owner.session].iter().any(|id|id.is_nil())
            ||!(1..=1_000_000).contains(&owner.generation){return Err(denied());}
        if transport.as_ref().ok_or_else(denied)?.owner()!=owner{return Err(denied());}
        let clock=crate::stream_video_winsock::SelectedWinsockDatagrams::from_selected_socket(&socket,index)?;
        if transport.as_ref().ok_or_else(denied)?.owner()!=owner{return Err(denied());}
        // No fallible operation remains after taking engine ownership.
        let transport=transport.take().ok_or_else(denied)?;
        let ingress=VideoIngress{owner,transport,closed:false,retired:false,video_retired:false};
        Ok(Self{socket:Some(socket),peer:expected_peer,ingress,clock,closed:false})
    }
}
impl<T:NativeSrtpReceiver,C:NativeDatagramReceipt> UdpVideoIngress<T,C>{
    pub fn owner(&self)->VideoOwner{self.ingress.owner}
    pub fn from_selected_socket(socket:UdpSocket,expected_peer:SocketAddr,owner:VideoOwner,transport:&mut Option<T>,clock:C)->Result<Self>{
        if socket.peer_addr().map_err(|_|denied())?!=expected_peer||expected_peer.port()==0||expected_peer.ip().is_unspecified()||expected_peer.ip().is_multicast(){return Err(denied());}
        if transport.as_ref().ok_or_else(denied)?.owner()!=owner
            ||[owner.capture,owner.game,owner.pair,owner.session].iter().any(|id|id.is_nil())
            ||!(1..=1_000_000).contains(&owner.generation){return Err(denied());}
        socket.set_nonblocking(true).map_err(|_|denied())?;
        let transport=transport.take().ok_or_else(denied)?;
        let ingress=VideoIngress{owner,transport,closed:false,retired:false,video_retired:false};
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
                let Some((length,receipt))=self.clock.receive(socket,&mut buffer)?else{break};
                if length==0||length>2048||length>buffer.len(){return Err(denied());}
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
trait PollingTransport{
    fn owner(&self)->VideoOwner;
    fn poll(&mut self,registry:&mut ReceiveRegistry,document:&ReceiveDocument,deadline:Instant)->Result<usize>;
    fn retire(&mut self,registry:&mut ReceiveRegistry,document:&ReceiveDocument,deadline:Instant)->Result<()>;
}
impl<T:NativeSrtpReceiver,C:NativeDatagramReceipt> PollingTransport for UdpVideoIngress<T,C>{
    fn owner(&self)->VideoOwner{UdpVideoIngress::owner(self)}
    fn poll(&mut self,registry:&mut ReceiveRegistry,document:&ReceiveDocument,deadline:Instant)->Result<usize>{UdpVideoIngress::poll(self,registry,document,deadline)}
    fn retire(&mut self,registry:&mut ReceiveRegistry,document:&ReceiveDocument,deadline:Instant)->Result<()>{self.retire_until(registry,document,deadline)}
}
/// Broker retains socket/crypto cleanup ownership independently from decoder
/// retirement. Failed poll seals the slot; only successful cleanup removes it.
#[derive(Default)]
pub struct TransportSlot{entry:Option<Box<dyn PollingTransport>>,sealed:bool,closed:bool}
impl TransportSlot{
    pub fn occupied(&self)->bool{self.entry.is_some()}
    pub fn attach<T:NativeSrtpReceiver+'static,C:NativeDatagramReceipt+'static>(&mut self,transport:&mut Option<UdpVideoIngress<T,C>>,registry:&ReceiveRegistry,document:&ReceiveDocument)->Result<()>{
        if self.closed||self.entry.is_some(){return Err(PlatformError::new(ErrorCode::Busy,"The native transport slot still requires retirement"));}
        let pending=transport.as_ref().ok_or_else(denied)?;
        if registry.state(document.id(),pending.owner())?!=crate::stream_video_receive::ReceiveState::Active{return Err(denied());}
        // Rejected attachment leaves socket/crypto/decoder cleanup ownership in
        // the caller's slot. Successful admission transfers it exactly once.
        self.entry=Some(Box::new(transport.take().ok_or_else(denied)?));self.sealed=false;Ok(())
    }
    pub fn maintain(&mut self,registry:&mut ReceiveRegistry,document:&ReceiveDocument,deadline:Instant)->Result<()>{
        let Some(entry)=self.entry.as_mut()else{return Ok(())};
        if !registry.state(document.id(),entry.owner()).is_ok_and(|state|state==crate::stream_video_receive::ReceiveState::Active){self.sealed=true;}
        if !self.sealed&&!self.closed{
            if entry.poll(registry,document,deadline).is_err(){self.sealed=true;}
            if !self.sealed&&registry.tick(document,entry.owner()).is_err(){self.sealed=true;}
        }
        if self.sealed||self.closed{
            entry.retire(registry,document,deadline)?;self.entry.take();
        }Ok(())
    }
    pub fn close_until(&mut self,registry:&mut ReceiveRegistry,document:&ReceiveDocument,deadline:Instant)->Result<()>{
        self.closed=true;self.sealed=true;self.maintain(registry,document,deadline)
    }
}
