//! Bounded ordinary PPS metadata tied to an explicitly supplied SPS. This does
//! not implement parameter replacement, slice decoding or decoder admission.
use crate::{invalid,native_h264_bits::ParameterSetBits,native_h264_sps::{Sps,scaling_list}};
use omniplay_contracts::{ErrorCode,PlatformError,Result};

#[derive(Clone,Debug,PartialEq,Eq)]
pub enum SliceGroups{
    Interleaved{run_lengths:Vec<u32>},
    Dispersed{groups:u8},
    Foreground{rectangles:Vec<(u32,u32)>},
    Changing{map_type:u8,direction:bool,change_rate:u32},
    Explicit{groups:u8,ids:Vec<u8>},
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Pps{
    pub id:u8,pub sps_id:u8,pub entropy_cabac:bool,pub bottom_field_order_present:bool,
    pub slice_groups:Option<SliceGroups>,pub reference_list0:u8,pub reference_list1:u8,
    pub weighted_prediction:bool,pub weighted_bipred:u8,pub initial_qp:i32,pub initial_qs:i32,
    pub chroma_qp_offset:i8,pub second_chroma_qp_offset:i8,pub deblocking_present:bool,
    pub constrained_intra:bool,pub redundant_picture_count:bool,pub transform_8x8:bool,
    /// Same missing/default/signaled distinctions as the SPS scaling lists.
    pub scaling_lists:Vec<Option<Vec<u8>>>,
}
impl Pps{
    /// Parse a single unframed PPS with exact SPS-ID binding. The caller must
    /// retain the actual SPS bytes/version; public metadata alone is not trust.
    pub fn parse(nal:&[u8],sps:&Sps)->Result<Self>{
        let map_units=map_units(sps)?;let width=sps.coded_width/16;
        let mut bits=ParameterSetBits::from_nal(nal)?;
        if bits.nal_type()!=8{return Err(invalid());}
        let id=bits.ue(255)? as u8;let sps_id=bits.ue(31)? as u8;
        if sps_id!=sps.id{return Err(invalid());}
        let entropy_cabac=bits.flag()?;let bottom_field_order_present=bits.flag()?;
        let group_count=bits.ue(7)? as u8+1;
        let slice_groups=if group_count==1{None}else{
            let map_type=bits.ue(6)? as u8;
            Some(match map_type{
                0=>{
                    let mut run_lengths=Vec::new();
                    for _ in 0..group_count{run_lengths.push(bits.ue(map_units-1)?+1);}
                    SliceGroups::Interleaved{run_lengths}
                },
                1=>SliceGroups::Dispersed{groups:group_count},
                2=>{
                    let mut rectangles=Vec::new();
                    for _ in 1..group_count{
                        let left=bits.ue(map_units-1)?;let right=bits.ue(map_units-1)?;
                        if left>right||left%width>right%width{return Err(invalid());}
                        rectangles.push((left,right));
                    }SliceGroups::Foreground{rectangles}
                },
                3..=5=>{
                    if group_count!=2{return Err(invalid());}
                    SliceGroups::Changing{map_type,direction:bits.flag()?,change_rate:bits.ue(map_units-1)?+1}
                },
                _=>{
                    let count=bits.ue(map_units-1)?+1;
                    if count!=map_units{return Err(invalid());}
                    let width=(u8::BITS-(group_count-1).leading_zeros())as u8;
                    let mut ids=Vec::new();
                    ids.try_reserve_exact(count as usize).map_err(|_|PlatformError::new(ErrorCode::ResourceLimit,"The H.264 slice-group map budget is unavailable"))?;
                    for _ in 0..count{
                        let group=bits.bits(width)? as u8;if group>=group_count{return Err(invalid());}ids.push(group);
                    }SliceGroups::Explicit{groups:group_count,ids}
                },
            })
        };
        let reference_list0=bits.ue(31)? as u8+1;let reference_list1=bits.ue(31)? as u8+1;
        let weighted_prediction=bits.flag()?;let weighted_bipred=bits.bits(2)? as u8;
        if weighted_bipred>2{return Err(invalid());}
        let qp_offset=6*(i32::from(sps.luma_depth)-8);
        let qp=bits.se(26+qp_offset)?;
        if qp>25{return Err(invalid());}
        let qs=bits.se(26)?;if qs>25{return Err(invalid());}
        let chroma_qp_offset=bits.se(12)? as i8;
        let deblocking_present=bits.flag()?;let constrained_intra=bits.flag()?;let redundant_picture_count=bits.flag()?;
        let mut transform_8x8=false;let mut scaling_lists=Vec::new();let mut second_chroma_qp_offset=chroma_qp_offset;
        if bits.more_data()?{
            transform_8x8=bits.flag()?;
            if bits.flag()?{
                let count=6+if transform_8x8{if sps.chroma_format==3{6}else{2}}else{0};
                for index in 0..count{
                    scaling_lists.push(if bits.flag()?{Some(scaling_list(&mut bits,if index<6{16}else{64})?)}else{None});
                }
            }
            second_chroma_qp_offset=bits.se(12)? as i8;
        }
        bits.finish()?;
        Ok(Self{id,sps_id,entropy_cabac,bottom_field_order_present,slice_groups,reference_list0,reference_list1,
            weighted_prediction,weighted_bipred,initial_qp:26+qp,initial_qs:26+qs,chroma_qp_offset,second_chroma_qp_offset,
            deblocking_present,constrained_intra,redundant_picture_count,transform_8x8,scaling_lists})
    }
}
fn map_units(sps:&Sps)->Result<u32>{
    // Sps has public metadata fields. Recheck arithmetic-relevant bounds rather
    // than assuming every caller supplied an untouched Sps::parse result.
    if sps.id>31||!(16..=8192).contains(&sps.coded_width)||!(16..=8192).contains(&sps.coded_height)||
        sps.coded_width%16!=0||sps.coded_height%16!=0||!sps.frame_only&&sps.coded_height%32!=0||
        sps.chroma_format>3||sps.separate_colour_plane&&sps.chroma_format!=3||
        !(8..=14).contains(&sps.luma_depth)||!(8..=14).contains(&sps.chroma_depth){return Err(invalid());}
    let height=sps.coded_height/16/if sps.frame_only{1}else{2};
    (sps.coded_width/16).checked_mul(height).filter(|count|*count>0&&*count<=262_144).ok_or_else(invalid)
}
