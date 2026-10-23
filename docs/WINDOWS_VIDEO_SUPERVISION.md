# Private Windows video supervisor candidate — item 56

Source authored only; no Rust/C++ compilation, helper launch, pipe command,
encoder activation, process fault or test was executed. No product command
invokes this candidate. Streaming remains disabled.

`platform/stream-input/src/video_client.rs` reuses the existing native private
helper launcher/process witness. Its location shares Windows helper ownership
code with input; this does not enable remote input or grant capture permissions.
The new fixed Video helper kind runs `--stdio-video-v1` in a creation-time job,
one process, 512 MiB committed process-memory limit, kill-on-close, restricted
inherited stdin/stdout/NUL handles and a cleared minimal system environment.
Input/gamepad launch limits stay 128 MiB. This is normal-user containment of an
admitted trusted helper, not a sandbox for an arbitrary executable.

The caller supplies the verified installed helper path, immutable file guard and
its admitted SHA-256, plus an original native game-process witness. The module
validates the hash syntax but does not independently discover/trust an installed
artifact or prove that a File was opened with the required sharing policy. Actual
installed-package admission and retained game/capture/document ownership remain
broker obligations. No PID scan or remote/renderer PID is process authority.

The immutable owner binds capture/game/pair/session/generation. A separate native
`VideoAuthorityLease` accepts strictly increasing observations at most 100 ms old
and expires 200 ms after observation. Initial setup has a 300 ms lease. Once
expired, closed or revoking, it cannot be revived. The host must refresh it from
an independent native observation lane even while configure/submit waits; keeping
the pipe alive or receiving a packet cannot refresh that authority. This lane is
not wired into current product lifecycle code by this checkpoint.

One private I/O thread serializes v1 requests and validates full framing, IDs,
HRESULTs, configuration echo, classification and payload sizes before admitting
responses. It polls every 8 ms while configured, even under continuous frame
commands. Configure is bounded by five seconds; frame/poll requests by 250 ms.
Polling remains separate from the native authority lease. The parent ties every
encoded timestamp and requested keyframe receipt to an accepted raw frame, bounds
outstanding receipts, and retires rather than dropping dependent output.

At most four queued packets/4 MiB are retained. Accepted-to-delivery age is at most
250 ms and local queue residence at most 100 ms. The independent 8 ms watchdog
checks queue age, native lease, original game-process liveness, pipe-call deadline,
actual I/O thread retirement and original helper-process liveness. It requests
CancelSynchronousIo and terminates only the privately retained job/process.
Cancellation is a request, not acknowledgement; the process wait is at most one
second. A blocked I/O thread holds the shared file guard/job and singleton permit
until actual retirement; an unconfirmed process retains a bounded tombstone and
blocks another process-local launch. No unfinished thread is synchronously joined.

Explicit Stop immediately fences/clears publication, allows only a bounded
one-second cleanup window, drains an already-issued Poll without accepting its
media and sends the terminal Stop request. Success requires the complete matching
S_OK receipt AND confirmed original helper process retirement. Killing the process,
EOF, timeout or an exit code without that receipt does not confirm encoder stop.
Dropping/revoking the authority lease instead hard-closes the transport; cleanup
may then be unknown. Stop should be requested before dropping the lease when a
positive native teardown receipt is needed.

An already-returned packet remains caller-owned. The future publication lane
must still compare current owner/capture generation and authority immediately
before publishing; this module cannot recall a packet handed to another thread.
Actual codec/level/bitstream admission and RTP packetization are not implemented
by the transport. Registered hardware-MFT classification is not proof of GPU use.

Remaining: actual broker/native capture admission and observation lane, installed
worker packaging/hash admission, capture/colour conversion, negotiated codec
translation, receiver/media transport and UI integration, cross-platform service
paths, provenance/distribution review and every required build/runtime/fault/
latency/energy qualification. None is waived by these source primitives.

## Owned-window v2 parent source

`VideoWorker::spawn_owned` now supplies a private native HWND bound to the retained
original game witness, and calls a typed launcher method that duplicates only
PROCESS_QUERY_LIMITED_INFORMATION/SYNCHRONIZE with inheritance. Its exact numeric
handle is passed as a fixed-mode argument; only that duplicate and the existing
three stdio handles enter the creation-time handle list. The original game is
not assigned to the helper's job and no raw renderer PID supplies authority.
The parent drops its inheritable duplicate immediately after process creation.
Helper containment stays one process/512 MiB; raw v1/input/gamepad paths keep
their existing argument/handle behavior.

Configure sends the immutable 69-byte v2 dictionary and validates the complete
74-byte success receipt. The consent/setup request allows at most 45 seconds,
while native lease observations still expire after 200 ms and overall original
grant time is unchanged. A separate admitted native observer must keep running
while the local user answers; a long pipe deadline does not grant that authority.
Explicit Stop during a blocked prompt may need process termination and remains
unconfirmed without its matching native receipt.

Owned-v2 disallows raw Frame injection and automatically polls encoded output.
The parent requires accepted capture sequences starting at one and increasing
exactly by one, nonnegative increasing timestamps and a real first keyframe.
Reported source age plus the FULL parent Poll round-trip must remain below
250 ms. That residual budget and the 100 ms local queue budget survive packet
copy/queue work; packets do not acquire a fresh source deadline on arrival.
Queue overflow/expiry and malformed/foreign-version data permanently close.

Reliable keyframe request replies yield a bounded monotonic next-frame fence.
The I/O lane retains up to eight distinct unresolved fences and requires an
actual keyframe at each fence before accepting that sequence. Repeated requests
for the same unaccepted frame coalesce. Intent receipts alone do not confirm
decoder refresh; existing dependent output is preserved in order.

The broker-private `WindowsOwnedVideo` wrapper configures this mode, exposes
unnegotiated packets/keyframe intent and confirmed revoke, with no raw producer
API. The previous mailbox/raw-v1 wrapper rejects an owned-mode worker. Neither
wrapper has a current installed-admission/service command or product caller.
Final codec/VUI/receiver negotiation, publication fences, native observation and
consent/Stop UI remain due. No launch, compiler, driver, capture, media, timing or
test execution occurred for this source checkpoint.

## Independent native observation lane source

The video lease now requires a typed `VideoAuthorityObservation` for the exact
capture/game/pair/session/generation, native document/capture/pair authorization
bits, and the original observation timestamp. Foreign owner, lost authority,
future/stale (>100 ms) observation or invalid sequence permanently closes the
transport; none can revive a lost lease. Four source-only admission tests were
authored, not run. They do not qualify native observation correctness.

`stream_video_authority.rs` adds one bounded process-local observer thread,
separate from the blocking configure/media caller. `NativeVideoAuthority` has no
default implementation that asserts success. Its installed native implementation
must recheck document/capture registration and pairing revocation/lifetime; a
signed certificate or cached renderer bit is insufficient. Current service has
no such native source implementation or lane caller yet. This source is therefore
not evidence that real pairing revocation/document closure reaches the worker.

The lane observes every 25 ms after a completed check and preserves that source's
timestamp. If a source blocks or stops refreshing, the independent worker lease
expires after its existing 200 ms; the lane cannot renew it after expiration.
Cancellation is cooperative and never joins a running thread. The source, lease
and singleton permit remain held through actual observer destruction, including
stalled cleanup. A Drop fence closes the lease before source destruction on
normal exit or panic. A hung source prevents a replacement lane instead of
accumulating detached observer threads. `finished` checks the actual thread handle,
not a claimed state flag. No native source, lane, process or test was executed.
