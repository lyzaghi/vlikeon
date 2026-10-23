# Native SDR conversion and Windows frame bridge — item 56

Source only. No build, converter execution, helper launch, encoded bitstream,
frame/device operation or test was run. Eight authored Rust cases remain
unexecuted. Streaming stays disabled; there is no new renderer/product command.

## Explicit byte/colour contract

Native `Frame` now declares colour encoding as well as pixel format. BGRA accepts
only opaque full-range sRGB; NV12 declares SDR BT.709 limited range with centered
2x2 chroma. Conversion rejects foreign capture/session/generation, changed size,
malformed stride/allocation, mismatched colour/format and nonopaque BGRA. Unknown,
HDR, wide-gamut and premultiplied-alpha surfaces cannot be admitted by guessing
this label: the native producer must supply correct metadata and perform needed
composition/colour conversion. HDR, alpha composition and scaling remain due.

`native_video_colour::tight_nv12` owns input/output. It preserves identity,
sequence and presentation time, strips admitted NV12 padding, or converts BGRA
using sRGB inverse transfer, BT.709 forward transfer, luma/colour-difference
matrix, studio-range quantization and unquantized 2x2 chroma averaging. A
once-initialized 256-value transfer table and bounded fallible allocation are
used. This is a CPU reference path, not a GPU/realtime or HDR claim. Visual
accuracy across real displays/decoder viewing conditions is unqualified.

MF input/output types now request matching primaries, transfer, matrix, limited
range and centered chroma. An encoder can reject these attributes. Acceptance
is not actual coded-VUI or receiver-colour proof: the receiver adapter must
verify final headers/decode. No arbitrary guessed NV12 is admitted as correct.

Formula/metadata references reviewed:

- [ITU-R BT.709-6](https://www.itu.int/dms_pubrec/itu-r/rec/bt/R-REC-BT.709-6-201506-I!!PDF-E.pdf), source transfer and signal/quantization tables.
- [W3C CSS Color 4](https://www.w3.org/TR/2024/CRD-css-color-4-20240213/), sRGB inverse transfer.
- [Microsoft extended colour information](https://learn.microsoft.com/en-us/windows/win32/medfound/extended-color-information), media-type colour fields.

## Broker-internal bridge

`platform/broker/src/stream_video.rs` connects the mailbox, conversion and Windows
worker client. It is an internal Windows module with no current service command,
capture producer or installed-admission caller. Attaching an admitted worker
uses its original remaining authority to bound the mailbox and configures it
without refreshing authority. Independent native lease observations must continue
while configure/submit blocks.

The admitted producer receives the generation's bounded raw mailbox. The bridge
preserves local receipt time through conversion: combined mailbox/conversion age
must stay within 100 ms. Authority is rechecked before submission; microseconds
convert safely to signed 100-nanosecond timestamps. Unaccepted raw frames may be
dropped. Keyframe intent persists through absent/stale/backpressured raw frames
until accepted. Encoded dependencies do not use this raw-frame drop policy.

Output is explicitly `UnnegotiatedVideoPacket` with its immutable owner. There is
no network publication method. Actual codec profile/level/framing/VUI admission,
packetization and final owner/authority publication fence remain due. Revoke
closes the producer mailbox before confirmed worker Stop; errors close delivery
without claiming successful native teardown.

Remaining: real owned Windows capture producer, resize/GPU conversion, native
consent/observation and installed admission, codec/receiver/transport/UI connection,
Linux and platform service completion, provenance and all build/runtime/fault/
colour/latency/energy evidence. Original item-56 scope remains open.
