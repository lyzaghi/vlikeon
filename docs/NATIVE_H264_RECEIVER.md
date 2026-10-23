# Native H.264 RTP receive stage — item 56

Source authored only. No parser/receiver, decoder, media, network, build or test
execution occurred. Seven inert source fixtures remain unexecuted. Current
service/native/browser transport does not call this module. Streaming stays off.

`native_h264_receive::Depacketizer` is an internal stage AFTER authenticated SRTP
decryption, anti-replay and a bounded reorder/retransmission lane. It is not a
public raw-datagram endpoint and does not supply those missing transport layers.
The caller must bind actual negotiated mode/payload type/SSRC and finite native
authority; construction cannot create pairing/consent or trust an arbitrary peer.

Each packet is at most 1400 bytes. RTP version, bound SSRC/payload type, sequence,
header length, CSRC, extension length and padding are checked before payload use.
Extensions are bounded/skipped structurally, not interpreted as trusted media
identity. In-order sequence wraps normally. Within an access unit all packets
share one timestamp; across completed units, forward modular timestamps are
required. The caller must resolve network reorder/retransmission before this
stage. A remaining gap/replay/malformed packet terminates this receiver generation;
there is no silent partial-frame recovery or unimplemented jitter guarantee.

Mode 0 accepts single NAL input. Mode 1 additionally reconstructs FU-A and STAP-A,
checking FU start/end/reserved/header continuity and aggregate lengths/NRI. Nested
aggregation/fragments and unsupported scalable/partitioned NALs are rejected.
Only a marker at a complete access-unit boundary yields owned Annex B bytes.
Initial output must contain IDR; that header condition is not proof that SPS/PPS,
slices, resolution, profile/level/VUI or decoder dependencies are actually valid.
The existing NAL layout admission is reused, not a full H.264 bitstream verifier.

One unit has at most 2 MiB reconstructed bytes, 256 NALs, 8192 packets and a
100 ms local assembly budget. Buffer growth is bounded/geometric and allocation
failure closes delivery. Original finite generation lifetime cannot be renewed
by packets. The host must call tick independently while packets stop arriving;
an incomplete frame cannot remain logically buffered indefinitely. close fences
future input and clears accumulated unit state. Already returned bytes remain
caller-owned and still need decoder/owner/age/publication checks.

These primitives do not create a video decoder or UI, negotiate codecs, cache/
validate parameter sets, request/recover keyframes, pace playback or synchronize
audio. Do not feed the returned bytes to a decoder without actual receiver codec
and native authority admission. Real loss/reconnect behavior, SRTP/ICE/TURN,
RTCP/NACK/PLI, jitter buffering, decoder integration and platform qualification
remain original unfinished requirements.

Seven source cases cover inert sender-FU-A round trip/sequence wrap, packet loss,
premature marker/timestamp change, source/payload binding, CSRC/extension/padding,
STAP-A/mode restrictions and initial-IDR/incomplete-frame age. No valid decodable
picture, codec/device/network compatibility or pass is asserted.

API reasoning follows [RFC 3550 section 5](https://www.rfc-editor.org/rfc/rfc3550.html#section-5)
and [RFC 6184 sections 5–7](https://www.rfc-editor.org/rfc/rfc6184.html#section-5).
All item-56 service/platform/receiver/transport/final qualification remains open.

The next source checkpoint adds the separate finite `native_rtp_reorder` stage
and preserves original packet receipt through ordered assembly. Returned units
now retain their earliest receipt time for downstream age checks. See
`NATIVE_RTP_REORDER.md`; no current network/decoder/product caller exists.
