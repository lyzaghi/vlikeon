# Item 54 — macOS owned-game capture source

Status: open. `engines/capture-macos` contains an authored ScreenCaptureKit
adapter library. Its Objective-C++ source has **not** been compiled with a
macOS SDK or executed on a Mac. It is not yet connected to the Rust broker,
private component bridge or WebRTC encoder. Native macOS capture is not
advertised or enabled by this checkpoint. Item 53 Linux builds/runtime work
was deferred by the owner; it remains open independently.

The owner subsequently requested deferring macOS build/test and moving to item
55 source work. No additional macOS build, SDK installation, device connection
or capture test is being run. Retain all integration and qualification gaps.

## Authored library path

`VLKMacCapture.h` is a private, trusted-host API. It accepts an exact window ID
and PID/start-time tuple only while the broker owns an unreaped directly
launched child. These fields must never be accepted from a renderer, peer,
saved profile or enumerated process. The host must bind the caller to the
authenticated game/pair/session, original deadline and selected installed
game. The library is not a substitute for that missing broker admission.

The adapter checks `proc_pidinfo` against PID, parent PID, non-zombie state and
start seconds/microseconds. The parent must be the current host process.
Shareable-content lookup then requires the exact window ID and owning PID,
an on-screen normal window and finite positive geometry. A native confirmation
names the selected application/window. Application audio is offered through
an initially unchecked checkbox. The existing macOS Screen Recording grant
must be present; no permission-reset, elevated helper, microphone, camera or
system-mix fallback is supplied by this library.

After confirmation the adapter rechecks process lifetime and creates an
`SCContentFilter` for that desktop-independent window. Configuration has four
finite profiles: 720p/1080p at 30/60 FPS, BGRA, queue depth three; selected audio
requests 48-kHz stereo and excludes the capture host's own process. Audio
selection records the user's choice, not evidence that samples are arriving
or that application isolation has been qualified.

Complete BGRA frames must match configured dimensions and finite stride/size
bounds before entering the mailbox. Audio must be 32-bit float PCM, 48-kHz
stereo, at most 4,800 frames and 65,536 reported bytes per sample buffer.
Other formats and incomplete/idle video frames are not delivered. A consumer
polls owned `CMSampleBufferRef` values and must release them; the API accepts
no output file or network address.

The mailbox retains at most two video samples and eight audio samples. A full
queue releases its oldest sample. Close fences producers and clears both
queues under one mutex; samples already removed remain owned by the consumer.
The host/encoder must impose a separate finite budget on those consumer-owned
buffers. ScreenCaptureKit's own queue and GPU-backed allocations remain
outside the mailbox count and require actual RSS/GPU qualification.

A main-queue watchdog rechecks process lifetime and the original monotonic
deadline every 100 ms, with a 45-second startup/selection bound. The lifetime
cannot exceed 30 minutes. Stop closes delivery before the asynchronous OS
stop call and dismisses this adapter's own modal consent when applicable.
Late start callbacks request stop rather than reopening delivery. An OS stop
error remains an error; a stalled stop is not reported as confirmed teardown.
The source-ended delegate closes delivery and retires the stream. Host code
must call stop on document, peer, broker-owner and authorization loss.

Apple documents window filters, application-level audio filtering, stream
configuration and sample delivery in
[Meet ScreenCaptureKit](https://developer.apple.com/videos/play/wwdc2022/10156/)
and its [capture sample](https://developer.apple.com/documentation/screencapturekit/capturing-screen-content-in-macos).
That documentation motivates the API choices; it does not prove this adapter
compiles or isolates actual game audio. Process/window inspection and capture
are separate operations; window-ID reuse, process exit between checks and
OS permission/source changes remain explicit runtime qualification cases.

## Existing-SDK build recipe

`tools/build_macos_capture.py` requires a real Mac, an existing Xcode SDK and
an explicit CMake executable. It builds a static adapter library in a fresh
architecture-specific output directory with two build jobs, hashes source and
artifact bytes, and writes a receipt. It does not install software, sign,
capture, execute tests or integrate/admit the library into a release.

```sh
python3 tools/build_macos_capture.py --cmake /absolute/path/to/cmake --architecture arm64
```

The alternative declared architecture is `x86_64`; deployment target is macOS
13.0. Both require real SDK build evidence before being offered in the product.

## Executed checks and remaining work

Antigravity batch `054a` compiled the **actual portable mailbox header** with
MSVC C++20, warnings as errors and optimized `NDEBUG` settings, then executed
ownership/bounds/eviction tests. It verified that a consumer-owned sample
survives close, queued/late samples are released, stopped queues cannot reopen,
and 100 producer/close races leave no retained samples. The same batch parsed
the build recipe and verified actual Windows refusal with exit 2. Frozen
sources stayed unchanged. Evidence: `qa/evidence/sequential/item-054/`.

Those checks do not compile `capture.mm`, CoreFoundation ownership bridging or
Apple availability declarations, and do not exercise ScreenCaptureKit, native
consent, sample formats, TCC or actual media. Item 54 still requires:

1. Native SDK compile/link on Mac and repair of any API/availability issues.
2. Broker-owned macOS child identity access and one capture capability bound to
   game/pair/session/document/lifetime, with native consent and current grant
   verification. No renderer-supplied PID/window authority.
3. Admission of the signed host/library through the exact installed package and
   extension-private media transport to the real WebRTC capture/encode path.
   The current Windows-only broker/component schemas must be versioned, not
   populated with invented FILETIME/HWND values for macOS.
4. Source-window liveness/reuse checks, cancellation/late callbacks, suspended
   host and main-queue/modal behavior, expiry and OS stop-ack failure handling
   on actual supported Mac versions/architectures.
5. Two applications with distinguishable pixels/audio proving exact selected
   window and application-only audio; verify no background-app/system/mic leak,
   resize/DPI/display movement, minimized/closed windows, audio/device changes,
   process exit and consumer backpressure. Do not infer isolation from a filter
   configuration or the local checkbox.
6. Parent/document/peer loss, source/permission revocation and crash cleanup,
   package signing/permission UX, latency and actual memory budgets. Antigravity
   executes frozen tests; Codex diagnoses/fixes and accepts the item.

## Antigravity App Batch 1 — Source Implementation Checkpoint

Source work implemented:
1. `platform/stream-input/src/game_process_macos.rs`: Child process lifetime inspection via `proc_pidinfo` (`PROC_PIDTBSDINFO`), verifying PID, PPID matching host, non-zombie state, and start time tv_sec/tv_usec. `GameWindow` mapping bound to the launched process.
2. `platform/stream-input/src/capture_macos.rs`: ScreenCaptureKit adapter caller binding to `VLKMacCapture` and `VLKMacEncoder`, enforcing window isolation, bounded mailbox (max 2 video, max 8 audio samples), 100ms watchdog with 45s startup and 30m max session bounds, and clean teardown.
3. `platform/core/src/installed.rs` & `platform/broker/src/stream_input.rs`: Enabled macOS target child admission, window listing, and capture selection.
Status: `SOURCE_READY_UNVERIFIED`. Real macOS Xcode SDK build/device qualification remains in acceptance queue.
