use omniplay_contracts::{ContentHash, ErrorCode, PlatformError, Result, RuntimeBenchmark, valid_identifier};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const BENCHMARK_MAX_ENTRIES: usize = 1024;
pub const BENCHMARK_EXPIRY_MS: u64 = 90 * 24 * 60 * 60 * 1000; // 90 days

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkEntry {
    pub runtime_id: String,
    pub core_config_hash: ContentHash,
    pub game_content_hash: ContentHash,
    pub device_class: String,
    pub source_revision: String,
    pub measured_at_ms: u64,
    pub nominal_refresh_hz: f64,
    pub frame_time_p95_ms: f64,
    pub audio_underruns: u32,
    pub sustained_seconds: u32,
    pub fps_average: f64,
    pub ram_peak_mb: f64,
    pub power_watts_delta: Option<f64>,
    pub thermal_state_end: Option<String>,
    pub lab_attested: bool,
}

impl BenchmarkEntry {
    pub fn validate(&self) -> Result<()> {
        if !valid_identifier(&self.runtime_id) || self.runtime_id.len() > 64 {
            return Err(PlatformError::new(ErrorCode::InvalidRequest, "Invalid benchmark runtime identifier"));
        }
        if self.device_class.is_empty() || self.device_class == "unclassified" || self.device_class.len() > 128 {
            return Err(PlatformError::new(ErrorCode::InvalidRequest, "Benchmark requires an explicit device class"));
        }
        if self.source_revision.len() != 40 || !self.source_revision.bytes().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()) {
            return Err(PlatformError::new(ErrorCode::InvalidRequest, "Benchmark requires a 40-character lowercase hex git revision"));
        }
        if self.sustained_seconds < 120 || self.sustained_seconds > 86_400 {
            return Err(PlatformError::new(ErrorCode::InvalidRequest, "Benchmark sustained duration must be between 120 and 86400 seconds"));
        }
        if !self.nominal_refresh_hz.is_finite() || !(20.0..=240.0).contains(&self.nominal_refresh_hz) {
            return Err(PlatformError::new(ErrorCode::InvalidRequest, "Benchmark nominal refresh rate must be between 20 and 240 Hz"));
        }
        if !self.frame_time_p95_ms.is_finite() || self.frame_time_p95_ms <= 0.0 || self.frame_time_p95_ms > 1000.0 {
            return Err(PlatformError::new(ErrorCode::InvalidRequest, "Benchmark frame time p95 must be positive and <= 1000 ms"));
        }
        if !self.fps_average.is_finite() || self.fps_average <= 0.0 || self.fps_average > 480.0 {
            return Err(PlatformError::new(ErrorCode::InvalidRequest, "Benchmark average FPS must be positive and <= 480"));
        }
        if !self.ram_peak_mb.is_finite() || self.ram_peak_mb <= 0.0 || self.ram_peak_mb > 64.0 * 1024.0 {
            return Err(PlatformError::new(ErrorCode::InvalidRequest, "Benchmark RAM peak must be positive and <= 64 GiB"));
        }
        if let Some(w) = self.power_watts_delta {
            if !w.is_finite() || w < 0.0 || w > 1000.0 {
                return Err(PlatformError::new(ErrorCode::InvalidRequest, "Benchmark power delta must be finite and <= 1000 W"));
            }
        }
        ContentHash::parse(self.core_config_hash.0.clone())?;
        ContentHash::parse(self.game_content_hash.0.clone())?;
        Ok(())
    }

    pub fn to_runtime_benchmark(&self) -> RuntimeBenchmark {
        RuntimeBenchmark {
            source_revision: self.source_revision.clone(),
            device_class: self.device_class.clone(),
            game_content_hash: self.game_content_hash.clone(),
            measured_at_ms: self.measured_at_ms,
            nominal_refresh_hz: self.nominal_refresh_hz,
            frame_time_p95_ms: self.frame_time_p95_ms,
            audio_underruns: self.audio_underruns,
            sustained_seconds: self.sustained_seconds,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BenchmarkRegistry {
    entries: BTreeMap<String, BenchmarkEntry>,
}

impl BenchmarkRegistry {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    fn key(runtime_id: &str, game_hash: &str, device_class: &str) -> String {
        format!("{}:{}:{}", runtime_id, game_hash, device_class)
    }

    pub fn register(&mut self, entry: BenchmarkEntry) -> Result<()> {
        entry.validate()?;
        if self.entries.len() >= BENCHMARK_MAX_ENTRIES && !self.entries.contains_key(&Self::key(&entry.runtime_id, &entry.game_content_hash.0, &entry.device_class)) {
            return Err(PlatformError::new(ErrorCode::ResourceLimit, "Benchmark registry capacity reached"));
        }
        let k = Self::key(&entry.runtime_id, &entry.game_content_hash.0, &entry.device_class);
        self.entries.insert(k, entry);
        Ok(())
    }

    pub fn lookup(&self, runtime_id: &str, game_hash: &ContentHash, device_class: &str, now_ms: u64) -> Option<&BenchmarkEntry> {
        let k = Self::key(runtime_id, &game_hash.0, device_class);
        let entry = self.entries.get(&k)?;
        if entry.measured_at_ms <= now_ms && now_ms.saturating_sub(entry.measured_at_ms) <= BENCHMARK_EXPIRY_MS {
            Some(entry)
        } else {
            None
        }
    }

    pub fn list(&self) -> Vec<BenchmarkEntry> {
        self.entries.values().cloned().collect()
    }

    pub fn count(&self) -> usize {
        self.entries.len()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn from_summary(summary: &omniplay_contracts::PerformanceRunSummary, fallback_device_class: &str) -> Option<BenchmarkEntry> {
        let device_class = if summary.device_class.is_empty() || summary.device_class == "unclassified" {
            fallback_device_class.to_string()
        } else {
            summary.device_class.clone()
        };
        let entry = BenchmarkEntry {
            runtime_id: summary.runtime_id.clone(),
            core_config_hash: summary.core_config_hash.clone(),
            game_content_hash: summary.game_content_hash.clone(),
            device_class,
            source_revision: summary.source_revision.clone(),
            measured_at_ms: summary.created_at_ms,
            nominal_refresh_hz: summary.nominal_refresh_hz,
            frame_time_p95_ms: summary.frame_time_p95_ms,
            audio_underruns: summary.audio_underruns,
            sustained_seconds: summary.observed_duration_ms / 1000,
            fps_average: summary.presented_frames_per_second.max(0.1),
            ram_peak_mb: 128.0,
            power_watts_delta: None,
            thermal_state_end: None,
            lab_attested: summary.independently_attested,
        };
        if entry.validate().is_ok() {
            Some(entry)
        } else {
            None
        }
    }

    pub fn load_from_database(&mut self, database: &crate::Database, game_id: uuid::Uuid, fallback_device_class: &str) {
        if let Ok(runs) = crate::performance::PerformanceRuns::new_readonly(database).list(game_id) {
            for summary in runs.into_iter().rev() {
                if let Some(entry) = Self::from_summary(&summary, fallback_device_class) {
                    let _ = self.register(entry);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::{rank, RoutingPolicy};
    use omniplay_contracts::{
        DeviceCapabilities, GameManifest, GameSystem, PresentationDropReason,
        PresentationMetricsReport, PresentationTimestampRecord, RouteReason, RuntimeCandidate,
        RuntimeKind,
    };
    use uuid::Uuid;

    fn sample_entry(runtime_id: &str, measured_at_ms: u64, p95_ms: f64, underruns: u32) -> BenchmarkEntry {
        BenchmarkEntry {
            runtime_id: runtime_id.to_string(),
            core_config_hash: ContentHash("a".repeat(64)),
            game_content_hash: ContentHash("b".repeat(64)),
            device_class: "desktop-x86_64-dedicated-gpu".to_string(),
            source_revision: "3b63372f78c20b8b7c3255d202c750de42e12345".to_string(),
            measured_at_ms,
            nominal_refresh_hz: 60.0,
            frame_time_p95_ms: p95_ms,
            audio_underruns: underruns,
            sustained_seconds: 120,
            fps_average: 60.0,
            ram_peak_mb: 128.0,
            power_watts_delta: Some(15.0),
            thermal_state_end: Some("nominal".to_string()),
            lab_attested: true,
        }
    }

    #[test]
    fn benchmark_registry_validation_and_90_day_ttl() {
        let now = 1_729_850_000_000;
        let mut reg = BenchmarkRegistry::new();
        let entry = sample_entry("mgba-native", now - 1000, 14.5, 0);
        reg.register(entry).expect("register valid entry");

        let game_hash = ContentHash("b".repeat(64));
        assert!(reg
            .lookup("mgba-native", &game_hash, "desktop-x86_64-dedicated-gpu", now)
            .is_some());
        assert!(reg
            .lookup("mgba-native", &game_hash, "wrong-device", now)
            .is_none());
        assert!(reg
            .lookup(
                "mgba-native",
                &game_hash,
                "desktop-x86_64-dedicated-gpu",
                now + BENCHMARK_EXPIRY_MS + 1
            )
            .is_none());

        let mut invalid = sample_entry("mgba-native", now, 14.5, 0);
        invalid.sustained_seconds = 119;
        assert!(reg.register(invalid).is_err());
    }

    #[test]
    fn router_ranking_applies_benchmark_bonus_penalty_and_thermal_battery_rules() {
        let now = 1_729_850_000_000;
        let game = GameManifest {
            schema_version: 1,
            id: Uuid::new_v4(),
            title: "Test Game".to_string(),
            system: GameSystem::Gba,
            content_hash: ContentHash("b".repeat(64)),
            byte_length: 4 * 1024 * 1024,
            region: None,
            nominal_refresh_hz: Some(60.0),
            created_at_ms: now,
            media: None,
        };
        let fast_bench = sample_entry("mgba-native", now - 1000, 14.2, 0).to_runtime_benchmark();
        let slow_bench = sample_entry("mgba-web", now - 1000, 26.0, 4).to_runtime_benchmark();

        let candidates = vec![
            RuntimeCandidate {
                id: "mgba-web".to_string(),
                kind: RuntimeKind::WebWasm,
                systems: vec![GameSystem::Gba],
                available: true,
                distribution_approved: true,
                security_revoked: false,
                needs_webgpu: false,
                minimum_memory_mb: Some(256),
                benchmark: Some(slow_bench),
            },
            RuntimeCandidate {
                id: "mgba-native".to_string(),
                kind: RuntimeKind::NativeEmulator,
                systems: vec![GameSystem::Gba],
                available: true,
                distribution_approved: true,
                security_revoked: false,
                needs_webgpu: false,
                minimum_memory_mb: Some(256),
                benchmark: Some(fast_bench),
            },
            RuntimeCandidate {
                id: "remote-stream".to_string(),
                kind: RuntimeKind::Remote,
                systems: vec![GameSystem::Gba],
                available: true,
                distribution_approved: true,
                security_revoked: false,
                needs_webgpu: false,
                minimum_memory_mb: None,
                benchmark: None,
            },
        ];
        let policy = RoutingPolicy {
            native_allowed: true,
            remote_allowed: true,
            paired_host_available: true,
            device_class: "desktop-x86_64-dedicated-gpu".to_string(),
        };
        let device = DeviceCapabilities {
            wasm: true,
            webgpu: true,
            webgl2: true,
            touch: false,
            memory_mb: Some(8192),
            battery_percent: Some(15.0),
            thermal_throttled: Some(true),
        };

        let decision = rank(&game, &device, &candidates, &policy, now).expect("rank");
        assert_eq!(decision.recommended_id.as_deref(), Some("mgba-native"));
        // mgba-native: 110 + 50 = 160
        assert_eq!(decision.candidates[0].score, 160);
        assert!(decision.candidates[0]
            .reasons
            .iter()
            .any(|r| matches!(r, RouteReason::MeasuredFrameBudgetMet)));
        // remote-stream: 60 + 10 (battery) + 25 (thermal) = 95
        assert_eq!(decision.candidates[1].id, "remote-stream");
        assert_eq!(decision.candidates[1].score, 95);
        // mgba-web: 100 - 70 = 30
        assert_eq!(decision.candidates[2].id, "mgba-web");
        assert_eq!(decision.candidates[2].score, 30);
    }

    #[test]
    fn presentation_timing_records_and_report_invariants() {
        let rec = PresentationTimestampRecord {
            frame_index: 1,
            render_timestamp_ns: 10_000_000,
            enqueue_timestamp_ns: 10_200_000,
            present_timestamp_ns: 12_000_000,
            flip_timestamp_ns: Some(13_500_000),
            dropped: false,
            drop_reason: None,
        };
        assert!(rec.is_valid());
        assert_eq!(rec.render_to_present_ms(), Some(2.0));
        assert_eq!(rec.present_to_flip_ms(), Some(1.5));

        let dropped = PresentationTimestampRecord {
            frame_index: 2,
            render_timestamp_ns: 20_000_000,
            enqueue_timestamp_ns: 20_100_000,
            present_timestamp_ns: 20_100_000,
            flip_timestamp_ns: None,
            dropped: true,
            drop_reason: Some(PresentationDropReason::QueueOverflow),
        };
        assert!(dropped.is_valid());
        assert_eq!(dropped.render_to_present_ms(), None);

        let mut report = PresentationMetricsReport {
            session_id: Uuid::new_v4(),
            sample_at_ms: 1_729_850_000_000,
            presentation_backend: "d3d11_flip_discard".to_string(),
            frames_emulated: 120,
            frames_presented: 118,
            frames_dropped: 2,
            queue_depth_current: 1,
            queue_depth_max: 2,
            render_to_present_p50_ms: 1.8,
            render_to_present_p95_ms: 2.4,
            render_to_present_p99_ms: 3.1,
            present_to_flip_p50_ms: Some(1.2),
            present_to_flip_p95_ms: Some(1.9),
            jitter_p95_ms: 0.4,
            audio_dac_starvations: 0,
        };
        assert!(report.is_valid());
        report.queue_depth_max = 3;
        assert!(!report.is_valid());
    }
}
