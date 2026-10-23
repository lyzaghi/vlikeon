# Dedicated Worker WebGL2 source

Current item-6 source also preserves declared display aspect independently of
the raw guest framebuffer. The Worker sets a bounded destination; the C frontend
blits into that destination without replacing it with raw dimensions. The new
`_om_web_hw_display_protocol() == 1` export is mandatory. Existing hardware core
artifacts must be rebuilt; missing/mismatched exports refuse admission. No such
rebuild or device test was executed. See `GRAPHICS_AUDIO_INPUT_ITEM6.md`.

The shared Web player now has an explicit hardware-video path. A core manifest
must declare `hardware_video_protocol: 1`, an unthreaded WASM module, result
protocol 2 and a finite `hardware_video` target. Bundle admission and renderer
validation accept only WebGL2, at most 1920 by 1080 pixels and at most that
area. A software core keeps its existing Canvas2D/RGBA path. Neither path can
silently consume the other's frame type. The initial N64 candidate declares
640 by 480. There is no installed or approved GPU core binary.

## Ownership and negotiation

The UI transfers its canvas to the emulator Worker before module creation.
The module receives that actual `OffscreenCanvas`; the Worker does not create
a Canvas2D context on it. An optional small OffscreenCanvas capability probe
in the UI informs runtime ranking when hardware candidates exist. The probe
releases its temporary context and does not prove that context creation inside
the Worker will succeed. Actual negotiation remains mandatory.

The C frontend uses the SDK's actual libretro callback type and exposes its
size for bounded pointer admission. It accepts GLES 3.0 semantics, bottom-left
origin, a reset callback and declared depth/stencil constraints. It rejects
other APIs/versions, debug contexts and repeated registrations. The frontend
creates a WebGL2 context on the transferred canvas with no main-thread proxy,
no antialiasing and a default or battery-mode power hint. These are requests;
they do not prove GPU selection, latency, power use or performance.

The Emscripten target mapping explicitly binds the module's canvas to one
special event target, avoiding a nonexistent Worker DOM. The mapping and GL
function lookup depend on the selected SDK and must be qualified against that
exact SDK. The function lookup returns WASM-callable entries for the SDK's
WebGL API. No native driver address is passed into the Worker.

The SDK documents canvas-target mappings, WebGL2 context creation, context
ownership and explicit presentation. Its current header also exposes the
WebGL function lookup bridge. These primary inputs informed the authored
adapter; a release SDK has not yet been compiled or qualified.
[Emscripten HTML5 API](https://emscripten.org/docs/api_reference/html5.h.html),
[WebGL header](https://github.com/emscripten-core/emscripten/blob/main/system/include/emscripten/html5_webgl.h).

## Private target and presentation

The core receives one private framebuffer, with RGBA8 color storage and optional
depth or packed depth/stencil storage. Allocation is checked against the
declared dimensions, actual context limits and framebuffer completeness. The
target is cleared before the core receives a reset callback. The SDK's explicit
offscreen presentation support has a separate backing surface. These finite
frontend targets do not bound the core's own textures, shaders, caches, driver
allocations, browser memory or total GPU consumption.

A hardware frame sentinel is admitted only for a declared, active hardware
session. Its dimensions must fit the target. Presentation blits the private
target to the canvas and explicitly commits the frame. Read/draw framebuffer
bindings and scissor enablement are restored. Battery presentation skips draws
without changing emulation frequency. Actual origin, resize, aspect, state
preservation, display cadence and driver behavior remain unqualified.

No automatic context restoration is attempted. A loss event, failed liveness
query, invalid frame or incomplete callback retires the session. The renderer
terminates the Worker through the existing failure path. Liveness is observed
before emulation and on both sides of save/load operations. No automatic
publication is authorized by a failed save request.

## Completion and teardown

Result protocol 2 extends the existing completion bridge with void run,
unload, deinitialize, initialize and reset operations. Each operation writes
its result and then an explicit completion marker after the C call returns.
Missing completion, reentry and malformed results poison further guest calls
and retain affected allocations until Worker retirement. Protocol 1 retains
its existing load/state/file operations and direct void lifecycle behavior;
protocol 0 retains its prior direct ABI. GPU admission specifically requires
protocol 2. No Promise/event-loop suspension is admitted by this bridge.

The hardware frontend separately uses a completion slot for reset, presentation,
destroy and context release. Reset is armed once after accepted content.
Destroy is disarmed before invoking the core callback. The context remains
alive while content unload and core deinitialization finish; its private
resources and SDK context are released afterward. Fresh guest initialization
creates a new context. Failed callbacks are never retried. A poisoned session
removes JavaScript listeners without calling more guest exports, and final
Worker termination owns the remaining browser resources.

An SDK context-release error remains sticky for that session and rejects the
cleanup completion. The frontend does not turn a failed SDK release into a
successful stop acknowledgement or retry a fallible core callback.

The shared frontend also exposes an optional typed C variadic no-op logging
callback. This avoids pretending that an ordinary JavaScript callback can
receive a core's C varargs, or leaving a required upstream log pointer null.
No ROM-derived diagnostic text is forwarded to the product UI.

## Remaining evidence

No WebGL/SDK/compiler/linker, callback, context loss/restoration, driver, Worker,
browser, game, save, device or test has run. The original platform scope still
requires real Chromium Android, desktop Chromium and iOS WebKit evidence,
actual GPU allocation/fault/termination behavior, accessibility and display
checks, timing/energy/memory measurements and store/security/release approval.
The N64 source producer is described in `WEB_N64.md`.
