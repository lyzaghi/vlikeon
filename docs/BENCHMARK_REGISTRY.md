# OmniPlay Benchmark Registry & Presentation Instrumentation Specification

This document defines the architecture, data schema, validation rules, and runtime router integration for the OmniPlay Benchmark Registry (Items 84–86) and controlled benchmark qualification (Item 87).

---

## 1. Benchmark Registry Architecture

The Benchmark Registry provides authoritative performance attestation for pairs of `(Runtime, Game, DeviceClass)`.

### 1.1 Key Invariants & Storage
- **Key Compound**: `(runtime_id, game_content_hash, device_class)`
- **Capacity**: Bounded to at most `1024` entries in memory to prevent resource exhaustion.
- **TTL Lifecycle**: Strict 90-day expiration window (`now - measured_at_ms <= 90 * 86_400_000 ms`).
- **No Mock or Inferred FPS**: Benchmark records must be backed by actual measured intervals; synthetic or estimated entries are rejected at ingestion.

### 1.2 Schema Definition

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BenchmarkEntry {
    pub runtime_id: String,
    pub core_config_hash: ContentHash,
    pub game_content_hash: ContentHash,
    pub device_class: String,
    pub source_revision: String,       // Exactly 40 lowercase hex characters
    pub measured_at_ms: u64,
    pub nominal_refresh_hz: f64,       // 20.0 .. 240.0 Hz
    pub frame_time_p95_ms: f64,        // > 0.0 and <= 1000.0 ms
    pub audio_underruns: u32,
    pub sustained_seconds: u32,        // >= 120 seconds
    pub fps_average: f64,              // > 0.0 and <= 480.0
    pub ram_peak_mb: f64,              // > 0.0 and <= 65536.0 MiB
    pub power_watts_delta: Option<f64>,// Finite, >= 0.0, <= 1000.0 W
    pub thermal_state_end: Option<String>,
    pub lab_attested: bool,
}
```

### 1.3 Validation Invariants
1. `runtime_id`: Valid identifier, length <= 64 characters.
2. `device_class`: Non-empty, != `"unclassified"`, length <= 128 characters.
3. `source_revision`: Exact 40 lowercase ASCII hex characters (git commit SHA).
4. `sustained_seconds`: Must be at least 120 seconds (2 minutes uninterrupted).
5. `nominal_refresh_hz`: Bounded within `[20.0, 240.0]`.
6. `frame_time_p95_ms`: Finite, strictly positive, `<= 1000.0` ms.
7. `game_content_hash` & `core_config_hash`: Valid SHA-256 digests (64 hex characters).

---

## 2. Runtime Selection & Router Scoring (Item 85)

The routing engine evaluates available candidates using the following scoring matrix:

### 2.1 Base Candidate Scores
- `InstalledGame`: 120
- `NativeEmulator`: 110
- `WebWasm`: 100
- `Remote`: 60

### 2.2 Benchmark Adjustments
When a qualified, non-expired benchmark exists for `(candidate.id, game.content_hash, device_class)`:
- **Budget Met**: If `frame_time_p95_ms <= (1000.0 / nominal_refresh_hz) * 1.10` AND `audio_underruns == 0`:
  - **Score**: `+50`
  - **Reason**: `MeasuredFrameBudgetMet`
- **Budget Exceeded**: If p95 frame time exceeds budget or audio underruns occur:
  - **Score**: `-70`
  - **Reason**: `MeasuredFrameBudgetExceeded`
- **Unmeasured**: If benchmark is absent or expired (> 90 days):
  - **Score**: `+0`
  - **Reason**: `UnmeasuredOnDevice`

### 2.3 Environmental Adaptations
- **Low Battery (< 20%)**: Adds `+10` to `Remote` runtime candidate (`BatteryConservation`).
- **Thermal Pressure (Throttled)**: Adds `+25` to `Remote` runtime candidate (`ThermalPressure`).

---

## 3. Presentation Timing Instrumentation (Item 86)

Brokered presentation helpers (Windows DXGI/SDL, Linux Wayland/EGL, macOS Metal/Layer, Web Canvas) instrument the complete frame delivery lifecycle:

```
[Core retro_run] ──render_ns──> [Presentation Queue] ──enqueue_ns──> [Helper Present] ──present_ns──> [Compositor Flip] ──flip_ns──> Display
```

### 3.1 Timing Invariants
1. `render_ns <= enqueue_ns <= present_ns <= flip_ns`
2. **Queue Bounds**: Presentation queue depth is capped at **2 frames**. Any subsequent frame emitted before the presentation helper retires a prior frame is dropped with `drop_reason: queue_overflow`.
3. **Metrics Reported**:
   - `render_to_present_p50_ms`, `render_to_present_p95_ms`, `render_to_present_p99_ms`
   - `present_to_flip_p50_ms`, `present_to_flip_p95_ms`
   - `jitter_p95_ms` (consecutive frame delivery variation)
   - `audio_dac_starvations`
