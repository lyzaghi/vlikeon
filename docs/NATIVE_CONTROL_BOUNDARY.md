# Native control framing and lifetime boundary

Authored source only. No native worker, broker, thread, pipe, compiler, SDK,
unit test or fault case ran. This extends Windows item 2; complete resource,
core, save, device and process qualification remain open.

## Actor request admission

The existing two-length-prefix wire remains protocol 1. Requests have exactly
`protocol`, `id`, `method` and `params`. Protocol/id are unsigned integers;
identity starts at 1 and advances consecutively through 2^63-1. The method is
one of the existing 17 control commands and params is an object. The 64-KiB
header is parsed before reading the binary-body length. A throwing parser
callback stops nesting beyond depth 32 and duplicate keys in any object;
unknown keys at the top-level envelope, unknown methods and malformed UTF-8/
JSON are terminal. No rejected header earns a save-sized body allocation.

Only `load` accepts input binary, up to the existing 256-MiB save envelope.
Every other command requires a zero binary length. One accepted command owns
the reader admission until main has completed/replied and released its input
buffer. Popping a packet alone no longer starts a second speculative load.
The pipe's OS/CRT buffers still exist; this is not a whole-process memory or
zero-copy claim, and save rollback/core/device memory needs separate accounting.

EOF between frames allows ordinary checked teardown without a save/stop
acknowledgement. Partial length/header/body, malformed input and allocation/I/O
failure mark the Inbox failed and produce nonzero exit after checked teardown.
The detached reader remains process-owned. Windows standard-handle replacement
must succeed before core loading; otherwise transport initialization fails.

## Trusted broker admission and replies

The broker checks the method, params object/depth, binary cap and timeout before
queueing. Its serializer has a 64-KiB writer bound and fallible reservation;
it does not allocate an arbitrarily large header and check afterward. A local
rejection neither consumes an identity nor writes a frame. Timeouts must be
positive and at most 300 seconds, with the existing shorter per-method limits
still chosen by each caller.

Replies use closed `ok` and `error` envelopes. Success requires an object result
and no error field; error requires one known code and no result field. Unknown
status, extra/duplicate envelope fields, wrong identity/protocol and invalid
field types are terminal before any binary length is read. Error frames must
then have zero binary length before their code can be treated as recoverable.
Only successful save/measurement-read replies accept binary output, within
their existing bounds and with nonempty data. No child-controlled error text,
path or game bytes are promoted into a user-facing message.

Cancellation, timeout, queue failure, sequence exhaustion and terminal response/
I/O failure invalidate the channel before requesting owned-process termination.
Future commands cannot be admitted even if termination is refused. Invalidation
is not proof that an already admitted call did nothing or that a thread/process
has exited. Channel destruction now transfers its worker handle/permit into the
bounded retirement pool in `NATIVE_CONTROL_WORKERS.md`. Caller process/job and
uncertain-exit work/debt ownership remain separate. Actual blocked kernel I/O,
whole-broker resource accounting and every original cleanup requirement remain due.

## Borrowed core buffers on exceptions

The common core invocation guard now terminates the owned actor directly in its
exception catch. It does not throw through caller frames first: load/serialize/
file-table callbacks may still retain a pointer to those host buffers. A
refused Windows self-termination parks that stack for the owning broker rather
than releasing it. Checked-pause exceptions use the same immediate path. This
does not undo C++ unwinding already performed inside a misbehaving core, or
prove that normal-returning cores respect every borrow contract.

The lifecycle fixture now expects EOF with exit 1 for unknown init/load
exceptions, without an error reply produced by host-frame unwinding. Normal
recoverable core results keep their declared handling. Earlier DLL/static/
callback-context retirement guards remain in force.

## Final evidence

`platform/native-runtime/src/channel_tests.rs` authors production codec and
local admission/poisoning cases. Cursor streams and the process counter prove
no real process, kernel pipe, job or physical allocation behavior by themselves.
`qa/native-lifecycle/wire_cases.py` authors 19 actual-worker malformed-frame/EOF
cases, including headers sent without any following body-length bytes. Run
these only in item 16, plus Windows case 26 for actual broker, stalled reader/
writer, cancellation, live memory, final saves and process retirement.
