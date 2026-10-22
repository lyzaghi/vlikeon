# Native PS1 cards and disc-aware file snapshots

The current native3 source additionally routes file-managed cards through the
frontend-owned bounded memory table and withholds actor OS work save/scratch
writes. See `PS1_MEMORY_CARDS.md` and the fresh selected source at
`out/native-sources/item2-ps1-memory-cards-source`. This remains unbuilt and
unverified; no automatic native2/native3 compatibility exception is claimed.

The preceding native2 software PS1 candidate source connects a checked core I/O barrier,
combined RAM/RTC/files progress, file-backed multi-disc snapshots, recovery and
the existing per-game second-card choice. Exact committed/adapted source is
materialized at `out/native-cores/beetle-psx`. No compiler, linker, Make, core,
file API, process/sandbox, browser/UI, application or test has run. No native
binary or catalog entry has been built/admitted. All original requirements stay
active; this source work is not qualified PS1 compatibility or release readiness.

## Pinned core and writer lifecycle

The source recipe retains Beetle PSX revision
`df2b53b306ba52d2116f8e8cc93b9ee33d239be7`. Preparation uses bounded Git archive
extraction, not arbitrary checkout files. The adapted source/version is
`0.9.44.1+omniplay.native2`; the product/save SemVer is
`0.9.44+1.omniplay.native2`. Archive, source inventory, card-adapter and option-
policy hashes remain beside the prepared source. `--prepare-only` never invokes
Make/compiler/linker. A later explicit `--build-prepared` requires the recorded
source/platform/version and explicit tool paths. Exact toolchain provenance,
binary evidence and compiled license review remain required.

The software/interpreter profile keeps JIT/GPU/direct optical devices disabled.
Its upstream `retro_run` body is retained behind a wrapper. Native frontend
pause stops guest-frame production and calls `omniplay_pause_guest`, which
flushes/joins the upstream asynchronous card writer. The wrapper restarts the
writer lazily only when another guest frame runs. Read-only CD prefetch may
continue during pause; unload joins that reader before directory replacement.
This does not claim hard OS I/O preemption: a blocked join/read/write is subject
to the existing broker deadline and contained-process termination policy.

`omniplay_flush_io_checked` returns one only after complete card capture, zero
for storage failure and minus one for invalid core/path/device policy. It first
joins the writer, then re-reads every allocated nonempty card device, including
unused slots. The upstream periodic writer has a void result and can reset
dirty bookkeeping after an error; capture therefore cannot rely on that counter.
The default first card remains libretro SRAM. File-managed cards are written
as complete 128-KiB images to private temporary names, checked for full write,
flush/close and rename, then their dirty/delay bookkeeping is reset. Root,
device size, filename, duplicate index and range errors reject capture. There
is no always-success fallback or claim that a void stdio flush proves savedata.

## Worker and signed package contract

New package fields are defaulted for existing runtimes:

| Field | PS1 candidate value | Purpose |
| --- | --- | --- |
| `filesystem_save_directory` | `save/PSX` | Private archived subtree |
| `core_save_directory` | `save/PSX` | Exact directory returned to this core |
| `filesystem_save_memory` | `true` | Include RAM/RTC alongside files |
| `file_io_protocol` | `1` | Checked core return-value ABI |
| `pause_barrier` | `true` | Join guest card producers before capture |

Signed catalog admission validates this combination, exposed option defaults,
private-directory equality and explicit checked-card support. Worker hello has
separate persistent-file/checked-I/O/combined-progress versions; the host requires
them before preparation. Worker preparation verifies exports and available
rollback state, repeats memory/files policy and acknowledges actual support.
The package recipe requires matching core-adapter/policy/library hashes and
keeps distribution review requirements outstanding. Writing these declarations
does not execute a core or approve it for distribution.

The second-card choice is default-off and can now be exposed by this native
candidate as well as the Web candidate. No memory-card index/path, network,
renderer or CPU/JIT override is supplied through that UI. Actual game-specific
requirements, selected firmware/region and runtime compatibility still need
qualification. Source alone cannot establish that enabling the second slot is
correct for a particular game.

## Combined formats and restore

In-game native PS1 progress uses `OPNM0001`: eight magic bytes, little-endian
32-bit memory/file lengths, the existing OPRM0001 RAM/RTC envelope, then the
existing OPSF0002 archive. The whole payload must fit 256 MiB. The PS1 metadata
uses `beetle-psx-opnm1-cards1-native64`. It is a distinct nonportable declaration;
no native/Web converter or shared root/casing claim is made.

File-aware OPNS0002 snapshots now retain disc index/tray, opaque state and the
archive together. Raw/no-file OPNS0001 remains in place. Unsupported disc status
is strictly zero-count/index/closed; actual counts stay within eight and selected
indices must belong to the list. Native preparation compares an M3U's actual
count with its retained manifest. Transitions verify open/select/close outcomes
and disc count; same-state transitions avoid unnecessary tray cycling.

Restore validates the complete target archive/envelope and memory/disc metadata
before mutation. It captures previous core/disc state plus files, unloads and
deinitializes the guest, replaces the private tree, reinstalls callbacks and
loads the same retained content. A snapshot selects its disc before loading core
state and verifies the resulting tray/index/list; in-game progress boots normally
and then restores SRAM/RTC. On failure, the old files, core and disc are restored
through the same stop/replace/reinitialize path. Failed recovery is fatal.
Storage/size errors are recoverable only when the healthy guest was untouched
or recovery succeeded. No committed save is overwritten by a rejected capture.

Existing PSP file-only OPSF0002 and OPNS0002 formats remain their own route.
The worker refuses a file-only policy when the core exposes RAM/RTC that would
otherwise be omitted. Existing packages with default fields keep their prior
configuration hash shape; new I/O/memory/directory fields enter compatibility
only when declared. Every effective option remains part of snapshot identity.

The shared source adaptation always includes PS1 card storage in snapshots,
even when direct frontend SRAM restoration left the private `data_used` flag
false. Archive admission also checks implicit-parent materialization budgets,
ambiguous parent case spellings and Unicode controls before creating files.
Windows invariant casing remains distinct from the Web codec policy. Existing
reparse/path/size protections stay in place, but handle-relative race resistance,
live filesystem quotas, full crash recovery and cross-platform evidence remain
required. Directory staging/rename is process-local recovery, not a proven
multi-file power-loss transaction.

## UI ownership and remaining evidence

The native panel serializes disc/load operations against save/export/exit,
pause/resume and measurement admission; autosave skips them and stale polls
cannot overwrite their session state. Disc changes pause before capturing
progress. Snapshot loads and disc changes remain paused afterward. Previous
immutable save history and explicit close-without-a-new-save handling remain.
The actual native window's user input, OS lifecycle and deadlines need the final
pass; no UI/worker behavior has been observed yet.

The final consolidated pass must build the adapted core/worker/broker; check
export ABI/version/receipt mismatches; exercise writer start/pause/join/restart,
every card method/index, short write/flush/close/rename/storage failures and
unused cards; RAM plus file capture, all envelope/Unicode/path/quota boundaries,
open handles, reparse/race and staging faults; multi-disc/tray/state round trips
and every restart/recovery failure; existing PSP/raw formats, compatibility and
UI overlap; firmware/region, actual containment, memory/latency/input/audio/video,
device/game/store/license/release evidence. All other original scope stays due.
