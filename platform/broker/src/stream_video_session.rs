//! Native Windows receive-session owner. No renderer command or unauthenticated
//! UDP endpoint; the real SRTP/ICE/session registry must call this lifecycle.
use crate::{stream_video::WindowsVideoDecoder,stream_video_authority::{NativeVideoAuthority,VideoAuthorityLane},
    stream_video_codec::WindowsH264Backend,stream_video_receive::{NativeVideoReceiver,ParameterAdmission,ReceiveState},stream_video::UnpresentedVideoFrame};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_stream_input::video_client::{VideoWorker,VideoOwner,DecoderReceipt,VideoAuthorityObservation};
use omniplay_streaming::{native_video::Profile,native_h264_rtp::PacketizationMode,native_rtp_reorder::OrderedReceiver};
use std::{sync::{Arc,Mutex},time::{Duration,Instant}};

fn unavailable()->PlatformError{PlatformError::new(ErrorCode::RuntimeUnavailable,"The native receive session is unavailable or requires retirement")}
pub struct SharedAuthority<S:NativeVideoAuthority>{source:Arc<Mutex<S>>}
impl<S:NativeVideoAuthority> Clone for SharedAuthority<S>{fn clone(&self)->Self{Self{source:self.source.clone()}}}
impl<S:NativeVideoAuthority> NativeVideoAuthority for SharedAuthority<S>{
    fn observe(&mut self,owner:VideoOwner)->Result<VideoAuthorityObservation>{self.source.lock().map_err(|_|unavailable())?.observe(owner)}
}
/// Native negotiated values only. Initial RTP sequence must come from the
/// authenticated capture-generation boundary, not the first arriving datagram.
pub struct WindowsReceiveConfiguration{
    pub owner:VideoOwner,pub profile:Profile,pub h264_profile:u8,pub constraints:u8,pub level:u8,
    pub packetization:PacketizationMode,pub payload_type:u8,pub ssrc:u32,pub initial_sequence:u16,
}
pub struct WindowsReceiveSession<S:NativeVideoAuthority>{
    receiver:NativeVideoReceiver<ParameterAdmission<WindowsH264Backend<SharedAuthority<S>>>>,
    authority:VideoAuthorityLane,deadline:Instant,
}
impl<S:NativeVideoAuthority> WindowsReceiveSession<S>{
    pub fn open(configuration:WindowsReceiveConfiguration,source:S,deadline:Instant)->Result<Self>{
        omniplay_core::stream_signal_authority::require_installed_pairing_policy()?;
        let now=Instant::now();let remaining=deadline.saturating_duration_since(now);
        if remaining.is_zero()||remaining>Duration::from_secs(1800){return Err(unavailable());}
        let mut shared=SharedAuthority{source:Arc::new(Mutex::new(source))};
        let observed=shared.observe(configuration.owner)?;let now=Instant::now();
        if observed.owner!=configuration.owner||!observed.document_alive||!observed.capture_authorized||!observed.pair_authorized||
            now.checked_duration_since(observed.checked_at).is_none_or(|age|age>Duration::from_millis(100))||now>=deadline{return Err(unavailable());}
        let backend=WindowsH264Backend::new(configuration.owner,configuration.profile,configuration.h264_profile,configuration.constraints,configuration.level,shared.clone())?;
        let ordered=OrderedReceiver::new(configuration.packetization,configuration.payload_type,configuration.ssrc,configuration.initial_sequence,deadline.saturating_duration_since(Instant::now()))?;
        let installed=crate::stream_video_installation::admit()?;
        let remaining=deadline.saturating_duration_since(Instant::now());if remaining.is_zero(){return Err(unavailable());}
        let(worker,lease)=VideoWorker::spawn_decoder_guarded(&installed.path,installed.artifact,installed.directories,installed.sha256,configuration.owner,remaining)?;
        // Start independent authority maintenance before synchronous configure.
        let mut authority=VideoAuthorityLane::start(shared,lease)?;
        let receiver=match WindowsVideoDecoder::attach(worker,configuration.profile).and_then(|decoder|backend.into_receiver(ordered,decoder)){
            Ok(receiver)=>receiver,Err(error)=>{authority.close();return Err(error);}
        };
        let mut session=Self{receiver,authority,deadline};
        if Instant::now()>=deadline{let _=session.close_until(deadline);return Err(unavailable());}Ok(session)
    }
    fn live(&mut self)->Result<()>{
        if Instant::now()>=self.deadline||self.receiver.state()!=ReceiveState::Active{
            let _=self.close_until(Instant::now()+Duration::from_millis(1000));return Err(unavailable());
        }Ok(())
    }
    /// Receipt captured at original native ingress; bytes only AFTER the real
    /// transport authenticated/decrypted and replay-filtered the RTP packet.
    pub fn push_authenticated(&mut self,bytes:&[u8],receipt:DecoderReceipt)->Result<()>{
        self.live()?;let result=self.receiver.push_authenticated(bytes,receipt);if result.is_err(){let _=self.close_until(Instant::now()+Duration::from_millis(1000));}result
    }
    pub fn tick(&mut self)->Result<()>{
        self.live()?;let result=self.receiver.tick();if result.is_err(){let _=self.close_until(Instant::now()+Duration::from_millis(1000));}result
    }
    pub fn take_unpresented(&mut self)->Result<Option<UnpresentedVideoFrame>>{
        self.live()?;let result=self.receiver.take_unpresented();if result.is_err(){let _=self.close_until(Instant::now()+Duration::from_millis(1000));}result
    }
    pub fn state(&self)->ReceiveState{
        if self.receiver.state()==ReceiveState::Revoked&&!self.authority.retirement_confirmed(){ReceiveState::CleanupRequired}else{self.receiver.state()}
    }
    pub fn close_until(&mut self,deadline:Instant)->Result<()>{
        self.receiver.revoke_until(deadline)?;
        // Keep authority alive until graceful decoder Stop is confirmed.
        self.authority.close();
        if !self.authority.retirement_confirmed()||Instant::now()>=deadline{return Err(unavailable());}Ok(())
    }
}
impl<S:NativeVideoAuthority> Drop for WindowsReceiveSession<S>{
    fn drop(&mut self){let _=self.receiver.revoke_until(Instant::now()+Duration::from_millis(1000));self.authority.close();}
}
