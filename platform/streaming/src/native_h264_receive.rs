//! Receive only AFTER authenticated SRTP removal, replay filtering and bounded
//! packet reordering. Not a network endpoint, jitter buffer, decoder or consent.
use crate::{invalid,unauthorized,native_h264_rtp::{AccessUnit,Framing,PacketizationMode}};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use std::time::{Duration,Instant};

const MAX_BYTES:usize=2*1024*1024;
pub struct ReceivedAccessUnit{pub annex_b:Vec<u8>,pub timestamp:u32,pub has_idr:bool,pub received_at:Instant}
struct Header<'a>{sequence:u16,timestamp:u32,marker:bool,payload:&'a[u8]}
fn header(bytes:&[u8],payload_type:u8,ssrc:u32)->Result<Header<'_>>{
    if !(13..=1400).contains(&bytes.len())||bytes[0]>>6!=2||bytes[1]&127!=payload_type||
        u32::from_be_bytes(bytes[8..12].try_into().map_err(|_|invalid())?)!=ssrc{return Err(invalid());}
    let mut offset=12+usize::from(bytes[0]&15)*4;
    if offset>bytes.len(){return Err(invalid());}
    if bytes[0]&0x10!=0{
        if bytes.len()-offset<4{return Err(invalid());}
        let words=u16::from_be_bytes([bytes[offset+2],bytes[offset+3]])as usize;
        offset=offset.checked_add(4+words*4).filter(|value|*value<=bytes.len()).ok_or_else(invalid)?;
    }
    let mut end=bytes.len();
    if bytes[0]&0x20!=0{
        let padding=usize::from(*bytes.last().ok_or_else(invalid)?);
        if padding==0||padding>end-offset{return Err(invalid());}end-=padding;
    }
    if offset>=end{return Err(invalid());}
    Ok(Header{sequence:u16::from_be_bytes([bytes[2],bytes[3]]),
        timestamp:u32::from_be_bytes(bytes[4..8].try_into().map_err(|_|invalid())?),marker:bytes[1]&0x80!=0,payload:&bytes[offset..end]})
}
pub(crate) fn checked_sequence(bytes:&[u8],payload_type:u8,ssrc:u32)->Result<u16>{Ok(header(bytes,payload_type,ssrc)?.sequence)}
/// One access unit at a time, finite bytes/NAL/packet/time budget. An error is
/// terminal for this media generation: caller requests fresh negotiated/keyframe
/// recovery rather than sending partially reconstructed dependencies to decoder.
pub struct Depacketizer{
    mode:PacketizationMode,payload_type:u8,ssrc:u32,deadline:Instant,closed:bool,
    sequence:Option<u16>,last_timestamp:Option<u32>,timestamp:Option<u32>,started:Option<Instant>,
    nal_header:Option<u8>,buffer:Vec<u8>,nals:usize,packets:usize,first:bool,
}
impl Depacketizer{
    pub fn new(mode:PacketizationMode,payload_type:u8,ssrc:u32,remaining:Duration)->Result<Self>{
        if !(96..=127).contains(&payload_type)||ssrc==0||remaining.is_zero()||remaining>Duration::from_secs(1800){return Err(invalid());}
        Ok(Self{mode,payload_type,ssrc,deadline:Instant::now()+remaining,closed:false,sequence:None,last_timestamp:None,
            timestamp:None,started:None,nal_header:None,buffer:Vec::new(),nals:0,packets:0,first:true})
    }
    pub fn close(&mut self){self.closed=true;self.buffer.clear();self.nal_header=None;}
    /// Call independently while packets stop arriving. A stalled incomplete
    /// frame cannot retain its bytes indefinitely or gain new authority.
    pub fn tick(&mut self)->Result<()>{self.check(Instant::now())}
    fn check(&mut self,now:Instant)->Result<()>{
        if self.closed||now>=self.deadline||self.started.is_some_and(|start|now.duration_since(start)>Duration::from_millis(100)){
            self.close();return Err(unauthorized());
        }Ok(())
    }
    pub fn push(&mut self,bytes:&[u8])->Result<Option<ReceivedAccessUnit>>{
        self.push_received(bytes,Instant::now())
    }
    pub(crate) fn push_received(&mut self,bytes:&[u8],received:Instant)->Result<Option<ReceivedAccessUnit>>{
        let result=self.receive(bytes,received);if result.is_err(){self.close();}result
    }
    fn append(&mut self,bytes:&[u8])->Result<()>{
        let length=self.buffer.len().checked_add(bytes.len()).filter(|length|*length<=MAX_BYTES).ok_or_else(invalid)?;
        if length>self.buffer.capacity(){
            let capacity=length.next_power_of_two().min(MAX_BYTES);
            self.buffer.try_reserve_exact(capacity-self.buffer.len()).map_err(|_|PlatformError::new(ErrorCode::ResourceLimit,"The native received-video buffer is unavailable"))?;
        }
        self.buffer.extend_from_slice(bytes);Ok(())
    }
    fn begin_nal(&mut self,header:u8)->Result<()>{
        if self.nals>=256||header&0x80!=0||!matches!(header&31,1|5|6|7|8|9|10|11|12){return Err(invalid());}
        self.nals+=1;self.append(&[0,0,0,1,header])
    }
    fn receive(&mut self,bytes:&[u8],now:Instant)->Result<Option<ReceivedAccessUnit>>{
        let current=Instant::now();self.check(current)?;
        if now>current||current.duration_since(now)>Duration::from_millis(100){return Err(invalid());}
        let packet=header(bytes,self.payload_type,self.ssrc)?;
        if self.sequence.is_some_and(|previous|packet.sequence!=previous.wrapping_add(1)){return Err(invalid());}
        if let Some(timestamp)=self.timestamp{
            if packet.timestamp!=timestamp{return Err(invalid());}
            self.started=Some(self.started.ok_or_else(invalid)?.min(now));
        }else{
            if self.last_timestamp.is_some_and(|previous|{
                let delta=packet.timestamp.wrapping_sub(previous);delta==0||delta>=0x8000_0000
            }){return Err(invalid());}
            self.timestamp=Some(packet.timestamp);self.started=Some(now);
        }
        self.sequence=Some(packet.sequence);self.packets+=1;if self.packets>8192{return Err(invalid());}
        let nal=packet.payload[0];
        match nal&31{
            28 if self.mode==PacketizationMode::NonInterleaved=>{
                if packet.payload.len()<3||nal&0x80!=0{return Err(invalid());}
                let fu=packet.payload[1];let start=fu&0x80!=0;let end=fu&0x40!=0;
                if fu&0x20!=0||start&&end{return Err(invalid());}
                let original=(nal&0xe0)|(fu&31);
                if start{
                    if self.nal_header.is_some(){return Err(invalid());}self.begin_nal(original)?;self.nal_header=Some(original);
                }else if self.nal_header!=Some(original){return Err(invalid());}
                self.append(&packet.payload[2..])?;if end{self.nal_header=None;}
            }
            24 if self.mode==PacketizationMode::NonInterleaved=>{
                if nal&0x80!=0||self.nal_header.is_some(){return Err(invalid());}
                let mut offset=1;let mut nri=0;
                while offset<packet.payload.len(){
                    if packet.payload.len()-offset<2{return Err(invalid());}
                    let size=u16::from_be_bytes([packet.payload[offset],packet.payload[offset+1]])as usize;offset+=2;
                    if size==0||size>packet.payload.len()-offset{return Err(invalid());}
                    nri=nri.max(packet.payload[offset]&0x60);
                    self.begin_nal(packet.payload[offset])?;self.append(&packet.payload[offset+1..offset+size])?;offset+=size;
                }
                if offset==1||nri!=nal&0x60{return Err(invalid());}
            }
            _=>{
                if self.nal_header.is_some(){return Err(invalid());}
                self.begin_nal(nal)?;self.append(&packet.payload[1..])?;
            }
        }
        if !packet.marker{return Ok(None);}
        if self.nal_header.is_some(){return Err(invalid());}
        let unit=AccessUnit::parse(&self.buffer,Framing::AnnexB)?;
        if self.first&&!unit.has_idr(){return Err(invalid());}let has_idr=unit.has_idr();
        self.check(Instant::now())?;
        let received_at=self.started.ok_or_else(invalid)?;
        let annex_b=std::mem::take(&mut self.buffer);self.last_timestamp=Some(packet.timestamp);
        self.timestamp=None;self.started=None;self.nals=0;self.packets=0;self.first=false;
        Ok(Some(ReceivedAccessUnit{annex_b,timestamp:packet.timestamp,has_idr,received_at}))
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::native_h264_rtp::Packetizer;
    fn receiver(mode:PacketizationMode)->Depacketizer{Depacketizer::new(mode,102,0x12345678,Duration::from_secs(30)).unwrap()}
    fn packets()->Vec<Vec<u8>>{
        let mut bytes=vec![0,0,0,1,0x65];bytes.extend(vec![0xaa;600]);
        let unit=AccessUnit::parse(&bytes,Framing::AnnexB).unwrap();
        Packetizer::new(PacketizationMode::NonInterleaved,102,0x12345678,65535,u32::MAX-10,256).unwrap().packetize(&unit,0).unwrap().packets
    }
    fn packet(payload:&[u8])->Vec<u8>{let mut bytes=vec![0x80,0xe6,0,0,0,0,0,1,0x12,0x34,0x56,0x78];bytes.extend(payload);bytes}
    #[test]fn sender_fua_round_trip_and_wrap(){
        let mut receiver=receiver(PacketizationMode::NonInterleaved);let packets=packets();
        assert!(receiver.push(&packets[0]).unwrap().is_none());assert!(receiver.push(&packets[1]).unwrap().is_none());
        let frame=receiver.push(&packets[2]).unwrap().unwrap();let mut expected=vec![0,0,0,1,0x65];expected.extend(vec![0xaa;600]);
        assert_eq!(frame.annex_b,expected);assert!(frame.has_idr);assert_eq!(frame.timestamp,u32::MAX-10);
    }
    #[test]fn loss_is_terminal_and_never_returns_partial_frame(){
        let mut receiver=receiver(PacketizationMode::NonInterleaved);let packets=packets();
        receiver.push(&packets[0]).unwrap();assert!(receiver.push(&packets[2]).is_err());
        assert!(receiver.push(&packet(&[0x65,1])).is_err());assert!(receiver.buffer.is_empty());
    }
    #[test]fn early_marker_and_timestamp_change_are_rejected(){
        let mut early=receiver(PacketizationMode::NonInterleaved);let mut packets=packets();packets[0][1]|=0x80;
        assert!(early.push(&packets[0]).is_err());
        let mut changed=receiver(PacketizationMode::NonInterleaved);packets[0][1]&=0x7f;changed.push(&packets[0]).unwrap();
        packets[1][7]^=1;assert!(changed.push(&packets[1]).is_err());
    }
    #[test]fn source_and_payload_type_are_bound(){
        for offset in [1,8]{let mut bytes=packet(&[0x65,1]);bytes[offset]^=1;
            assert!(receiver(PacketizationMode::NonInterleaved).push(&bytes).is_err());
        }
    }
    #[test]fn csrc_extension_padding_are_bounded_and_skipped(){
        let original=packet(&[0x65,0xaa]);let mut bytes=original[..12].to_vec();bytes[0]=0xb1;
        bytes.extend([1,2,3,4]); // One CSRC.
        bytes.extend([0xbe,0xde,0,1,9,8,7,6]); // One four-byte extension word.
        bytes.extend([0x65,0xaa,0,0,0,4]);
        let result=receiver(PacketizationMode::NonInterleaved).push(&bytes).unwrap().unwrap();assert_eq!(result.annex_b,vec![0,0,0,1,0x65,0xaa]);
        bytes[18]=0xff;assert!(receiver(PacketizationMode::NonInterleaved).push(&bytes).is_err());
        let mut invalid=original;invalid[0]|=0x20;invalid.push(0);assert!(receiver(PacketizationMode::NonInterleaved).push(&invalid).is_err());
    }
    #[test]fn stap_a_and_mode_restrictions(){
        let bytes=packet(&[0x78,0,2,0x67,1,0,2,0x68,2,0,2,0x65,3]);
        let output=receiver(PacketizationMode::NonInterleaved).push(&bytes).unwrap().unwrap();assert!(output.has_idr);
        assert!(receiver(PacketizationMode::SingleNal).push(&bytes).is_err());
        assert!(receiver(PacketizationMode::SingleNal).push(&packets()[0]).is_err());
    }
    #[test]fn first_nonidr_and_stalled_partial_frame_are_rejected(){
        assert!(receiver(PacketizationMode::NonInterleaved).push(&packet(&[0x41,1])).is_err());
        let mut stalled=receiver(PacketizationMode::NonInterleaved);stalled.push(&packets()[0]).unwrap();
        assert!(stalled.check(Instant::now()+Duration::from_millis(101)).is_err());assert!(stalled.buffer.is_empty());
    }
}
