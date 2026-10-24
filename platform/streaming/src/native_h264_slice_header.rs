//! Bounded ordinary slice-header syntax, not entropy/DPB/decoder admission.
use crate::{invalid,native_h264_bits::SliceBits,native_h264_parameters::ParameterSets,
    native_h264_slice::{SlicePrefix,SliceKind},native_h264_pps::SliceGroups};
use omniplay_contracts::Result;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ListEdit{SubtractShort(u32),AddShort(u32),LongTerm(u32)}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct WeightedReference{pub luma:Option<(i16,i16)>,pub chroma:Option<[(i16,i16);2]>}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Weights{pub luma_denom:u8,pub chroma_denom:Option<u8>,pub list0:Vec<WeightedReference>,pub list1:Vec<WeightedReference>}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum MemoryEdit{ForgetShort(u32),ForgetLong(u32),MakeLong{difference:u32,index:u8},LimitLong(u8),Reset,CurrentLong(u8)}
#[derive(Clone,Debug,PartialEq,Eq)]
pub enum ReferenceMarking{NonReference,Idr{discard_prior:bool,long_term:bool},Sliding,Adaptive(Vec<MemoryEdit>)}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Deblocking{pub disabled:u8,pub alpha:i8,pub beta:i8}
#[derive(Clone,Debug)]
pub struct SliceHeader{
    pub prefix:SlicePrefix,pub direct_spatial:Option<bool>,pub active_references:[u8;2],
    pub list0_edits:Vec<ListEdit>,pub list1_edits:Vec<ListEdit>,pub weights:Option<Weights>,
    pub marking:ReferenceMarking,pub cabac_init:Option<u8>,pub qp:i32,pub qs:Option<i32>,
    pub sp_switch:Option<bool>,pub deblocking:Deblocking,pub group_change_cycle:Option<u32>,
    /// RBSP bit counts exclude NAL header and emulation-prevention bytes.
    /// They are NOT byte offsets into the original NAL or entropy validity proof.
    pub header_rbsp_bits:usize,pub data_rbsp_bits:usize,
}
impl SliceHeader{
    pub fn parse(nal:&[u8],parameters:&ParameterSets)->Result<Self>{
        let(prefix,mut bits)=SlicePrefix::read(nal,parameters)?;
        let resolved=parameters.check(prefix.picture().parameters())?;
        let sps=resolved.sps();let pps=resolved.pps();let kind=prefix.kind();
        let intra=matches!(kind,SliceKind::I|SliceKind::Si);let b=kind==SliceKind::B;
        let direct_spatial=if b{Some(bits.flag()?)}else{None};
        let mut active_references=if intra{[0,0]}else{[pps.reference_list0,if b{pps.reference_list1}else{0}]};
        if !intra&&bits.flag()?{
            active_references[0]=bits.ue(31)? as u8+1;
            if b{active_references[1]=bits.ue(31)? as u8+1;}
        }
        let reference_limit=if prefix.picture().field().is_some(){32}else{16};
        if active_references.iter().any(|count|*count>reference_limit){return Err(invalid());}
        let max_pic_num=(1u32<<sps.frame_num_bits)*if prefix.picture().field().is_some(){2}else{1};
        let list0_edits=if intra{Vec::new()}else{list_edits(&mut bits,active_references[0],max_pic_num)?};
        let list1_edits=if b{list_edits(&mut bits,active_references[1],max_pic_num)?}else{Vec::new()};
        let weights=if (pps.weighted_prediction&&matches!(kind,SliceKind::P|SliceKind::Sp))||(pps.weighted_bipred==1&&b){
            let chroma=sps.chroma_format!=0&&!sps.separate_colour_plane;
            let luma_denom=bits.ue(7)? as u8;let chroma_denom=if chroma{Some(bits.ue(7)? as u8)}else{None};
            Some(Weights{luma_denom,chroma_denom,list0:weight_list(&mut bits,active_references[0],chroma)?,
                list1:if b{weight_list(&mut bits,active_references[1],chroma)?}else{Vec::new()}})
        }else{None};
        let marking=if !prefix.picture().reference(){ReferenceMarking::NonReference}
            else if prefix.picture().idr().is_some(){ReferenceMarking::Idr{discard_prior:bits.flag()?,long_term:bits.flag()?}}
            else if bits.flag()?{ReferenceMarking::Adaptive(memory_edits(&mut bits,max_pic_num)?)}else{ReferenceMarking::Sliding};
        let cabac_init=if pps.entropy_cabac&&!intra{Some(bits.ue(2)? as u8)}else{None};
        let depth_offset=6*(i32::from(sps.luma_depth)-8);
        let delta=bits.se((depth_offset+pps.initial_qp).abs().max((51-pps.initial_qp).abs()))?;
        let qp=pps.initial_qp+delta;
        if !(-depth_offset..=51).contains(&qp){return Err(invalid());}
        let sp_switch=if kind==SliceKind::Sp{Some(bits.flag()?)}else{None};
        let qs=if matches!(kind,SliceKind::Sp|SliceKind::Si){
            let value=pps.initial_qs+bits.se(51)?;if !(0..=51).contains(&value){return Err(invalid());}Some(value)
        }else{None};
        let mut deblocking=Deblocking{disabled:0,alpha:0,beta:0};
        if pps.deblocking_present{
            deblocking.disabled=bits.ue(2)? as u8;
            if deblocking.disabled!=1{deblocking.alpha=bits.se(6)? as i8;deblocking.beta=bits.se(6)? as i8;}
        }
        let group_change_cycle=if let Some(SliceGroups::Changing{change_rate,..})=&pps.slice_groups{
            let map_units=(sps.coded_width/16)*(sps.coded_height/16)/if sps.frame_only{1}else{2};
            let maximum=map_units.div_ceil(*change_rate);
            let width=(u32::BITS-maximum.leading_zeros())as u8;
            let cycle=bits.bits(width)?;if cycle>maximum{return Err(invalid());}Some(cycle)
        }else{None};
        let header_rbsp_bits=bits.position();
        if pps.entropy_cabac{bits.cabac_alignment()?;}
        let data_rbsp_bits=bits.position();
        Ok(Self{prefix,direct_spatial,active_references,list0_edits,list1_edits,weights,marking,cabac_init,qp,qs,sp_switch,
            deblocking,group_change_cycle,header_rbsp_bits,data_rbsp_bits})
    }
}
fn list_edits(bits:&mut SliceBits<'_>,active:u8,max_pic_num:u32)->Result<Vec<ListEdit>>{
    let mut values=Vec::new();if !bits.flag()?{return Ok(values);}
    for _ in 0..active{
        let operation=bits.ue(3)?;
        if operation==3{return Ok(values);}
        values.push(match operation{
            0=>ListEdit::SubtractShort(bits.ue(max_pic_num-1)?+1),
            1=>ListEdit::AddShort(bits.ue(max_pic_num-1)?+1),
            2=>ListEdit::LongTerm(bits.ue(max_pic_num-1)?),
            _=>return Err(invalid()),
        });
    }
    if bits.ue(3)?!=3{return Err(invalid());}Ok(values)
}
fn pair(bits:&mut SliceBits<'_>)->Result<(i16,i16)>{
    let weight=bits.se(128)?;let offset=bits.se(128)?;
    if weight>127||offset>127{return Err(invalid());}Ok((weight as i16,offset as i16))
}
fn weight_list(bits:&mut SliceBits<'_>,active:u8,chroma_present:bool)->Result<Vec<WeightedReference>>{
    let mut values=Vec::new();
    for _ in 0..active{
        let luma=if bits.flag()?{Some(pair(bits)?)}else{None};
        let chroma=if chroma_present&&bits.flag()?{Some([pair(bits)?,pair(bits)?])}else{None};
        values.push(WeightedReference{luma,chroma});
    }Ok(values)
}
fn memory_edits(bits:&mut SliceBits<'_>,max_pic_num:u32)->Result<Vec<MemoryEdit>>{
    let mut values=Vec::new();let mut reset=false;let mut current_long=false;let mut limit_long=false;let mut prior_edit=false;
    for _ in 0..64{
        let operation=bits.ue(6)?;
        if operation==0{return Ok(values);}
        if matches!(operation,1..=3){if reset{return Err(invalid());}prior_edit=true;}
        values.push(match operation{
            0=>return Err(invalid()),
            1=>MemoryEdit::ForgetShort(bits.ue(max_pic_num-1)?+1),
            2=>MemoryEdit::ForgetLong(bits.ue(max_pic_num-1)?),
            3=>MemoryEdit::MakeLong{difference:bits.ue(max_pic_num-1)?+1,index:bits.ue(15)? as u8},
            4=>{if limit_long{return Err(invalid());}limit_long=true;MemoryEdit::LimitLong(bits.ue(16)? as u8)},
            5=>{if reset||prior_edit||current_long{return Err(invalid());}reset=true;MemoryEdit::Reset},
            _=>{if current_long{return Err(invalid());}current_long=true;MemoryEdit::CurrentLong(bits.ue(15)? as u8)},
        });
    }
    if bits.ue(6)?!=0{return Err(invalid());}Ok(values)
}
