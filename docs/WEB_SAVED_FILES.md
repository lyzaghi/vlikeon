# WASM persistent files and restart saves

The shared worker, player and resource-assembly source now connect a bounded
private `/saves` filesystem, combined in-game/snapshot envelopes and guest
restart with recovery of prior files/state on rejection. This is authored
source. No core catalog advertises it yet; no advanced WASM binary, browser,
core, flush, decoder, compiler or test has run. All original work stays due.

## Explicit admission and I/O contract

A core may opt in only with `save_files` in `capabilities`, integer
`saved_files_protocol: 1`, `saved_files_io: "single_threaded_barrier_v1"` and
`wasm_threads: false`. Shared assembly refuses inconsistent policy or missing/
oversized format versions. The player passes an exact policy object; the
worker repeats shape/values/export checks and acknowledges the protocol. An
available bounded snapshot is required at load for rollback.

The adapted core must export `_om_web_flush_save_io`, returning exactly one
only after all guest/private pending savedata writes have reached `/saves`,
without concurrent producers or advancing the guest. Zero/error rejects the
save. Shared `_om_web_flush_files` then calls C `fflush(NULL)` and must return
zero. C stdio alone cannot flush core-private caches, guest async I/O, threads
or data the emulator has not written. There is no generic always-success core
barrier. Every adaptation must retain exact source/build provenance and prove
its I/O/restart semantics before release admission. Raw cores keep their formats.

The worker suspends its run timer during synchronous file capture; no
`retro_run` overlaps barriers or encoding. It restarts the timer only if it had
been running. Restore requires a paused/ready session. This source contract
does not establish hard native I/O preemption or actual PSP compatibility.

## Filesystem and codec

`saved-file-codec.js` writes OPSF0002 and reads OPSF0002 plus file-only OPSF0001.
It preserves empty directories and zero-byte files, bounds complete archives
at 256 MiB, regular files at 4096, materialized entries including parents at
8192, paths at 512 UTF-8 bytes/eight components and components at 255 bytes.
It validates exact extents, strict UTF-8, names, duplicates, parent conflicts,
aliases, ambiguous parent spellings, trailing bytes and materialized implicit-
parent budgets before import. Output order is UTF-8 byte order. Decode uses
views; payload reads/copies use at most 64 KiB per operation.

Names preserve Unicode spelling while refusing canonical/case aliases,
reserved devices, ambiguous dot/space endings, controls and Windows-reserved
punctuation. JS casing is not claimed equivalent to Windows invariant Unicode
casing. Wire shape alone does not prove native/root-subtree interoperability.

`saved-files.js` implements ordinary files/directories, bounded sparse zero
reads, block writes, checked truncation/seek, enumeration, rename and unlink.
It rejects symbolic/special files, unsafe paths, mmap and preallocation.
C errno/mode/seek constants come from the retained frontend. Logical archive
bytes/entries and allocated 64 KiB blocks have separate bounds. Namespace changes
validate descendant paths before mutation; truncated tails are zeroed.
Overwritten inodes are evicted by identity from Emscripten's lookup hash;
current wrappers may repeat that idempotent eviction. Open-stream data remains
owned by this backend until actual descriptor retirement.

Open-unlinked nodes consume quota until actual stream retirement, including
duplicated descriptors observed through FS streams. Capture includes only the
linked namespace. Import requires an empty mount without live handles; disposal
refuses live handles. These are application counters, not whole-browser/kernel
heap quotas or protection against another core/MEMFS directory exhausting
memory. The separate `/system` and `/game` mounts remain read-only. The writable
tree has no host path capability.

## Envelopes and compatibility

All integer fields are unsigned little-endian 32-bit values. Whole combined
payloads must fit the existing 256 MiB save limit.

| Kind | Magic | Layout after the 8-byte magic |
| --- | --- | --- |
| In-game progress | `OPWH0001` | memory length, file length, OPRM0001 RAM/RTC envelope, OPSF0002 archive |
| Emulator snapshot | `OPWF0001` | state length, file length, opaque state, OPSF0002 archive |

The memory envelope may contain zero RAM/RTC bytes for a file-only game. The
opaque state is the raw core format or OPDS0001 for disc-enabled cores. Both
outer lengths and the complete file archive are validated before mutation.
A snapshot includes selected disc/tray. In-game progress starts from the normal
content entry and does not promise a resumed execution point or selected disc.

Compatibility adds `.opwh0001` to the in-game version or `.opwf0001` after any
existing `.opds0001` snapshot suffix. The exact I/O policy enters configuration
hashes only for opted-in cores. `portable_in_game_format` remains null; no
native PSP/subtree converter is declared. No-file formats/hashes stay unchanged.

## Restore, recovery and UI ownership

The session validates incoming files and inner state, drains I/O and captures
prior opaque/disc state and files separately. It unloads/deinitializes the guest,
requires every saved-file descriptor closed, replaces/imports the mount,
reinstalls retained callbacks, initializes/reloads the same content and applies
memory or state. ROM/media and read-only firmware ownership remain across the
restart. AV/disc negotiation repeats; audio queues/input geometry are cleared.

Failure repeats stop/replace/boot with the previous files and opaque/disc state.
Failed recovery is fatal and requires reopening. Timed-out saves/restores retire
the worker so late mutation cannot silently complete behind a rejected UI
action. Earlier immutable committed saves remain available. This is process-
local recovery, not proof of power-loss durability or external FS atomicity.

History, startup candidates, autosave, suspension, disc-change saves, exit and
live export recognize file-only progress. Restore ownership prevents overlapping
save/export/disc/exit/resume; autosave skips it. The UI refreshes disc state after
restart, including in-game loads. Auto-resume requires the same runtime/pause
revision and a visible focused page; a newer lifecycle pause prevents resume.
Mobile OS deadlines can still interrupt work; background survival is unproven.

## Deferred consolidated qualification

The single final pass must cover codec byte/Unicode/path/count fixtures and
native wire comparisons without guessed portability; file/seek/truncate/rename
errors, sparse tails/maximum sizes, quotas/allocation failures, open/unlinked/
duplicated descriptors, inode cache and forbidden operations; actual core
barriers, unload/restart, every rollback stage and fatal recovery; disc plus
RAM/RTC/file consistency, unchanged raw formats, history/conflicts/export, UI
overlap/lifecycle/timeout and real memory/latency/device evidence.

Primary contracts: [Emscripten filesystem APIs](https://emscripten.org/docs/api_reference/advanced-apis.html)
and [FS rename/hash ownership](https://github.com/emscripten-core/emscripten/blob/main/src/lib/libfs.js).
The concrete toolchain/core build must be pinned/retained before admission.

The authored SNES candidate now connects upstream SRAM/enhancement-chip RAM
and RTC filenames to checked bounded VFS buffers, rejects duplicate save names
and supplies the private capture barrier. Its libretro memory regions remain
empty; the same combined envelopes therefore carry progress in files. A
separate declared result-completion bridge handles its cooperative fiber
exports. No SNES file/core operation was executed. See `WEB_SNES.md` for source
bounds, protocol details and the remaining SDK/game/recovery qualification.

The MD candidate now uses the same file envelopes with zero direct SRAM/RTC
regions. Its explicit barrier serializes every declared canonical Pak SRAM/
EEPROM image into the private mount, checking writer mode/length/offset and
full publication. This avoids treating one mapper's raw memory accessor as the
complete progress format. Native MD source uses its checked file-only
OPSF0002/OPNS0002 route at `save/MD`. The current 0.3 candidates add the explicit
bounded `ares-md-canonical-files-v1` in-game profile, descriptor and exact older
readers. No path has been executed or qualified. See `WEB_MEGA_DRIVE.md` and
`MEGA_DRIVE_PORTABLE_SAVES.md`.
