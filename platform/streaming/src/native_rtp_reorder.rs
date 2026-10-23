//! Bounded post-SRTP receive ordering. No key exchange, authentication/replay
//! security, pacing or network endpoint is provided. Bind to one admitted source.
use crate::{invalid,unauthorized,native_h264_receive::{checked_sequence,Depacketizer,ReceivedAccessUnit},native_h264_rtp::PacketizationMode};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use std::{collections::BTreeMap,time::{Duration,Instant}};
struct Queued{bytes:Vec<u8>,received:Instant}
pub struct OrderedReceiver{
    receiver:Depacketizer,payload_type:u8,ssrc:u32,next:u16,queue:BTreeMap<u16,Queued>,closed:bool,
}
impl OrderedReceiver{
    /// initial_sequence must come from an authenticated capture-generation
    /// boundary. Do not infer it from whichever reordered datagram arrives first.
    pub fn new(mode:PacketizationMode,payload_type:u8,ssrc:u32,initial_sequence:u16,remaining:Duration)->Result<Self>{
        Ok(Self{receiver:Depacketizer::new(mode,payload_type,ssrc,remaining)?,payload_type,ssrc,next:initial_sequence,
            queue:BTreeMap::new(),closed:false})
    }
    pub fn close(&mut self){self.closed=true;self.queue.clear();self.receiver.close();}
    fn check(&mut self,now:Instant)->Result<()>{
        if self.closed||self.queue.values().any(|value|now.duration_since(value.received)>Duration::from_millis(50)){
            self.close();return Err(unauthorized());
        }
        if let Err(error)=self.receiver.tick(){self.close();return Err(error);}Ok(())
    }
    /// Must run independently while packets stop arriving. No missing packet
    /// can keep a receive generation or incomplete frame pending indefinitely.
    pub fn tick(&mut self)->Result<()>{self.check(Instant::now())}
    /// Consumes only already-authenticated RTP with SRTP removed. Equal queued
    /// duplicates/old retransmits yield no output, not renewed wait/age budgets.
    /// A conflicting duplicate, oversized jump, expired gap or buffer failure
    /// closes the generation. Missing dependencies are never silently skipped.
    pub fn push(&mut self,bytes:&[u8])->Result<Vec<ReceivedAccessUnit>>{
        let result=self.receive(bytes,Instant::now());if result.is_err(){self.close();}result
    }
    fn receive(&mut self,bytes:&[u8],received:Instant)->Result<Vec<ReceivedAccessUnit>>{
        self.check(received)?;let sequence=checked_sequence(bytes,self.payload_type,self.ssrc)?;
        let distance=sequence.wrapping_sub(self.next);
        if distance>=0x8000{return Ok(Vec::new());} // Old authenticated retransmission.
        if distance>=64{return Err(invalid());}
        if let Some(previous)=self.queue.get(&sequence){
            if previous.bytes!=bytes{return Err(invalid());}return Ok(Vec::new());
        }
        if self.queue.len()>=64{return Err(invalid());}
        let mut owned=Vec::new();owned.try_reserve_exact(bytes.len()).map_err(|_|resource())?;owned.extend_from_slice(bytes);
        self.queue.insert(sequence,Queued{bytes:owned,received});
        let mut frames=Vec::new();frames.try_reserve_exact(4).map_err(|_|resource())?;
        while let Some(packet)=self.queue.remove(&self.next){
            if let Some(frame)=self.receiver.push_received(&packet.bytes,packet.received)?{
                if frames.len()>=4{return Err(invalid());}frames.push(frame);
            }
            self.next=self.next.wrapping_add(1);
        }
        let now=Instant::now();self.check(now)?;
        if frames.iter().any(|frame|now.duration_since(frame.received_at)>Duration::from_millis(100)){return Err(unauthorized());}
        Ok(frames)
    }
}
fn resource()->PlatformError{PlatformError::new(ErrorCode::ResourceLimit,"The native RTP reorder budget is unavailable")}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::native_h264_rtp::{AccessUnit,Framing,Packetizer};
    fn receiver()->OrderedReceiver{OrderedReceiver::new(PacketizationMode::NonInterleaved,102,17,65535,Duration::from_secs(30)).unwrap()}
    fn packets()->Vec<Vec<u8>>{
        let mut bytes=vec![0,0,1,0x65];bytes.extend(vec![0xaa;600]);let unit=AccessUnit::parse(&bytes,Framing::AnnexB).unwrap();
        Packetizer::new(PacketizationMode::NonInterleaved,102,17,65535,0,256).unwrap().packetize(&unit,0).unwrap().packets
    }
    #[test]fn reorders_across_sequence_wrap_preserving_first_receipt(){
        let mut receiver=receiver();let packets=packets();
        assert!(receiver.push(&packets[1]).unwrap().is_empty());let earliest=receiver.queue.get(&0).unwrap().received;
        assert!(receiver.push(&packets[0]).unwrap().is_empty());let result=receiver.push(&packets[2]).unwrap();
        assert_eq!(result.len(),1);assert!(result[0].has_idr);assert_eq!(result[0].received_at,earliest);
        assert_eq!(result[0].annex_b.len(),605);
    }
    #[test]fn equal_duplicate_does_not_refresh_receipt_or_buffer(){
        let mut receiver=receiver();let packets=packets();receiver.push(&packets[1]).unwrap();
        let original=receiver.queue.get(&0).unwrap().received;assert!(receiver.push(&packets[1]).unwrap().is_empty());
        assert_eq!(receiver.queue.len(),1);assert_eq!(receiver.queue.get(&0).unwrap().received,original);
    }
    #[test]fn conflicting_duplicate_is_terminal(){
        let mut receiver=receiver();let mut packets=packets();receiver.push(&packets[1]).unwrap();
        packets[1][14]^=1;assert!(receiver.push(&packets[1]).is_err());assert!(receiver.queue.is_empty());assert!(receiver.tick().is_err());
    }
    #[test]fn missing_gap_expires_without_new_packets(){
        let mut receiver=receiver();receiver.push(&packets()[1]).unwrap();
        assert!(receiver.check(Instant::now()+Duration::from_millis(51)).is_err());assert!(receiver.queue.is_empty());
    }
    #[test]fn oversized_jump_and_wrong_source_are_rejected(){
        let mut bytes=packets()[0].clone();bytes[2..4].copy_from_slice(&63u16.to_be_bytes());assert!(receiver().push(&bytes).is_err());
        let mut bytes=packets()[0].clone();bytes[8]^=1;assert!(receiver().push(&bytes).is_err());
    }
    #[test]fn old_authenticated_retransmit_is_not_delivered_twice(){
        let mut receiver=receiver();let packets=packets();for packet in &packets{receiver.push(packet).unwrap();}
        assert!(receiver.push(&packets[0]).unwrap().is_empty());assert!(receiver.tick().is_ok());
    }
}
