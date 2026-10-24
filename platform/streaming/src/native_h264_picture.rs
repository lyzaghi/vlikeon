//! Primary-picture header consistency only. Not AU completeness, entropy/DPB
//! validation, parameter installation or negotiated native decoder admission.
use crate::{invalid,native_h264_rtp::AccessUnit,native_h264_parameters::ParameterSets,
    native_h264_slice::{PictureIdentity,SliceKind},native_h264_slice_header::{SliceHeader,ReferenceMarking}};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use std::collections::BTreeSet;

pub struct PrimaryPictureHeaders{identity:PictureIdentity,headers:Vec<SliceHeader>}
impl PrimaryPictureHeaders{
    pub fn identity(&self)->&PictureIdentity{&self.identity}
    pub fn headers(&self)->&[SliceHeader]{&self.headers}
    /// Checks one primary picture's ordinary slices against retained parameters.
    /// Caller must install/verify in-band parameters BEFORE this call. This
    /// object never changes parameter versions or admits bytes to a decoder.
    pub fn parse(unit:&AccessUnit<'_>,parameters:&ParameterSets)->Result<Self>{
        let mut headers:Vec<SliceHeader>=Vec::new();
        headers.try_reserve_exact(unit.nal_count()).map_err(|_|PlatformError::new(ErrorCode::ResourceLimit,"The H.264 picture-header budget is unavailable"))?;
        let mut starts=BTreeSet::new();let mut kinds=0u8;let mut required:Option<SliceKind>=None;
        for nal in unit.nals(){
            if !matches!(nal[0]&31,1|5){continue;}
            let header=SliceHeader::parse(nal,parameters)?;
            // Redundant pictures and separate planes need a multi-picture AU
            // coordinator. Do not silently drop or mix those extra pictures.
            if header.prefix.picture().redundant()!=0||header.prefix.picture().colour_plane().is_some(){return Err(invalid());}
            if !starts.insert(header.prefix.first_mb()){return Err(invalid());}
            if let Some(first)=headers.first(){
                if first.prefix.picture()!=header.prefix.picture()||first.marking!=header.marking||first.group_change_cycle!=header.group_change_cycle||first.sp_switch.unwrap_or(false)!=header.sp_switch.unwrap_or(false){return Err(invalid());}
            }
            let kind=header.prefix.kind();kinds|=kind_bit(kind);
            if header.prefix.same_kind_required(){
                if required.is_some_and(|prior|prior!=kind){return Err(invalid());}required=Some(kind);
            }
            headers.push(header);
        }
        if headers.is_empty()||!starts.contains(&0)||required.is_some_and(|kind|kinds!=kind_bit(kind)){return Err(invalid());}
        let first=&headers[0];let identity=first.prefix.picture().clone();
        let resolved=parameters.check(identity.parameters())?;
        // Parameter NALs in this bounded unit must be exactly the versions used
        // by its slices. No hidden replacement or unrelated parameter ID here.
        for nal in unit.nals(){
            match nal[0]&31{
                7 if nal!=resolved.encoded_sps()=>return Err(invalid()),
                8 if nal!=resolved.encoded_pps()=>return Err(invalid()),
                _=>{},
            }
        }
        if unit.has_idr()!=identity.idr().is_some()||matches!(&first.marking,ReferenceMarking::NonReference)!=!identity.reference(){return Err(invalid());}
        Ok(Self{identity,headers})
    }
}
fn kind_bit(kind:SliceKind)->u8{1<<match kind{SliceKind::P=>0,SliceKind::B=>1,SliceKind::I=>2,SliceKind::Sp=>3,SliceKind::Si=>4}}
