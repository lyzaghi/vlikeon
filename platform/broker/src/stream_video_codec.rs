//! Concrete admission for the existing Windows progressive SDR decoder lane.
//! No network-selected constructor, renderer command or default native observer.
use crate::{stream_video_authority::NativeVideoAuthority,stream_video_receive::NativePictureBackend};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_stream_input::video_client::{VideoOwner,DecodedPictureReceipt};
use omniplay_streaming::{native_video::Profile,native_h264_receive::ReceivedAccessUnit,
    native_h264_parameters::{ResolvedParameters,ParameterBinding},native_h264_picture::PrimaryPictureHeaders,
    native_h264_auxiliary::Auxiliary,native_h264_slice::SliceKind,native_rtp_reorder::OrderedReceiver};
use std::time::{Duration,Instant};

fn unavailable()->PlatformError{PlatformError::new(ErrorCode::UnsupportedCapability,"The negotiated H.264 picture is incompatible with this native decoder generation")}
/// Values must come from the admitted native negotiation owner, not renderer
/// flags. This is the existing 720/1080, 30/60, progressive BT.709 SDR path.
pub struct WindowsH264Backend<S:NativeVideoAuthority>{owner:VideoOwner,profile:Profile,profile_idc:u8,constraints:u8,level:u8,source:S}
pub struct WindowsH264Proof{binding:ParameterBinding,timestamp:u32,received:Instant}
impl<S:NativeVideoAuthority> WindowsH264Backend<S>{
    pub fn into_receiver(self,ordered:OrderedReceiver<omniplay_stream_input::video_client::DecoderReceipt>,decoder:crate::stream_video::WindowsVideoDecoder)
        ->Result<crate::stream_video_receive::NativeVideoReceiver<crate::stream_video_receive::ParameterAdmission<Self>>>{
        if decoder.owner()!=self.owner||decoder.profile()!=self.profile{return Err(unavailable());}
        let admission=crate::stream_video_receive::ParameterAdmission::new(self.owner,self);
        Ok(crate::stream_video_receive::NativeVideoReceiver::new(ordered,decoder,admission))
    }
    pub fn new(owner:VideoOwner,profile:Profile,profile_idc:u8,constraints:u8,level:u8,source:S)->Result<Self>{
        profile.validate()?;
        if !matches!(profile_idc,66|77|100)||constraints&3!=0||!matches!(level,31|32|40|41|42)||
            [owner.capture,owner.game,owner.pair,owner.session].iter().any(|id|id.is_nil())||!(1..=1_000_000).contains(&owner.generation){return Err(unavailable());}
        Ok(Self{owner,profile,profile_idc,constraints,level,source})
    }
    fn live(&mut self,owner:VideoOwner,received:Instant)->Result<()>{
        if owner!=self.owner{return Err(unavailable());}
        let observation=self.source.observe(owner)?;let now=Instant::now();
        if observation.owner!=owner||!observation.document_alive||!observation.capture_authorized||!observation.pair_authorized||
            now.checked_duration_since(observation.checked_at).is_none_or(|age|age>Duration::from_millis(100))||
            now.checked_duration_since(received).is_none_or(|age|age>Duration::from_millis(100)){return Err(unavailable());}Ok(())
    }
    fn format(&self,parameters:&ResolvedParameters<'_>)->Result<()>{
        let sps=parameters.sps();let pps=parameters.pps();let p=self.profile;
        if sps.profile!=self.profile_idc||sps.constraints!=self.constraints||sps.level!=self.level||
            sps.chroma_format!=1||sps.separate_colour_plane||sps.luma_depth!=8||sps.chroma_depth!=8||!sps.frame_only||sps.adaptive_fields||sps.transform_bypass||sps.gaps_allowed||
            sps.coded_width!=p.width||sps.coded_height!=p.height.div_ceil(16)*16||sps.visible!=[0,0,p.width,p.height]||
            pps.slice_groups.is_some()||pps.redundant_picture_count{return Err(unavailable());}
        let (max_mbs,max_rate,max_dpb,max_bitrate):(u32,u32,u32,u32)=match self.level{
            31=>(3600,108000,18000,14_000_000),32=>(5120,216000,20480,20_000_000),
            40=>(8192,245760,32768,20_000_000),41=>(8192,245760,32768,50_000_000),
            42=>(8704,522240,34816,50_000_000),_=>return Err(unavailable()),
        };
        let mbs=(sps.coded_width/16)*(sps.coded_height/16);
        let bitrate=if sps.profile==100{u64::from(max_bitrate)*5/4}else{u64::from(max_bitrate)};
        if mbs>max_mbs||mbs*p.fps>max_rate||u32::from(sps.reference_frames)>16.min(max_dpb/mbs)||u64::from(p.bitrate_bps)>bitrate{return Err(unavailable());}
        let vui=sps.vui.as_ref().ok_or_else(unavailable)?;
        let square=match vui.aspect{Some((1,None))=>true,Some((255,Some((w,h))))=>w!=0&&w==h,_=>false};
        let video=vui.video.as_ref().ok_or_else(unavailable)?;
        let timing=vui.timing.as_ref().ok_or_else(unavailable)?;
        let restrictions=vui.restrictions.as_ref().ok_or_else(unavailable)?;
        if !square||video.full_range||video.colour!=Some([1,1,1])||vui.chroma_location!=Some((1,1))||vui.picture_structure||
            !timing.fixed_rate||u64::from(timing.time_scale)!=2*u64::from(timing.units_in_tick)*u64::from(p.fps)||
            restrictions.reorder_frames!=0||restrictions.buffering_frames>16.min(max_dpb/mbs){return Err(unavailable());}
        for hrd in [&vui.nal_hrd,&vui.vcl_hrd].into_iter().flatten(){
            if hrd.cpb.iter().any(|cpb|cpb.bitrate>bitrate){return Err(unavailable());}
        }
        if sps.profile==66&&(pps.entropy_cabac||pps.transform_8x8){return Err(unavailable());}
        if sps.profile==77&&pps.transform_8x8{return Err(unavailable());}Ok(())
    }
}
impl<S:NativeVideoAuthority> NativePictureBackend for WindowsH264Backend<S>{
    type Proof=WindowsH264Proof;
    fn prepare(&mut self,owner:VideoOwner,frame:&ReceivedAccessUnit,picture:&PrimaryPictureHeaders,parameters:&ResolvedParameters<'_>)->Result<Self::Proof>{
        self.live(owner,frame.received_at)?;self.format(parameters)?;
        for header in picture.headers(){
            if matches!(header.prefix.kind(),SliceKind::Sp|SliceKind::Si)||self.profile_idc==66&&header.prefix.kind()==SliceKind::B{return Err(unavailable());}
        }
        // Recovery/timing/HDR and other stateful SEI need their actual semantic
        // implementation. Only opaque user-data payloads pass this SDR lane.
        for nal in picture.auxiliary(){
            if let Auxiliary::Sei(sei)=nal{
                for (kind,bytes) in sei.messages(){
                    if !matches!((kind,bytes.len()),(4,1..)|(5,16..))||kind==4&&bytes[0]==255&&bytes.len()<2{return Err(unavailable());}
                }
            }
        }
        self.live(owner,frame.received_at)?;
        Ok(WindowsH264Proof{binding:parameters.binding(),timestamp:frame.timestamp,received:frame.received_at})
    }
    fn recheck(&mut self,owner:VideoOwner,frame:&ReceivedAccessUnit,parameters:&ResolvedParameters<'_>,proof:&Self::Proof)->Result<()>{
        if proof.binding!=parameters.binding()||proof.timestamp!=frame.timestamp||proof.received!=frame.received_at{return Err(unavailable());}
        self.format(parameters)?;self.live(owner,frame.received_at)
    }
    fn decoded(&mut self,owner:VideoOwner,frame:&ReceivedAccessUnit,parameters:&ResolvedParameters<'_>,proof:Self::Proof,_completion:&DecodedPictureReceipt)->Result<()>{
        // Receiver already matched the opaque native completion to the exact
        // accepted timestamp/owner/receipt before reaching this callback.
        self.recheck(owner,frame,parameters,&proof)
    }
}
