//! Bounded SPS/PPS and slice-prefix readers, not decoder/stream admission.
//! H.264 sections 7.3.1, 7.3.2 and 9.1. Full trailing validation is parameter-set
//! only; slice CABAC padding and extension NALs need separate syntax handling.
use crate::invalid;
use omniplay_contracts::{ErrorCode,PlatformError,Result};

const MAX_PARAMETER_NAL:usize=64*1024;
pub struct ParameterSetBits{kind:u8,rbsp:Vec<u8>,offset:usize,failed:bool}
impl ParameterSetBits{
    /// Supply a single unframed SPS/PPS NAL including its header, never Annex B
    /// delimiters or an RTP aggregate. No native authority is produced here.
    pub fn from_nal(nal:&[u8])->Result<Self>{
        if !(2..=MAX_PARAMETER_NAL).contains(&nal.len())||nal[0]&0x80!=0||nal[0]&0x60==0||!matches!(nal[0]&31,7|8)||nal.last()==Some(&0){return Err(invalid());}
        let rbsp=decode_payload(nal,nal.len()-1)?;
        if rbsp.is_empty()||rbsp.last()==Some(&0){return Err(invalid());}
        Ok(Self{kind:nal[0]&31,rbsp,offset:0,failed:false})
    }
    pub fn nal_type(&self)->u8{self.kind}
    fn reject<T>(&mut self)->Result<T>{self.failed=true;Err(invalid())}
    pub fn bits(&mut self,count:u8)->Result<u32>{
        if self.failed||count>32||usize::from(count)>self.rbsp.len()*8-self.offset{return self.reject();}
        let mut value=0u32;
        for _ in 0..count{value=(value<<1)|u32::from((self.rbsp[self.offset/8]>>(7-self.offset%8))&1);self.offset+=1;}
        Ok(value)
    }
    pub fn flag(&mut self)->Result<bool>{Ok(self.bits(1)?!=0)}
    /// At most 31 leading zeroes, then a bounded suffix. Syntax callers provide
    /// the field's own semantic maximum; do not allocate using an unchecked ue.
    pub fn ue(&mut self,maximum:u32)->Result<u32>{
        let mut leading=0u8;
        while self.bits(1)?==0{leading+=1;if leading>31{return self.reject();}}
        let suffix=self.bits(leading)?;
        let value=((1u64<<leading)-1)+u64::from(suffix);
        if value>u64::from(maximum){return self.reject();}Ok(value as u32)
    }
    pub fn se(&mut self,maximum_absolute:i32)->Result<i32>{
        if maximum_absolute<0{return self.reject();}
        let code=self.ue(u32::MAX-1)?;
        let magnitude=(i64::from(code)+1)/2;
        if magnitude>i64::from(maximum_absolute){return self.reject();}
        Ok(if code&1==0{-(magnitude as i32)}else{magnitude as i32})
    }
    /// Distinguish optional PPS extension data from the final stop/alignment bits.
    pub fn more_data(&self)->Result<bool>{
        if self.failed{return Err(invalid());}
        let remaining=self.rbsp.len()*8-self.offset;
        if remaining==0{return Err(invalid());}
        if remaining>8{return Ok(true);}
        let mut value=0u8;
        for offset in self.offset..self.rbsp.len()*8{value=(value<<1)|((self.rbsp[offset/8]>>(7-offset%8))&1);}
        Ok(value!=(1u8<<(remaining-1)))
    }
    /// Consume the exact SPS/PPS trailing-bit syntax, with no extra bytes.
    /// This proves framing only; it does not prove parsed SPS/PPS semantics.
    pub fn finish(mut self)->Result<()>{
        if self.bits(1)?!=1{return self.reject();}
        while self.offset%8!=0{if self.bits(1)?!=0{return self.reject();}}
        if self.offset!=self.rbsp.len()*8{return self.reject();}Ok(())
    }
}
/// Only the bounded prefix is inspected. Slice entropy data, CABAC alignment,
/// end padding and trailing syntax are deliberately NOT validated by this reader.
pub(crate) struct SliceBits<'a>{nal:&'a[u8],index:usize,zeros:u8,current:u8,left:u8,offset:usize,failed:bool}
impl<'a> SliceBits<'a>{
    pub(crate) fn from_nal(nal:&'a[u8])->Result<Self>{
        if !(2..=2*1024*1024).contains(&nal.len())||nal[0]&0x80!=0||!matches!(nal[0]&31,1|5)||nal[0]&31==5&&nal[0]&0x60==0{return Err(invalid());}
        Ok(Self{nal,index:1,zeros:0,current:0,left:0,offset:0,failed:false})
    }
    fn reject<T>(&mut self)->Result<T>{self.failed=true;Err(invalid())}
    fn byte(&mut self)->Result<u8>{
        loop{
            if self.failed||self.index>=self.nal.len()||self.index>4096{return self.reject();}
            let byte=self.nal[self.index];self.index+=1;
            if self.zeros==2{
                if byte<=2{return self.reject();}
                if byte==3{
                    if self.nal.get(self.index).is_none_or(|next|*next>3){return self.reject();}
                    self.zeros=0;continue;
                }
            }
            self.zeros=if byte==0{self.zeros+1}else{0};return Ok(byte);
        }
    }
    pub(crate) fn bits(&mut self,count:u8)->Result<u32>{
        if self.failed||count>32{return self.reject();}
        let mut value=0u32;
        for _ in 0..count{
            if self.left==0{self.current=self.byte()?;self.left=8;}
            self.left-=1;self.offset+=1;value=(value<<1)|u32::from((self.current>>self.left)&1);
        }Ok(value)
    }
    pub(crate) fn flag(&mut self)->Result<bool>{Ok(self.bits(1)?!=0)}
    pub(crate) fn position(&self)->usize{self.offset}
    pub(crate) fn cabac_alignment(&mut self)->Result<()>{
        while self.offset%8!=0{if self.bits(1)?!=1{return self.reject();}}Ok(())
    }
    pub(crate) fn ue(&mut self,maximum:u32)->Result<u32>{
        let mut leading=0u8;
        while self.bits(1)?==0{leading+=1;if leading>31{return self.reject();}}
        let value=((1u64<<leading)-1)+u64::from(self.bits(leading)?);
        if value>u64::from(maximum){return self.reject();}Ok(value as u32)
    }
    pub(crate) fn se(&mut self,maximum:i32)->Result<i32>{
        if maximum<0{return self.reject();}
        let code=self.ue(u32::MAX-1)?;let magnitude=(i64::from(code)+1)/2;
        if magnitude>i64::from(maximum){return self.reject();}
        Ok(if code&1==0{-(magnitude as i32)}else{magnitude as i32})
    }
}
fn decode_payload(nal:&[u8],limit:usize)->Result<Vec<u8>>{
    let end=nal.len().min(limit+1);let mut rbsp=Vec::new();
    rbsp.try_reserve_exact(end-1).map_err(|_|PlatformError::new(ErrorCode::ResourceLimit,"The H.264 syntax-reader budget is unavailable"))?;
    let mut zeros=0;let mut index=1;
    while index<end{
        let byte=nal[index];
        if zeros==2{
            if byte<=2{return Err(invalid());}
            if byte==3{
                if nal.get(index+1).is_none_or(|next|*next>3){return Err(invalid());}
                zeros=0;index+=1;continue;
            }
        }
        rbsp.push(byte);zeros=if byte==0{zeros+1}else{0};index+=1;
    }Ok(rbsp)
}
