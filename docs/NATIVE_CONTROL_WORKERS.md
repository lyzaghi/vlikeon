# Owned native control worker retirement

Source authored only. No worker pool, OS thread probe, compiler, SDK, native
process, I/O thread, unit test or acceptance case has executed during this work.
This addresses a Windows item-2 resource/lifetime component; the whole Windows
and original product qualification requirements remain open.

## Capacity before launch

The native control library now has one process-wide pool of 16 slots. A slot
covers an unused prelaunch reservation, an attached IPC worker or a retained
retiring worker. This is an internal admission policy, not a promise of 16 games,
measured scalability or a whole-machine thread/RAM limit. The broker's existing
session policy remains separate. The worker builder requests a 2-MiB stack;
actual OS stack/commit/RSS, payload memory and other broker threads need their
own resource accounting and final measurements.

Windows preparation checks cancellation and reserves a slot before staging
native work, copy debt, container or process creation. The shared Linux source
reserves before creating its cgroup/actor pair; earlier Linux work staging is
still governed by its existing owner. Dropping an unused reservation or a failed
thread-spawn attempt returns that unused capacity. An exhausted pool returns
ResourceLimit; it does not create a weaker untracked worker or clear old debt.

The reservation transfers into an OwnedControlWorker only after thread creation.
Its permit stays outside the worker closure. Thread-body return or an actor-exit
observation alone cannot decrement the count. The control receiver still owns
its Files, current encoded header and binary buffers until it actually ends.

## Close and collection

Dropping or abandoning WorkerChannel closes future command admission and moves
the unique thread handle plus permit into a fixed 16-entry retirement registry.
Channel destruction performs no pipe I/O, expanding retirement allocation or
thread join. A blocked reader/writer can remain owned without blocking channel
destruction. Mutex scheduling and the separate actor/helper termination waits
still need actual responsiveness qualification.

Each new reservation first polls at most those 16 retained records. On Windows,
the safe sandbox wrapper polls the held thread object with
WaitForSingleObject(handle, 0). Only its signaled result admits join; timeout,
query failure and current-thread identity retain ownership. Joining happens
outside the registry mutex. Capacity returns only after join has finished,
including a normally joined Rust panic. No background reaper thread is spawned.
An already finished record can therefore remain charged until the next admission
attempt performs collection.

The distinction matters because Rust documents that is_finished can precede
full thread termination, while join includes thread-local destruction. Windows
documents that the thread object is signaled on termination and a zero-time
wait returns immediately. These are API contracts, not measurements of this
implementation: [Rust JoinHandle](https://doc.rust-lang.org/std/thread/struct.JoinHandle.html),
[Windows thread termination](https://learn.microsoft.com/en-us/windows/win32/procthread/terminating-a-thread),
[zero-time wait](https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-waitforsingleobject).

Unexpected record loss retains its permit permanently instead of claiming a
join. That conservative failure consumes capacity for the broker's remaining
lifetime. There is no timer-based, actor-status-based or caller-controlled
refund. Normal tracked records are reusable after observed completion and join.
The non-Windows source retains an is_finished precheck before join; its permit
also remains held through join, but Linux/macOS completion and responsiveness
qualification are separate requirements in their numbered work items.

## Separate proofs

The Windows startup/manual work collector uses a separate process-wide pool
with a one-worker limit and the same handle/permit retirement mechanism. Its
result snapshot can be terminal before the thread ends; `NativeWorkJobs` now
checks the held OS thread instead of joining after `is_finished`. Broker close
requests cooperative cancellation and parks its owner. A replacement broker's
aggregate status observes the retained collector and cannot spawn another one
until actual completion. Polling status does not join; admission collects ended
handles. There is no renderer-controlled reset of either pool. The source now
requests the same 2-MiB stack for the collector; this remains unmeasured.

Retiring an IPC worker neither terminates nor acknowledges the actor/helper,
commits a final save, retires an AppContainer/cgroup, releases a work lease nor
deletes a work directory/copy-debt row. The existing real process/job/container
and work-retirement chain remains responsible for those operations. A refused
actor termination and a blocked broker I/O thread retain their respective owners.
No process-death or storage-reclamation authority comes from the slot counter.

The implementation uses only safe Rust in native-runtime. The Windows handle
query remains in the existing target-specific sandbox FFI layer and accepts a
borrowed JoinHandle, not a disk PID, arbitrary handle number or caller path.

## Final evidence

Eight pool cases are authored in control_workers_tests.rs: finite/prelaunch
capacity, close while a real thread is gated, panic retirement, isolated test
pools, single-slot retirement, actual singleton recovery-owner replacement,
unexpected record loss and Windows TLS cleanup held after body return. The production entry point
always uses the single global pool; private test instances are not a product
configuration interface. No case has run. Execute the test filters and real
I/O/process cases in qa/native-lifecycle/CONTROL_WORKERS.md and Windows case 27
only in item 16, with exact artifacts and retained OS observations.
