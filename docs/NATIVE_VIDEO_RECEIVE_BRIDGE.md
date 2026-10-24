# Native ordered RTP to decoder bridge — item 56

Native datagram receive now requires atomic byte/ancillary-receipt retrieval,
replacing the unusable post-recv timestamp lookup contract. DecoderReceipt adds
software-QPC ingress conversion: local/boot clocks are sampled before current
QPC, elapsed queue age is rounded conservatively and subtracted from both clock
domains. Future/stale/invalid counters fail. The native caller still must prove
software timestamp provenance; hardware timestamp units cannot use this method.
Actual Winsock recvmsg timestamp configuration/ancillary parsing remains open.
See Microsoft Winsock timestamping documentation. No build/test/socket operation ran.

Broker now retains a native TransportSlot alongside its receive registry. Native
attachment requires a matching active video owner; idle/request maintenance polls
the connected UDP adapter and ticks the receiver. Poll/lifetime failures seal
the slot and jointly retire transport/video; failed cleanup retains the slot
instead of replacing it. While occupied, it owns video cleanup ordering so a
separate registry pass cannot erase its retirement target. Shutdown closes the
document before coordinated cleanup. Actual SRTP/ICE and OS ingress timestamp
providers still need implementations and the admitted negotiation caller must
attach them. No network/helper/build/test operation ran.

UdpVideoIngress now owns an already connected ICE-selected UDP socket and polls
up to 32 datagrams without blocking, forwarding through VideoIngress to the
registry. A full UDP-sized buffer detects oversize instead of accepting truncated
prefixes; encrypted packets have no heap backlog. Error/retirement closes the
socket and preserves transport/video cleanup. A required native OS receipt
adapter must correlate actual ingress time including kernel queue residence;
there is no fresh post-dequeue timestamp fallback. The Windows ancillary-time
adapter, actual ICE/DTLS/SRTP implementation and product scheduler/caller remain
missing. No socket/network/build/test operation ran.

VideoIngress now connects a required native SRTP receiver backend directly to
the broker registry with original ingress receipts, exact media-owner checks and
bounded datagram/decrypted RTP sizes. Authentication/decryption/replay rejection
must occur in that backend before RTP is delivered; there is no plaintext
fallback or implementation claiming cryptography from a boolean. Retirement
fences ingress and attempts both transport and video cleanup under one deadline,
retaining separate confirmed states for retries. The actual ICE/DTLS/SRTP engine,
socket/event caller and joint session owner are still missing. This is adapter
source wiring, not an operational network transport. No build/test ran.

The broker control loop now receives framed input from one bounded reader lane
and runs receive maintenance on 25-ms idle timeouts. Partial stdin headers/bodies
and packet silence no longer block the main loop's maintenance opportunity.
Database/session owners remain on the broker thread; framed request order and
backpressure are retained. Queued complete messages use zeroizing storage.
EOF/error terminates through broker cleanup. The reader's blocking OS read is
not falsely joined or claimed cancellable at shutdown; process exit ends it.
This is cooperative idle maintenance, not preemption of file dialogs, slow
broker work, output writes or native calls. Transport caller and actual device
qualification remain open. Build/test remain deferred.

Broker now creates a native receive-document lifetime for its channel and closes
it before shutdown cleanup. A private native open method binds sessions to that
owner. The message loop invokes registry maintenance at each admitted request
boundary, including the asset-range branch. Maintenance ticks active receivers
without an RTP push and seals/reaps failed or abandoned entries using its supplied
cleanup budget. This is request-boundary maintenance, not an independent timer:
the current blocking input loop still needs a scheduler for idle-channel cleanup.
Independent worker authority expiry remains in effect during silence. Navigation/
focus document issuance and authenticated transport caller remain unfinished.
Build/test stay deferred.

ReceiveRegistry now wraps its real native authority source with the document's
weak lifetime before session launch. Both the independent authority lane and
codec callback reject a closed/dropped document before and after observing the
source, preserving its original timestamp. The wrapper cannot keep a document
alive or restore a closed one. Thus packet silence does not bypass document
revocation; helper authority still expires independently if observation blocks.
Status reports cleanup debt for lost documents. Actual product document issuance,
maintenance scheduling and authenticated transport caller remain unfinished.
No build/test or helper execution occurred.

ReceiveRegistry now accepts an opaque native ReceiveDocument lifetime instead
of a caller-supplied UUID for media operations. Close/drop fences new delivery;
post-read admission also rejects output when the document closed while blocked.
Cleanup stores only a weak lifetime and retained ID, seals the entry before
retirement, and offers independent maintain() reaping for abandoned documents.
The real native document owner and independent maintenance scheduler still need
product wiring; this token does not replace the actual authority observer or
authenticate transport. No build/test ran.

Broker now owns a Windows ReceiveRegistry and invokes its deadline-bearing close
from broker shutdown. One exact document/media owner can open/push/tick/take or
retire a native receive session. Replacement is refused while any old entry is
retained; failed cleanup keeps the same owner retryable. Registry close seals
new admission permanently without silently removing uncertain retirement. No
renderer command, document capability issuer or authenticated transport caller
is added; those still need actual native scope/ICE/SRTP integration. This source
has not been built/tested or executed against helpers/devices.

`WindowsReceiveSession<S>` now orchestrates installed worker admission, guarded
decoder launch, independent native authority maintenance, codec/reorder receiver
construction and push/tick/unpresented/close lifecycle under one original session
deadline. The codec backend and authority lane share the actual native observer;
its original observation time is preserved. Decoder Stop precedes authority-lane
cancellation. Retirement requires a joined observer without panic as well as the
decoder receipt; partial cleanup remains CleanupRequired and retryable. Failed
setup uses existing worker/guard retention rather than admitting a replacement.
No renderer route, policy enablement, raw UDP/SRTP endpoint or registered service
session is installed by this source change. Actual authenticated ICE/SRTP caller,
native scope registry, display/audio and device qualification remain open. No
build/test/helper/network execution ran.

`WindowsH264Backend<S>` now supplies concrete codec admission for the existing
progressive SDR FIFO lane. Its native-only constructor pins negotiated profile
66/77/100, exact constraints/level 3.1–4.2 and the configured 720/1080 30/60 profile.
It checks 8-bit 4:2:0 geometry/crop, selected level frame/rate/reference capacity,
explicit square-pixel BT.709 limited centered-chroma VUI, fixed timing and zero
reorder restrictions, plus baseline/main syntax limits. Stateful/unimplemented
SEI is refused; bounded user-data envelopes remain supported. Actual native
authority is freshly observed on prepare/retry/completion. `into_receiver` checks
the decoder's retained configuration/owner and constructs the real parameter +
POC/DPB admission path. No product service/transport caller is installed yet.
Full profile constraint semantics, HRD/bitrate/entropy/coverage validation,
missing-metadata negotiation, extended formats/SEI/fields/reorder and platform
qualification remain open. This is the current lane's implementation, not a
redefinition of the broader Remote Play scope. Build/test remain deferred.

ParameterAdmission now owns FrameDecodeState and puts the staged POC/reference
plan in each admission proof. Its matched-output callback commits that plan
after backend completion checks. The progressive FIFO receiver defers admission
until a picture reaches the queue head and has only one uncommitted decoder
picture in flight; S_FALSE retains the same plan. Later dependent pictures are
planned after the previous picture's confirmed metadata commit. Original age
and the combined four-unit queue limit remain unchanged. This is wiring for the
existing low-delay progressive FIFO candidate, not completed general B-frame
reorder, field/MBAFF, gap recovery or full codec coverage/negotiation admission.
The installed native codec/owner backend, actual transport/service caller and
device qualification remain open. No build/test ran.

The private Windows client now creates an opaque DecodedPictureReceipt only
after output matches an accepted input's timestamp/profile/original receipt.
Receiver moves accepted inputs/proofs to an in-flight queue, correlates output
with that exact owner/timestamp/original receipt, then consumes the proof through
the required admission/backend `decoded` callback. Pending plus in-flight stays
within four compressed units; errors/retirement clear both. Input S_OK and
S_FALSE do not invoke completion. Existing decoder v3 remains one-input/one-output
FIFO and does not qualify general B-frame reorder behavior. Backend code still
must connect this callback to the staged POC/DPB owner and implement negotiated
codec/colour/coverage checks. No new wire assertion or renderer receipt exists.
Build/test and real decoder correlation/device qualification remain deferred.

`FrameDecodeState` now coordinates progressive POC, per-slice pre-marking
reference lists and post-decode reference marking in one staged plan. Retry
rechecks are read-only. Joint commit preflights both components and every list
against their owner/version/parameter bindings before mutation under exclusive
access; an unexpected partial failure closes both components. Native decoder
completion still must be observed for the exact picture before calling commit;
the API itself is not a completion receipt. Actual decoder correlation/caller,
fields/gaps/output and full negotiated admission remain unfinished. Build/test
remain deferred.

The frame-reference component now consumes `PictureOrderUpdate` directly for the
exact same parsed picture and parameter pair. Retained serials store the derived
post-MMCO5 POC; B-list construction uses the current staged pre-reset POC and
those stored values. The earlier external serial/POC mapping input is removed.
This connects POC derivation and progressive reference lists/marking; a joint
native owner still needs to coordinate both commits with actual decode results.
Build/test and field/gap/output/backend completion remain outstanding.

Native POC derivation now has source implementation in `native_h264_poc.rs` for
types 0/1/2: reference-LSB/MSB wrap, frame-number offsets, cycle offsets and
nonreference adjustments, top/bottom field counts and MMCO5 normalization. A
staged update exposes pre-reset counts for current prediction and normalized
counts for future reference retention. Commit binds the original POC owner,
version and still-current parameter pair; retries do not implicitly advance it.
Arithmetic uses wide intermediates with a finite count bound. Separate planes,
redundant pictures, gap/non-existing-picture generation, field pairing, output
buffering and actual decoder completion integration remain unfinished. The
reference-list component still needs to consume these derived counts through
its native decode owner. No build/test/parser/device execution ran.

Progressive-frame reference state now builds per-slice P/SP/B prediction lists:
descending wrapped PicNum for P/SP, POC partitions and identical-list swap for B,
then ascending long-term references. Short/long-term list edits insert the exact
retained serial, remove later duplicates and require enough active references.
Results retain DPB-table identity/version and SPS/PPS binding for stale recheck.
B-list construction requires a complete serial-to-POC mapping from the future
native POC owner; RTP time never substitutes for that mapping. Actual POC
derivation, fields/MBAFF, frame-gap/non-existing reference handling and decoder
completion/backend integration remain open. No build/test ran.

Frame reference marking now has source implementation in
`native_h264_frame_references.rs`: bounded short/long-term entries, frame-number
wrap, IDR reset, sliding-window eviction and ordered MMCO 1–6 effects. Updates
are staged without changing retained state, then committed once against both
the original reference-table version and still-current SPS/PPS binding. Wrong
targets, long-term limits, capacity and stale/cross-table commits fail. This
component covers progressive frame pictures; fields/MBAFF need the remaining
complementary-field coordinator. It does not establish reference-list existence,
POC/output order, frame-number-gap recovery, entropy validity or actual decoder
completion. No backend/receiver caller invokes it yet; build/test remain deferred.

`ParameterAdmission<B>` now implements the receiver admission contract by
processing actual Annex B parameter NALs in wire order, retaining SPS/PPS versions,
parsing picture/slice/auxiliary headers and binding the exact AU hash, RTP stamp,
original receipt and IDR flag into its queued proof. Retry revalidates that same
proof and parameter binding; any failed/partial operation closes the parameter
generation. A mandatory `NativePictureBackend` still supplies actual negotiated
codec/colour, DPB/reference and native-owner checks; there is no permissive default
or installed product caller. This connects the authored syntax modules to a real
receiver admission implementation without claiming full backend completion.
Build/test and end-to-end decoder/device qualification remain deferred.

Queued compressed units now retain their original ReceivedAccessUnit metadata
and an admission-provider-specific native proof. `NativeH264Admission` requires
an explicit read-only `recheck` before every submit, including S_FALSE retries.
That check must bind the exact AU, parameter/negotiated versions, native owner
and decode/reference order without advancing reference state a second time.
The receiver checks original receipt age and decoder authority again after the
recheck. No byte copy or refreshed receipt is created, and any error fences and
retires the generation. Proof implementations must remain bounded and avoid a
second compressed-frame allocation. There is still no actual admission provider
or default-success implementation. Build/test and runtime qualification remain
deferred; this closes a queue-boundary contract gap, not full codec admission.

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
