use crate::{ContentHash,PerformancePreset};
use serde::{Deserialize,Serialize};
use uuid::Uuid;

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerformanceRunInput {
    pub id:Uuid,pub game_id:Uuid,pub game_content_hash:ContentHash,pub runtime_id:String,
    pub runtime_version:semver::Version,pub source_revision:String,pub runtime_build_hash:ContentHash,
    pub core_config_hash:ContentHash,pub preset:PerformancePreset,pub nominal_refresh_hz:f64,
    pub observed_duration_ms:u32,pub warmup_ms:u32,pub frames_presented:u32,pub frames_emulated:u32,
    pub dropped_frames:u32,pub audio_underruns:u32,pub frame_intervals_ms:Vec<f64>,
}

#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct PerformanceRunSummary {
    pub id:Uuid,pub game_id:Uuid,pub created_at_ms:u64,pub platform:String,pub device_class:String,
    pub runtime_id:String,pub runtime_version:semver::Version,pub source_revision:String,
    pub runtime_build_hash:ContentHash,pub game_content_hash:ContentHash,pub core_config_hash:ContentHash,
    pub preset:PerformancePreset,pub observed_duration_ms:u32,pub warmup_ms:u32,
    pub nominal_refresh_hz:f64,pub frames_presented:u32,pub frames_emulated:u32,
    pub dropped_frames:u32,pub audio_underruns:u32,pub frame_time_p50_ms:f64,
    pub frame_time_p95_ms:f64,pub frame_time_p99_ms:f64,pub frames_per_second:f64,
    pub presented_frames_per_second:f64,pub measurement_source:String,pub independently_attested:bool,
}

#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct PerformanceComparison {pub baseline:PerformanceRunSummary,pub candidate:PerformanceRunSummary,pub frame_time_p95_change_percent:f64,pub audio_underrun_change:i64,pub controlled_hardware_comparison:bool}

#[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum NativeMeasurementState {Warming,Measuring,Complete,Cancelled}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeMeasurementStatus {
    pub id:Uuid,pub state:NativeMeasurementState,pub elapsed_ms:u32,
    pub warmup_ms:u32,pub target_ms:u32,pub cancel_reason:Option<String>,
}
impl NativeMeasurementStatus {
    pub fn validate(&self)->crate::Result<()> {
        let valid=self.target_ms==120_000&&self.elapsed_ms<=240_000&&self.warmup_ms<=60_000&&
            self.cancel_reason.as_ref().is_none_or(|r|!r.is_empty()&&r.len()<=64&&r.bytes().all(|b|b.is_ascii_lowercase()||b==b'_'))&&
            match self.state {
                NativeMeasurementState::Warming=>self.cancel_reason.is_none(),
                NativeMeasurementState::Measuring=>self.warmup_ms>=10_000&&self.cancel_reason.is_none(),
                NativeMeasurementState::Complete=>(120_000..=180_000).contains(&self.elapsed_ms)&&self.warmup_ms>=10_000&&self.cancel_reason.is_none(),
                NativeMeasurementState::Cancelled=>self.cancel_reason.is_some(),
            };
        if valid{Ok(())}else{Err(crate::PlatformError::new(crate::ErrorCode::CorruptData,"Native measurement status is invalid"))}
    }
}

#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct NativeMeasurementReport {pub capture:NativeMeasurementStatus,pub summary:Option<PerformanceRunSummary>}
