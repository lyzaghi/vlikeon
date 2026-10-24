//! Native picture-order derivation (types 0/1/2). Metadata only: no output queue,
//! gap-picture generation, complementary-field pairing or decoder completion.
use crate::{invalid,native_h264_parameters::{ParameterSets,ParameterBinding},native_h264_sps::PictureOrder,
    native_h264_slice::{PictureIdentity,PictureOrderPrefix},native_h264_slice_header::{SliceHeader,ReferenceMarking,MemoryEdit}};
use omniplay_contracts::Result;
use std::sync::Arc;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct PictureCounts{pub top:Option<i64>,pub bottom:Option<i64>}
impl PictureCounts{
    pub fn picture(&self)->Result<i64>{match (self.top,self.bottom){(Some(a),Some(b))=>Ok(a.min(b)),(Some(a),None)|(None,Some(a))=>Ok(a),_=>Err(invalid())}}
    fn shifted(self,amount:i64)->Result<Self>{Ok(Self{top:self.top.map(|v|number(i128::from(v)-i128::from(amount))).transpose()?,bottom:self.bottom.map(|v|number(i128::from(v)-i128::from(amount))).transpose()?})}
}
#[derive(Clone,PartialEq,Eq)]
struct Configuration{frame_bits:u8,order:PictureOrder}
#[derive(Clone,Default)]
struct State{configuration:Option<Configuration>,previous_frame:u32,frame_offset:i64,reference_msb:i64,reference_lsb:i64,last:Option<PictureIdentity>}
pub struct PictureOrderState{identity:Arc<()>,version:u64,state:State,closed:bool}
pub struct PictureOrderUpdate{identity:Arc<()>,expected:u64,binding:ParameterBinding,next:State,decoded:PictureCounts,retained:PictureCounts,reset:bool}
impl PictureOrderUpdate{
    /// Before MMCO5 normalization, for current-picture reference-list formation.
    pub fn decoded(&self)->PictureCounts{self.decoded}
    /// After MMCO5 normalization, for retaining the newly decoded reference.
    pub fn retained(&self)->PictureCounts{self.retained}
    pub fn resets_references(&self)->bool{self.reset}
}
impl Default for PictureOrderState{fn default()->Self{Self::new()}}
impl PictureOrderState{
    pub fn new()->Self{Self{identity:Arc::new(()),version:0,state:State::default(),closed:false}}
    pub fn close(&mut self){self.closed=true;self.state=State::default();}
    /// Stage against the exact retained parameter binding. No state is advanced
    /// until the owning decoder commits at its confirmed picture boundary.
    pub fn prepare(&self,header:&SliceHeader,parameters:&ParameterSets)->Result<PictureOrderUpdate>{
        if self.closed||self.version>=10_000_000{return Err(invalid());}
        let identity=header.prefix.picture();let resolved=parameters.check(identity.parameters())?;let sps=resolved.sps();
        if identity.colour_plane().is_some()||identity.redundant()!=0{return Err(invalid());}
        let config=Configuration{frame_bits:sps.frame_num_bits,order:sps.picture_order.clone()};
        let idr=identity.idr().is_some();
        if !idr&&self.state.configuration.as_ref()!=Some(&config){return Err(invalid());}
        if self.state.last.as_ref()==Some(identity){return Err(invalid());}
        let mut next=if idr{State::default()}else{self.state.clone()};
        let maximum=1i64<<sps.frame_num_bits;let current=identity.frame_num();
        let frame_offset=if idr{0}else if self.state.previous_frame>current{
            number(i128::from(self.state.frame_offset)+i128::from(maximum))?
        }else{self.state.frame_offset};
        let field=identity.field();
        let reset=matches!(&header.marking,ReferenceMarking::Adaptive(edits) if edits.contains(&MemoryEdit::Reset));
        if reset&&!identity.reference(){return Err(invalid());}
        let decoded=match (&sps.picture_order,identity.order()){
            (PictureOrder::Lsb{bits},PictureOrderPrefix::Lsb{value,bottom_delta})=>{
                let maximum=1i64<<*bits;let lsb=i64::from(*value);
                let previous_msb=if idr{0}else{self.state.reference_msb};
                let previous_lsb=if idr{0}else{self.state.reference_lsb};
                let msb=if lsb<previous_lsb&&previous_lsb-lsb>=maximum/2{
                    number(i128::from(previous_msb)+i128::from(maximum))?
                }else if lsb>previous_lsb&&lsb-previous_lsb>maximum/2{
                    number(i128::from(previous_msb)-i128::from(maximum))?
                }else{previous_msb};
                let base=number(i128::from(msb)+i128::from(lsb))?;
                let counts=PictureCounts{top:if field==Some(true){None}else{Some(base)},bottom:match field{
                    Some(false)=>None,Some(true)=>Some(base),None=>Some(number(i128::from(base)+i128::from(*bottom_delta))?),
                }};
                if identity.reference(){next.reference_msb=msb;next.reference_lsb=lsb;}
                counts
            },
            (PictureOrder::Cycle{non_reference,top_to_bottom,offsets,..},PictureOrderPrefix::Cycle{delta0,delta1})=>{
                let mut absolute=if offsets.is_empty(){0}else{number(i128::from(frame_offset)+i128::from(current))?};
                if !identity.reference()&&absolute>0{absolute-=1;}
                let mut expected=0i128;
                if absolute>0{
                    let count=offsets.len()as i64;
                    let cycle=(absolute-1)/count;let within=((absolute-1)%count)as usize;
                    let sum:i128=offsets.iter().map(|value|i128::from(*value)).sum();
                    expected=i128::from(cycle)*sum+offsets[..=within].iter().map(|value|i128::from(*value)).sum::<i128>();
                }
                if !identity.reference(){expected+=i128::from(*non_reference);}
                let top=number(expected+i128::from(*delta0))?;
                let bottom=number(expected+i128::from(*top_to_bottom)+i128::from(*delta0)+if field.is_none(){i128::from(*delta1)}else{0})?;
                PictureCounts{top:if field==Some(true){None}else{Some(top)},bottom:if field==Some(false){None}else{Some(bottom)}}
            },
            (PictureOrder::DecodeOrder,PictureOrderPrefix::DecodeOrder)=>{
                let value=if idr{0}else{number(2*(i128::from(frame_offset)+i128::from(current))-if identity.reference(){0}else{1})?};
                PictureCounts{top:if field==Some(true){None}else{Some(value)},bottom:if field==Some(false){None}else{Some(value)}}
            },
            _=>return Err(invalid()),
        };
        let retained=if reset{decoded.shifted(decoded.picture()?)?}else{decoded};
        next.configuration=Some(config);next.previous_frame=if reset{0}else{current};next.frame_offset=if reset{0}else{frame_offset};
        if reset{
            next.reference_msb=0;
            next.reference_lsb=if field==Some(true){0}else{retained.top.ok_or_else(invalid)?};
        }
        next.last=Some(identity.clone());
        Ok(PictureOrderUpdate{identity:self.identity.clone(),expected:self.version,binding:identity.parameters().clone(),next,decoded,retained,reset})
    }
    pub fn commit(&mut self,update:PictureOrderUpdate,parameters:&ParameterSets)->Result<()>{
        if self.closed||!Arc::ptr_eq(&self.identity,&update.identity)||self.version!=update.expected||self.version>=10_000_000{return Err(invalid());}
        parameters.check(&update.binding)?;self.state=update.next;self.version+=1;Ok(())
    }
}
fn number(value:i128)->Result<i64>{
    if !(-(1i128<<50)..=(1i128<<50)).contains(&value){return Err(invalid());}Ok(value as i64)
}
