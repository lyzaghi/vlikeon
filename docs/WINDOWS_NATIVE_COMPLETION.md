# Item 2: Windows native runtime completion

The Windows native implementation is still source only. It is not marked
complete and no compiler, process, AppContainer, Job Object, pipe, ACL, profile
deletion, filesystem cleanup or test has been executed during this work.

## Source changes in this pass

Native IPC worker ownership now has 16 process-wide reserved/active/retiring
slots. Windows preparation reserves before work/process staging; closing a
channel transfers its thread handle into fixed retirement storage without
joining a running I/O worker. Windows collection polls the actual held OS thread
object, then joins outside the registry lock before refunding capacity. Failed
queries and unexpected owner loss retain their charge. The existing actor/job/
container/work-retirement proofs remain separate. Eight real-thread/pool/TLS
case sources and Windows case 27 are authored, not run. The recovery collector
has its own process-wide single slot with retained ownership across broker
replacement; terminal result publication no longer admits an early join. See
`NATIVE_CONTROL_WORKERS.md`; no whole-RAM, SDK or process behavior is qualified.

Native control now validates the bounded/depth-limited/duplicate-free header,
closed method and consecutive identity before reading a payload length. The
reader admits one outstanding command until main releases its input buffer.
Malformed/truncated framing exits nonzero; boundary EOF performs checked
teardown without a save/stop receipt. The broker bounds request serialization,
checks closed success/error envelopes before binary allocation and invalidates
channels on timeout/cancel/terminal faults. Unknown core exceptions now terminate
inside the guard before host load/snapshot buffers unwind. Rust codec/admission,
13 lifecycle and 19 actual-worker wire cases are authored, not run. See
`NATIVE_CONTROL_BOUNDARY.md` and Windows case 26. Full broker/process/resource/
core/save qualification and all original item-2 requirements remain open.

Guest pause now requires a declared numeric protocol and checked core export;
void return alone no longer publishes quiescence. MD/PS1/PSP and the unadmitted
N64 candidate author their actual owner/writer/thread receipts. Initial prepare,
resume and AV transitions honor that barrier, including a prior pre-entry GL
failure. Unknown pause/unload/deinit lifetime cannot be replayed during restore
or shutdown. Fatal command failures now keep a nonzero actor exit status.
All core API invocations now share an entry/unknown-exception guard, and public
content commands require Ready instead of accepting a partially prepared load.
The Windows owned-actor primitive uses self-termination without DLL detach,
with retained ownership on refusal. Actual worker/DLL/pipe and live-producer
fixture source and detach witnesses are authored, not executed. See
`NATIVE_GUEST_PAUSE.md`, `qa/native-lifecycle/ACCEPTANCE.md` and Windows case 25.
Original package rebuild, source, driver and whole Windows requirements remain.

Native audio enqueue receipts now report only frames actually copied into the
bounded worker ring and reject impossible counter occupancy before subtraction.
N64 pending output offers are capped to the worker's 16384-frame admission and
retain unacknowledged frames. Enqueue acknowledgement is not device playback;
existing latency trimming and intentionally disabled output remain separate.
Actual device/presentation routes and partial-frame handling in every core
require acceptance case 24. GL procedure-name length checks now precede reads
at the maximum index. No source/test copy proves runtime audio or GL behavior.

The production native ring now stages output and latency trimming privately;
only a complete successful sink receipt releases ring slots and changes priming
or underrun state. Both producer and consumer reject impossible occupancy.
Pinned SDL's chunked queue writer can return false after copying a prefix, so
the worker latches a fault and clears queued SDL audio on rejection instead of
replaying the retained ring. Clearing is not physical playback retraction.
Preparation/paused producer flushes deliberately discard audio; they do not wait
for a stopped device. Stream lock/clear/format/unlock/pause/resume failures are
no longer silent. Failed device pause still attempts the guest pause barrier;
successful device resume precedes publication of running/started state. A
repeated resume while already running does not reset a live producer's ring.

Presentation audio batches now allocate and encode fully before one mutex-owned
commit, use allocation-free list splicing and retain documented oldest-packet
trimming. Engine batch receipts now honor presentation rejection. Local media
generations reject audio/video copies racing a same-epoch pause or stale-media
clear. None of these queue receipts acknowledges transport/helper/device output.
`qa/native-audio` now authors actual production-ring cases and the failure/race
matrix for item 16. No new case, toolchain or runtime was executed.

Graphics callback release now retains ownership when making the context
current fails before invocation. A later attempt may retry while the DLL and
callback context remain live. An exception after callback entry latches unknown
release and prevents callback replay or new negotiation/reset. Successful
synchronous callback return disarms its registration. Engine stop refuses
reentrant teardown during release; shutdown/release failure exits the owned
actor nonzero before further deinit or DLL unload. GlVideo destruction no
longer masks an unknown callback lifetime and destroys its context anyway.
This is callback-lifetime source, not a GPU completion/fence receipt, lost-
context recovery implementation or proof the callback freed every resource.
Actual acceptance case 22 and original driver/worker/broker qualification stay.

Context-reset callback entry now has the same active/unknown lifetime gates.
Reset readiness is published only after successful callback return. Nested
release/reset/present/negotiation cannot run while reset is active, and stop
cannot unload content from inside reset. A reset exception latches failure;
stop refuses before core unload and shutdown exits the actor before deinit or
DLL destruction. Context acquisition failure before callback entry remains
retryable without falsely publishing reset completion. This conservative
exception path does not claim that partially initialized graphics resources
can be recovered in-process. Native acceptance case 23 remains unexecuted.

N64 primary SRAM/EEPROM/Flash/mempak bytes remain in its existing libretro
save-memory block, captured by the worker's memory envelope when that package
route is eventually admitted. Peripheral RAM/DD/RTC flush is a separate
contract and does not itself capture that primary block. The combined archive,
fault/stop capture order and explicit failed-session recovery still need actual
worker evidence; the current N64 candidate remains disabled.

Both the AppContainer actor and trusted presentation helper now join their
already-limited kill-on-close Job Object inside `CreateProcessW`, through
`PROC_THREAD_ATTRIBUTE_JOB_LIST`. Membership is queried before recording the
PID/creation-time identity and resuming the suspended process. Unsupported or
failed association rejects launch without a later-assignment fallback. The job
handle is not inherited. This addresses the former broker-crash interval
between process creation and `AssignProcessToJobObject`.

A profile lifecycle mutex serializes initial ACL grants, the entire launch
attempt and retirement. The container refuses new grants after launch or
retirement. A concurrent caller cannot retire an identity while its launch is
still establishing process ownership. Child guards retain the existing active
and uncertain-exit accounting. Actual OS behavior remains unverified.

Confirmed Windows work retirement now has a small immutable proof in the owning
profile's `native-retirement/<work-id>.json`, outside the work tree that will be
deleted. It is published only after the original ownership chain, exclusive
work lease, actor/helper exit identities and exact AppContainer retirement have
passed. The storage row must be in the exact profile/work `retiring` phase.

Deletion and later retries hold a separate exclusive collector lease. An
interrupted removal can therefore continue even after the internal owner lease
or process records have already been removed. A proof cannot delete a
reserved/staged or reused UUID, follow a reparse tree, free a retained copy debt
while its directory remains, or signal a PID. The ledger is released only after
literal directory absence; the external proof is removed last. A missing
directory and already-released row are accepted only for finishing that same
confirmed proof's cleanup.

The asynchronous startup/manual owner collects a bounded inventory of work UUIDs and external proofs, processes
at most 32 candidates per pass and persists a rotating UUID cursor. Unknown,
live or malformed entries remain retained while later entries still receive a
turn. Independent root-entry caps bound inventory work. The existing generic
source does not collect Linux sessions through this Windows path.

The Windows Settings maintenance source now reports finite profile reservation
and phase counts, follows an actual owned worker and supports explicit bounded
retry/cancellation. Its startup pass runs off the broker control thread;
per-document job IDs, late-result cancellation and worker/thread lifetime
admission remain explicit. See `WINDOWS_NATIVE_MAINTENANCE.md`. No command,
worker or UI has run.

The pre-launch source now serializes reservation/directory/owner-lease handoff
against recovery. Schema 15 stamps only new gated Windows reservations with an
immutable creation protocol, and the collector also inventories ledger-only
debts. Missing-directory or pre-owner partial preparations can use that exact
pre-launch authority while old/ambiguous and staged debts remain retained. See
`WINDOWS_PRELAUNCH_RECOVERY.md`; these sources have not executed.

New launch source also checks the suspended actor's exact AppContainer SID,
low integrity and no-write-up token policy, then seals its private registry
and exact default filesystem tree with medium/no-write-up labels before process
identity publication/resume. A single-use lookup thread checks its own effective
token, reverts and is joined before a real SID-bound registry key is mutated.
No broker/preparation thread is impersonated. Storage-probe-v2 and its explicit
final-pass harness are authored, not executed. See `WINDOWS_DEFAULT_STORAGE.md`
and `WINDOWS_REGISTRY_STORAGE.md`.

The source now also connects a bounded frontend-owned memory-file table to the
Windows MD worker/core route, checked canonical-file capture and staged restore.
Its declared mode removes OS write grants from the actor's work save/scratch
trees, requires exact install/prepare acknowledgements and admits only matching
core/header/artifact receipts. Other cores retain their earlier routes pending
separate adaptation. See `NATIVE_MEMORY_FILES.md`; no binary/catalog or runtime
evidence was produced.

The native3 PS1 source now uses the same bounded memory-file mode for card
loading, synchronous/asynchronous publication and checked combined capture.
Card swaps join older queued jobs; failed reads preserve guest NV and block
cleanup overwrites. Its exact ABI/source/artifact declarations remove actor OS
save/scratch write grants through the shared broker route. See
`PS1_MEMORY_CARDS.md`; compilation, real async/FrontIO/OS evidence and other
cores' write routes remain open.

N64 now has scratch-only C stdio/C++ stream/gzip bridge source and actual
adapter/store/vendor-codec qualification cases. Writers stage complete images,
charge old/new pages concurrently and preserve prior names on failure. Gzip
decoding has a finite expanded-byte budget and checked framing. Unconfirmed
callback close prevents table reuse until actor retirement. N64 configuration,
shader/texture/file-cache integration and exact ABI/package lifecycle still
remain due; no N64 package or OS write grant has changed. See `N64_MEMORY_IO.md`.

A separate committed-source gl2 candidate now routes generated config, checked
ROM-database/custom-settings reads and shader key/binary caches through those
adapters. It stages full bounded shader admission before GL, bounds driver
capture, preserves old cache publication on faults and owns checked Windows
root/guest fiber conversion/retirement with explicit TLS. Read failure prevents
core readiness. The candidate is unbuilt/unselected; remaining texture/file/
dump/peripheral writers and full core/package/OS/lifecycle qualification still
block removal of N64 OS write grants and item-2 completion.

The gl2 source now also adapts actual GLideNHQ memory gzip and file-index caches
to scratch. Complete nested-payload/index admission precedes cache replacement;
read/write/publication faults retain prior cache data, and late reopen failure
is distinguished from an already committed image. Exact format-aware LRU and
finite payload/metadata limits are authored. Real adapted-cache fixture sources
remain unexecuted. Texture dumps, other core/peripheral state writes, real pool/
transient/driver resource bounds and exact package/full-OS qualification remain
open; no N64 OS write permission is removed by this source checkpoint.

The actual N64 TxMemBuf source now delegates to bounded shared/workspace arrays
with checked dimensions, staged pair allocation, one-lane owner scopes and
busy-retirement retention. Filter initialization reports actual readiness,
output scale/allocation failure is checked and cache dimensions must fit the
real pool. The core refuses table uninstall before pool retirement; the common
worker now immediately exits its owned actor on an unconfirmed uninstall,
before DLL/static/callback-context destruction or successful stop reply. Actual
pool/TxMemBuf fault/ceiling cases are authored, not executed. Other pipeline
temporaries/writers, combined peaks and full core/job/process/device evidence
remain open; this does not close Windows item 2.

Selected N64 texture dump source now publishes PNG through bounded scratch
callbacks, with checked source/stride, a custom 8-MiB codec allocator and C-only
error/longjmp cleanup. Texture debug logs are finite transactional scratch data;
native frontend logging treats message content as data. Real encoder/store/codec
and prepared-diagnostic case sources remain unexecuted. Readonly image loading
is unchanged; other state/peripheral writers, decoder/temporary resource bounds
and actual compiled core/OS/product capture evidence remain due.

The actual prepared N64 file-storage/utilities and Transfer Pak RAM now use a
distinct persistent memory namespace, staged exact reads with checked close,
failure-retained dirty ranges and full-image retry. ROM-fingerprint/port names
replace the invalid empty-string RAM default; old RAM paths are readonly
imports and retained memory saves take precedence. Actual storage/store/quota
qualification sources are authored, not executed. DD destination/import/format,
state contracts, producer/flush/capture acknowledgement and full core/package/
OS qualification remain open. Source flags alone cannot acknowledge a save.

Selected 64DD source now stages MAME/SDK/D64 format and expanded-length
admission, actual geometry/sector bounds, memory save destinations and checked
readonly legacy imports. Bad retained saves cannot fall back to a fresh base.
Failed final publication retains owned descriptors/RAM, with deinit/uninstall
refusal; an unstarted IPL rejection releases without creating a default save.
The actual IPL source bounds its path/read/signature/size before core copy.
Actual-source synthetic/format/geometry/fault/ownership cases are authored,
not executed. Full DD/controller/OS evidence, aggregate producer pause/flush/
capture acknowledgement, package admission and all original final scope remain.

The N64 candidate now authors actual thread/root/guest-owned preparation,
interpreter/interrupt safe boundaries, guest-side snapshots and checked all-
peripheral capture. Failed close retains RAM; input/import errors cannot be
cleaned up through a default-data writer. Snapshot mutex init/destroy and bad-
magic retry, stopped-guest lifetime and malformed-return actor exit are explicit.
The common worker requires table detach acknowledgement before restore swaps,
with conservative context retention on reattach refusal. Actual owner/helper/
storage and real Windows-fiber case sources are authored, not run. These do
not prove full core/GPU/worker/process/restore behavior or close item 2; exact
package admission, remaining writers/resources and original qualification stay.

## Remaining source and final qualification

`WINDOWS_ITEM2_CLOSURE.md` is the source-closure checklist for the requested
transition to item 3. It separates unfinished implementation from the single
deferred execution pass, without moving open Windows obligations out of this
item or treating source snapshots as completion.

- Interrupted profile creation without an ownership acknowledgement and launch
  intent without a durable process acknowledgement still retain work/debt. The
  new atomic job primitive does not prove legacy ambiguous records ended.
- Full legacy retained-work migration and real maintenance status/retry/worker
  qualification still need completion.
- Private copy reservations cover declared game/firmware copies. Default
  AppContainer filesystem/private-registry sealing is authored but unqualified;
  externally granted registry access,
  other cores' scratch/save writes, helper/driver caches and physical
  whole-profile disk accounting still need complete resource controls.
- The external proof uses the existing flushed immutable-file publication
  primitive. Hardware/power-loss namespace durability, ancestor replacement,
  external same-user mutation and Windows file-sharing behavior remain actual
  filesystem implementation/qualification concerns; no stronger guarantee is
  inferred from a successful source copy.
- Actual approved worker/core/helper/package builds, mitigation and nested-job
  compatibility, pipe closure/blocked kernel I/O, foreground/input/audio/device
  behavior, final-save/crash recovery and every fault boundary remain in the
  consolidated item-16 verification.

Primary API contracts:

The current N64 source additionally bounds snapshot queues/PIF pointers and
TLB fields/lookups before restore mutation, uses admitted DD ID offsets and
finite final-page map/unmap walks, and guards interpreter random TLB indices.
Transfer Pak snapshot selectors and actual GB packet/storage/camera bounds,
high-bank RAM offsets and rumble bank separation are also source-authored.
The separate gl3 candidate adds bounded MBC3 time/halt/carry and independent
latched reads with explicit snapshot 1.10/legacy-import source; it is unbuilt,
unqualified and not admitted. Content/port RTC battery binding, staged boot and
checked RAM/RTC retirement source is added; actual persistence/device timing
remain unverified.
Flash/SI complete-range copies, DD C2/zone bounds and known snapshot device
selectors are also source-authored, with actual integrated qualification due.
PI adjusted complete-RAM admission, bounded framebuffer descriptor/mapping/
callback source and a finite six-slot graphics identity lookup are also authored.
Direct SRAM/ROM/DD complete DMA ranges and short swapped SRAM dirty-notification
source are also authored. Image-loader admission, pending graphics-address
storage, pixel/GPU bounds and actual full core/worker/OS evidence remain due.
The selected color renderer now has fixed exact pending-byte coverage and
bounded CPU conversion/staging geometry, without retained framebuffer borrows
from write notifications. GPU completion/error receipts, full texture/driver
accounting, image-loader admission and actual visual/lifecycle qualification
remain open; CPU fixture source does not establish renderer execution.
Actual runtime behavior and
complete semantic CPU/peripheral snapshot admission remain unverified and open;
see `N64_MEMORY_IO.md`. This source change does not close Windows item 2 or
change package admission/OS write grants.

- https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute
- https://devblogs.microsoft.com/oldnewthing/20181116-00/?p=100245
- https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects

