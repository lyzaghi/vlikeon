//! Private authenticated RTP -> reorder -> admission -> decoder bridge. No
//! current network/service caller, SRTP implementation or default codec admission.
use crate::stream_video::{WindowsVideoDecoder,UnpresentedVideoFrame};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_stream_input::video_client::{DecoderReceipt,VideoOwner};
use omniplay_streaming::{native_h264_receive::ReceivedAccessUnit,native_rtp_reorder::OrderedReceiver};
use std::{collections::VecDeque,time::{Duration,Instant}};

/// Mandatory installed native receiver check of the ACTUAL parameter sets,
/// profile/level/resolution/VUI/dependencies and negotiated owner/codec. Existing
/// NAL layout checks do not implement this. No implementation blindly admits.
pub trait NativeH264Admission{
    fn admit(&mut self,owner:VideoOwner,frame:&ReceivedAccessUnit)->Result<()>;
}
fn unavailable()->PlatformError{PlatformError::new(ErrorCode::RuntimeUnavailable,"The native receive generation is unavailable or requires recovery")}
#[derive(Default)]
struct Timeline{last:Option<u32>,ticks:u64}
impl Timeline{
    fn advance(&mut self,timestamp:u32)->Result<i64>{
        let delta=match self.last{None=>0,Some(previous)=>{
            let delta=timestamp.wrapping_sub(previous);if delta==0||delta>=0x8000_0000{return Err(unavailable());}u64::from(delta)
        }};
        let ticks=self.ticks.checked_add(delta).filter(|value|*value<=1800*90_000).ok_or_else(unavailable)?;
        let time=i64::try_from(u128::from(ticks)*1000/9).map_err(|_|unavailable())?;
        self.last=Some(timestamp);self.ticks=ticks;Ok(time)
    }
}
struct Pending{bytes:Vec<u8>,timestamp:i64,receipt:DecoderReceipt}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ReceiveState{Active,Revoked,CleanupRequired}
pub struct NativeVideoReceiver<A:NativeH264Admission>{
    ordered:OrderedReceiver<DecoderReceipt>,decoder:WindowsVideoDecoder,admission:A,
    owner:VideoOwner,pending:VecDeque<Pending>,timeline:Timeline,state:ReceiveState,
}
impl<A:NativeH264Admission> NativeVideoReceiver<A>{
    pub fn new(ordered:OrderedReceiver<DecoderReceipt>,decoder:WindowsVideoDecoder,admission:A)->Self{
        let owner=decoder.owner();Self{ordered,decoder,admission,owner,pending:VecDeque::new(),timeline:Timeline::default(),state:ReceiveState::Active}
    }
    pub fn state(&self)->ReceiveState{self.state}
    /// Native ingress captures receipt BEFORE authentication/reorder. The caller
    /// supplies only successfully authenticated/decrypted/anti-replayed RTP here.
    /// This method is not a raw network endpoint or a source of SRTP trust.
    pub fn push_authenticated(&mut self,bytes:&[u8],receipt:DecoderReceipt)->Result<()>{
        if self.state!=ReceiveState::Active{return Err(unavailable());}
        let result=(||{
            if !receipt.within_receive_age(){return Err(unavailable());}
            let frames=self.ordered.push_stamped(bytes,receipt.local_received_at(),receipt)?;
            for value in frames{
                if self.pending.len()>=4||value.frame.received_at!=value.metadata.local_received_at()||!value.metadata.within_receive_age(){return Err(unavailable());}
                self.admission.admit(self.owner,&value.frame)?;
                // Slow parameter-set/native checks do not start a new age budget.
                if !value.metadata.within_receive_age(){return Err(unavailable());}
                let timestamp=self.timeline.advance(value.frame.timestamp)?;
                self.pending.push_back(Pending{bytes:value.frame.annex_b,timestamp,receipt:value.metadata});
            }
            self.pump_inner()
        })();if result.is_err(){self.retire();}result
    }
    fn pump_inner(&mut self)->Result<()>{
        self.decoder.check_authority()?;self.ordered.tick()?;
        while let Some(frame)=self.pending.front(){
            if !frame.receipt.within_receive_age(){return Err(unavailable());}
            if !self.decoder.submit_admitted_borrowed(&frame.bytes,frame.timestamp,frame.receipt)?{break;}
            self.pending.pop_front();
        }Ok(())
    }
    /// Run even when no new packets arrive. S_FALSE keeps compressed bytes for
    /// ordered retry, never skips a dependent picture to make a queue look fresh.
    pub fn tick(&mut self)->Result<()>{
        if self.state!=ReceiveState::Active{return Err(unavailable());}let result=self.pump_inner();if result.is_err(){self.retire();}result
    }
    pub fn take_unpresented(&mut self)->Result<Option<UnpresentedVideoFrame>>{
        self.tick()?;let result=self.decoder.take_unpresented();
        if result.as_ref().is_ok_and(|value|value.as_ref().is_some_and(|frame|frame.owner!=self.owner||!frame.frame.within_receive_age())){
            self.retire();return Err(unavailable());
        }
        if result.is_err(){self.retire();}result
    }
    fn retire(&mut self){
        let _=self.revoke_until(Instant::now()+Duration::from_millis(1000));
    }
    /// Fence delivery before cleanup; the decoder retains its original worker.
    /// Retrying CleanupRequired may reap that same worker, never reopens receive
    /// admission and never replaces missing native Stop acknowledgement.
    pub fn revoke_until(&mut self,deadline:Instant)->Result<()>{
        if self.state==ReceiveState::Revoked{return Ok(());}
        // Fence first. EOF/timeout/forced process retirement may close delivery
        // without a native Stop receipt; retain that uncertainty for the caller.
        self.state=ReceiveState::CleanupRequired;self.ordered.close();self.pending.clear();
        self.decoder.revoke_until(deadline)?;
        // A backend must not turn a late completion into a timely receipt.
        if Instant::now()>=deadline{return Err(unavailable());}
        self.state=ReceiveState::Revoked;Ok(())
    }
    pub fn revoke(&mut self)->Result<()>{
        self.revoke_until(Instant::now()+Duration::from_millis(1000))
    }
}
impl<A:NativeH264Admission> Drop for NativeVideoReceiver<A>{fn drop(&mut self){if self.state==ReceiveState::Active{self.retire();}}}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn rtp_timestamp_wrap_rebases_to_100ns(){let mut time=Timeline::default();assert_eq!(time.advance(u32::MAX-10).unwrap(),0);assert_eq!(time.advance(89989).unwrap(),10_000_000);}
    #[test]fn replay_does_not_advance_timeline(){let mut time=Timeline::default();time.advance(100).unwrap();assert!(time.advance(100).is_err());assert!(time.advance(99).is_err());assert_eq!(time.advance(90100).unwrap(),10_000_000);}
    #[test]fn large_jump_and_lifetime_are_bounded(){let mut time=Timeline::default();time.advance(0).unwrap();assert!(time.advance(0x8000_0000).is_err());assert!(time.advance(1800*90_000+1).is_err());assert_eq!(time.advance(1800*90_000).unwrap(),18_000_000_000);}
}
