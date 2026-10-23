# Independent native capture/encode service — item 56

Status: initial source primitives, not an implemented or qualified end-to-end
service. Build and test remain deferred by the owner. Streaming remains disabled.

`platform/streaming/src/native_video.rs` supplies private Rust primitives for
native adapters. It does not deserialize renderer or remote commands. Its codec
selection accepts bounded backend capability records and receiver codec records,
requiring matching codec/profile/level/packetization, actual profile capacity and
an explicit realtime capability. Auto preserves backend order; a preference can
allow negotiated fallback, while hardware-required selection refuses a software
candidate. These records do not discover an encoder or establish licensing.

The raw-frame mailbox owns at most two admitted BGRA8 or NV12 frames for one
capture/session/generation. It checks the finite 720p/1080p 30/60 FPS profile,
stride, exact allocation length, monotonic sequence and timestamps. Backpressure
evicts raw frames; the consumer takes the newest frame and discards frames older
than 100 ms by local receipt time. It closes permanently on expiry or explicit
stop. No encoded inter-frame stream may use this raw-frame eviction policy.

Native adapters must bound allocations before creating these owned buffers and
convert OS timestamps into a strictly increasing capture-relative microsecond
timeline. A resolution change requires explicit reconfiguration/new generation;
this mailbox does not silently reinterpret the bytes or change encoder settings.
An already-dequeued frame remains owned by its consumer: the service must check
current authority before encoding and before publishing output. Mailbox close
does not preempt an OS callback or prove native encoder/capture shutdown.

Remaining implementation and qualification:

1. Add the independent native service executable, trusted owner/capture admission,
   private bounded transport and supervision. Bind authority to original process
   handles and the document/pair/capture lifetime; retain failed teardown.
2. Connect Windows capture surfaces/application audio and real Media Foundation
   encoder discovery/configuration. Connect the macOS capture source to
   VideoToolbox and implement the approved Linux capture/encoder path.
3. Implement backend-specific negotiated codec/profile/level translation, format
   conversion, realtime rate control, keyframe requests, timestamp/discontinuity
   handling, bounded encoded output and explicit encoder drain/stop receipts.
4. Connect the admitted native media path to transport, receiver and product UI.
   Keep browser WebRTC capture as its own accurately described path. Do not
   advertise hardware encoding from capability records alone.
5. Complete codec distribution/provenance review and platform/device performance,
   overload, delayed callback, expiry, revoke, crash and teardown qualification
   when build/test execution resumes. Antigravity executes; Codex reviews/fixes.

No OS encoder, frame producer, service process, network path or device was run
for this checkpoint. Item 56 remains unchecked, as does unfinished item 55.
