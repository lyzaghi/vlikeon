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

## macOS encoder source candidate

`engines/capture-macos/src/encoder.mm` now supplies a real VideoToolbox API path,
included by the existing static-library CMake recipe. `VLKMacEncoder` accepts
complete fixed-profile BGRA samples compatible with `VLKMacCapture` and submits
them with strictly increasing presentation timestamps. The host must call it
from one dedicated helper queue; it is not automatically attached to capture.

H.264 Main/AutoLevel or HEVC Main/AutoLevel is requested, with checked realtime,
bitrate/FPS, GOP and no-reordering settings. The candidate fails if a required
property is unsupported. Hardware-required creation uses Apple's explicit
requirement flag and queries the resulting session; hardware availability and
actual performance remain unknown until run. The actual encoded sample format
description must be admitted by future receiver negotiation before publication;
an AutoLevel request does not prove a particular remote profile/level match.

At most two submissions are outstanding. Shared callback state and one-shot
completion tickets account for callbacks that may execute before submission
returns, asynchronously or on an error path. Callbacks retain no raw encoder
object pointer. Output accepts complete single-sample matching-dimension codec
buffers, requires a sync sample for first/explicit-keyframe submissions, and
retains at most four buffers/4 MiB with a 2 MiB per-buffer limit. A dropped/error,
expired, reordered, over-age or over-budget output closes the whole delivery
generation. Encoded dependencies are not silently evicted.

`copyNextEncodedSample` transfers +1 Core Media sample ownership in output order,
including its actual format description. It is not Annex-B or RTP packetization.
The consumer must release it and independently fence late publication after
authority loss. `deliveryClosed` distinguishes terminal failure from an empty
queue. `closeDelivery` immediately clears/fences callbacks; `stop` separately
invalidates/releases the VideoToolbox session. OS calls can block and require the
future external process supervisor. This adapter does not assert that closing a
mailbox proves capture/encoder shutdown, and does not encode audio.

API contracts were reviewed against Apple's
[encode callback documentation](https://developer.apple.com/documentation/videotoolbox/vtcompressionsessionencodeframe(_:imagebuffer:presentationtimestamp:duration:frameproperties:infoflagsout:outputhandler:)),
[hardware-required selection](https://developer.apple.com/documentation/videotoolbox/kvtvideoencoderspecification_requirehardwareacceleratedvideoencoder),
and [session invalidation](https://developer.apple.com/documentation/videotoolbox/vtcompressionsessioninvalidate(_:)).
No macOS SDK compilation, encoder/device execution or callback/timing test ran.
Windows/Linux backends, actual service/capture/transport integration, native
authority wiring and all remaining final scope remain due.
