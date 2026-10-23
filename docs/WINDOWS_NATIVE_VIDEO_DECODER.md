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
