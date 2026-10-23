# Game routing and optional local recommendations

Current item-13 fixes and open scope: `ITEM13_PERFORMANCE_ROUTING_ASSISTANCE.md`.
Assistant acceptance is transactional with its audit and retains proposals after
failed commits; Web checks expiry inside its storage transaction. Web ranking now
checks catalog hardware/thread policies and strict device fields. Unknown device
classes cannot confer benchmark ranking credit. Source review only, no tests.

The router in `platform/core/src/router.rs` filters runtime availability, systems, distribution approval, revocation, WASM/GPU/memory requirements and platform consent before ranking. It never changes runtime inside a live game. Unknown memory, battery or thermal values remain unknown. A benchmark can affect ranking only when its game hash, device class, source revision, age, sustained duration and measured frame budget are applicable. The broker currently discards renderer-supplied benchmark claims and marks all native and remote candidates unavailable until their real registries exist. No synthetic benchmark or assumed FPS result is populated.

The current privileged Game Hub requests a choice among its bundled web-core catalog. Core execution independently checks module origin, WASM byte length, SHA-256, system and runtime version. A routing recommendation does not grant a filesystem capability or approve a native executable. Native sandbox and streaming registries remain outstanding.

Per-game settings persist separately from global settings. The web runtime applies volume, bounded audio buffering and presentation mode when preparing a new game. Accurate mode presents every available core frame. Smooth mode uses a minimum audio queue of three emulation frames. Battery mode limits video presentation to 30 Hz while the libretro clock and audio continue at their negotiated nominal rate. The audio worklet bounds its queue and re-primes after starvation. None of these modes claims a measured FPS or battery improvement on an unmeasured device.

The optional assistant is explicitly `local_rules_v1`. It produces advice for audio, uneven frame timing, controls, saves, battery use and privacy. No external model has been connected, no API key is required and no content is sent to an AI provider. The global opt-in defaults to false. Proposed audio or presentation changes are bounded, expire after five minutes, are bound to the current configuration hash and require a user action before changing the next-session preferences. The assistant cannot execute shell commands, change GPU/OS settings, inject into installed games or modify emulation speed. A later model adapter must pass the same proposal boundary; arbitrary model output is not an executable action.

Web-session accounting is observational and separate from native runtime control. It credits monotonic elapsed time between renderer lifecycle reports, flushes bounded pending playtime transactionally and fails abandoned sessions after missing heartbeats. It does not claim that the broker owns the WASM worker. Renderer heartbeat failure pauses the local runtime; saves remain independently exportable. A browser restart marks interrupted persisted sessions failed instead of inventing successful recovery.

No runtime, policy, timing or user-interface checks have been executed during this authoring phase. Hardware benchmarks, preset comparisons, lifecycle races, privacy checks and the real engine compatibility matrix remain part of the final consolidated verification.
