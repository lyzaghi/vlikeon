use crate::{ErrorCode,GameSystem,PlatformError,Result,RuntimeKind,SessionConfig};
use serde::{Deserialize,Serialize};
use uuid::Uuid;

#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceCapabilities {
    pub wasm:bool,pub webgpu:bool,pub webgl2:bool,pub touch:bool,
    pub memory_mb:Option<u32>,pub battery_percent:Option<f64>,pub thermal_throttled:Option<bool>,
}
impl DeviceCapabilities {
    pub fn validate(&self)->Result<()> {
        if self.memory_mb.is_some_and(|v|v==0||v>4*1024*1024)||self.battery_percent.is_some_and(|v|!v.is_finite()||!(0.0..=100.0).contains(&v)){return Err(PlatformError::new(ErrorCode::InvalidRequest,"Device capabilities are outside their bounds"));}Ok(())
    }
}

#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCandidate {
    pub id:String,pub kind:RuntimeKind,pub systems:Vec<GameSystem>,pub available:bool,
    pub distribution_approved:bool,pub security_revoked:bool,pub needs_webgpu:bool,
    pub minimum_memory_mb:Option<u32>,pub benchmark:Option<RuntimeBenchmark>,
}

#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeBenchmark {
    pub source_revision:String,pub device_class:String,pub game_content_hash:crate::ContentHash,
    pub measured_at_ms:u64,pub nominal_refresh_hz:f64,pub frame_time_p95_ms:f64,
    pub audio_underruns:u32,pub sustained_seconds:u32,
}

#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum RouteReason {AvailableLocalRuntime,RequiresWebAssembly,RequiresWebGpu,NotEnoughMemory,NotInstalled,Unapproved,Revoked,WrongSystem,ConsentRequired,UnmeasuredOnDevice,MeasuredFrameBudgetMet,MeasuredFrameBudgetExceeded,RemoteRequiresPairedHost,BatteryConservation,ThermalPressure}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct RankedRuntime {pub id:String,pub kind:RuntimeKind,pub eligible:bool,pub score:i32,pub reasons:Vec<RouteReason>}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct RouteDecision {pub recommended_id:Option<String>,pub candidates:Vec<RankedRuntime>,pub automatic_mid_session_switch:bool}

#[derive(Clone,Copy,Debug,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum AdviceTopic {Stutter,Audio,Controls,Saves,Battery,Privacy}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct SessionProposal {pub id:Uuid,pub game_id:Uuid,pub expires_at_ms:u64,pub before_hash:crate::ContentHash,pub after:SessionConfig,pub reason_code:String}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct LocalAdvice {pub source:String,pub locale:String,pub message:String,pub proposal:Option<SessionProposal>,pub external_model_used:bool}
