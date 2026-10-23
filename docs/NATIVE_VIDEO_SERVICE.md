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

1. Finish independent service packaging/integration and trusted owner/capture admission,
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
Windows/Linux backend completion, actual service/capture/transport integration, native
authority wiring and all remaining final scope remain due.

## Windows encoder source candidate

`engines/encode-windows` adds a separate static-library recipe and real Media
Foundation API path for H.264 Main Level 4.2 with tightly packed NV12 input. Both
synchronous output pulling and asynchronous NeedInput/HaveOutput event credits
are authored. It bounds accepted pending frames, MF buffer requests, packet and
header sizes, output queue memory and receipt age. A hardware-required request
admits only a registered hardware MFT; actual GPU execution is not established.

Checked low-latency/no-B-frame settings, strict output timestamps and requested
keyframe receipts enforce the current backend contract. Slow input/OS operations
cannot earn a fresh processing clock. Close fences output; explicit stop retains
references on failed teardown. NoWait event reads do not preempt OS methods.
The future service must supervise the process and supply live authority checks,
capture/conversion, receiver negotiation and publication fencing. This candidate
does not yet connect to the Rust selection primitives, capture or product UI.
`engines/encode-windows/README.md` records exact behavior, API references and
unsupported paths. No Windows configure/build, MFT activation or test was run.

## Windows independent encoder worker source

The Windows recipe now also defines `omniplay-video-worker.exe` with exact
`--stdio-video-v1` entry and pipe-only input/output. A separate bounded reader,
bounded writer and COM MTA actor connect real NV12 submissions to the encoder
library. The binary protocol covers immutable capture/session/generation labels,
finite configuration, raw frame submission, output polling and terminal Stop.
Labels do not establish capture/owner permission. There is no window capture,
file/network output, arbitrary backend path or renderer command surface.

The actor polls independently of arriving commands, retains the original overall
lifetime and requires a responsive private parent lease. Stop supersedes unsent
ordinary replies, fences delivery before OS teardown and returns the actual stop
HRESULT. A partial outgoing frame cannot be followed by an apparently valid Stop
receipt on the same corrupted stream. Existing OS calls/writes can still block;
parent supervision and rejection of late media remain mandatory. Bounded queues
and atomic flags do not preempt the OS or prove process retirement.

The worker is authored, not built/run/installed or called by the product. Installed
artifact admission, job/process supervision, native capture/conversion, actual
receiver codec/bitstream negotiation, host authority integration and transport
publication fences are still missing. `engines/encode-windows/PROTOCOL.md` records
the exact contract. All original item-56 completion requirements remain open.

The next source checkpoint adds a private Rust Windows parent client and
independent process/authority/pipe watchdog in `video_client.rs`, using the typed
creation-time Video job launcher, original process witnesses, bounded packet
ownership and matching Stop receipts. `WINDOWS_VIDEO_SUPERVISION.md` records
the exact lease, memory, timeout, cleanup and caller-publication requirements.
No current product command calls it. Installed admission, live native capture/
document/pair observation, codec/receiver integration and all qualification remain
unfinished. No build, helper/encoder execution or test ran.

An internal Windows broker bridge now connects admitted raw mailboxes through
explicit SDR colour conversion to worker submissions. Local raw-frame receipt
age survives conversion, keyframe intent survives raw backpressure, and output
remains explicitly unnegotiated. MF types request matching BT.709 limited-range
centered-chroma metadata. `NATIVE_VIDEO_COLOUR.md` records colour contracts,
references, eight unexecuted source cases and remaining capture/negotiation/
product work. No real capture producer, service command or media publication
path is connected by this checkpoint; no build/test was executed.

The subsequent source checkpoint adds a separate WGC/D3D11 Windows capture
class with retained original process/native item, local confirmation, bounded
raw frame ownership, SDR display admission and GPU aspect-fit BGRA readback.
`WINDOWS_OWNED_VIDEO_CAPTURE.md` records its behavior, caller obligations and
limitations. It is not connected to the encoder worker/Rust bridge/product UI;
no capture, dialog, build or test ran. Private native capture transport/admission,
application audio, actual colour/receiver/transport integration and all original
platform/service qualification requirements remain unfinished.

The C++ worker now has a separate owned-window v2 mode connecting retained
native process/window capture, opaque BGRA scaling, explicit SDR conversion and
MF encoding in one actor. Original receipt age and accepted native sequence
follow encoded output, reliable keyframe intent has a sequence fence, and Stop
requires both capture/encoder close results. `OWNED_PROTOCOL.md` records the exact
wire/caller contract. Current Rust typed launch/client still cannot invoke v2;
native admission/UI, actual receiver/transport integration and all original
platform/qualification scope remain due. No build, launch, prompt or test ran.

The next source checkpoint connects Rust's typed original-game-handle launcher
to owned-v2, validates native capture sequence/timestamps/source-age plus local
RTT, and retains reliable keyframe fences until actual keyframe output. An
internal `WindowsOwnedVideo` wrapper exposes only unnegotiated output and revoke;
raw-v1 mailbox injection stays separate. `WINDOWS_VIDEO_SUPERVISION.md` records
these contracts. Actual installed admission, native authority observation lane,
service/UI invocation and receiver/transport remain unconnected. Build/test is
still deferred; no process or media operation was executed.

The next source checkpoint adds typed exact-owner native authority observations
and one independent observer lane. It rejects lost/stale/foreign evidence and
retains the lane permit through actual source destruction. Four additional
source-only admission tests are authored, unexecuted. No current native
document/capture/pair-revocation source implements the observer interface and no
service caller starts it; real authority integration remains unfinished. This
does not waive installed admission, receiver/transport/UI or platform evidence.

The next source checkpoint adds installed inventory/hash admission for the fixed
Windows helper and retains non-delete-sharing ancestor/file guards through
worker/thread retirement. A private StreamInputs handoff matches prepared capture
ownership/original process identity, consumes one attempt and recomputes lifetime
after hash admission. No service command calls it. `WINDOWS_VIDEO_INSTALLATION.md`
records exact behavior and missing package/native observation/UI/receiver work.
No install, admission, helper launch, capture, build or test ran.

The next source checkpoint adds an explicit Windows video helper snapshot/build/
staging recipe and fixed-entry structural PE admission in the existing release
packager. Static runtime/asInvoker/mitigation settings are authored; seven inert
PE fixture cases remain unexecuted. `WINDOWS_VIDEO_PACKAGING.md` records scope,
toolchain/provenance/signature limitations and remaining requirements. No build,
staging, parser/test, package, release or helper execution ran.

The next source checkpoint adds bounded explicit H.264 NAL framing and RTP mode
0/1 packetization, including FU-A, 90 kHz timestamps, sequence wrap and final
access-unit marker handling. `NATIVE_H264_RTP.md` records limits and missing
codec/VUI/parameter-set/receiver/SRTP/network work. Eight inert source fixtures
are authored, not executed. No current broker/transport caller invokes it and
no parser, network, decoder, build or test ran. Full original scope remains open.

The next source checkpoint adds bounded RTP receive-header admission and H.264
single-NAL/FU-A/STAP-A reconstruction after the required authenticated/reordered
transport stage. It rejects incomplete/expired/replayed units and initially
requires IDR layout, without claiming full bitstream/decoder compatibility.
Seven inert source cases remain unexecuted. `NATIVE_H264_RECEIVER.md` records
limits and missing SRTP/jitter/codec/decoder/product integration. No receiver,
network, decoder, build or test executed; original requirements remain open.

The next source checkpoint connects a bounded 64-packet/50-ms RTP reorder lane
to the depacketizer, preserving earliest local receipt through its original
100-ms assembly budget. Six inert source fixtures are authored and unexecuted.
`NATIVE_RTP_REORDER.md` records duplicate/gap/source behavior and missing actual
SRTP/transport/loss-recovery/decoder/product wiring. No parser, receiver, network,
build or test execution ran. Original completion scope remains unchanged.

The next source checkpoint adds a fixed Microsoft H.264 decoder candidate to
the Windows static recipe, with synchronous MFT lifecycle, bounded pending/NV12
output, original receive-age/timestamp fences and actual 2D buffer/crop checks.
`WINDOWS_NATIVE_VIDEO_DECODER.md` records the native caller/bitstream/secure IPC/
display gaps. No current worker or product caller invokes it; no compiler,
decoder/media/device operation or test ran. Full original scope remains open.

The next source checkpoint connects the Windows decoder to private worker-v3
configuration/compressed-input/poll/Stop commands and raw NV12 replies, retaining
original local receive-clock deadlines. `DECODE_PROTOCOL.md` records the bounded
contract and required typed viewer-side admission/watchdog. Current Rust/game
video client does not invoke v3 and cannot stand in for viewer authority. Native
receiver/codec/clock admission, secure transport and presentation/UI remain due.
No build, protocol/decoder/media execution or test ran.

The next source checkpoint adds Rust decoder-v3 job launch/pipe admission,
opaque original ingress boot/Instant receipts and exact accepted-input/NV12
output matching. Viewer decode requires native receive/document/pair authority
without a local game handle. A private broker wrapper returns only unpresented
frames. `WINDOWS_DECODER_SUPERVISION.md` records required ingress/reorder/codec/
native service and display gaps. No current product path calls v3, and no
compiler, helper/decoder/media/clock operation or test ran.
