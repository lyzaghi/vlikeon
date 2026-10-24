//! Joint progressive-picture POC/reference metadata transaction. An actual
//! native decoder owner must supply the confirmed completion boundary; parser
//! success or input acceptance alone never proves picture decode completion.
use crate::{invalid,native_h264_parameters::ParameterSets,native_h264_picture::PrimaryPictureHeaders,
    native_h264_poc::{PictureOrderState,PictureOrderUpdate,PictureCounts},
    native_h264_frame_references::{FrameReferences,FrameReferenceUpdate,FramePredictionLists,FrameReference}};
use omniplay_contracts::{ErrorCode,PlatformError,Result};

pub struct FrameDecodeState{order:PictureOrderState,references:FrameReferences,closed:bool}
/// Contains no compressed media or decoder handle. One exact picture's staged
/// POC, pre-marking prediction lists and post-decode reference marking.
pub struct FrameDecodePlan{order:PictureOrderUpdate,references:FrameReferenceUpdate,lists:Vec<FramePredictionLists>}
impl FrameDecodePlan{
    pub fn decoded_order(&self)->PictureCounts{self.order.decoded()}
    pub fn retained_order(&self)->PictureCounts{self.order.retained()}
    pub fn prediction_lists(&self)->&[FramePredictionLists]{&self.lists}
    pub fn resets_references(&self)->bool{self.order.resets_references()}
}
impl Default for FrameDecodeState{fn default()->Self{Self::new()}}
impl FrameDecodeState{
    pub fn new()->Self{Self{order:PictureOrderState::new(),references:FrameReferences::new(),closed:false}}
    pub fn close(&mut self){self.closed=true;self.order.close();self.references.close();}
    pub fn references(&self)->&[FrameReference]{self.references.entries()}
    pub fn prepare(&self,picture:&PrimaryPictureHeaders,parameters:&ParameterSets)->Result<FrameDecodePlan>{
        if self.closed||picture.headers().is_empty()||picture.headers().len()>256{return Err(invalid());}
        let order=self.order.prepare(&picture.headers()[0],parameters)?;
        let mut lists=Vec::new();
        lists.try_reserve_exact(picture.headers().len()).map_err(|_|PlatformError::new(ErrorCode::ResourceLimit,"The H.264 decode-plan budget is unavailable"))?;
        for header in picture.headers(){lists.push(self.references.prediction_lists(header,parameters,&order)?);}
        let references=self.references.prepare(picture,parameters,&order)?;
        Ok(FrameDecodePlan{order,references,lists})
    }
    /// Read-only retry admission. It does not repeat marking or update POC.
    pub fn recheck(&self,plan:&FrameDecodePlan,parameters:&ParameterSets)->Result<()>{
        if self.closed{return Err(invalid());}
        self.order.recheck(&plan.order,parameters)?;
        self.references.recheck(&plan.references,parameters)?;
        for lists in &plan.lists{self.references.recheck_lists(lists,parameters)?;}
        Ok(())
    }
    /// Native owner calls only after confirming that THIS picture completed
    /// decoding. This API is not a decoder receipt and has no renderer route.
    /// Both components preflight under exclusive access before either mutates.
    pub fn commit_decoded(&mut self,plan:FrameDecodePlan,parameters:&ParameterSets)->Result<()>{
        self.recheck(&plan,parameters)?;
        let result=(||{
            self.order.commit(plan.order,parameters)?;
            self.references.commit(plan.references,parameters)
        })();
        // Any unexpected partial failure retires both components; never expose
        // one advanced component as a reusable prediction generation.
        if result.is_err(){self.close();}result
    }
}
