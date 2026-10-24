//! Bounded ordinary SPS metadata, not negotiated decoder admission. Parameter
//! tables, PPS/slices, reference dependencies and backend limits remain separate.
use crate::{invalid,native_h264_bits::ParameterSetBits};
use omniplay_contracts::Result;

#[derive(Clone,Debug,PartialEq,Eq)]
pub enum PictureOrder{
    Lsb{bits:u8},
    Cycle{always_zero:bool,non_reference:i32,top_to_bottom:i32,offsets:Vec<i32>},
    DecodeOrder,
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Sps{
    pub id:u8,pub profile:u8,pub constraints:u8,pub level:u8,
    pub chroma_format:u8,pub separate_colour_plane:bool,pub luma_depth:u8,pub chroma_depth:u8,
    pub transform_bypass:bool,pub scaling_lists:Vec<Option<Vec<u8>>>,
    pub frame_num_bits:u8,pub picture_order:PictureOrder,pub reference_frames:u8,pub gaps_allowed:bool,
    pub coded_width:u32,pub coded_height:u32,pub frame_only:bool,pub adaptive_fields:bool,pub direct_8x8:bool,
    /// Pixel rectangle [left, top, width, height], derived with chroma/field units.
    pub visible:[u32;4],pub vui:Option<Vui>,
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct VideoSignal{pub format:u8,pub full_range:bool,pub colour:Option<[u8;3]>}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Timing{pub units_in_tick:u32,pub time_scale:u32,pub fixed_rate:bool}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Cpb{pub bitrate:u64,pub size_bits:u64,pub constant:bool}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Hrd{pub cpb:Vec<Cpb>,pub initial_delay_bits:u8,pub removal_delay_bits:u8,pub output_delay_bits:u8,pub time_offset_bits:u8}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Restrictions{
    pub motion_outside:bool,pub bytes_per_picture_denom:u32,pub bits_per_mb_denom:u32,
    pub horizontal_mv_bits:u32,pub vertical_mv_bits:u32,pub reorder_frames:u32,pub buffering_frames:u32,
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Vui{
    /// None: absent; (0,None): unspecified; (255,Some): extended SAR.
    pub aspect:Option<(u8,Option<(u16,u16)>)>,pub overscan:Option<bool>,pub video:Option<VideoSignal>,
    pub chroma_location:Option<(u32,u32)>,pub timing:Option<Timing>,pub nal_hrd:Option<Hrd>,pub vcl_hrd:Option<Hrd>,
    pub low_delay_hrd:Option<bool>,pub picture_structure:bool,pub restrictions:Option<Restrictions>,
}
impl Sps{
    /// One complete unframed ordinary SPS, at most 64 KiB. Allocation counts and
    /// loops are capped independently of coded dimensions; no pixels are decoded.
    pub fn parse(nal:&[u8])->Result<Self>{
        let mut bits=ParameterSetBits::from_nal(nal)?;
        if bits.nal_type()!=7{return Err(invalid());}
        let profile=bits.bits(8)? as u8;let constraints=bits.bits(8)? as u8;let level=bits.bits(8)? as u8;
        if constraints&3!=0||!matches!(level,9|10|11|12|13|20|21|22|30|31|32|40|41|42|50|51|52|60|61|62){return Err(invalid());}
        let id=bits.ue(31)? as u8;
        let mut chroma_format=1;let mut separate_colour_plane=false;let mut luma_depth=8;let mut chroma_depth=8;
        let mut transform_bypass=false;let mut scaling_lists=Vec::new();
        if matches!(profile,44|83|86|100|110|118|122|128|134|135|138|139|244){
            chroma_format=bits.ue(3)? as u8;
            if chroma_format==3{separate_colour_plane=bits.flag()?;}
            luma_depth=8+bits.ue(6)? as u8;chroma_depth=8+bits.ue(6)? as u8;
            transform_bypass=bits.flag()?;
            if bits.flag()?{
                for index in 0..(if chroma_format==3{12}else{8}){
                    scaling_lists.push(if bits.flag()?{Some(scaling_list(&mut bits,if index<6{16}else{64})?)}else{None});
                }
            }
        }else if !matches!(profile,66|77|88){return Err(invalid());}
        let frame_num_bits=4+bits.ue(12)? as u8;
        let picture_order=match bits.ue(2)?{
            0=>PictureOrder::Lsb{bits:4+bits.ue(12)? as u8},
            1=>{
                let always_zero=bits.flag()?;let non_reference=bits.se(i32::MAX)?;let top_to_bottom=bits.se(i32::MAX)?;
                let count=bits.ue(255)?;let mut offsets=Vec::new();
                for _ in 0..count{offsets.push(bits.se(i32::MAX)?);}
                PictureOrder::Cycle{always_zero,non_reference,top_to_bottom,offsets}
            },
            _=>PictureOrder::DecodeOrder,
        };
        let reference_frames=bits.ue(16)? as u8;let gaps_allowed=bits.flag()?;
        let coded_width=(bits.ue(511)?+1)*16;let map_height=bits.ue(511)?+1;
        let frame_only=bits.flag()?;let adaptive_fields=if frame_only{false}else{bits.flag()?};
        let coded_height=map_height*16*if frame_only{1}else{2};
        if coded_height>8192{return Err(invalid());}
        let direct_8x8=bits.flag()?;
        if !frame_only&&!direct_8x8{return Err(invalid());}
        let mut crop=[0u32;4];
        if bits.flag()?{for value in &mut crop{*value=bits.ue(8192)?;}}
        let array=if separate_colour_plane{0}else{chroma_format};
        let unit_x=if matches!(array,1|2){2}else{1};
        let unit_y=(if array==1{2}else{1})*if frame_only{1}else{2};
        let left=crop[0]*unit_x;let right=crop[1]*unit_x;let top=crop[2]*unit_y;let bottom=crop[3]*unit_y;
        if left+right>=coded_width||top+bottom>=coded_height{return Err(invalid());}
        let visible=[left,top,coded_width-left-right,coded_height-top-bottom];
        let vui=if bits.flag()?{Some(vui(&mut bits)?)}else{None};
        if vui.as_ref().and_then(|v|v.restrictions.as_ref()).is_some_and(|r|r.buffering_frames<u32::from(reference_frames)){return Err(invalid());}
        bits.finish()?;
        Ok(Self{id,profile,constraints,level,chroma_format,separate_colour_plane,luma_depth,chroma_depth,transform_bypass,scaling_lists,
            frame_num_bits,picture_order,reference_frames,gaps_allowed,coded_width,coded_height,frame_only,adaptive_fields,direct_8x8,visible,vui})
    }
}
/// An empty present list denotes the explicit use-default matrix flag. Missing
/// lists and fallback matrix derivation are intentionally distinct and deferred.
fn scaling_list(bits:&mut ParameterSetBits,count:usize)->Result<Vec<u8>>{
    let mut last=8i32;let mut next=8i32;let mut values=Vec::new();
    for index in 0..count{
        if next!=0{
            let delta=bits.se(128)?;if delta>127{return Err(invalid());}
            next=(last+delta+256)%256;
            if index==0&&next==0{return Ok(Vec::new());}
        }
        let value=if next==0{last}else{next};values.push(value as u8);last=value;
    }Ok(values)
}
fn hrd(bits:&mut ParameterSetBits)->Result<Hrd>{
    let count=bits.ue(31)?+1;let rate_scale=bits.bits(4)?;let size_scale=bits.bits(4)?;let mut cpb=Vec::new();
    for _ in 0..count{
        let bitrate=(u64::from(bits.ue(u32::MAX-1)?)+1)<<(6+rate_scale);
        let size_bits=(u64::from(bits.ue(u32::MAX-1)?)+1)<<(4+size_scale);
        cpb.push(Cpb{bitrate,size_bits,constant:bits.flag()?});
    }
    Ok(Hrd{cpb,initial_delay_bits:bits.bits(5)? as u8+1,removal_delay_bits:bits.bits(5)? as u8+1,
        output_delay_bits:bits.bits(5)? as u8+1,time_offset_bits:bits.bits(5)? as u8})
}
fn vui(bits:&mut ParameterSetBits)->Result<Vui>{
    let aspect=if bits.flag()?{
        let id=bits.bits(8)? as u8;
        let extended=if id==255{
            let width=bits.bits(16)? as u16;let height=bits.bits(16)? as u16;
            if width==0||height==0{return Err(invalid());}Some((width,height))
        }else{if id>16{return Err(invalid());}None};Some((id,extended))
    }else{None};
    let overscan=if bits.flag()?{Some(bits.flag()?)}else{None};
    let video=if bits.flag()?{
        let format=bits.bits(3)? as u8;if format>5{return Err(invalid());}
        let full_range=bits.flag()?;
        let colour=if bits.flag()?{Some([bits.bits(8)? as u8,bits.bits(8)? as u8,bits.bits(8)? as u8])}else{None};
        Some(VideoSignal{format,full_range,colour})
    }else{None};
    let chroma_location=if bits.flag()?{Some((bits.ue(5)?,bits.ue(5)?))}else{None};
    let timing=if bits.flag()?{
        let units_in_tick=bits.bits(32)?;let time_scale=bits.bits(32)?;let fixed_rate=bits.flag()?;
        if units_in_tick==0||time_scale==0{return Err(invalid());}Some(Timing{units_in_tick,time_scale,fixed_rate})
    }else{None};
    let nal_hrd=if bits.flag()?{Some(hrd(bits)?)}else{None};
    let vcl_hrd=if bits.flag()?{Some(hrd(bits)?)}else{None};
    let low_delay_hrd=if nal_hrd.is_some()||vcl_hrd.is_some(){Some(bits.flag()?)}else{None};
    let picture_structure=bits.flag()?;
    let restrictions=if bits.flag()?{
        let value=Restrictions{motion_outside:bits.flag()?,bytes_per_picture_denom:bits.ue(16)?,bits_per_mb_denom:bits.ue(16)?,
            horizontal_mv_bits:bits.ue(16)?,vertical_mv_bits:bits.ue(16)?,reorder_frames:bits.ue(16)?,buffering_frames:bits.ue(16)?};
        if value.reorder_frames>value.buffering_frames{return Err(invalid());}Some(value)
    }else{None};
    Ok(Vui{aspect,overscan,video,chroma_location,timing,nal_hrd,vcl_hrd,low_delay_hrd,picture_structure,restrictions})
}
