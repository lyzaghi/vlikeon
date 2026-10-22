use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameMetrics {
    pub session_id: Uuid,
    pub sample_at_ms: u64,
    pub window_duration_ms: u32,
    pub frames_presented: u32,
    pub dropped_frames: u32,
    pub audio_underruns: u32,
    pub frame_time_p50_ms: f64,
    pub frame_time_p95_ms: f64,
    pub frame_time_p99_ms: f64,
    pub cpu_busy_percent: Option<f64>,
    pub gpu_busy_percent: Option<f64>,
    pub thermal_throttled: Option<bool>,
    pub battery_percent: Option<f64>,
    pub rtt_ms: Option<f64>,
    pub packet_loss_percent: Option<f64>,
    pub decode_queue_ms: Option<f64>,
}

impl FrameMetrics {
    pub fn is_valid(&self) -> bool {
        let times = [self.frame_time_p50_ms, self.frame_time_p95_ms, self.frame_time_p99_ms];
        let percentages = [self.cpu_busy_percent, self.gpu_busy_percent, self.battery_percent, self.packet_loss_percent];
        self.window_duration_ms > 0 && self.window_duration_ms <= 300_000 &&
            times.iter().all(|n| n.is_finite() && *n >= 0.0) &&
            times[0] <= times[1] && times[1] <= times[2] &&
            percentages.iter().flatten().all(|n| n.is_finite() && (0.0..=100.0).contains(n)) &&
            [self.rtt_ms, self.decode_queue_ms].iter().flatten().all(|n| n.is_finite() && *n >= 0.0)
    }
}
