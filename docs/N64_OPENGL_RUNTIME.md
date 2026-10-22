# N64 GLideN64/HLE product profile source

A distinct WebGL2/HLE/cached-interpreter producer and dedicated Worker graphics
frontend are now authored in `build_mupen64plus_web.py`. They require explicit
context/lifecycle completion and remain uncompiled and unapproved. See
`WEB_N64.md` and `WEB_HARDWARE_VIDEO.md`; no native/Web save compatibility or
platform qualification is implied.

The product candidate is `mupen64plus-gl-native`, using Mupen64Plus-Next at `12edd2c74a517ff86dfa8cfc71ad75e4c10486d5` with a separately identified `3.0.0+omniplay.gl1.12edd2c74a51` core interface. It is distinct from the research-only Angrylion profile. `tools/build_mupen64plus_gl.py --prepare-only` has authored and materialized the adapted corresponding source; no compiler or emulator has run.

This source profile selects GLideN64 RDP, HLE RSP and the cached CPU interpreter. Angrylion, CXD4 and both parallel backend source directories are excluded from its prepared tree. Build flags also exclude those backends and CPU dynarec, explicitly select desktop OpenGL core symbols, retain non-executable instruction-cache storage, use OS fibers preserving floating-point state and disable compiler fast-math. Generated configuration uses private scratch storage. No network capability or executable-heap exception is added to the AppContainer.

The selected root/component notices use GPL v2 terms; the package records a conservative GPL-2.0-only profile pending the full compiled component inventory and corresponding-source review. Excluding incompatible/unresolved inputs does not establish clearance for every remaining source. This profile is not distribution-approved and has no built binary. Its receipt retains explicit outstanding review items. The Angrylion profile remains independently prohibited from release approval.

Native worker source now has a separate `opengl_core` adapter. Its compiled backend identity must match the catalog and handshake. The catalog bounds the N64 target to 640×480; the Rust registry also validates general target dimensions/area. Libretro negotiation accepts only the declared OpenGL core context range and bottom-left origin. The worker requests robust access/reset notification, verifies the resulting robust context and reset-status API, and fails unavailable when required driver behavior is absent. There is no fallback to an undeclared renderer.

The core receives a private offscreen framebuffer with bounded color/depth/stencil storage, cleared before use. It receives no browser graphics context or cross-process surface. Presentation blits that target to the native window with aspect-correct letterboxing, DPI-aware dimensions and restored framebuffer/scissor/color-mask/sRGB state. The worker follows the core's clock and avoids an additional blocking swap clock. Battery mode reduces presentation without changing emulation speed. These timing/power behaviors require actual measurement; they are not benchmark results.

Hardware context reset occurs after content load before Ready. A fresh context is created when a core is reinitialized, avoiding leaked graphics-state assumptions. Teardown disarms callbacks before fallible cleanup; callbacks cannot be retried after core deinitialization/unloading. Core DLL destructors run with the graphics context still alive. Stop acknowledgement follows owned runtime cleanup; a hung stop remains subject to the broker's existing deadline/job termination.

The common save-memory envelope, pre-Start state-job initialization, exact-size state loads, truthful load results and rollback remain connected. Save compatibility includes backend/target policy and exact artifact/options/content identity. N64 cartridge byte-order formats are admitted explicitly. GPU/native/mobile save portability, LLE/other microcode coverage, JIT profiles, other desktop platform backends, capture/streaming and device qualification remain separate required work.

N64/PSP controls now keep physical D-pad and analog-stick input separate. Keyboard I/J/K/L supplies the left stick while arrow keys remain D-pad input; native Game Hub help explains those mappings. Other digital-pad systems retain their stick-to-D-pad convenience. PSP CHD is also admitted by the media contract, matching the selected core's declared formats. No input, cartridge-format or CHD compatibility test has run.

The final consolidated pass must compile the core/worker, inspect actual imports/licenses/content, verify AppContainer/driver behavior, exercise robust-context loss, fullscreen/DPI/aspect handling, controller input, rejected-state rollback and all supported content formats, and collect real frame/audio/performance evidence. No runtime, compatibility, security or release-readiness result is claimed by source preparation.
