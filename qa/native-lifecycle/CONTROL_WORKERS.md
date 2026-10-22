# Native control worker final qualification

Authored only. Run after all source authoring in item 16, using the exact
qualified Rust/Windows SDK, complete locked workspace and retained source/
artifact hashes. Do not infer OS thread, process, disk or memory results from
unexecuted source or counters alone.

1. Build/run the native-runtime crate's `control_workers::tests` filter and
   existing `channel::tests` with the exact qualified toolchain, frozen/offline
   dependencies and `--test-threads=1`. Require all eight pool cases on Windows.
   The deliberate TLS gate must have a prestarted probe thread and must release
   its gates before asserting; setup failure or timeout is a failed case.
   Other platforms omit the Windows-specific case and need their own numbered
   completion audit. Record per-case output and SDK/OS/build identities.
2. Fill all 16 reservation/active/retiring slots through actual preparations.
   The next Windows preparation must return ResourceLimit before native work/
   copy-debt/container/process creation. Drop unused preparation reservations
   and qualify real thread-creation failure; exactly their unused capacity may
   return, without hiding an actor/helper that was already created.
3. Block real worker stdout reads and stdin writes at each framing boundary,
   including a current maximum load buffer. Cancel, time out, close and drop
   the native adapter/channel while an I/O thread remains in the kernel. Prove
   channel destruction does not join the running thread, retained ownership
   remains charged and future preparations cannot bypass the 16-slot limit.
   Observe real broker threads/handles/private/commit memory and UI response;
   the configured stack request and a slot count are not physical RAM evidence.
4. Hold thread-local cleanup after the thread body returns. Poll its actual held
   Windows thread handle and require the slot to stay charged until the OS
   thread ends and join returns. Inject timeout/failed handle query; no guessed
   refund is allowed. Let one tracked worker finish and confirm exactly one
   replacement reservation becomes possible. Race completion, close/abandon,
   simultaneous reservations and repeated collection across actual threads.
5. Exercise control-worker panic/unwind, late reply after cancellation, channel
   disconnect, spawn failure and interrupted owner transfer. Retained records
   must neither allocate beyond their fixed registry nor duplicate/refund a
   permit. An explicit qualification-only lost-record injection must retain its
   charge for the broker lifetime; do not restore capacity by editing counters.
6. Independently refuse actor/helper termination and verify their original
   process/job/container and work/debt ownership. IPC completion alone cannot
   release that storage or produce a successful final-save/stop receipt. Repeat
   real final-save, worker/broker crash, default storage, maintenance/recovery
   and original Windows tests. Other broker/background/helper/core resources
   and whole-profile physical accounting remain required.
7. Hold the real recovery collector in filesystem I/O and in TLS cleanup after
   publishing a terminal result. Close its broker owner, construct a replacement
   and query status/start. The earlier collector must remain visible as busy;
   start must refuse without joining it or creating a second thread. Release
   the gate, observe the held OS thread ended and allow exactly one replacement.
   Repeat cancellation, panic and spawn failure, and fill the independent IPC
   pool while recovery runs. Neither pool may refund the other's ownership.

Retain exact failure points, actual thread handles/state, actor/helper identity,
work/ledger/proof state, input/output byte counts, timing and evidence locations.
An observed function return, changed counter or status flag is insufficient to
declare process/job retirement or complete item 2.
