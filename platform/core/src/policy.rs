use omniplay_contracts::{Capability, FrameMetrics, RuntimeKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfiguration { pub resolution_scale: f64, pub stream_bitrate_mbps: f64 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdjustmentReason { SustainedGpuPressure, SustainedNetworkLoss, RegressionRollback }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Adjustment { pub before: PerformanceConfiguration, pub after: PerformanceConfiguration, pub reason: AdjustmentReason, pub at_ms: u64 }

pub struct PolicyContext {
    pub enabled: bool,
    pub permission_granted: bool,
    pub anti_cheat_sensitive: bool,
    pub runtime: RuntimeKind,
    pub capabilities: BTreeSet<Capability>,
    pub nominal_refresh_hz: f64,
}

struct Observation { baseline_p95: f64, until_ms: u64, action: Adjustment }

#[derive(Default)]
pub struct PerformanceController { pressure_since_ms: Option<u64>, network_since_ms: Option<u64>, last_sample_ms: Option<u64>, cooldown_until_ms: u64, observation: Option<Observation> }

impl PerformanceController {
    pub fn observe(&mut self, context: &PolicyContext, configuration: &PerformanceConfiguration, metric: &FrameMetrics, now_ms: u64) -> Option<Adjustment> {
        if !metric.is_valid() || metric.sample_at_ms > now_ms || now_ms - metric.sample_at_ms > 10_000 || self.last_sample_ms.is_some_and(|last| metric.sample_at_ms <= last) { return None; }
        if self.last_sample_ms.is_some_and(|last| metric.sample_at_ms - last > 10_000) { self.pressure_since_ms = None; self.network_since_ms = None; }
        self.last_sample_ms = Some(metric.sample_at_ms);
        if !context.enabled || !context.permission_granted || context.anti_cheat_sensitive || !context.nominal_refresh_hz.is_finite() || !(20.0..=240.0).contains(&context.nominal_refresh_hz) {
            self.pressure_since_ms = None;
            self.network_since_ms = None;
            return None;
        }
        if let Some(observation) = &self.observation {
            if metric.frame_time_p95_ms > observation.baseline_p95 * 1.10 && metric.sample_at_ms >= observation.action.at_ms.saturating_add(5_000) {
                let action = Adjustment { before: configuration.clone(), after: observation.action.before.clone(), reason: AdjustmentReason::RegressionRollback, at_ms: now_ms };
                self.observation = None;
                self.cooldown_until_ms = now_ms.saturating_add(120_000);
                return Some(action);
            }
            if now_ms < observation.until_ms { return None; }
            self.observation = None;
        }
        if now_ms < self.cooldown_until_ms { return None; }
        let frame_budget = 1000.0 / context.nominal_refresh_hz;
        let gpu_pressure = metric.frame_time_p95_ms > frame_budget * 1.20 && metric.gpu_busy_percent.is_some_and(|v|v > 90.0);
        let network_pressure = context.runtime == RuntimeKind::Remote && metric.packet_loss_percent.is_some_and(|v|v > 2.0);
        self.pressure_since_ms = sustained(self.pressure_since_ms,gpu_pressure,now_ms);
        self.network_since_ms = sustained(self.network_since_ms,network_pressure,now_ms);
        let mut next = configuration.clone();
        let reason = if self.network_since_ms.is_some_and(|start|now_ms.saturating_sub(start) >= 30_000) && configuration.stream_bitrate_mbps.is_finite() && configuration.stream_bitrate_mbps > 2.0 {
            next.stream_bitrate_mbps = (configuration.stream_bitrate_mbps * 0.85).clamp(2.0,80.0);
            AdjustmentReason::SustainedNetworkLoss
        } else if context.capabilities.contains(&Capability::DynamicResolution) && self.pressure_since_ms.is_some_and(|start|now_ms.saturating_sub(start) >= 30_000) && configuration.resolution_scale.is_finite() && configuration.resolution_scale > 0.5 {
            next.resolution_scale = (configuration.resolution_scale - 0.1).clamp(0.5,1.0);
            AdjustmentReason::SustainedGpuPressure
        } else { return None; };
        let action = Adjustment { before: configuration.clone(), after: next, reason, at_ms: now_ms };
        self.observation = Some(Observation { baseline_p95: metric.frame_time_p95_ms, until_ms: now_ms.saturating_add(30_000), action: action.clone() });
        self.cooldown_until_ms = now_ms.saturating_add(60_000);
        self.pressure_since_ms = None;
        self.network_since_ms = None;
        Some(action)
    }
}

fn sustained(previous: Option<u64>, active: bool, now: u64) -> Option<u64> { if active { Some(previous.unwrap_or(now)) } else { None } }
