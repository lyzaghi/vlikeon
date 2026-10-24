# Native ordered RTP to decoder bridge — item 56

Receiver retirement now accepts an original caller deadline through
`revoke_until`, fences ordered/pending data before decoder cleanup, and passes
that deadline into the existing decoder/worker retirement path. A positive,
timely Stop/process/I/O result is required to publish Revoked. CleanupRequired
can explicitly retry reaping the same retained decoder owner; receive admission
stays closed and no Stop receipt is invented. This supersedes the earlier
always-error CleanupRequired revoke behavior. Confirmed Revoked remains an
idempotent historical result rather than new backend work. Default/error/Drop
cleanup uses a separate best-effort budget. No build/test or timing/device
qualification ran, and full session retirement/transport integration stays open.

Primary-picture header consistency now has the source primitive described in
`NATIVE_H264_PICTURE_HEADERS.md`. Complete AU/DPB/negotiation validation and the
actual admission implementation remain unfinished; this bridge does not invoke
the primitive or treat header consistency as decoder permission.

Ordinary complete slice-header syntax now has the source parser documented in
`NATIVE_H264_SLICE_HEADER.md`. Actual DPB/reference validation, access-unit
coverage, entropy/decoder and native negotiation admission remain unfinished.
This bridge still has no default-success codec admission or parser caller.

Ordinary slice identity-prefix parsing now exists as described in
`NATIVE_H264_SLICE_PREFIX.md`. It is not complete header/AU/reference validation
and is not yet called by this bridge; full native bitstream admission remains
required before decoder submission.

Retained parameter versions now exist as described in
`NATIVE_H264_PARAMETER_VERSIONS.md`. The actual access-unit/slice consumer,
parameter-change/IDR recovery, full negotiation checks and decoder bridge
integration remain outstanding; the bridge still has no default admission.

Ordinary PPS metadata parsing now exists as described in `NATIVE_H264_PPS.md`.
The actual bitstream/negotiation admission implementation, retained parameter
versions and slice/reference checks remain unfinished. No bridge caller invokes
this parser or treats parsed metadata as decoder authority.

Ordinary SPS/VUI/HRD metadata now has the bounded parser described in
`NATIVE_H264_SPS.md`. Parameter-set dependencies, full semantic/negotiation
admission and the actual receiver implementation remain outstanding. No current
bridge path calls the parser or uses it as implicit decoder authority.

Parameter-set parsing source now has the bounded RBSP reader described in
`NATIVE_H264_PARAMETER_BITS.md`. Full SPS/PPS/VUI semantics, dependency/negotiation
checks and the actual NativeH264Admission implementation remain missing. The
bridge does not invoke this primitive or gain a default-success admission path.

Source only. No Rust/C++ compiler, RTP parser, clock, worker/decoder, network,
media, display or test execution occurred. No current service/UI/SRTP endpoint
calls the new broker-private NativeVideoReceiver. Streaming remains disabled.

The bounded OrderedReceiver now accepts host-owned receipt metadata together
with its original local Instant. Each of at most 64 queued packets keeps its
own metadata; only the earliest receipt of the access unit currently assembling
is retained separately. On completion, that metadata travels with the unit.
There is no unbounded timestamp side map or requirement to clone all metadata.
Duplicate packets cannot replace receipt time/metadata. Future or >100 ms old
stamps are rejected, and reordering/assembly preserve their original age budgets.

The Windows bridge uses opaque DecoderReceipt from native ingress, passed through
authentication/decryption to push_authenticated and then through ordered assembly.
The selected receipt must exactly match the assembled frame's earliest Instant.
Native caller must obtain it at original ingress, not after crypto/queues. The
generic metadata facility is not itself cryptographic admission; only trusted
native receipt objects are appropriate for the actual receiver adapter.

A mandatory NativeH264Admission implementation checks ACTUAL negotiated owner,
SPS/PPS/profile/level/resolution/VUI and decoder dependencies before bytes queue
for the decoder. There is no default successful implementation and the current
product has not supplied one. NAL headers/IDR markers/layout checks do not replace
this missing bitstream/negotiation work. Slow admission is followed by another
original-receipt age check; it cannot mint a fresh frame or revive worker authority.

The bridge converts forward modular 90 kHz timestamps into capture-relative
100 ns decoder time. The first frame establishes zero; later deltas must be
positive, below half the modular range and within a total 30-minute timeline.
Failed/replayed jumps do not advance timeline state. This is one stream's video
clock conversion, not audio synchronization, sender-report mapping or playout.

At most four complete admitted compressed units (2 MiB each) wait for submit.
The bridge borrows each pending unit into the private decoder client, so S_FALSE
does not consume/discard caller bytes. Retrying preserves order and the same old
receipt; pending age/overflow failure retires the generation. No dependent coded
picture is silently skipped. Four units is a finite worst-case allocation budget,
not a promise that the tight age budget can sustain that backlog in production.

tick checks native decoder authority and receive gap/assembly time independently
of new packet arrival, then retries pending input. Output stays explicitly
UnpresentedVideoFrame and is rechecked for owner/age before handoff. Errors fence
the reorder/pending data and request decoder revocation; cleanup failure is not
reported as success. Already-returned bytes need final native authority/age/
colour/presentation fencing at the actual display boundary.

Four new source cases cover earliest metadata through reorder/duplicate and
three RTP clock cases (wrap, replay without state advance, jump/lifetime bound).
They are inert and unexecuted, separate from the six earlier reorder cases.
No codec/network/device qualification or test pass is implied.

Remaining: actual authenticated SRTP/ICE/TURN/native ingress endpoint and replay
source, installed/native viewer admission/observer/service caller, full actual
codec/parameter-set/VUI implementation, NACK/PLI/loss-generation recovery,
display/audio/UI and all original platform/device/fault/latency qualification.
The source connects stages but does not complete or enable end-to-end Remote Play.

Receive state now distinguishes Active, confirmed Revoked and CleanupRequired.
An error fences buffered data before Stop; only a positive combined native
Stop/process-retirement result earns Revoked. The original operation error is
returned while state retains cleanup uncertainty for the owning service. Repeated
revoke is idempotently successful only for already confirmed Revoked, never for
CleanupRequired. No current product registry persists or displays this state;
actual lifecycle/recovery integration and qualification remain unfinished.
