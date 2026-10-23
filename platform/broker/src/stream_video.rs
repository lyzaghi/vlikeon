//! Private source bridge, not a product command/capture producer. Native host
//! admission and an independent VideoAuthorityLease observer remain mandatory.
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_stream_input::video_client::{VideoOwner,VideoPacket,VideoProfile,VideoWorker};
use omniplay_streaming::{native_video::{Binding,FrameMailbox,PixelFormat,Profile},native_video_colour::tight_nv12};
use std::{sync::Arc,time::Duration};

pub enum PumpResult{NoFrame,Accepted,DroppedRaw}
/// This type deliberately does not promise negotiated/RTP-ready output. The
/// actual H.264 type, headers and VUI must pass receiver admission separately.
pub struct UnnegotiatedVideoPacket{pub owner:VideoOwner,pub packet:VideoPacket}
pub struct WindowsVideoFrames{
    worker:VideoWorker,mailbox:Arc<FrameMailbox>,profile:Profile,binding:Binding,closed:bool,keyframe_pending:bool,
}
fn unavailable()->PlatformError{PlatformError::new(ErrorCode::RuntimeUnavailable,"The native video frame generation is unavailable or requires cleanup")}
impl WindowsVideoFrames{
    /// The caller already owns an admitted job-contained worker and maintains
    /// its independent native lease while configure blocks. No lease is renewed.
    pub fn attach(mut worker:VideoWorker,profile:Profile,source_format:PixelFormat,require_hardware:bool)->Result<Self>{
        if worker.owned_capture()||worker.decoder_mode(){return Err(unavailable());}
        profile.validate()?;
        let owner=worker.owner();let binding=Binding{capture:owner.capture,session:owner.session,generation:owner.generation};
        let mailbox=Arc::new(FrameMailbox::new(binding,profile,source_format,worker.remaining_authorized()?)?);
        worker.configure(VideoProfile{width:profile.width,height:profile.height,fps:profile.fps,bitrate:profile.bitrate_bps,require_hardware})?;
        worker.remaining_authorized()?;
        Ok(Self{worker,mailbox,profile,binding,closed:false,keyframe_pending:true})
    }
    /// Give only to the admitted native capture producer, never a renderer or
    /// peer. The producer supplies bounded owned bytes and explicit SDR colour.
    pub fn producer(&self)->Result<Arc<FrameMailbox>>{
        if self.closed{return Err(unavailable());}self.worker.remaining_authorized()?;Ok(self.mailbox.clone())
    }
    pub fn pump(&mut self,keyframe:bool)->Result<PumpResult>{
        if self.closed{return Err(unavailable());}
        self.keyframe_pending|=keyframe;
        let result=(||{
            self.worker.remaining_authorized()?;
            let Some((frame,received))=self.mailbox.take_latest_timed()? else{return Ok(PumpResult::NoFrame);};
            let frame=tight_nv12(frame,self.binding,self.profile)?;
            if received.elapsed()>Duration::from_millis(100){return Ok(PumpResult::DroppedRaw);}
            let timestamp=frame.timestamp_us.checked_mul(10).and_then(|value|i64::try_from(value).ok()).ok_or_else(unavailable)?;
            // A successful conversion cannot renew native capture authority.
            self.worker.remaining_authorized()?;
            if self.worker.submit(frame.bytes,timestamp,self.keyframe_pending)?{
                self.keyframe_pending=false;Ok(PumpResult::Accepted)
            }else{Ok(PumpResult::DroppedRaw)}
        })();
        if result.is_err(){let _=self.mailbox.close();self.closed=true;let _=self.worker.close();}
        result
    }
    pub fn take_unnegotiated(&mut self)->Result<Option<UnnegotiatedVideoPacket>>{
        if self.closed{return Err(unavailable());}
        match self.worker.take_packet(){
            Ok(packet)=>Ok(packet.map(|packet|UnnegotiatedVideoPacket{owner:self.worker.owner(),packet})),
            Err(error)=>{let _=self.mailbox.close();self.closed=true;let _=self.worker.close();Err(error)},
        }
    }
    pub fn revoke(&mut self)->Result<()>{
        if self.closed{return Err(unavailable());}
        self.closed=true;let mailbox=self.mailbox.close();let stopped=self.worker.stop_confirmed();mailbox?;stopped
    }
}
impl Drop for WindowsVideoFrames{fn drop(&mut self){let _=self.mailbox.close();let _=self.worker.close();}}

/// Owned-v2 keeps capture/conversion/encoding inside the job-contained helper.
/// No raw producer API. Independent authority observations remain mandatory.
pub struct WindowsOwnedVideo{worker:VideoWorker,closed:bool}
impl WindowsOwnedVideo{
    pub fn attach(mut worker:VideoWorker,profile:Profile,require_hardware:bool)->Result<Self>{
        profile.validate()?;if !worker.owned_capture(){return Err(unavailable());}
        worker.configure(VideoProfile{width:profile.width,height:profile.height,fps:profile.fps,bitrate:profile.bitrate_bps,require_hardware})?;
        worker.remaining_authorized()?;Ok(Self{worker,closed:false})
    }
    pub fn request_keyframe(&mut self)->Result<u64>{
        if self.closed{return Err(unavailable());}
        match self.worker.request_keyframe(){Ok(fence)=>Ok(fence),Err(error)=>{self.closed=true;let _=self.worker.close();Err(error)}}
    }
    pub fn take_unnegotiated(&mut self)->Result<Option<UnnegotiatedVideoPacket>>{
        if self.closed{return Err(unavailable());}
        match self.worker.take_packet(){
            Ok(packet)=>Ok(packet.map(|packet|UnnegotiatedVideoPacket{owner:self.worker.owner(),packet})),
            Err(error)=>{self.closed=true;let _=self.worker.close();Err(error)},
        }
    }
    pub fn revoke(&mut self)->Result<()>{
        if self.closed{return Err(unavailable());}self.closed=true;self.worker.stop_confirmed()
    }
}
impl Drop for WindowsOwnedVideo{fn drop(&mut self){let _=self.worker.close();}}

pub struct UnpresentedVideoFrame{pub owner:VideoOwner,pub frame:omniplay_stream_input::video_client::DecodedVideoFrame}
/// Private viewer adapter, not receiver/codec/display permission. Separate
/// installed admission and native receive/document/pair observation are due.
pub struct WindowsVideoDecoder{worker:VideoWorker,closed:bool}
impl WindowsVideoDecoder{
    pub fn attach(mut worker:VideoWorker,profile:Profile)->Result<Self>{
        profile.validate()?;if !worker.decoder_mode(){return Err(unavailable());}
        worker.configure(VideoProfile{width:profile.width,height:profile.height,fps:profile.fps,bitrate:profile.bitrate_bps,require_hardware:false})?;
        worker.remaining_authorized()?;Ok(Self{worker,closed:false})
    }
    pub fn submit_admitted(&mut self,annex_b:Vec<u8>,timestamp_100ns:i64,receipt:omniplay_stream_input::video_client::DecoderReceipt)->Result<bool>{
        if self.closed{return Err(unavailable());}
        match self.worker.submit_compressed(annex_b,timestamp_100ns,receipt){
            Ok(accepted)=>Ok(accepted),Err(error)=>{self.closed=true;let _=self.worker.close();Err(error)},
        }
    }
    pub fn take_unpresented(&mut self)->Result<Option<UnpresentedVideoFrame>>{
        if self.closed{return Err(unavailable());}
        match self.worker.take_decoded(){
            Ok(frame)=>Ok(frame.map(|frame|UnpresentedVideoFrame{owner:self.worker.owner(),frame})),
            Err(error)=>{self.closed=true;let _=self.worker.close();Err(error)},
        }
    }
    pub fn revoke(&mut self)->Result<()>{
        if self.closed{return Err(unavailable());}self.closed=true;self.worker.stop_confirmed()
    }
}
impl Drop for WindowsVideoDecoder{fn drop(&mut self){let _=self.worker.close();}}
