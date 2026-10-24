//! Ordinary non-VCL syntax envelopes. SEI payload meaning is retained for the
//! native negotiation/decoder adapter, not silently interpreted as authority.
use crate::{invalid,native_h264_bits::decode_payload,native_h264_slice::SliceKind};
use omniplay_contracts::Result;
use std::ops::Range;

pub struct SeiMessage{pub payload_type:u32,range:Range<usize>}
pub struct Sei{rbsp:Vec<u8>,messages:Vec<SeiMessage>}
impl Sei{
    pub fn messages(&self)->impl ExactSizeIterator<Item=(u32,&[u8])>+'_ {
        self.messages.iter().map(|message|(message.payload_type,&self.rbsp[message.range.clone()]))
    }
}
pub enum Auxiliary{Delimiter{primary_type:u8},Sei(Sei),EndSequence,EndStream,Filler}
impl Auxiliary{
    pub fn parse(nal:&[u8])->Result<Self>{
        if nal.is_empty()||nal.len()>2*1024*1024||nal[0]&0xe0!=0{return Err(invalid());}
        match nal[0]&31{
            9=>{if nal.len()!=2||nal[1]&31!=16{return Err(invalid());}Ok(Self::Delimiter{primary_type:nal[1]>>5})},
            10=>{if nal.len()!=1{return Err(invalid());}Ok(Self::EndSequence)},
            11=>{if nal.len()!=1{return Err(invalid());}Ok(Self::EndStream)},
            12=>{
                if nal.len()<2||nal.last()!=Some(&128)||!nal[1..nal.len()-1].iter().all(|byte|*byte==255){return Err(invalid());}
                Ok(Self::Filler)
            },
            6=>{
                if nal.len()<4||nal.len()>64*1024{return Err(invalid());}
                let rbsp=decode_payload(nal,nal.len()-1)?;
                if rbsp.last()!=Some(&128){return Err(invalid());}
                let end=rbsp.len()-1;let mut offset=0;let mut messages=Vec::new();
                while offset<end{
                    if messages.len()>=256{return Err(invalid());}
                    let payload_type=extended(&rbsp,&mut offset,end)?;
                    let size=extended(&rbsp,&mut offset,end)? as usize;
                    let next=offset.checked_add(size).filter(|next|*next<=end).ok_or_else(invalid)?;
                    messages.push(SeiMessage{payload_type,range:offset..next});offset=next;
                }
                if messages.is_empty(){return Err(invalid());}Ok(Self::Sei(Sei{rbsp,messages}))
            },
            _=>Err(invalid()),
        }
    }
    pub fn permits_slice(&self,kind:SliceKind)->bool{
        let Self::Delimiter{primary_type}=self else{return false;};
        match *primary_type{
            0=>kind==SliceKind::I,
            1=>matches!(kind,SliceKind::I|SliceKind::P),
            2=>matches!(kind,SliceKind::I|SliceKind::P|SliceKind::B),
            3=>kind==SliceKind::Si,
            4=>matches!(kind,SliceKind::Si|SliceKind::Sp),
            5=>matches!(kind,SliceKind::I|SliceKind::Si),
            6=>kind!=SliceKind::B,
            7=>true,
            _=>false,
        }
    }
}
fn extended(bytes:&[u8],offset:&mut usize,end:usize)->Result<u32>{
    let mut value=0u32;
    loop{
        if *offset>=end{return Err(invalid());}
        let byte=bytes[*offset];*offset+=1;value=value.checked_add(u32::from(byte)).ok_or_else(invalid)?;
        if byte!=255{return Ok(value);}
    }
}
