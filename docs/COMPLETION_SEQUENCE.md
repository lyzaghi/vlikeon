# User-ordered completion sequence

The user requested sequential work on items 1 through 16 and a report when each
item is finished. Source authoring and the final comprehensive qualification
remain separate; the newer concurrent-test instruction below supersedes the
earlier deferral for explicitly released batches. Source work, executed verification and release
approval are tracked separately. A source snapshot alone cannot close an item.

The owner assigns Antigravity test execution only. Codex and the owner retain
result review, diagnosis, fix decisions and source edits. The owner now requests
concurrent work: Antigravity may run exact released test batches on frozen
copies while Codex writes source; full acceptance remains item 16. See
`QA_EXECUTION_POLICY.md`.

Current source item: **2 — Windows native runtime lifecycle and containment**.
The current substep investigates real AppContainer storage and loader behavior
after adding retained-work reasons, operator guidance and bounded broker workers.
Selected concurrent Antigravity batches have compiled the broker/worker and
passed the dedicated 13 lifecycle and 19 wire cases on real Windows processes.
These use fixture DLLs/dummy drivers and do not replace the full production
broker/process/core/save/device qualification required by item 16.
The recovery worker also retains a process-wide single slot across broker
replacement. `WINDOWS_ITEM2_CLOSURE.md` is the remaining source-closure list;
no item-2 completion or transition to item 3 has been recorded.
Batch034 on source04cbd74 now passes one actual device-free actor media case
under production Win32k/AppContainer/job launch with a fixture core and headless
presentation peer. Static actor imports are KERNEL32 only. Batch035 rebuilds
the direct SDL mode on the same source and passes its 35 existing regression
cases. Production helper/core/device and the remaining closure obligations stay
open; these results do not complete item 2.
The five reason counters do not grant deletion authority or complete legacy
migration. The three retained-work regression fixtures passed in batch 005;
actual batch028 now passes the two assigned storage probes; this establishes
only their explicit observations, not whole-core storage compatibility.
Earlier N64 source work adds bounded snapshot event queues, PIF pointer
preflight, format-selected DD ID reads, TLB field/lookup admission and finite
last-page map/unmap walks with an interpreter random-index guard.
Transfer Pak selector admission, complete GB storage/packet extents, full-width
high RAM banks, rumble bank separation and bounded camera startup/output are
also authored. Bounded MBC3 time/halt/carry and persistent latch source now has
a separate gl3/1.10 snapshot candidate with legacy imports. RTC battery binding,
staged boot/offline time and checked RAM/RTC retirement are now source-authored.
Actual persistence/device timing and remaining upstream cartridge implementations
still require completion/qualification.
Flash/SI complete-range memory checks, DD C2 sector/zone bounds, active PIF
opcode admission and known snapshot device selectors are also source-authored;
actual PI/CPU/graphics/device and all original qualification remain open.
PI adjusted RDRAM admission, per-page framebuffer readback, bounded descriptor/
mapping/CPU-wrapper source and the six-slot graphics cache fix are now authored.
Direct SRAM/ROM/DD complete-range DMA, physical short-SRAM-write notifications,
checked source words and nonzero-only code invalidation are now source-authored.
Fixed exact dirty-byte coverage and bounded CPU color conversion/upload geometry
are now source-authored. Image-loader admission, whole GPU resource accounting
and submission/driver failure acknowledgement remain work; actual CPU/core/
graphics/device qualification remains due.
ROM boot-extent/signature/word/size admission, byte-wise format conversion and
bounded header-name diagnostics are now authored. Full frontend loader ordering,
metadata and actual ROM/boot/core compatibility qualification remain due.
Frontend early ROM/path admission, companion allocation checks, staged header
query and retained ROM-open/failed-close ownership are now authored. Complete
setup/graphics/audio/subsystem lifecycle and actual qualification remain due.
Checked partial audio allocation/cleanup and explicit frontend readiness are
now authored. Full AI/sample/resampler/output/backpressure, audio-device and
graphics-context failure closure remain unfinished and unqualified.
Finite audio output buffering, bounded zero/partial consumer handling and
root backpressure are now authored. Producer resampler/AI bounds, snapshot
audio transitions and actual stalled-device/worker qualification remain due.
Audio callback borrow now gates root reentry/reset/deinit/callback replacement
and audio retirement gates final owner/table detach. Actual export retry and
CoreShutdown/context/device qualification and producer bounds remain due.
AI complete-RAM publication and immutable private backend sample copies are
now authored. AI duration/delayed-carry/snapshot timing and resampler output
admission remain source work; real audio/driver/worker qualification remains due.
Wide AI duration admission and delayed-carry checks are now authored, preserving
admitted arithmetic order. FIFO failure rollback/overflow, zero events, snapshot
timing and real scheduling compatibility remain open, alongside resampler bounds.
Non-Apple sinc pre-write phase/output admission and bounded input subdivision
are now authored. Actual C/SIMD phase/sample equivalence, filter internals,
Apple accumulation and low-rate queue pressure remain unfinished/unqualified.
Sinc filter-table/tap allocation bounds and per-instance processor capture are
now authored, with an actual pinned-sinc/allocator qualification target. All
new cases remain unexecuted; real ISA/sample/device qualification remains due.
AI first-slot start and pending-slot promotion now stage descriptor/status
publication until admission succeeds. Void event insertion receipts, fatal
CPU stop, snapshot/zero/full-FIFO semantics and actual timing remain open.
AI checked pool insertion and failure-stop source are now authored; actual
event delivery/CPU-return/save retirement and other event producers remain due.
Explicit post-stop peripheral save recovery now has source admission distinct
from failed CPU readiness. Actual guest unwind/worker capture/crash persistence
and all original Windows completion requirements remain open.
Common native graphics callback release now retains pre-invocation failures,
latches unknown callback exceptions and prevents DLL unload on failed release.
Real callback/driver/process and final-save qualification remain due.
Native context reset now also gates callback reentry and publishes readiness
only after return; unknown reset failure prevents core/DLL unload. Actual
reset/release/driver and process-exit acceptance remains unexecuted.
Native audio ring receipts now report copied frames, and N64 offers match the
worker batch cap; actual partial/zero admission, latency and device qualification
remain open.
Native consumer slots/priming/underruns now commit only after a complete sink
receipt; rejection latches failure and clears SDL's potentially partial queue.
Device-control receipts, deliberately discarded paused producer flushes,
atomic presentation batch admission and same-epoch media-clear generations are
now source-authored. Actual production-ring cases and full device/core/transport
failure acceptance remain unexecuted in `qa/native-audio` and Windows case 24.
Checked guest-pause protocol 1 is now authored across worker, selected native
adapters, build receipts, catalog admission and Windows/Linux caller identity.
Unconfirmed pause and unload/deinit callbacks prevent replay or DLL destruction.
Windows fatal actor termination now bypasses DLL detach through the dedicated
owned-process primitive; actual worker/DLL/pipe/producer and detach-witness
fixture source is in `qa/native-lifecycle`. Case 25 and all real process/core/
save/driver qualification remain unexecuted. Legacy barrier packages need rebuild.
Complete semantic snapshot/resource
admission, remaining writer closure and actual Windows qualification are open.
Item 1's authored integration is retained for actual item-16 qualification;
verified completion has not been claimed.

| Item | Scope | Source state | Verified completion |
| --- | --- | --- | --- |
| 1 | Desktop Chromium, Game Hub, profiles, permissions and build/update integration | Source authored; current desktop overlay and build/upgrade recipes retained | Pending final pass; remains open |
| 2 | Windows native runtime lifecycle and containment | In progress; current source audit | Pending final pass |
| 3 | Linux native runtime lifecycle and containment | Pending sequential completion audit | Pending final pass |
| 4 | macOS native runtime lifecycle and containment | Pending sequential completion audit | Pending final pass |
| 5 | v1 retro systems and core adapters | Pending sequential completion audit | Pending final pass |
| 6 | Graphics, audio and input | Pending sequential completion audit | Pending final pass |
| 7 | Saves, library and storage | Pending sequential completion audit | Pending final pass |
| 8 | Remote Play and installed PC/AAA bridge | Pending sequential completion audit | Pending final pass |
| 9 | Android product | Pending sequential completion audit | Pending final pass |
| 10 | iOS/iPadOS product | Pending sequential completion audit | Pending final pass |
| 11 | Web/PWA product | Pending sequential completion audit | Pending final pass |
| 12 | Accounts and encrypted save synchronization | Pending sequential completion audit | Pending final pass |
| 13 | Performance, routing and optional assistance | Pending sequential completion audit | Pending final pass |
| 14 | Install, repair, uninstall and updates | Pending sequential completion audit | Pending final pass |
| 15 | Security, licensing and release preparation | Pending sequential completion audit | Pending final pass |
| 16 | Consolidated build, integration, device and fault verification | Deferred until all source authoring is complete | Not started |

Existing implementation in every item is preserved. This table records the
requested work order and does not reset or remove earlier implementation.

## Item 1 current source changes

- Shared exact live Game Hub document admission for browser and native commands.
- Lazy trusted-storage profile identity with rejection of corrupt/nil records.
- Bounded authentication/replay admission and generation-aware native ports.
- Document-owned native sessions, selected file grants, imports, transfer
  directions, media and input handles across a shared profile broker.
- Cleanup handling for late native session/preparation creation results.
- Obsolete generated component-resource retirement during source staging.
- Windows/Linux/macOS development/release GN selection.
- Frozen original/overlay/resource/recipe inventories and an explicit later
  full Chromium dependency/build procedure with resumable phase journal.
- Derived component-key/allowlist/native-origin agreement and explicit upstream
  snapshot migration admission.
- Three-sided source-only review for an explicitly newer upstream commit,
  preserving current originals/product edits and active revision selection.
- Authored document-boundary unit cases and a real desktop acceptance matrix,
  both reserved for the final consolidated pass.

See `DESKTOP_BROWSER_BUILD.md` for concrete source/verification completion gates.
No build, browser or test has been executed for these changes.

## Item 1 requirement-to-source trace

| Requirement | Authored source | Still required to verify |
| --- | --- | --- |
| Minimal revision-bound Chromium integration | `browser/upstream.lock.json`, `browser/patches/series.json`, `tools/generate_browser_patch.py` | Exact target builds and upstream-upgrade rehearsal |
| Normal/private profiles and product isolation | Component loader/allowlist/product-path patches, `apps/game-hub/manifest.json` | Real new-tab, private-window and two-profile behavior |
| Live-document and transient capability authority | `component-context.js`, `component-ownership.js`, `background.js`, independent broker profile/origin checks | Real multi-tab/worker/native-message/fault cases |
| Ordinary browsing/dashboard | `browser-service.js`, `browser-dashboard.js`, native Chromium controls | Live routing, tab changes, metadata privacy and native-host failure behavior |
| Exact source staging and target build procedure | `stage_browser_integration.py`, `prepare_desktop_browser.py`, `build_desktop_browser.py`, three desktop GN pairs | Actual compiler/SDK, dependency, artifact and rebuild comparison |
| Maintained upstream migration | `prepare_chromium_upgrade.py`, unchanged active pin and three-sided review output contract | Candidate adaptation, actual candidate build and security freshness review |
| Final evidence plan | `qa/browser/DESKTOP_ACCEPTANCE.md`, authored document/context unit sources | Execute the full matrix in item 16 |

Item 1 remains open until that final evidence exists. Core artifacts, packaging,
performance and release policy are covered by their original later items and
must still be connected to the final desktop build.

## Item 2 current source work

- Creation-time Job Object association for both Windows actor and presentation
  helper, verified before journal identity publication and resume.
- Serialized AppContainer grant/launch/retirement lifecycle with no new grants
  after launch.
- External confirmed-retirement proof, separate collector lease and retiring
  ledger admission for resumable partial work deletion.
- Bounded rotating startup recovery cursor.
- Creation-lease/owner-lease handoff, immutable schema-15 Windows pre-launch
  reservation marker and bounded ledger-only recovery without legacy promotion.
- Authored actual Windows sharing/partial-creation/legacy-retention cases and a
  schema-14 to schema-15 debt-preservation case, reserved for item 16.
- Fresh default AppContainer filesystem sealing and exact actor token/low
  integrity/no-write-up admission, with an authored real-actor final-pass probe.
- Exact-token single-use private registry lookup, matching effective SID grant,
  bounded link-refusing preflight and registry/filesystem sealing before process
  publication/resume; expanded actual-actor probe-v2 and caller-context cases.
- Windows Settings status/inspect/recover/stop flow with typed reservation
  totals, a single off-control-thread startup/manual owner, cooperative
  cancellation and document-owned job IDs.
- Bounded memory-file ABI/store, common save capture/atomic staged replacement,
  and an exact Windows MD core/worker/package route without actor OS save/scratch
  write grants; standalone contract and actual core/OS acceptance sources.
- Native3 PS1 FrontIO/async/checked-card memory publication, read-failure
  preservation, pre-swap writer drain and exact no-OS-write package admission;
  real C bridge/store qualification sources, all unexecuted.
- N64 scratch-only C stdio/C++ stream/gzip adapters with atomic complete-image
  publication, page/stream/decoded-byte limits and unresolved-close retention;
  actual adapter/store/vendor-codec qualification source, all unexecuted. N64
  core/cache integration and package/OS-write admission remain in progress.
- Separate pinned N64 gl2 configuration/database/custom-settings and shader
  integration source; bounded complete pre-GL cache decoding, checked coroutine
  init/deinit and owned Windows root/guest fiber retirement. Texture/file/dump/
  peripheral write closure, exact package admission and real qualification
  remain due; the candidate is not enabled or built.
- Actual N64 texture-memory gzip and file-index cache source now use bounded
  staged records, checked nested expansion/index/offsets, snapshot-backed pending
  data and complete-image publication. Actual adapted-cache and independent
  format/fault fixture sources are authored. Dumps/other core/peripheral writes,
  real shared/thread pool bounds and full package/OS qualification remain due.
- N64's actual shared/workspace pool now has checked 4096/one-lane admission,
  finite live/transient payload, staged allocation and owned borrow/retirement
  scopes. Filter init/output/OOM acknowledgement and standalone deposterize are
  authored; cache cases now use that real pool. Common worker source exits its
  actor before unload/context destruction on refused table uninstall. Pipeline
  temporaries, remaining writers and actual full core/OS evidence remain due.
- Selected N64 texture PNG dumps now use a C encoder with bounded source/stride,
  8-MiB codec allocation and checked scratch publication. Texture diagnostics
  use finite scratch logs; native messages use literal-format callbacks. Actual
  encoder/store/codec and prepared-diagnostic qualification sources are authored,
  all unexecuted. Other state/peripheral writes and original final scope remain.

- Selected N64 persistent file-storage/utility and Transfer Pak RAM source now
  uses a separate save namespace, exact staged read/close admission, dirty full-
  image retry and readonly legacy import. Actual storage/store fault/quota case
  sources are authored, not executed. DD/state/producer acknowledgement and the
  full package/OS/final scope remain open.

- Selected 64DD source now stages bounded MAME/SDK/D64 admission, expansion
  length, real geometry/sector ranges, owned memory saves and readonly legacy
  import. Failed close retains its descriptor and blocks deinit/uninstall;
  unstarted IPL rejection does not manufacture a default save. Actual-source
  synthetic/fault cases are authored, not executed; producer/capture/package/
  OS and every original final requirement remain due.

- Selected N64 source now has thread/fiber-owned real prepare/paused guest
  checkpoints, checked snapshots/peripheral capture and retained dirty-close
  ownership. Snapshot mutex/early-rejection lifetime is explicit. The common
  worker requires actual table detach before restoring memory files. Actual
  owner/storage and Windows-fiber qualification sources are authored, not run;
  full core/package/OS/restore/device verification and remaining scope stay open.

Item 2 is still in progress. `WINDOWS_NATIVE_COMPLETION.md` lists concrete source
gaps and later qualification obligations. No Windows runtime operation or test
has run.
