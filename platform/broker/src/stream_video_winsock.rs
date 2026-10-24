//! Atomic Winsock datagram/ancillary receive. Native interface provenance must
//! establish software QPC timestamps; hardware counters are not interchangeable.
use crate::stream_video_ingress::NativeDatagramReceipt;
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_stream_input::video_client::DecoderReceipt;
use std::{net::UdpSocket,os::windows::io::AsRawSocket,mem::{size_of,size_of_val,zeroed}};
use windows_sys::Win32::Networking::WinSock::*;
use windows_sys::Win32::NetworkManagement::{IpHelper::{ConvertInterfaceIndexToLuid,GetInterfaceActiveTimestampCapabilities,INTERFACE_TIMESTAMP_CAPABILITIES},Ndis::NET_LUID_LH};

fn denied()->PlatformError{PlatformError::new(ErrorCode::UnsupportedCapability,"Native Winsock software ingress timestamp is unavailable")}
/// Native route/interface owner must recheck active SOFTWARE timestamp source,
/// including route/interface changes. No renderer boolean or default success.
pub trait SoftwareTimestampSource{
    fn require_current(&mut self,socket:&UdpSocket)->Result<()>;
    /// Interface index comes from this datagram's native packet-info cmsg.
    fn require_received_interface(&mut self,socket:&UdpSocket,index:u32)->Result<()>;
}
/// Native ICE/socket owner must establish the actual receive interface, not an
/// outbound route guess. Wildcard/multi-interface reception is not admissible.
/// The binding must remain exclusive while packets can be queued on this socket;
/// a rebind requires a fresh socket and timestamp adapter.
pub trait NativeReceiveInterface{
    fn current_receive_luid(&mut self,socket:&UdpSocket)->Result<u64>;
}
/// Reads active driver configuration on every receive boundary. Supported-but-
/// disabled software timestamps and any enabled hardware timestamps are rejected.
pub struct InterfaceSoftwareTimestamp<B:NativeReceiveInterface>{binding:B,luid:u64,socket:usize,closed:bool}
impl<B:NativeReceiveInterface> InterfaceSoftwareTimestamp<B>{
    pub fn from_native_binding(socket:&UdpSocket,mut binding:B)->Result<Self>{
        let luid=binding.current_receive_luid(socket)?;
        if luid==0{return Err(denied());}
        let mut source=Self{binding,luid,socket:socket.as_raw_socket()as usize,closed:false};
        source.require_current(socket)?;Ok(source)
    }
}
impl<B:NativeReceiveInterface> SoftwareTimestampSource for InterfaceSoftwareTimestamp<B>{
    fn require_received_interface(&mut self,socket:&UdpSocket,index:u32)->Result<()>{
        let mut luid:NET_LUID_LH=unsafe{zeroed()};
        if index==0||unsafe{ConvertInterfaceIndexToLuid(index,&mut luid)}!=0||unsafe{luid.Value}!=self.luid{
            self.closed=true;return Err(denied());
        }
        self.require_current(socket)
    }
    fn require_current(&mut self,socket:&UdpSocket)->Result<()>{
        let result=(||{
            if self.closed||socket.as_raw_socket()as usize!=self.socket||self.binding.current_receive_luid(socket)?!=self.luid{return Err(denied());}
            let luid=NET_LUID_LH{Value:self.luid};
            let mut caps:INTERFACE_TIMESTAMP_CAPABILITIES=unsafe{zeroed()};
            if unsafe{GetInterfaceActiveTimestampCapabilities(&luid,&mut caps)}!=0{return Err(denied());}
            let h=&caps.HardwareCapabilities;
            let hardware=[h.PtpV2OverUdpIPv4EventMessageReceive,h.PtpV2OverUdpIPv4AllMessageReceive,
                h.PtpV2OverUdpIPv4EventMessageTransmit,h.PtpV2OverUdpIPv4AllMessageTransmit,
                h.PtpV2OverUdpIPv6EventMessageReceive,h.PtpV2OverUdpIPv6AllMessageReceive,
                h.PtpV2OverUdpIPv6EventMessageTransmit,h.PtpV2OverUdpIPv6AllMessageTransmit,
                h.AllReceive,h.AllTransmit,h.TaggedTransmit];
            if caps.SoftwareCapabilities.AllReceive==0||hardware.iter().any(|&flag|flag!=0)
                ||self.binding.current_receive_luid(socket)?!=self.luid{return Err(denied());}
            Ok(())
        })();
        if result.is_err(){self.closed=true;}result
    }
}
pub struct WinsockDatagrams<S:SoftwareTimestampSource>{socket:usize,recv:LPFN_WSARECVMSG,source:S,closed:bool}
impl<S:SoftwareTimestampSource> WinsockDatagrams<S>{
    pub fn configure(socket:&UdpSocket,mut source:S)->Result<Self>{
        source.require_current(socket)?;socket.set_nonblocking(true).map_err(|_|denied())?;
        let native=socket.as_raw_socket()as usize;
        let enabled=1i32;
        let ipv4=socket.local_addr().map_err(|_|denied())?.is_ipv4();
        let (level,option)=if ipv4{(IPPROTO_IP,IP_PKTINFO)}else{(IPPROTO_IPV6,IPV6_PKTINFO)};
        if unsafe{setsockopt(native,level,option,(&enabled as *const i32).cast(),size_of::<i32>()as i32)}==SOCKET_ERROR{return Err(denied());}
        // IPv4-mapped traffic on a dual-stack IPv6 socket needs IPv4 packet info
        // as well. Failure is fatal rather than silently accepting unstamped NICs.
        if !ipv4{
            let mut only=0i32;let mut length=size_of::<i32>()as i32;
            if unsafe{getsockopt(native,IPPROTO_IPV6,IPV6_V6ONLY,(&mut only as *mut i32).cast(),&mut length)}==SOCKET_ERROR
                ||length!=size_of::<i32>()as i32{return Err(denied());}
            if only==0&&unsafe{setsockopt(native,IPPROTO_IP,IP_PKTINFO,(&enabled as *const i32).cast(),size_of::<i32>()as i32)}==SOCKET_ERROR{return Err(denied());}
        }
        let mut recv:LPFN_WSARECVMSG=None;let mut bytes=0u32;let mut id=WSAID_WSARECVMSG;
        let result=unsafe{WSAIoctl(native,SIO_GET_EXTENSION_FUNCTION_POINTER,(&mut id as *mut _).cast(),size_of_val(&id)as u32,
            (&mut recv as *mut _).cast(),size_of::<LPFN_WSARECVMSG>()as u32,&mut bytes,std::ptr::null_mut(),None)};
        if result==SOCKET_ERROR||recv.is_none()||bytes as usize!=size_of::<LPFN_WSARECVMSG>(){return Err(denied());}
        let mut config:TIMESTAMPING_CONFIG=unsafe{zeroed()};config.Flags=TIMESTAMPING_FLAG_RX;
        let result=unsafe{WSAIoctl(native,SIO_TIMESTAMPING,(&mut config as *mut _).cast(),size_of::<TIMESTAMPING_CONFIG>()as u32,
            std::ptr::null_mut(),0,&mut bytes,std::ptr::null_mut(),None)};
        if result==SOCKET_ERROR{return Err(denied());}
        source.require_current(socket)?;
        Ok(Self{socket:native,recv,source,closed:false})
    }
}
impl<S:SoftwareTimestampSource> NativeDatagramReceipt for WinsockDatagrams<S>{
    fn receive(&mut self,socket:&UdpSocket,buffer:&mut[u8])->Result<Option<(usize,DecoderReceipt)>>{
        let result=(||{
            if self.closed||socket.as_raw_socket()as usize!=self.socket||buffer.len()<65_536{return Err(denied());}
            self.source.require_current(socket)?;
            // usize storage gives WSACMSGHDR native alignment. Parsing uses
            // checked lengths and unaligned copies, never peer-provided pointers.
            let mut control=[0usize;64];let capacity=size_of_val(&control);
            let mut data=WSABUF{len:buffer.len()as u32,buf:buffer.as_mut_ptr()};
            let mut message:WSAMSG=unsafe{zeroed()};message.lpBuffers=&mut data;message.dwBufferCount=1;
            message.Control=WSABUF{len:capacity as u32,buf:control.as_mut_ptr().cast()};
            let mut received=0u32;
            let status=unsafe{self.recv.ok_or_else(denied)?(self.socket,&mut message,&mut received,std::ptr::null_mut(),None)};
            if status==SOCKET_ERROR{
                let error=unsafe{WSAGetLastError()};
                if error==WSAEWOULDBLOCK||error==WSAEINTR{return Ok(None);}return Err(denied());
            }
            if message.dwFlags!=0||received==0||received as usize>buffer.len()||message.Control.len as usize>capacity{return Err(denied());}
            let length=message.Control.len as usize;let header=size_of::<WSACMSGHDR>();let alignment=size_of::<usize>();
            let aligned=|value:usize|value.checked_add(alignment-1).map(|v|v&!(alignment-1)).ok_or_else(denied);
            let mut offset=0usize;let mut stamp=None;let mut interface=None;
            while offset<length{
                if length-offset<header{return Err(denied());}
                let pointer=unsafe{control.as_ptr().cast::<u8>().add(offset)};
                let cmsg=unsafe{std::ptr::read_unaligned(pointer.cast::<WSACMSGHDR>())};
                let size=cmsg.cmsg_len;
                if size<header||size>length-offset{return Err(denied());}
                if cmsg.cmsg_level==SOL_SOCKET&&cmsg.cmsg_type==SO_TIMESTAMP{
                    let payload=aligned(header)?;
                    if stamp.is_some()||size!=payload+size_of::<u64>(){return Err(denied());}
                    stamp=Some(unsafe{std::ptr::read_unaligned(pointer.add(payload).cast::<u64>())});
                }
                if cmsg.cmsg_level==IPPROTO_IP&&cmsg.cmsg_type==IP_PKTINFO{
                    let payload=aligned(header)?;
                    if interface.is_some()||size!=payload+size_of::<IN_PKTINFO>(){return Err(denied());}
                    let info=unsafe{std::ptr::read_unaligned(pointer.add(payload).cast::<IN_PKTINFO>())};
                    interface=Some(info.ipi_ifindex);
                }
                if cmsg.cmsg_level==IPPROTO_IPV6&&cmsg.cmsg_type==IPV6_PKTINFO{
                    let payload=aligned(header)?;
                    if interface.is_some()||size!=payload+size_of::<IN6_PKTINFO>(){return Err(denied());}
                    let info=unsafe{std::ptr::read_unaligned(pointer.add(payload).cast::<IN6_PKTINFO>())};
                    interface=Some(info.ipi6_ifindex);
                }
                let next=aligned(size)?;
                if next>length-offset{if size!=length-offset{return Err(denied());}offset=length;}else{offset+=next;}
            }
            self.source.require_received_interface(socket,interface.ok_or_else(denied)?)?;
            let stamp=i64::try_from(stamp.ok_or_else(denied)?).map_err(|_|denied())?;
            let receipt=DecoderReceipt::from_software_qpc_ingress(stamp)?;
            Ok(Some((received as usize,receipt)))
        })();if result.is_err(){self.closed=true;}result
    }
}
