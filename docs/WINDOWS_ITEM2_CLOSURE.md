# Windows item 2 source closure

The owner requested finishing item 2 before moving to item 3. Item 2 is still
open. This checklist distinguishes implementation gaps from the execution
reserved for full final acceptance. The owner now also permits exact test
batches by Antigravity concurrently with Codex source authoring; see
`QA_EXECUTION_POLICY.md`. Earlier source-only audit notes are retained below;
selected real Windows batches now have the explicitly scoped evidence recorded
here. There is no measured completion percentage.

## Authored host boundaries, awaiting item 16

| Boundary | Source | Execution still required |
| --- | --- | --- |
| Fresh AppContainer, exact token, creation-time job ownership, suspended admission and owned process retirement | `platform/sandbox/src/windows.rs`, `windows/storage.rs`, `windows/registry_storage.rs`, `windows/presentation.rs` | Real OS/SDK, mitigation, nested-job, token, label, crash and driver cases |
| Closed bounded IPC, cancellation, terminal channel admission and retained broker I/O workers | `platform/native-runtime/src/channel.rs`, `control_workers.rs`, `engines/native/src/protocol.cpp`, `main.cpp` | Real blocked pipes, cancellation/termination refusal, memory and process observations |
| Guest pause acknowledgement and unknown-callback lifetime retention | `engines/native/src/engine.cpp`, `gl_video.cpp`, `owned_actor_exit.h`, exact adapter/catalog contracts | Actual core/DLL/producer, stop/restore/save and detach-witness cases |
| Copy-debt reservation, creation lease, staging acknowledgement and confirmed resumable retirement | `platform/core/src/native_work_storage.rs`, `platform/native-runtime/src/runtime.rs`, `work.rs` | Migration, sharing, crash ordering, partial deletion and filesystem durability |
| Startup/manual maintenance, document-owned controls and one retained collector across broker replacement | `platform/broker/src/native_work_jobs.rs`, `platform/native-runtime/src/control_workers.rs`, Game Hub maintenance controls | Real status/cancel/UI, blocked filesystem calls, broker replacement and TLS lifetime |

The IPC pool has 16 process-wide slots. The separate collector pool has one.
Neither a terminal result, cancelled request nor dropped broker refunds a live
thread. Actual Windows thread completion precedes join and refund. These source
limits are not measurements of whole-machine memory or storage.

## Source obligations still preventing closure

1. **Complete Windows writable-resource admission.** MD and native3 PS1 have
   bounded frontend memory-file routes. The selected N64 candidate remains
   unbuilt, unadmitted and unfinished; other core routes still have earlier OS
   scratch/save grants. Finish the remaining adapters and exact worker/package
   acknowledgement, or supply an equally explicit bounded host storage policy
   for those routes. Include permitted helper/driver cache behavior and distinguish
   copy reservations from actual physical writable storage. Do not enable the
   N64 candidate or call its source-copy receipt an approved package.
   Protocol-zero packages with no declared filesystem save or scratch-system
   storage now withhold OS scratch write grants in source, with a distinct
   opaque-state policy identity. Actual access/compatibility case 28 is pending;
   explicit legacy writers and helper/cache quotas remain open. Batch 009's
   passing memory-store/audio contracts do not establish that OS decision.
2. **Finish retained legacy-work handling.** Schema 15 preserves old debt as
   protocol zero; new prelaunch and confirmed-retirement recovery are authored.
   Missing container ownership or process acknowledgement cannot authorize
   deletion, profile adoption or signalling a stored PID. Complete the legacy
   migration/maintenance path with explicit retained/unsupported outcomes and
   bounded operator recovery, preserving uncertain originals. Existing retained
   counters do not by themselves establish a complete migration workflow.
   Five fixed retention counters and localized retry/preservation guidance are
   now authored, including explicit legacy/incomplete/other-platform outcomes.
   These complete the explanatory subpart, not the full operator recovery path.
3. **Close the remaining core-dependent Windows save/lifetime paths.** The
   selected N64 source still has unresolved semantic snapshot admission,
   producer/writer/resource and package closure. Keep these dependencies visible
   here and in item 5; shared audio/graphics dependencies also remain in item 6.
   Finish the required source contracts before claiming that Windows can admit
   every original v1 route. Package exclusion is not completion of the route.

4. **Finish contained worker dependency closure.** Actual batch022 staged loading
   reached ADVAPI32, COMBASE and USERENV but failed USER32 initialization in the
   earlier SDL worker. Source04cbd74 now builds a separate software actor without
   SDL implementation linkage; batch034 inventories only KERNEL32 imports and
   passes actual contained input/video/audio/pause/save/stop with a fixture core
   and private headless presentation peer. Both processes exit zero and the fresh
   profile retires. Desktop-worker and truncated-PE negative controls also pass.
   This is one scoped integration case, not production core/device acceptance.
   Complete production-core dependencies and trusted SDL presentation deployment,
   retain the separation in `engines/native`, SDL build selection and receipts;
   retain the required actor syscall/token/job boundaries. A successful direct
   worker with dummy SDL drivers, a passing standalone storage probe, or excluding
   an original route from the catalog does not complete this implementation.

These are implementation obligations; a selected parallel test batch does not
close them or replace the final acceptance pass. The historical detailed notes in `WINDOWS_NATIVE_COMPLETION.md` remain
the supporting record. This list does not remove any original system/platform
or claim that dependencies assigned another number have been completed.

Actual batch028 passed the two explicit AppContainer storage cases on 942cda6:
both grant modes, all seven required observations, actor exit zero and unchanged
source. This resolves those fixture observations only. It also exposed/fixed a
real registry value-write gap: package ALLOW masks must lose mutation rights;
medium labels or package DENY ACEs alone did not enforce the required behavior.
See `qa/evidence/antigravity/batch-028/`. Whole-profile quotas, core/helper/cache
writes, package decision case28 and all other original Windows cases remain due.

## Transition and verified completion

When the implementation obligations above are closed with exact source paths,
record the item-2 source handoff and proceed to item 3. Keep verified completion
pending until item 16 builds the approved broker/worker/helper/core artifacts
and executes every Windows case in `qa/windows/NATIVE_ACCEPTANCE.md`, including
the lifecycle, wire, audio, storage and ownership suites. Source materialization,
hashing and GitHub publication do not satisfy either missing implementation or
runtime verification.
