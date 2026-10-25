use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationDropReason {
    QueueOverflow,
    LateFrame,
    FocusLoss,
    ThermalThrottle,
    DisplayDisconnected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationTimestampRecord {
    pub frame_index: u64,
    pub render_timestamp_ns: u64,
    pub enqueue_timestamp_ns: u64,
    pub present_timestamp_ns: u64,
    pub flip_timestamp_ns: Option<u64>,
    pub dropped: bool,
    pub drop_reason: Option<PresentationDropReason>,
}

impl PresentationTimestampRecord {
    pub fn is_valid(&self) -> bool {
        if self.render_timestamp_ns == 0 || self.enqueue_timestamp_ns < self.render_timestamp_ns {
            return false;
        }
        if !self.dropped {
            if self.present_timestamp_ns < self.enqueue_timestamp_ns {
                return false;
            }
            if let Some(flip) = self.flip_timestamp_ns {
                if flip < self.present_timestamp_ns {
                    return false;
                }
            }
            self.drop_reason.is_none()
        } else {
            self.drop_reason.is_some()
        }
    }

    pub fn render_to_present_ms(&self) -> Option<f64> {
        if self.dropped || self.present_timestamp_ns < self.render_timestamp_ns {
            None
        } else {
            Some((self.present_timestamp_ns - self.render_timestamp_ns) as f64 / 1_000_000.0)
        }
    }

    pub fn present_to_flip_ms(&self) -> Option<f64> {
        if self.dropped {
            None
        } else {
            self.flip_timestamp_ns.and_then(|flip| {
                if flip >= self.present_timestamp_ns {
                    Some((flip - self.present_timestamp_ns) as f64 / 1_000_000.0)
                } else {
                    None
                }
            })
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationMetricsReport {
    pub session_id: Uuid,
    pub sample_at_ms: u64,
    pub presentation_backend: String,
    pub frames_emulated: u64,
    pub frames_presented: u64,
    pub frames_dropped: u64,
    pub queue_depth_current: u32,
    pub queue_depth_max: u32,
    pub render_to_present_p50_ms: f64,
    pub render_to_present_p95_ms: f64,
    pub render_to_present_p99_ms: f64,
    pub present_to_flip_p50_ms: Option<f64>,
    pub present_to_flip_p95_ms: Option<f64>,
    pub jitter_p95_ms: f64,
    pub audio_dac_starvations: u32,
}

impl PresentationMetricsReport {
    pub fn is_valid(&self) -> bool {
        let times = [
            self.render_to_present_p50_ms,
            self.render_to_present_p95_ms,
            self.render_to_present_p99_ms,
            self.jitter_p95_ms,
        ];
        self.queue_depth_max <= 2
            && self.frames_emulated >= self.frames_presented.saturating_add(self.frames_dropped)
            && times.iter().all(|t| t.is_finite() && *t >= 0.0)
            && times[0] <= times[1]
            && times[1] <= times[2]
            && self.present_to_flip_p50_ms.zip(self.present_to_flip_p95_ms).map_or(true, |(p50, p95)| {
                p50.is_finite() && p95.is_finite() && p50 >= 0.0 && p50 <= p95
            })
            && !self.presentation_backend.is_empty()
            && self.presentation_backend.len() <= 64
    }
}
