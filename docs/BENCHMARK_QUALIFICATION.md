# OmniPlay Controlled Benchmark Qualification Matrix (Group J, Item 87)

This document establishes the hardware test matrix, controlled lab environment parameters, acceptance criteria, and deferred qualification queue for Item 87.

---

## 1. Controlled Lab Environment Specifications

To ensure reproducible, scientifically rigorous benchmarks without synthetic mocking, the following lab parameters are strictly enforced:

| Parameter | Lab Condition Specification |
|:---|:---|
| **Ambient Temperature** | 21°C ± 1°C in an environmentally controlled enclosure |
| **System Power** | AC power supply connected, battery pre-charged to >= 80% (or dedicated battery drain runs) |
| **Background Processes** | Clean boot, OS indexing, background updates, and antivirus scans disabled |
| **Display Configuration** | Hardware fixed at 60.0 Hz or 120.0 Hz; adaptive sync / VRR disabled |
| **Warm-Up Period** | Minimum 10 seconds of active frame rendering before interval collection |
| **Sustained Duration** | Minimum 120 seconds (2 minutes) of continuous frame sampling |
| **Sample Storage** | Bounded binary float64 intervals; max 36,000 samples per run |

---

## 2. Platform Qualification Matrix

| Target Platform | Reference Hardware | Primary Core Runtimes | Status |
|:---|:---|:---|:---|
| **Windows Desktop** | Intel Core i7-12700H, RTX 3060, 16 GB RAM | `mgba-native`, `beetle-psx-native`, `bsnes-native` | `SOURCE_READY_UNVERIFIED` (Lab execution deferred) |
| **macOS Desktop** | Apple M2 Max, 32 GB unified memory | `mgba-native`, `beetle-psx-native` | `SOURCE_READY_UNVERIFIED` (Lab execution deferred) |
| **Linux Desktop** | AMD Ryzen 7 5800X, RX 6700 XT, Ubuntu 24.04 | `mgba-native`, `beetle-psx-native`, `bsnes-native` | `SOURCE_READY_UNVERIFIED` (Lab execution deferred) |
| **Android Mobile** | Snapdragon 8 Gen 2 / 8s Gen 3, 12 GB RAM | `mgba-native`, `remote` | `SOURCE_READY_UNVERIFIED` (Lab execution deferred) |
| **iOS Mobile** | iPhone 15 Pro (A17 Pro), iOS 17.5 | `mgba-native` (no-JIT), `remote` | `SOURCE_READY_UNVERIFIED` (Lab execution deferred) |
| **Web / PWA** | Chromium 128+, WebAssembly SIMD enabled | `mgba-wasm`, `bsnes-wasm` | `SOURCE_READY_UNVERIFIED` (Lab execution deferred) |

---

## 3. Automated Qualification Suite

The automated benchmark test suite is located at `qa/benchmark_qualification.py`.

### Execution Command:
```bash
py qa/benchmark_qualification.py
```

### Verified Test Cases:
1. `test_benchmark_entry_validation`: Rejects non-hex git SHA, durations < 120s, invalid refresh rates, unclassified device classes.
2. `test_benchmark_registry_lifecycle_and_ttl`: Validates exact lookup keying, 1024-entry ceiling, and 90-day automated TTL expiration.
3. `test_runtime_router_selection`: Verifies `+50` bonus for met budgets, `-70` penalty for exceeded budgets, and remote score adjustments under low battery / thermal throttle.
4. `test_presentation_timing_math`: Verifies monotonic ordering (`render <= enqueue <= present <= flip`) and 2-frame bounded presentation queue drops.

---

## 4. Acceptance Queue & Hardware Qualification Queue

In accordance with project policy, physical hardware benchmark execution is deferred until all 109 items reach code-complete status.

| Item ID | Description | Status | Blocking Dependency |
|:---|:---|:---|:---|
| **84** | Benchmark registry bound to runtime, core config, game ROM, and device | `SOURCE_READY_UNVERIFIED` | Unit verification passing; hardware lab deployment deferred |
| **85** | Native/remote runtime selection based on registry and credentials | `SOURCE_READY_UNVERIFIED` | Router engine implemented; live device lab verification deferred |
| **86** | Presentation timing instrumentation (render -> enqueue -> present -> flip) | `SOURCE_READY_UNVERIFIED` | Native & web collectors authored; physical swapchain trace deferred |
| **87** | Controlled FPS, latency, RAM, power, and thermal benchmarks | `WAITING_PERMISSION_OR_RESOURCE` | Physical device lab, thermal probes, and hardware power meters |
