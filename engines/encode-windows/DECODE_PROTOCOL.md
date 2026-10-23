# Private Windows decoder worker v3 — authored, unexecuted

The existing worker binary now has exact entry `--stdio-decode-v3`, with private
pipe stdin/stdout only. No game HANDLE, capture/window token, file, network URL,
arbitrary codec CLSID or renderer decoder API is accepted. The C++ actor invokes
the fixed Microsoft H.264 decoder. Current Rust launcher/client and service/UI
do not invoke v3. No build, worker/decoder launch, frame or test ran.

Use the same 20-byte little-endian VLKV framing as [v1](PROTOCOL.md), with version
3. Request IDs start at one and increase exactly by one, bounded at 10,000,000.
Reader/writer command queues remain at most two entries. Unknown/malformed/short/
cross-version/oversized input is terminal. Kind 5 (encoder keyframe intent) is
forbidden. No stdout log text is permitted.

## Configure — kind 1

The first request contains the existing 61-byte binding/profile dictionary:
capture UUID (16), session UUID (16), generation (8), width (4), height (4), FPS
(4), reference bitrate (4), remaining ms (4), hardware-required byte (1).
Bindings must be nonzero, generation 1–1,000,000, profile 720p/1080p at 30/60,
reference bitrate 500,000–30,000,000 and lifetime at most 30 minutes. The bitrate
field is a shared profile label, not decoder rate control. Hardware-required
must be zero: this candidate does not implement hardware-required decoder
admission or claim measured acceleration. Successful status `0x8000` contains
S_OK (4), exact 61-byte echo and a zero classification byte (66 total).

This dictionary does not prove authenticated peer identity, actual negotiated
codec/SPS/PPS/VUI or native receiver/document authority. The parent must admit
those and maintain independent authority before sending any compressed input.
Configure cannot enable trust by choosing its own UUIDs or receipt timestamps.
The original lifetime includes command transfer and decoder setup.

## Compressed frame — kind 2

Payload has nonnegative presentation timestamp in 100 ns (8 bytes), original
local authenticated receive GetTickCount64 milliseconds (8), then one complete
Annex B access unit (1–2 MiB). Total payload is 17–2,097,168 bytes. The receipt
clock is the SAME local Windows boot clock in the trusted parent/helper, never
a remote wall clock/timestamp. It must include reorder and IPC delay. A parent
adapter using Rust Instant must conservatively bridge those clocks, not restart
the budget at the pipe write. Clock conversion is not implemented by this mode.

Future or >100 ms old receipt is rejected. Presentation time is strictly
increasing and limited to capture-relative 30 minutes. A status reply `0x8000`
contains HRESULT: S_OK means accepted, S_FALSE means not consumed. Keep/retry
unconsumed dependent compressed input within its original age or retire the
generation; never drop arbitrary pictures and continue. Failed HRESULT/EOF is
terminal. Full bitstream/receiver/permission admission is not delegated to the
bounded header parser or fixed decoder CLSID.

## Poll — kind 3

Empty payload; returns S_FALSE status if no decoded frame. Successful decoded
reply kind `0x8002` has the following payload:

| Offset | Size | Value |
| --- | --- | --- |
| 0 | 8 | Nonnegative presentation time, 100 ns |
| 8 | 4 | Visible width |
| 12 | 4 | Visible height |
| 16 | 4 | Tight NV12 row stride, equal to width |
| 20 | 8 | Original receiver GetTickCount64 receipt |
| 28 | 4 | NV12 bytes, exactly width*height*3/2 |
| 32 | variable | Owned tightly packed NV12 Y/UV bytes |

Profile/format/timestamp/input correspondence is checked by the Decoder before
the reply. The future parent must independently validate exact version/request
ID/profile/length, expected accepted timestamp and original receipt age, plus
current peer/document/media-generation authority. Raw reply maximum is
3,110,432 bytes. It is not encoded/RTP output or a browser image payload.

Output is queued only within its original receipt+100 ms deadline. A blocked
WriteFile cannot be revoked by a flag: parent timeout/cancellation and late-byte
fencing remain mandatory. Decoded data is not routed to a renderer by this mode.
Actual colour/stride/crop/device compatibility still requires qualification.

## Stop/lifetime and missing integration

Stop (kind 4, empty) supersedes unsent ordinary replies and fences decoder
delivery before native stop. The actual stop HRESULT returns in `0x8000`;
failed stop, EOF, kill or exit code is not a positive native teardown receipt.
The parent requires the full matching S_OK plus original process retirement.
Partial old output cannot be followed by a misleading Stop receipt on a corrupt
stream. The actor polls MF every 8 ms outside OS calls and requires private
parent activity within 500 ms after setup. Transport liveness is not permission.

The decoder/pipe/OS can block, so the typed decoder launcher must retain
an admitted immutable artifact, restricted handle list, creation-time job,
resource bounds, original helper witness and independent authority/call watchdog.
The Rust parent now has a distinct decoder-v3 mode with no local game witness
and a mandatory native viewer authority lease. `WINDOWS_DECODER_SUPERVISION.md`
documents the client/receipt/output source. Actual receiver clock/codec/native
admission, SRTP/transport, presentation/UI and all platform/fault/performance
qualification remain unfinished. No build, protocol execution or test occurred.
