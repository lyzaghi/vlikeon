//! Frame-picture reference marking metadata. Field/MBAFF, POC derivation,
//! output buffering and actual decode completion remain separate integrations.
use crate::{invalid,native_h264_parameters::{ParameterSets,ParameterBinding},native_h264_picture::PrimaryPictureHeaders,
    native_h264_slice::{PictureIdentity,SliceKind},native_h264_slice_header::{ReferenceMarking,MemoryEdit,SliceHeader,ListEdit}};
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
/// Actual derived picture orders from the native POC owner, keyed by retained
/// serials, not remote timestamps. This type does not calculate or verify POC.
pub struct FramePictureOrders{pub current:i64,pub references:Vec<(u64,i64)>}
pub struct FramePredictionLists{identity:Arc<()>,version:u64,binding:ParameterBinding,list0:Vec<u64>,list1:Vec<u64>}
impl FramePredictionLists{
    pub fn list0(&self)->&[u64]{&self.list0}
    pub fn list1(&self)->&[u64]{&self.list1}
}
impl Default for FrameReferences{fn default()->Self{Self::new()}}
impl FrameReferences{
    pub fn new()->Self{Self{identity:Arc::new(()),version:0,state:State::default(),closed:false}}
    pub fn close(&mut self){self.closed=true;self.state=State::default();}
    pub fn entries(&self)->&[FrameReference]{&self.state.entries}
    /// Build lists from PRE-marking reference state. B pictures require actual
    /// picture orders; arrival/RTP timestamps must never stand in for POC.
    pub fn prediction_lists(&self,header:&SliceHeader,parameters:&ParameterSets,orders:Option<&FramePictureOrders>)->Result<FramePredictionLists>{
        if self.closed{return Err(invalid());}
        let picture=header.prefix.picture();let resolved=parameters.check(picture.parameters())?;let sps=resolved.sps();
        if !sps.frame_only||picture.field().is_some()||picture.colour_plane().is_some()||picture.redundant()!=0{return Err(invalid());}
        let kind=header.prefix.kind();let mut list0=Vec::new();let mut list1=Vec::new();
        if !matches!(kind,SliceKind::I|SliceKind::Si){
            let config=Configuration{frame_num_bits:sps.frame_num_bits,references:sps.reference_frames,width:sps.coded_width,height:sps.coded_height};
            if !self.state.started||self.state.configuration!=Some(config)||self.state.entries.is_empty(){return Err(invalid());}
            let current=picture.frame_num();let maximum=1u32<<sps.frame_num_bits;
            let mut short:Vec<_>=self.state.entries.iter().filter(|entry|entry.long_term.is_none()).collect();
            let mut long:Vec<_>=self.state.entries.iter().filter(|entry|entry.long_term.is_some()).collect();
            long.sort_by_key(|entry|entry.long_term);
            if kind==SliceKind::B{
                let orders=orders.ok_or_else(invalid)?;
                if orders.references.len()!=self.state.entries.len(){return Err(invalid());}
                let mut seen=std::collections::BTreeSet::new();
                for (serial,_) in &orders.references{
                    if !seen.insert(*serial)||!self.state.entries.iter().any(|entry|entry.serial==*serial){return Err(invalid());}
                }
                let by_serial:std::collections::BTreeMap<_,_>=orders.references.iter().copied().collect();
                // Exact membership was checked above for every retained serial.
                let order=|entry:&FrameReference|by_serial[&entry.serial];
                short.sort_by_key(|entry|order(entry));
                let before:Vec<_>=short.iter().filter(|entry|order(entry)<orders.current).rev().map(|entry|entry.serial).collect();
                let after:Vec<_>=short.iter().filter(|entry|order(entry)>=orders.current).map(|entry|entry.serial).collect();
                list0.extend_from_slice(&before);list0.extend_from_slice(&after);
                list1.extend(short.iter().filter(|entry|order(entry)>orders.current).map(|entry|entry.serial));
                list1.extend(short.iter().filter(|entry|order(entry)<=orders.current).rev().map(|entry|entry.serial));
                list0.extend(long.iter().map(|entry|entry.serial));list1.extend(long.iter().map(|entry|entry.serial));
                if list0==list1&&list1.len()>1{list1.swap(0,1);}
            }else{
                short.sort_by_key(|entry|std::cmp::Reverse(entry.picture_number(current,maximum)));
                list0.extend(short.iter().map(|entry|entry.serial));list0.extend(long.iter().map(|entry|entry.serial));
            }
            edit_list(&mut list0,&header.list0_edits,header.active_references[0],&self.state,current,maximum)?;
            if kind==SliceKind::B{edit_list(&mut list1,&header.list1_edits,header.active_references[1],&self.state,current,maximum)?;}
            else if header.active_references[1]!=0||!header.list1_edits.is_empty(){return Err(invalid());}
        }else if header.active_references!=[0,0]||!header.list0_edits.is_empty()||!header.list1_edits.is_empty(){return Err(invalid());}
        Ok(FramePredictionLists{identity:self.identity.clone(),version:self.version,binding:picture.parameters().clone(),list0,list1})
    }
    pub fn recheck_lists(&self,lists:&FramePredictionLists,parameters:&ParameterSets)->Result<()>{
        if self.closed||!Arc::ptr_eq(&self.identity,&lists.identity)||self.version!=lists.version{return Err(invalid());}
        parameters.check(&lists.binding)?;Ok(())
    }
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
fn edit_list(list:&mut Vec<u64>,edits:&[ListEdit],active:u8,state:&State,current:u32,maximum:u32)->Result<()>{
    if active==0||active>16||edits.len()>usize::from(active){return Err(invalid());}
    let mut predicted=current;
    for (index,edit) in edits.iter().enumerate(){
        let serial=match *edit{
            ListEdit::SubtractShort(difference)|ListEdit::AddShort(difference)=>{
                if difference==0||difference>maximum{return Err(invalid());}
                predicted=if matches!(edit,ListEdit::SubtractShort(_)){(predicted+maximum-difference)%maximum}else{(predicted+difference)%maximum};
                let target=if predicted>current{i64::from(predicted)-i64::from(maximum)}else{i64::from(predicted)};
                state.entries.iter().find(|entry|entry.long_term.is_none()&&entry.picture_number(current,maximum)==target).ok_or_else(invalid)?.serial
            },
            ListEdit::LongTerm(number)=>{
                if number>15{return Err(invalid());}
                state.entries.iter().find(|entry|entry.long_term==Some(number as u8)).ok_or_else(invalid)?.serial
            },
        };
        if index>list.len(){return Err(invalid());}
        list.insert(index,serial);
        let mut next=index+1;
        while next<list.len(){if list[next]==serial{list.remove(next);}else{next+=1;}}
    }
    if list.len()<usize::from(active){return Err(invalid());}
    list.truncate(usize::from(active));Ok(())
}
