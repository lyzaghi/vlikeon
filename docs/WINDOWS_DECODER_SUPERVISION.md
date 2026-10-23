# Windows decoder v3 parent source — item 56

Source only. No compiler, helper/decoder/media process, pipe, clock, device or
test was executed. No current service/UI/receiver transport calls this candidate.

`spawn_decoder_guarded` selects fixed `--stdio-decode-v3` in a creation-time
one-process/512-MiB kill-on-close job with restricted stdio. Installed artifact/
hash/ancestor guards remain mandatory and survive I/O retirement/tombstones.
Viewer decode has no local game witness and inherits no game handle. Encode
constructors still require their original native game witness; only decode
can omit it. Exact-owner native receive/document/pair lease remains mandatory.
For viewers, capture_authorized means admitted RECEIVE-generation authority.
No actual native service observer/caller is wired by this checkpoint.

V3 framing/config/response kind/profile/hardware-zero classification is checked.
Dedicated compressed-submit/decoded-take APIs reject encoder-mode mixing. The
process-local singleton currently serializes video helpers; broader concurrent
role/session arbitration is still due.

`DecoderReceipt::at_native_ingress` captures local Windows boot ticks BEFORE
Instant and must be carried from actual original ingress through authentication,
reorder and assembly. Do not construct it after those queues. Private fields
cannot be supplied from renderer/network timestamps. Checks use both original
clocks and preserve them on retries. An interrupted clock capture cannot turn
an old tick into fresh media. Actual peer/codec/SPS/PPS/VUI admission is separate.

The existing RTP ordering stage retains Instant but is not yet connected to
opaque decoder receipts. The real receiver adapter must retain/select the
earliest corresponding packet receipt for every unit. RTP unwrap/rebasing also
remains due; accepting a receipt object does not implement that bridge.

The I/O lane matches decoded timestamp and exact echoed boot receipt to the
oldest accepted compressed input, then checks fixed visible dimensions, tight
stride and exact NV12 length. Reply size is bounded before allocation. At most
two decoded frames queue; compressed records are separately bounded. Boot/local
age stays within 100 ms across queue/decode/pipe/copy/take. The independent
watchdog checks decoded queue age, authority, call deadline and actual process/
thread retirement. Output never refreshes original frame/generation lifetime.

Late/malformed/foreign output, overflow or expired authority closes transport.
Revoke clears both media queues; pending Poll can be drained without publishing.
Stop success requires a matching native S_OK plus original process retirement.
Cancellation is a request and never joins unfinished threads; retained guards/
permit prevent accumulating blocked replacements. OS stalls remain unqualified.

The broker-private WindowsVideoDecoder connects configure/submit/take/revoke.
Output is explicitly UnpresentedVideoFrame with its immutable owner. There is
no display/renderer method. Already-returned bytes still need current exact
owner/native authority, colour/decoder validity and final age before presentation.
S_FALSE compressed input must be retried in order within its OLD age or retire
the generation; it cannot be discarded like a raw capture frame.

Remaining: real installed viewer admission/caller, native receiver authority,
authenticated ingress/reorder/codec bridge, SRTP/ICE/TURN/RTCP/recovery, display/
audio/UI and all original platform/device/fault/colour/latency qualification.
No build, runtime pass or item completion is claimed.

The subsequent source checkpoint adds metadata-preserving ordered assembly and
a broker-private ingress/reorder/admission/decoder bridge. See
`NATIVE_VIDEO_RECEIVE_BRIDGE.md`. It still has no actual authenticated ingress
endpoint, full codec admission implementation or product caller.
