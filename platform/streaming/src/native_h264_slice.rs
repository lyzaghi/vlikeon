//! Ordinary nonpartitioned slice identity prefix only. Does not parse reference
//! list edits, marking, weights, full header, entropy data or prove decodability.
use crate::{invalid,native_h264_bits::SliceBits,native_h264_parameters::{ParameterSets,ParameterBinding},native_h264_sps::PictureOrder};
use omniplay_contracts::Result;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum SliceKind{P,B,I,Sp,Si}
#[derive(Clone,Debug,PartialEq,Eq)]
pub enum PictureOrderPrefix{Lsb{value:u32,bottom_delta:i32},Cycle{delta0:i32,delta1:i32},DecodeOrder}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct PictureIdentity{
    parameters:ParameterBinding,frame_num:u32,field:Option<bool>,colour_plane:Option<u8>,
    reference:bool,idr:Option<u16>,order:PictureOrderPrefix,redundant:u8,
}
impl PictureIdentity{
    pub fn parameters(&self)->&ParameterBinding{&self.parameters}
    pub fn frame_num(&self)->u32{self.frame_num}
    /// None: frame; Some(false): top field; Some(true): bottom field.
    pub fn field(&self)->Option<bool>{self.field}
    pub fn colour_plane(&self)->Option<u8>{self.colour_plane}
    pub fn reference(&self)->bool{self.reference}
    pub fn idr(&self)->Option<u16>{self.idr}
    pub fn order(&self)->&PictureOrderPrefix{&self.order}
    pub fn redundant(&self)->u8{self.redundant}
}
#[derive(Clone,Debug)]
pub struct SlicePrefix{first_mb:u32,kind:SliceKind,same_kind:bool,nal_reference_idc:u8,picture:PictureIdentity}
impl SlicePrefix{
    pub fn first_mb(&self)->u32{self.first_mb}
    pub fn kind(&self)->SliceKind{self.kind}
    pub fn same_kind_required(&self)->bool{self.same_kind}
    pub fn nal_reference_idc(&self)->u8{self.nal_reference_idc}
    pub fn picture(&self)->&PictureIdentity{&self.picture}
    /// Binds the prefix to actual retained parameter versions. Equality of
    /// identities is only a picture-grouping prerequisite, never full AU proof.
    pub fn parse(nal:&[u8],parameters:&ParameterSets)->Result<Self>{
        let mut bits=SliceBits::from_nal(nal)?;
        let first_mb=bits.ue(262_143)?;let slice_type=bits.ue(9)?;
        let kind=match slice_type%5{0=>SliceKind::P,1=>SliceKind::B,2=>SliceKind::I,3=>SliceKind::Sp,_=>SliceKind::Si};
        let pps_id=bits.ue(255)? as u8;let resolved=parameters.resolve(pps_id)?;
        let sps=resolved.sps();let pps=resolved.pps();let idr_nal=nal[0]&31==5;
        if (idr_nal||sps.reference_frames==0)&&!matches!(kind,SliceKind::I|SliceKind::Si){return Err(invalid());}
        let colour_plane=if sps.separate_colour_plane{let plane=bits.bits(2)? as u8;if plane>2{return Err(invalid());}Some(plane)}else{None};
        let frame_num=bits.bits(sps.frame_num_bits)?;
        if idr_nal&&frame_num!=0{return Err(invalid());}
        let field=if !sps.frame_only&&bits.flag()?{Some(bits.flag()?)}else{None};
        let picture_mbs=(sps.coded_width/16)*(sps.coded_height/16)/if field.is_some(){2}else{1};
        let address=first_mb*if sps.adaptive_fields&&field.is_none(){2}else{1};
        if address>=picture_mbs{return Err(invalid());}
        let idr=if idr_nal{Some(bits.ue(65_535)? as u16)}else{None};
        let order=match &sps.picture_order{
            PictureOrder::Lsb{bits:width}=>{
                let value=bits.bits(*width)?;
                let bottom_delta=if pps.bottom_field_order_present&&field.is_none(){bits.se(i32::MAX)?}else{0};
                PictureOrderPrefix::Lsb{value,bottom_delta}
            },
            PictureOrder::Cycle{always_zero,..}=>{
                let delta0=if *always_zero{0}else{bits.se(i32::MAX)?};
                let delta1=if !*always_zero&&pps.bottom_field_order_present&&field.is_none(){bits.se(i32::MAX)?}else{0};
                PictureOrderPrefix::Cycle{delta0,delta1}
            },
            PictureOrder::DecodeOrder=>PictureOrderPrefix::DecodeOrder,
        };
        let redundant=if pps.redundant_picture_count{bits.ue(127)? as u8}else{0};
        let nal_reference_idc=(nal[0]>>5)&3;
        let picture=PictureIdentity{parameters:resolved.binding(),frame_num,field,colour_plane,reference:nal_reference_idc!=0,idr,order,redundant};
        Ok(Self{first_mb,kind,same_kind:slice_type>=5,nal_reference_idc,picture})
    }
}
