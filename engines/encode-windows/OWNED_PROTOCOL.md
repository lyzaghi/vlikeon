# Owned-window capture + encoder worker v2 — authored, unexecuted

This is a separate private mode of `omniplay-video-worker.exe`:
`--stdio-owned-video-v2 <inherited-game-handle-decimal>`. The existing raw NV12
v1 entry/protocol remains separate. No new renderer command, public socket,
arbitrary file/CLSID or network destination is accepted. Nothing was built,
launched, captured, encoded or tested for this checkpoint.

The decimal argument must name an inherited native process handle with query/
synchronize access. It is NOT a PID. The trusted parent must duplicate the
original retained game handle with those exact rights, put only that handle
plus private stdin/stdout/NUL on a creation-time handle allowlist, and retain
its own original game/helper witnesses and installed file guard. This typed
owned-v2 launch path and client are not yet implemented in the Rust parent.
Never grant broader handle inheritance or accept renderer/peer handle numbers
to make this mode work. Parent supervision/native lease/admission remain due.

## Binary framing and configuration

The 20-byte little-endian `VLKV` header is the same shape as
[raw v1](PROTOCOL.md), but its version field is 2. IDs start at 1, increase by one
and are bounded at 10,000,000. Reader/writer queues and payload limits remain
finite. Cross-version, unknown, malformed, short or reordered frames terminate.
Raw Frame (kind 2) is forbidden in owned v2.

Configure (kind 1, first request only) contains the exact 61-byte v1 private
capture/session/generation/profile/lifetime/hardware dictionary, then an 8-byte
nonzero HWND (69 bytes total). The HWND must belong to the original inherited
process and survive native capture-item validation. It is not permission by
itself. The worker presents the separate default-No native window confirmation;
the normal OS capture indicator remains enabled. No microphone/system audio is
captured. Pair/document/native-viewer admission remains a parent obligation.

The native capture Source retains a host-alive callback checking original pipe
EOF/Stop/failure/deadline. It is consulted before/after the local prompt, setup,
StartCapture and frame production, so known cancellation cannot authorize a new
capture start. Native calls/prompt can still block; only an external process
watchdog can bound that wait. The future parent must maintain fresh native
authority while Configure waits and allow a finite user-consent setup window.

After local Yes, WGC capture and MF encode are connected inside the same MTA
actor. Encoder remaining lifetime is recalculated after consent; the original
header-receipt deadline never extends. Successful status reply `0x8000` contains
S_OK (4 bytes), exact 69-byte configuration echo and a registered-hardware MFT
classification byte: 74 bytes total. This is not measured GPU or codec/VUI proof.

## Autonomous native media path

The actor services WGC raw frames and MF events between pipe commands. Capture
owns two native frames, GPU-scales with aspect preserved/black letterboxing,
and returns opaque BGRA with original native callback receipt time. The C++
`to_nv12` reference path applies the same explicit sRGB-to-limited BT.709/centered
chroma contract as the Rust converter. Native colour/ICC/HDR/driver accuracy
and cross-language conversion vectors still require execution qualification.

Raw capture/convert age must stay within 100 ms before submit. Stop, parent
lease or original deadline is rechecked after blocking work. Unaccepted or stale
raw frames can be dropped; keyframe intent stays pending until accepted. Every
accepted native frame receives a sequence and retained timestamp/receipt/keyframe
record. At most six records exist (two MFT pending plus four output packets).
Encoded output must match the oldest retained record. No encoded dependency is
discarded to reduce backlog. The process still requires parent keepalive after
setup; that transport liveness is not independent pair/document authorization.

## Poll, keyframes and Stop

Poll (kind 3, empty) returns ordinary S_FALSE status when empty. An output reply
`0x8001` has this v2 payload before encoded/header byte arrays:

| Offset | Size | Value |
| --- | --- | --- |
| 0 | 8 | Nonnegative signed presentation timestamp, 100 ns |
| 8 | 1 | Keyframe flag 0/1 |
| 9 | 4 | Encoded byte length, at most 2 MiB |
| 13 | 4 | Sequence-header length, at most 64 KiB |
| 17 | 8 | Accepted native capture sequence, starting at 1 |
| 25 | 4 | Original callback-to-reply construction age, ms, below 250 |
| 29 | variable | Encoded bytes followed by sequence-header bytes |

The native sequence is independent of pipe request IDs. Timestamp, keyframe
and sequence must be checked by the future parent, which must also conservatively
add its full local Poll round-trip time to the reported age before accepting
media. Native pending/output queues and writer retain the original 250 ms source
deadline; the writer also limits its own queue age to 100 ms. A blocked WriteFile
cannot be recalled by a flag: reject late bytes and supervise the actual process.

Keyframe (kind 5, empty) sets reliable pending intent. Its `0x8000` reply contains
S_OK (4 bytes), then the next accepted native sequence (8 bytes) as a fence.
The reply acknowledges intent only. Preserve old dependent packets and require
a real keyframe at/after the fence before considering the decoder refreshed.
Already accepted pre-fence frames are not relabeled as fresh keyframes.

Stop (kind 4) supersedes unsent ordinary output. The actor fences both capture
and encoder, asks both to stop, and replies with the actual combined HRESULT.
Any failed close remains failure. The original matching receipt AND independently
confirmed process retirement remain required; EOF/kill/exit code are not a native
close acknowledgement. Stop during a blocked native prompt/driver call may need
parent termination and remain unconfirmed. No unbounded I/O-thread join is added.

## Remaining product work

The C++ owned-v2 path connects actual capture, conversion and encoder source.
Current Rust launch/client/bridge still use raw v1 and do not invoke it. Finish
typed least-privilege handle launch, v2 request/receipt/age/sequence/keyframe
validation, native admission/consent/Stop UI, installed packaging, actual codec/
VUI/receiver negotiation and publication fencing/transport. Application audio,
other platform services and all original item-56 scope remain open. No build,
dialog, capture/driver, colour-vector, media or fault test was executed.
