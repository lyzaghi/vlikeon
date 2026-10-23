//! Private native media primitives. No renderer/peer deserialization or OS
//! encoder discovery is provided here. Capabilities must come from admitted
//! installed backends and the authenticated receiver's negotiated media path.
use crate::{invalid,unauthorized};
use omniplay_contracts::Result;
use std::{collections::VecDeque,sync::Mutex,time::{Duration,Instant}};
use uuid::Uuid;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Codec{H264,Vp8,Vp9,Av1,Hevc}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum PixelFormat{Nv12,Bgra8}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Profile{pub width:u32,pub height:u32,pub fps:u32,pub bitrate_bps:u32}
impl Profile{
    pub fn validate(self)->Result<()>{
        if !matches!((self.width,self.height),(1280,720)|(1920,1080))||
            !matches!(self.fps,30|60)||!(500_000..=30_000_000).contains(&self.bitrate_bps){return Err(invalid());}Ok(())
    }
}
/// Exact codec/profile/level identifiers originate from the backend negotiation
/// adapter. Matching a MIME name alone cannot establish H.264/HEVC compatibility.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct CodecIdentity{pub codec:Codec,pub profile:String,pub level:u16,pub packetization:u8}
impl CodecIdentity{
    fn valid(&self)->bool{!self.profile.is_empty()&&self.profile.len()<=64&&
        self.profile.bytes().all(|value|value.is_ascii_alphanumeric()||matches!(value,b'_'|b'-'|b'.'))&&
        self.level>0&&self.packetization<=1}
}
#[derive(Clone,Debug)]
pub struct EncoderCapability{
    pub identity:CodecIdentity,pub input:PixelFormat,
    pub max_width:u32,pub max_height:u32,pub max_fps:u32,pub max_bitrate_bps:u32,
    /// Backend observation, not inferred from codec name or platform.
    pub hardware:bool,
    /// The installed backend explicitly supports the low-delay configuration.
    pub realtime:bool,
}
/// Preserve backend ordering in Auto. An explicit preference moves matching
/// candidates first, while optional fallback still requires an exact receiver
/// intersection. No codec, acceleration or patent/distribution right is invented.
pub fn select_encoder<'a>(local:&'a[EncoderCapability],receiver:&[CodecIdentity],
    profile:Profile,preferred:Option<Codec>,allow_fallback:bool,require_hardware:bool)->Result<&'a EncoderCapability>{
    profile.validate()?;
    if local.len()>64||receiver.is_empty()||receiver.len()>64||receiver.iter().any(|value|!value.valid()){return Err(invalid());}
    let eligible=|candidate:&&EncoderCapability|candidate.identity.valid()&&candidate.realtime&&
        (!require_hardware||candidate.hardware)&&candidate.max_width>=profile.width&&candidate.max_width<=8192&&
        candidate.max_height>=profile.height&&candidate.max_height<=8192&&
        candidate.max_fps>=profile.fps&&candidate.max_fps<=240&&
        candidate.max_bitrate_bps>=profile.bitrate_bps&&candidate.max_bitrate_bps<=200_000_000&&
        receiver.contains(&candidate.identity);
    if let Some(codec)=preferred{
        if let Some(candidate)=local.iter().filter(eligible).find(|candidate|candidate.identity.codec==codec){return Ok(candidate);}
        if !allow_fallback{return Err(invalid());}
    }
    local.iter().find(eligible).ok_or_else(invalid)
}

/// Fixed configuration for one host-authorized capture generation. Replacing
/// capture requires a new mailbox rather than reopening a stopped generation.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Binding{pub capture:Uuid,pub session:Uuid,pub generation:u64}
impl Binding{fn valid(self)->bool{!self.capture.is_nil()&&!self.session.is_nil()&&self.generation>0}}
pub struct Frame{
    pub binding:Binding,pub sequence:u64,pub timestamp_us:u64,
    pub width:u32,pub height:u32,pub format:PixelFormat,
    /// BGRA row stride, or the shared NV12 Y/UV row stride.
    pub stride:u32,pub bytes:Vec<u8>,
}
impl Frame{
    fn validate(&self,binding:Binding,profile:Profile,format:PixelFormat)->Result<()>{
        if self.binding!=binding||self.sequence==0||self.sequence>u64::from(u32::MAX)||
            self.width!=profile.width||self.height!=profile.height||self.format!=format{return Err(invalid());}
        let minimum=match format{PixelFormat::Bgra8=>self.width.checked_mul(4),PixelFormat::Nv12=>Some(self.width)}.ok_or_else(invalid)?;
        if self.stride<minimum||self.stride>minimum+4096||self.stride%4!=0{return Err(invalid());}
        let rows=match format{PixelFormat::Bgra8=>self.height,PixelFormat::Nv12=>self.height+self.height/2};
        let length=(self.stride as usize).checked_mul(rows as usize).ok_or_else(invalid)?;
        if length>16*1024*1024||self.bytes.len()!=length{return Err(invalid());}Ok(())
    }
}
struct Queued{frame:Frame,received:Instant}
struct State{closed:bool,last_sequence:u64,last_timestamp:Option<u64>,frames:VecDeque<Queued>}
/// Two owned raw frames maximum. Producer overload discards the oldest raw
/// frame; encoded dependent frames must NOT use this drop policy. Local receipt
/// age, not a peer timestamp, bounds delivery latency.
pub struct FrameMailbox{binding:Binding,profile:Profile,format:PixelFormat,deadline:Instant,state:Mutex<State>}
impl FrameMailbox{
    pub fn new(binding:Binding,profile:Profile,format:PixelFormat,remaining:Duration)->Result<Self>{
        profile.validate()?;
        if !binding.valid()||remaining.is_zero()||remaining>Duration::from_secs(30*60){return Err(unauthorized());}
        Ok(Self{binding,profile,format,deadline:Instant::now()+remaining,
            state:Mutex::new(State{closed:false,last_sequence:0,last_timestamp:None,frames:VecDeque::with_capacity(2)})})
    }
    pub fn push(&self,frame:Frame)->Result<()>{
        frame.validate(self.binding,self.profile,self.format)?;
        let mut state=self.state.lock().map_err(|_|unauthorized())?;
        if state.closed||Instant::now()>=self.deadline{state.closed=true;state.frames.clear();return Err(unauthorized());}
        if frame.sequence<=state.last_sequence||state.last_timestamp.is_some_and(|previous|frame.timestamp_us<=previous){return Err(invalid());}
        state.last_sequence=frame.sequence;state.last_timestamp=Some(frame.timestamp_us);
        if state.frames.len()==2{state.frames.pop_front();}
        state.frames.push_back(Queued{frame,received:Instant::now()});Ok(())
    }
    pub fn take_latest(&self)->Result<Option<Frame>>{
        let mut state=self.state.lock().map_err(|_|unauthorized())?;
        if state.closed||Instant::now()>=self.deadline{state.closed=true;state.frames.clear();return Err(unauthorized());}
        let latest=state.frames.pop_back();state.frames.clear();
        Ok(latest.filter(|value|value.received.elapsed()<=Duration::from_millis(100)).map(|value|value.frame))
    }
    /// Stops delivery before native teardown. This is not an OS stop receipt;
    /// the owning service must separately retain unresolved backend cleanup.
    pub fn close(&self)->Result<()>{let mut state=self.state.lock().map_err(|_|unauthorized())?;state.closed=true;state.frames.clear();Ok(())}
}
