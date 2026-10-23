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
