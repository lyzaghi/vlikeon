# Windows native H.264 decoder candidate — item 56

Authored source only. No CMake/compiler, COM/MF activation, decoder, picture,
driver/device, parser or test was executed. The C++ worker's new v3 mode now
invokes Decoder as source, but no current Rust/service/product caller invokes v3.

`engines/encode-windows/src/decoder.cpp` uses the fixed Microsoft H.264 decoder
CLSID, not a peer-supplied component or arbitrary DLL path. The caller must own
a dedicated COM MTA thread and a separately supervised installed helper process.
The candidate has no socket, key exchange, SRTP, permission or display API.
Registered OS codec availability/trust and package provenance still require
release qualification; no acceleration/performance claim is made.

## Input and original lifetime

The admitted native receiver must first authenticate/decrypt/reorder transport
and validate actual negotiated SPS/PPS/profile/level/VUI, progressive SDR BT.709
dimensions and complete Annex B access units. Existing NAL/RTP layout checks are
not full bitstream admission. The decoder class does not manufacture that proof.
It accepts at most 2 MiB compressed input and 720p/1080p at 30/60 FPS, with finite
original generation lifetime. Microsecond/RTP timestamp unwrap/rebasing and real
native capture/session authority remain caller responsibilities.

Input timestamps increase and are bound to capture-relative 100 ns values within
30 minutes. The receiver's original GetTickCount64 receipt must survive reorder/
IPC/conversion; the candidate refuses future or >100 ms old input and never
refreshes age after a slow OS call. Cross-language Instant/tick conversion must
be implemented conservatively by the real receiver adapter, not guessed.

Low-latency mode and maximum coded dimensions are requested through the system
decoder's attribute contract. At most two accepted units remain pending. S_FALSE
means an input was not consumed: retain/retry it in order within its old age
budget or retire the generation; dependent compressed pictures cannot be skipped.
The supported low-delay path expects one output per accepted unit with exact
timestamp order. Reordered/delayed/incompatible output fails this candidate.

## Output admission and memory ownership

The synchronous MFT output loop has a finite iteration count. Stream changes
only admit the original visible profile and progressive NV12. A padded coded
height up to the next 16-line boundary needs an exact zero-offset minimum display
aperture for the visible dimensions; unknown/different crop or format is rejected.
Default stride must be positive and bounded. General resolution change, interlace,
other codecs/pixel formats and a DXGI presentation path remain future work.

Provider-owned or caller-allocated samples are owned/released exactly once,
including error/event paths. IMF2DBuffer2 read locks expose actual scanline,
pitch and accessible memory bounds; the code checks row extents before copying.
Plain media buffers use bounded contiguous locks as fallback. Unsupported layouts
fail rather than guessing. Visible luma rows and UV rows are copied separately;
UV begins after the full coded Y surface, not the cropped visible Y height.
Every successful lock is unlocked before output transfer. Returned frames own
tightly packed NV12 bytes and preserve original timestamp/receipt.

At most two completed frames are queued; overflow, original lifetime/age expiry,
bad timestamp, incompatible media type or failed copy closes delivery. The host
must poll independently while input stops and check generation/authority/age
again before presenting an already-returned frame. No display or actual colour
reproduction is established by copied bytes or requested media attributes.

close_delivery fences the queue; explicit stop flushes/ends streaming and requests
IMFShutdown when exposed. Failure keeps COM references in the live Decoder for
retained cleanup. Destructor cleanup is best effort only. Codec/COM calls can
block: only external original-process/job supervision can bound them. A local
close or process exit without a matching native receipt is not positive teardown.

Primary API contracts reviewed: [Microsoft H.264 decoder](https://learn.microsoft.com/en-us/windows/win32/medfound/h-264-video-decoder),
[low-latency mode](https://learn.microsoft.com/en-us/windows/win32/medfound/codecapi-avlowlatencymode)
and [bounded 2D read locks](https://learn.microsoft.com/en-us/windows/win32/api/mfobjects/nf-mfobjects-imf2dbuffer2-lock2dsize).
Installed SDK declarations were inspected read-only, not compiled or executed.

The next source checkpoint connects Decoder to the worker's private v3 pipe
actor. `engines/encode-windows/DECODE_PROTOCOL.md` records exact framing, original
local receipt, raw NV12 replies and Stop semantics. No protocol was run.

The next source checkpoint adds the distinct Rust v3 parent, original ingress
receipt handling and broker-private unpresented output. See
`WINDOWS_DECODER_SUPERVISION.md`; no current product/secure receiver invokes it.

Remaining: actual native decoder installed/viewer admission and observer wiring,
negotiated parameter-set/VUI admission and secure receiver connection, display
surface/UI/audio synchronization, other platform decoders and the complete
build/runtime/fault/colour/latency/energy/device qualification. Item 56 stays open.

## Actual output-type fences after static review

`vlikeon_decode_type.hpp` now checks all present minimum/geometric/pan-scan
apertures, even at exact visible height. Each crop must agree with the original
zero-offset profile; padded height needs a minimum/geometric aperture, not only
pan-scan. Enabled pan-scan, rotation, stereoscopic video, nonsquare pixels,
nonprogressive data and odd/invalid stride are rejected. Failure cannot assign
new output geometry.

Pre-input placeholders may omit colour fields, but conflicting fields fail.
Actual samples require explicit BT.709 primaries/transfer/matrix, limited range,
centered chroma and square pixels. The actual CURRENT type is checked after every
ProcessOutput, not only enumeration/SetOutputType. Missing fields are not inferred
from bytes. An MFT omitting metadata can therefore be rejected; real supported-
device compatibility and a valid bitstream-derived metadata path need execution
qualification. Stamping guessed fields is not a valid workaround.

`tests/decode_type.cpp` authors inert dictionary fixtures for exact/padded and
conflicting crop, malformed aperture, missing/conflicting colour, aspect,
pan-scan/rotation/3D, stride and failure-state preservation. Its CMake target is
EXCLUDE_FROM_ALL, not built/run by the worker recipe. The source is snapshotted
to keep the copied CMake tree complete. No fixture/compiler/dictionary API or
decoder ran; these are not actual SPS/VUI/colour or receiver/display evidence.
Metadata reasoning follows Microsoft's [picture aspect ratio](https://learn.microsoft.com/en-us/windows/win32/medfound/picture-aspect-ratio)
and [uncompressed video types](https://learn.microsoft.com/en-us/windows/win32/medfound/uncompressed-video-media-types).
