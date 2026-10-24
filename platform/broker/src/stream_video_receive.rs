//! Private authenticated RTP -> reorder -> admission -> decoder bridge. No
//! current network/service caller, SRTP implementation or default codec admission.
use crate::stream_video::{WindowsVideoDecoder,UnpresentedVideoFrame};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_stream_input::video_client::{DecoderReceipt,DecodedPictureReceipt,VideoOwner};
use omniplay_streaming::{native_h264_receive::ReceivedAccessUnit,native_rtp_reorder::OrderedReceiver};
use std::{collections::VecDeque,time::{Duration,Instant}};
use omniplay_streaming::{native_h264_rtp::{AccessUnit,Framing},native_h264_parameters::{ParameterSets,ParameterBinding,ResolvedParameters},native_h264_picture::PrimaryPictureHeaders};
use sha2::{Digest,Sha256};

/// Mandatory installed native receiver check of the ACTUAL parameter sets,
/// profile/level/resolution/VUI/dependencies and negotiated owner/codec. Existing
/// NAL layout checks do not implement this. No implementation blindly admits.
pub trait NativeH264Admission{
    /// Bounded native-only proof tied to this exact AU, parameter versions,
    /// negotiated generation and reference/decode order. Never serialize it or
    /// retain another compressed-frame copy in it; queue capacity is four.
    type Proof;
    fn admit(&mut self,owner:VideoOwner,frame:&ReceivedAccessUnit)->Result<Self::Proof>;
    /// Read-only revalidation of the SAME admitted frame before each submit,
    /// including S_FALSE retries. Must not advance DPB/reference/parameter state
    /// again, renew receive age or turn stale parameters into a fresh proof.
    /// No default-success implementation: caller supplies actual native state.
    fn recheck(&mut self,owner:VideoOwner,frame:&ReceivedAccessUnit,proof:&Self::Proof)->Result<()>;
    /// Consumes the one admitted proof only after matched decoder output. Input
    /// acceptance/S_FALSE cannot call this or commit POC/reference transitions.
    fn decoded(&mut self,owner:VideoOwner,frame:&ReceivedAccessUnit,proof:Self::Proof,completion:&DecodedPictureReceipt)->Result<()>;
}
/// Installed codec/native backend supplies the remaining negotiated profile,
/// colour, DPB/reference and current-owner checks. Syntax parsing is handled by
/// ParameterAdmission, never delegated to a renderer or a remote declaration.
pub trait NativePictureBackend{
    type Proof;
    fn prepare(&mut self,owner:VideoOwner,frame:&ReceivedAccessUnit,picture:&PrimaryPictureHeaders,parameters:&ResolvedParameters<'_>)->Result<Self::Proof>;
    fn recheck(&mut self,owner:VideoOwner,frame:&ReceivedAccessUnit,parameters:&ResolvedParameters<'_>,proof:&Self::Proof)->Result<()>;
    fn decoded(&mut self,owner:VideoOwner,frame:&ReceivedAccessUnit,parameters:&ResolvedParameters<'_>,proof:Self::Proof,completion:&DecodedPictureReceipt)->Result<()>;
}
pub struct ParameterAdmission<B:NativePictureBackend>{owner:VideoOwner,parameters:ParameterSets,backend:B,closed:bool}
pub struct ParameterAdmissionProof<P>{binding:ParameterBinding,backend:P,hash:[u8;32],timestamp:u32,received:Instant,idr:bool}
impl<B:NativePictureBackend> ParameterAdmission<B>{
    pub fn new(owner:VideoOwner,backend:B)->Self{Self{owner,parameters:ParameterSets::new(),backend,closed:false}}
    fn fence(&mut self){self.closed=true;self.parameters.close();}
}
impl<B:NativePictureBackend> NativeH264Admission for ParameterAdmission<B>{
    type Proof=ParameterAdmissionProof<B::Proof>;
    fn admit(&mut self,owner:VideoOwner,frame:&ReceivedAccessUnit)->Result<Self::Proof>{
        let result=(||{
            if self.closed||owner!=self.owner{return Err(unavailable());}
            let unit=AccessUnit::parse(&frame.annex_b,Framing::AnnexB)?;
            if unit.has_idr()!=frame.has_idr{return Err(unavailable());}
            let mut slices=false;
            // Process parameter NALs in actual wire order. A PPS before its SPS
            // needs an already retained dependency; do not reorder the stream.
            for nal in unit.nals(){
                match nal[0]&31{
                    1|5=>slices=true,
                    7=>{if slices{return Err(unavailable());}self.parameters.install_sps(nal)?;},
                    8=>{if slices{return Err(unavailable());}self.parameters.install_pps(nal)?;},
                    _=>{},
                }
            }
            let picture=PrimaryPictureHeaders::parse(&unit,&self.parameters)?;
            let binding=picture.identity().parameters().clone();
            let parameters=self.parameters.check(&binding)?;
            let backend=self.backend.prepare(owner,frame,&picture,&parameters)?;
            Ok(ParameterAdmissionProof{binding,backend,hash:Sha256::digest(&frame.annex_b).into(),timestamp:frame.timestamp,received:frame.received_at,idr:frame.has_idr})
        })();
        // Partial parameter changes cannot leak into another frame after error.
        if result.is_err(){self.fence();}result
    }
    fn recheck(&mut self,owner:VideoOwner,frame:&ReceivedAccessUnit,proof:&Self::Proof)->Result<()>{
        let result=(||{
            if self.closed||owner!=self.owner||frame.timestamp!=proof.timestamp||frame.received_at!=proof.received||frame.has_idr!=proof.idr||
                <[u8;32]>::from(Sha256::digest(&frame.annex_b))!=proof.hash{return Err(unavailable());}
            let parameters=self.parameters.check(&proof.binding)?;
            self.backend.recheck(owner,frame,&parameters,&proof.backend)
        })();if result.is_err(){self.fence();}result
    }
    fn decoded(&mut self,owner:VideoOwner,frame:&ReceivedAccessUnit,proof:Self::Proof,completion:&DecodedPictureReceipt)->Result<()>{
        let result=(||{
            self.recheck(owner,frame,&proof)?;
            let parameters=self.parameters.check(&proof.binding)?;
            self.backend.decoded(owner,frame,&parameters,proof.backend,completion)
        })();if result.is_err(){self.fence();}result
    }
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
struct Pending<P>{frame:ReceivedAccessUnit,proof:P,timestamp:i64,receipt:DecoderReceipt}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ReceiveState{Active,Revoked,CleanupRequired}
pub struct NativeVideoReceiver<A:NativeH264Admission>{
    ordered:OrderedReceiver<DecoderReceipt>,decoder:WindowsVideoDecoder,admission:A,
    owner:VideoOwner,pending:VecDeque<Pending<A::Proof>>,in_flight:VecDeque<Pending<A::Proof>>,timeline:Timeline,state:ReceiveState,
}
impl<A:NativeH264Admission> NativeVideoReceiver<A>{
    pub fn new(ordered:OrderedReceiver<DecoderReceipt>,decoder:WindowsVideoDecoder,admission:A)->Self{
        let owner=decoder.owner();Self{ordered,decoder,admission,owner,pending:VecDeque::new(),in_flight:VecDeque::new(),timeline:Timeline::default(),state:ReceiveState::Active}
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
                if self.pending.len()+self.in_flight.len()>=4||value.frame.received_at!=value.metadata.local_received_at()||!value.metadata.within_receive_age(){return Err(unavailable());}
                let proof=self.admission.admit(self.owner,&value.frame)?;
                // Slow parameter-set/native checks do not start a new age budget.
                if !value.metadata.within_receive_age(){return Err(unavailable());}
                let timestamp=self.timeline.advance(value.frame.timestamp)?;
                self.pending.push_back(Pending{frame:value.frame,proof,timestamp,receipt:value.metadata});
            }
            self.pump_inner()
        })();if result.is_err(){self.retire();}result
    }
    fn pump_inner(&mut self)->Result<()>{
        self.decoder.check_authority()?;self.ordered.tick()?;
        if self.in_flight.front().is_some_and(|frame|!frame.receipt.within_receive_age()){return Err(unavailable());}
        while let Some(frame)=self.pending.front(){
            if !frame.receipt.within_receive_age(){return Err(unavailable());}
            self.admission.recheck(self.owner,&frame.frame,&frame.proof)?;
            // Revalidation can block on native state; it must not extend either
            // ingress age or decoder authority before this immediate handoff.
            if !frame.receipt.within_receive_age(){return Err(unavailable());}
            self.decoder.check_authority()?;
            if !self.decoder.submit_admitted_borrowed(&frame.frame.annex_b,frame.timestamp,frame.receipt)?{break;}
            let accepted=self.pending.pop_front().ok_or_else(unavailable)?;
            self.in_flight.push_back(accepted);
        }Ok(())
    }
    /// Run even when no new packets arrive. S_FALSE keeps compressed bytes for
    /// ordered retry, never skips a dependent picture to make a queue look fresh.
    pub fn tick(&mut self)->Result<()>{
        if self.state!=ReceiveState::Active{return Err(unavailable());}let result=self.pump_inner();if result.is_err(){self.retire();}result
    }
    pub fn take_unpresented(&mut self)->Result<Option<UnpresentedVideoFrame>>{
        self.tick()?;
        let result=(||{
            let Some(output)=self.decoder.take_unpresented()?else{return Ok(None)};
            let accepted=self.in_flight.pop_front().ok_or_else(unavailable)?;
            if output.owner!=self.owner||!output.frame.within_receive_age()||
                !output.frame.completed_picture().matches(self.owner,accepted.timestamp,accepted.frame.received_at){return Err(unavailable());}
            self.admission.decoded(self.owner,&accepted.frame,accepted.proof,output.frame.completed_picture())?;
            if !output.frame.within_receive_age(){return Err(unavailable());}
            self.decoder.check_authority()?;Ok(Some(output))
        })();
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
        self.state=ReceiveState::CleanupRequired;self.ordered.close();self.pending.clear();self.in_flight.clear();
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
