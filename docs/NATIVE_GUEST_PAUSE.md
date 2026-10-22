# Checked native guest pause and retirement

Source authored, not compiled or executed. This contract supports item 2;
it does not close Windows, core compatibility or final qualification.

## Admission

The native catalog now has `pause_barrier_protocol`, defaulting to numeric zero.
Zero is admitted only with `pause_barrier: false`, for the existing synchronous
core route. `pause_barrier: true` requires protocol 1. Missing legacy protocol,
unknown versions and inconsistent declarations are rejected; rebuild the exact
core and worker rather than promoting a void callback into an acknowledgement.

The worker advertises protocol 1 in `hello`; Windows and Linux callers check it
for declared barrier packages and send the exact numeric value in `prepare`.
The worker resolves `int omniplay_pause_guest_checked_v1(void)`. It never falls
back to `void omniplay_pause_guest(void)`. Build receipts and package generation
carry the numeric protocol. Its nonzero value also enters save configuration
identity; this adds no new state-format or cross-core compatibility exception.
Original core/version names are retained, with artifact and corresponding-source
hashes still required. Old receipts must not be relabelled as newly built cores.

## Receipt and ownership

Only return value 1 acknowledges producer quiescence. Zero, any other integer
and an escaping exception latch an unconfirmed pause. That callback is not
replayed, paused state is not published, and save replacement/resume/unload are
refused. A GL make-current failure before entry retains the live callback and
can be retried. Resume and AV changes must finish that barrier before resetting
audio. Initial preparation also obtains the barrier after successful content
load and any declared boot-completion callback.

MD acknowledges its existing cooperative owner/fiber boundary with threaded
video disabled. PS1 joins its writer, then checks actual thread/running state
and pending/writing slots. PSP checks for a parked emulation thread, or a
disabled thread that was never started. The unadmitted N64 candidate accepts
its healthy owned boundary or confirmed stopped guest, with no audio callback
borrow. The stopped path preserves the separately checked peripheral recovery
contract; it does not resume a failed guest or acknowledge persistence itself.
Legacy void exports remain in these cores for separately supported consumers;
the new worker does not use them.

All worker core API invocations now use one callback-entry guard, with checked
pause keeping its distinct receipt guard. An exception during initialization,
load, run, snapshot, file-table operations or other core calls latches unknown
ownership, prevents further core calls and stops DLL/static/callback-context
destruction. GL release retains its own pre-entry retry and post-entry failure
rules. Shutdown no longer swallows a failed guest pause and continues teardown.
Fatal command errors produce a nonzero actor exit, and an already-faulted actor
cannot report successful stop merely because teardown returned.
Public content operations require Ready, so a partially prepared loaded core
cannot be started or captured as if preparation had succeeded. Ordinary false
core results retain their API-specific handling; they are not C++ exceptions.

## Windows actor termination

`owned_actor_exit.h` is restricted to the disposable broker-owned actor. On
Windows it calls `TerminateProcess` on the current process. A refused call
parks the caller without returning or unloading; broker job termination and
actual observed process exit are still required. Other platforms retain their
existing `_Exit` route pending their own lifecycle qualification. This is not
a general application shutdown helper, final-save receipt or a durability claim.

Microsoft documents that `ExitProcess` sends DLL detach notifications, whereas
`TerminateProcess` does not. The latter is appropriate for this already-failed
actor's unknown callback ownership; it must not replace ordinary acknowledged
save and teardown. Termination still depends on pending I/O cancellation and
real process observation. See [DLL entry-point notifications](https://learn.microsoft.com/en-us/windows/win32/dlls/dllmain)
and [TerminateProcess](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-terminateprocess).

The common worker, graphics destructor and prepared N64 fatal owner/snapshot
paths now use this primitive. Other upstream CRT/abort/fatal paths and every
real core/driver/SDK combination remain part of the complete qualification.

## Required evidence

`qa/native-lifecycle` authors an actual qualification DLL and worker harness,
with a live producer, initialization/load/run exceptions, refused/invalid/
throwing pause, unload/deinit exceptions,
legacy void-only export and protocol mismatch. DLL destructor and detach
witnesses fail if unknown ownership reaches them. No fixture, OS termination,
build or test has run. Windows acceptance case 25 additionally requires the
actual broker, job, core, helper, final-save and fault matrix; direct dummy-driver
fixture results alone cannot prove those product requirements.
