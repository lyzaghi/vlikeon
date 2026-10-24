//! Frame-picture reference marking metadata. Field/MBAFF, reference-list/POC,
//! output buffering and actual decode completion remain separate integrations.
use crate::{invalid,native_h264_parameters::{ParameterSets,ParameterBinding},native_h264_picture::PrimaryPictureHeaders,
    native_h264_slice::PictureIdentity,native_h264_slice_header::{ReferenceMarking,MemoryEdit}};
use omniplay_contracts::Result;
use std::sync::Arc;

#[derive(Clone,Debug)]
pub struct FrameReference{serial:u64,frame_num:u32,long_term:Option<u8>}
impl FrameReference{
    pub fn serial(&self)->u64{self.serial}
    pub fn frame_num(&self)->u32{self.frame_num}
    pub fn long_term_index(&self)->Option<u8>{self.long_term}
    pub fn picture_number(&self,current:u32,maximum:u32)->i64{
        if self.frame_num>current{i64::from(self.frame_num)-i64::from(maximum)}else{i64::from(self.frame_num)}
    }
}
#[derive(Clone,Copy,PartialEq,Eq)]
struct Configuration{frame_num_bits:u8,references:u8,width:u32,height:u32}
#[derive(Clone,Default)]
struct State{entries:Vec<FrameReference>,maximum_long:Option<u8>,configuration:Option<Configuration>,last:Option<PictureIdentity>,started:bool}
pub struct FrameReferences{identity:Arc<()>,version:u64,state:State,closed:bool}
/// A staged, table-local transition. Dropping it leaves retained references
/// unchanged. It cannot be replayed after another successful commit.
pub struct FrameReferenceUpdate{identity:Arc<()>,expected:u64,binding:ParameterBinding,next:State}
impl Default for FrameReferences{fn default()->Self{Self::new()}}
impl FrameReferences{
    pub fn new()->Self{Self{identity:Arc::new(()),version:0,state:State::default(),closed:false}}
    pub fn close(&mut self){self.closed=true;self.state=State::default();}
    pub fn entries(&self)->&[FrameReference]{&self.state.entries}
    /// Stage marking AFTER the caller has established actual picture/reference
    /// decode validity. Header parsing alone does not establish that prerequisite.
    /// This component handles complete progressive frame pictures only; field
    /// and MBAFF pictures need the complementary-field reference coordinator.
    pub fn prepare(&self,picture:&PrimaryPictureHeaders,parameters:&ParameterSets)->Result<FrameReferenceUpdate>{
        if self.closed||self.version>=10_000_000{return Err(invalid());}
        let identity=picture.identity();let resolved=parameters.check(identity.parameters())?;let sps=resolved.sps();
        if !sps.frame_only||identity.field().is_some()||identity.colour_plane().is_some()||identity.redundant()!=0{return Err(invalid());}
        let config=Configuration{frame_num_bits:sps.frame_num_bits,references:sps.reference_frames,width:sps.coded_width,height:sps.coded_height};
        let header=picture.headers().first().ok_or_else(invalid)?;
        let idr=identity.idr().is_some();
        if !idr&&(!self.state.started||self.state.configuration!=Some(config)){return Err(invalid());}
        if self.state.last.as_ref()==Some(identity){return Err(invalid());}
        let mut next=self.state.clone();
        if idr{next=State{configuration:Some(config),started:true,..State::default()};}
        let current=identity.frame_num();let maximum=1u32<<config.frame_num_bits;
        let capacity=usize::from(config.references.max(1));
        let mut current_long=None;let mut reset=false;
        match &header.marking{
            ReferenceMarking::NonReference=>{if identity.reference()||idr{return Err(invalid());}},
            ReferenceMarking::Idr{long_term,..}=>{
                if !idr||!identity.reference()||current!=0||*long_term&&config.references==0{return Err(invalid());}
                if *long_term{next.maximum_long=Some(0);current_long=Some(0);}
            },
            ReferenceMarking::Sliding=>{
                if idr||!identity.reference(){return Err(invalid());}
                if next.entries.len()>=capacity{
                    let index=next.entries.iter().enumerate().filter(|(_,entry)|entry.long_term.is_none())
                        .min_by_key(|(_,entry)|entry.picture_number(current,maximum)).map(|(index,_)|index).ok_or_else(invalid)?;
                    next.entries.remove(index);
                }
            },
            ReferenceMarking::Adaptive(edits)=>{
                if idr||!identity.reference()||edits.len()>64{return Err(invalid());}
                let mut limit_seen=false;let mut short_edit=false;
                for edit in edits{
                    match *edit{
                        MemoryEdit::ForgetShort(difference)=>{
                            if reset{return Err(invalid());}short_edit=true;
                            let index=short_index(&next,current,maximum,difference)?;next.entries.remove(index);
                        },
                        MemoryEdit::ForgetLong(number)=>{
                            if reset{return Err(invalid());}short_edit=true;
                            if number>15||current_long==Some(number as u8){return Err(invalid());}
                            let index=next.entries.iter().position(|entry|entry.long_term==Some(number as u8)).ok_or_else(invalid)?;next.entries.remove(index);
                        },
                        MemoryEdit::MakeLong{difference,index}=>{
                            if reset||current_long==Some(index){return Err(invalid());}short_edit=true;long_allowed(&next,index)?;
                            let source=short_index(&next,current,maximum,difference)?;let serial=next.entries[source].serial;
                            next.entries.retain(|entry|entry.long_term!=Some(index));
                            next.entries.iter_mut().find(|entry|entry.serial==serial).ok_or_else(invalid)?.long_term=Some(index);
                        },
                        MemoryEdit::LimitLong(plus_one)=>{
                            if limit_seen||plus_one>config.references||current_long.is_some_and(|index|index>=plus_one){return Err(invalid());}
                            limit_seen=true;next.maximum_long=plus_one.checked_sub(1);
                            next.entries.retain(|entry|entry.long_term.is_none_or(|index|index<plus_one));
                        },
                        MemoryEdit::Reset=>{
                            if reset||short_edit||current_long.is_some(){return Err(invalid());}
                            next.entries.clear();next.maximum_long=None;reset=true;
                        },
                        MemoryEdit::CurrentLong(index)=>{
                            if current_long.is_some(){return Err(invalid());}long_allowed(&next,index)?;
                            next.entries.retain(|entry|entry.long_term!=Some(index));current_long=Some(index);
                        },
                    }
                }
            },
        }
        if identity.reference(){
            if next.entries.len()>=capacity{return Err(invalid());}
            let frame_num=if reset{0}else{current};
            if current_long.is_none()&&next.entries.iter().any(|entry|entry.long_term.is_none()&&entry.frame_num==frame_num){return Err(invalid());}
            next.entries.push(FrameReference{serial:self.version+1,frame_num,long_term:current_long});
        }
        next.last=Some(identity.clone());
        Ok(FrameReferenceUpdate{identity:self.identity.clone(),expected:self.version,binding:identity.parameters().clone(),next})
    }
    /// Native decode owner commits the same staged transition at its chosen
    /// confirmed decode boundary. This does not acknowledge decoder completion.
    pub fn commit(&mut self,update:FrameReferenceUpdate,parameters:&ParameterSets)->Result<()>{
        if self.closed||!Arc::ptr_eq(&self.identity,&update.identity)||self.version!=update.expected||self.version>=10_000_000{return Err(invalid());}
        parameters.check(&update.binding)?;
        self.state=update.next;self.version+=1;Ok(())
    }
}
fn long_allowed(state:&State,index:u8)->Result<()>{if state.maximum_long.is_none_or(|maximum|index>maximum){Err(invalid())}else{Ok(())}}
fn short_index(state:&State,current:u32,maximum:u32,difference:u32)->Result<usize>{
    if difference==0||difference>maximum{return Err(invalid());}
    let target=i64::from(current)-i64::from(difference);
    state.entries.iter().position(|entry|entry.long_term.is_none()&&entry.picture_number(current,maximum)==target).ok_or_else(invalid)
}
