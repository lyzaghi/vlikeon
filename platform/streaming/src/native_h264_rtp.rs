//! Bounded native H.264/RTP primitives, not SDP negotiation, decoder admission,
//! SRTP or a network sender. Call only after trusted backend access-unit and
//! actual codec/profile/level/VUI/receiver admission. Never guess framing.
use crate::invalid;
use omniplay_contracts::{ErrorCode,PlatformError,Result};

const MAX_BYTES:usize=2*1024*1024;
const MAX_NALS:usize=256;
const MAX_PACKETS:usize=8192;
#[derive(Clone,Copy)]
pub enum Framing{AnnexB,LengthPrefixed{length_bytes:u8}}
#[derive(Clone,Copy,PartialEq,Eq)]
pub enum PacketizationMode{SingleNal,NonInterleaved}
pub struct AccessUnit<'a>{nals:Vec<&'a[u8]>,idr:bool}
impl<'a> AccessUnit<'a>{
    /// One complete backend-delimited access unit, in decode order. This checks
    /// transport layout only, not slice boundaries, parameter sets or decodability.
    pub fn parse(bytes:&'a[u8],framing:Framing)->Result<Self>{
        if bytes.is_empty()||bytes.len()>MAX_BYTES{return Err(invalid());}
        let mut nals=Vec::new();let mut idr=false;let mut non_idr=false;
        nals.try_reserve_exact(MAX_NALS).map_err(|_|resource())?;
        let mut push=|nal:&'a[u8]|->Result<()>{
            // The initial realtime backend supports ordinary nonpartitioned
            // slices/IDR/SEI/SPS/PPS/AUD/end/filler, not scalable/multiview NALs
            // or prepacketized STAP/FU input. forbidden_zero_bit must be zero.
            if nal.is_empty()||nal[0]&0x80!=0||!matches!(nal[0]&31,1|5|6|7|8|9|10|11|12)||nals.len()>=MAX_NALS{return Err(invalid());}
            idr|=nal[0]&31==5;non_idr|=nal[0]&31==1;nals.push(nal);Ok(())
        };
        match framing{
            Framing::LengthPrefixed{length_bytes}=>{
                if !matches!(length_bytes,1|2|4){return Err(invalid());}
                let mut offset=0;let width=usize::from(length_bytes);
                while offset<bytes.len(){
                    if bytes.len()-offset<width{return Err(invalid());}
                    let mut length=0usize;for byte in &bytes[offset..offset+width]{length=(length<<8)|usize::from(*byte);}
                    offset+=width;if length==0||length>bytes.len()-offset{return Err(invalid());}
                    push(&bytes[offset..offset+length])?;offset+=length;
                }
            }
            Framing::AnnexB=>{
                let(start,prefix)=start_code(bytes,0).ok_or_else(invalid)?;
                if start!=0{return Err(invalid());}let mut offset=prefix;
                loop{
                    let next=start_code(bytes,offset);let mut end=next.map(|(index,_)|index).unwrap_or(bytes.len());
                    // Annex B byte-stream trailing_zero_8bits are not payload.
                    while end>offset&&bytes[end-1]==0{end-=1;}
                    push(&bytes[offset..end])?;
                    match next{Some((index,size))=>offset=index+size,None=>break}
                }
            }
        }
        if nals.is_empty()||idr==non_idr{return Err(invalid());}Ok(Self{nals,idr})
    }
    pub fn has_idr(&self)->bool{self.idr}
    pub fn nal_count(&self)->usize{self.nals.len()}
    /// Borrow already bounded NAL slices; this adds no codec/decoder authority.
    pub fn nals(&self)->impl ExactSizeIterator<Item=&'a[u8]>+'_ {self.nals.iter().copied()}
}
fn start_code(bytes:&[u8],offset:usize)->Option<(usize,usize)>{
    let mut index=offset;
    while index+3<=bytes.len(){
        if bytes[index..].starts_with(&[0,0,0,1]){return Some((index,4));}
        if bytes[index..].starts_with(&[0,0,1]){return Some((index,3));}
        index+=1;
    }None
}
fn resource()->PlatformError{PlatformError::new(ErrorCode::ResourceLimit,"The native RTP packet budget is unavailable")}
pub struct RtpBatch{pub packets:Vec<Vec<u8>>,pub timestamp:u32,pub has_idr:bool}
pub struct Packetizer{
    mode:PacketizationMode,payload_type:u8,ssrc:u32,sequence:u16,timestamp_base:u32,
    max_rtp_bytes:usize,last_pts:Option<i64>,
}
impl Packetizer{
    /// Values come from admitted RTP/SDP state. SSRC/initial sequence/timestamp
    /// randomness and collision handling belong to the RTP session controller.
    /// max_rtp_bytes excludes outer IP/UDP/DTLS/SRTP/tag/extensions overhead;
    /// the caller must reserve those before setting this per-packet bound.
    pub fn new(mode:PacketizationMode,payload_type:u8,ssrc:u32,sequence:u16,timestamp_base:u32,max_rtp_bytes:usize)->Result<Self>{
        if !(96..=127).contains(&payload_type)||ssrc==0||!(256..=1400).contains(&max_rtp_bytes){return Err(invalid());}
        Ok(Self{mode,payload_type,ssrc,sequence,timestamp_base,max_rtp_bytes,last_pts:None})
    }
    /// Transactional packet construction: validation/allocation failure does not
    /// consume sequence or presentation state. Caller must publish the complete
    /// batch in order under current capture authority, or retire the generation.
    pub fn packetize(&mut self,unit:&AccessUnit<'_>,timestamp_100ns:i64)->Result<RtpBatch>{
        if !(0..=18_000_000_000).contains(&timestamp_100ns)||self.last_pts.is_some_and(|previous|timestamp_100ns<=previous){return Err(invalid());}
        let payload_bound=self.max_rtp_bytes-12;let fragment_bound=payload_bound-2;
        let mut count=0usize;
        for nal in &unit.nals{
            let packets=if nal.len()<=payload_bound{1}else{
                if self.mode==PacketizationMode::SingleNal{return Err(invalid());}
                (nal.len()-1).div_ceil(fragment_bound)
            };
            count=count.checked_add(packets).filter(|value|*value<=MAX_PACKETS).ok_or_else(resource)?;
        }
        let timestamp=self.timestamp_base.wrapping_add(((timestamp_100ns as u128)*9/1000)as u32);
        let mut packets=Vec::new();packets.try_reserve_exact(count).map_err(|_|resource())?;
        let mut sequence=self.sequence;
        for nal in &unit.nals{
            if nal.len()<=payload_bound{
                packets.push(self.packet(sequence,timestamp,nal)?);sequence=sequence.wrapping_add(1);
            }else{
                let slices=nal[1..].chunks(fragment_bound);let fragments=slices.len();
                for(index,chunk)in slices.enumerate(){
                    let mut payload=Vec::new();payload.try_reserve_exact(chunk.len()+2).map_err(|_|resource())?;
                    payload.push((nal[0]&0xe0)|28);
                    let start=if index==0{0x80}else{0};let end=if index+1==fragments{0x40}else{0};
                    payload.push((nal[0]&31)|start|end);
                    payload.extend_from_slice(chunk);
                    packets.push(self.packet(sequence,timestamp,&payload)?);sequence=sequence.wrapping_add(1);
                }
            }
        }
        // Exactly the last packet of the complete access unit carries M=1.
        packets.last_mut().ok_or_else(invalid)?[1]|=0x80;
        self.sequence=sequence;self.last_pts=Some(timestamp_100ns);
        Ok(RtpBatch{packets,timestamp,has_idr:unit.idr})
    }
    fn packet(&self,sequence:u16,timestamp:u32,payload:&[u8])->Result<Vec<u8>>{
        let mut bytes=Vec::new();bytes.try_reserve_exact(12+payload.len()).map_err(|_|resource())?;
        bytes.extend_from_slice(&[0x80,self.payload_type]);bytes.extend_from_slice(&sequence.to_be_bytes());
        bytes.extend_from_slice(&timestamp.to_be_bytes());bytes.extend_from_slice(&self.ssrc.to_be_bytes());
        bytes.extend_from_slice(payload);Ok(bytes)
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    fn sender(mode:PacketizationMode)->Packetizer{Packetizer::new(mode,102,0x12345678,65535,u32::MAX-100,256).unwrap()}
    #[test]fn single_nal_rtp_header_and_clock(){
        let unit=AccessUnit::parse(&[0,0,1,0x65,0x88],Framing::AnnexB).unwrap();
        let batch=sender(PacketizationMode::NonInterleaved).packetize(&unit,10_000_000).unwrap();
        assert_eq!(batch.packets.len(),1);assert_eq!(batch.timestamp,(u32::MAX-100).wrapping_add(90000));
        let packet=&batch.packets[0];assert_eq!(&packet[..4],&[0x80,0xe6,0xff,0xff]);
        assert_eq!(&packet[4..8],&batch.timestamp.to_be_bytes());assert_eq!(&packet[8..],&[0x12,0x34,0x56,0x78,0x65,0x88]);
    }
    #[test]fn fua_reassembles_payload_and_wraps_sequence(){
        let mut bytes=vec![0,0,0,1,0x65];bytes.extend(vec![0x55;600]);
        let unit=AccessUnit::parse(&bytes,Framing::AnnexB).unwrap();
        let batch=sender(PacketizationMode::NonInterleaved).packetize(&unit,0).unwrap();
        assert_eq!(batch.packets.len(),3);let mut rebuilt=vec![0x65];
        for(index,packet)in batch.packets.iter().enumerate(){
            assert!(packet.len()<=256);assert_eq!(packet[12],0x7c);
            let fragment_flags=if index==0{0x80}else if index==2{0x40}else{0};
            assert_eq!(packet[13],5|fragment_flags);
            assert_eq!(u16::from_be_bytes([packet[2],packet[3]]),65535u16.wrapping_add(index as u16));
            assert_eq!(packet[1]&0x80,if index==2{0x80}else{0});rebuilt.extend_from_slice(&packet[14..]);
        }assert_eq!(rebuilt.as_slice(),&bytes[4..]);
    }
    #[test]fn multiple_nals_mark_only_access_unit_end(){
        let bytes=[0,0,0,1,0x67,0x4d,0,42,0,0,1,0x68,0xaa,0,0,1,0x65,0xbb];
        let unit=AccessUnit::parse(&bytes,Framing::AnnexB).unwrap();assert_eq!(unit.nal_count(),3);
        let batch=sender(PacketizationMode::NonInterleaved).packetize(&unit,0).unwrap();
        assert_eq!(batch.packets.iter().map(|packet|packet[1]&0x80).collect::<Vec<_>>(),vec![0,0,0x80]);
    }
    #[test]fn explicit_length_prefix_widths(){
        for width in [1u8,2,4]{
            let mut bytes=vec![0;width as usize];bytes[width as usize-1]=2;bytes.extend([0x41,0xaa]);
            let unit=AccessUnit::parse(&bytes,Framing::LengthPrefixed{length_bytes:width}).unwrap();
            assert!(!unit.has_idr());assert_eq!(unit.nal_count(),1);
        }
    }
    #[test]fn malformed_framing_is_rejected(){
        for bytes in [&[0,0,0,0][..],&[0,0,0,3,0x65][..],&[0,0,0][..]]{
            assert!(AccessUnit::parse(bytes,Framing::LengthPrefixed{length_bytes:4}).is_err());
        }
        assert!(AccessUnit::parse(&[0,0,1],Framing::AnnexB).is_err());
        assert!(AccessUnit::parse(&[0,0,1,0x65],Framing::LengthPrefixed{length_bytes:3}).is_err());
    }
    #[test]fn forbidden_aggregation_and_nonpicture_input_is_rejected(){
        for bytes in [&[0,0,1,0xe5,1][..],&[0,0,1,0x7c,1][..],&[0,0,1,0x67,1][..],
            &[0,0,1,0x65,1,0,0,1,0x41,1][..]]{
            assert!(AccessUnit::parse(bytes,Framing::AnnexB).is_err());
        }
    }
    #[test]fn mode_zero_failure_does_not_consume_state(){
        let mut bytes=vec![0,0,1,0x65];bytes.extend(vec![0xaa;600]);
        let large=AccessUnit::parse(&bytes,Framing::AnnexB).unwrap();let mut packetizer=sender(PacketizationMode::SingleNal);
        assert!(packetizer.packetize(&large,0).is_err());
        let small=AccessUnit::parse(&[0,0,1,0x65,1],Framing::AnnexB).unwrap();
        let packet=packetizer.packetize(&small,0).unwrap();assert_eq!(&packet.packets[0][2..4],&[0xff,0xff]);
    }
    #[test]fn replay_and_oversized_access_units_are_rejected(){
        let unit=AccessUnit::parse(&[0,0,1,0x65,1],Framing::AnnexB).unwrap();let mut packetizer=sender(PacketizationMode::NonInterleaved);
        packetizer.packetize(&unit,10).unwrap();assert!(packetizer.packetize(&unit,10).is_err());assert!(packetizer.packetize(&unit,9).is_err());
        assert!(AccessUnit::parse(&vec![1;MAX_BYTES+1],Framing::AnnexB).is_err());
    }
}
