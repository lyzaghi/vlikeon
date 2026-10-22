# PS1 software WASM candidate source

`tools/build_beetle_psx_web.py --prepare-only` has materialized the exact pinned
Beetle PSX `df2b53b306ba52d2116f8e8cc93b9ee33d239be7` source and authored
adaptations in `out/web-cores/beetle-psx`. Git archive supplies committed files
without copying arbitrary checkout/untracked contents. Extraction refuses links,
special files, escaping paths and source-count/byte excess. The retained receipt
records the archive, adapted source inventory, frontend and adapter hashes.
No compiler, linker, Make, core, application, browser or test has run.

## Candidate profile

The explicit build recipe selects the software interpreter, disables Lightrec,
the threaded recompiler, native threading, GPU and direct optical devices, and
uses the single-threaded CD interface. It disables upstream Emscripten whole-
disc precache and the mandatory SIMD flag. It retains libretro-common VFS/string/
stream sources in the standalone archive, rather than assuming a RetroArch
frontend provides their symbols. CHD and PBP source backends remain selected;
their real wasm32/large-offset/codec/device behavior is not qualified here.

The shared native positional-read or bounded OPFS media route provides actual
paths for CUE/M3U/CHD/ISO/PBP and raw track content. No full-disc cache or extra
JavaScript ROM copy is requested by this profile. A baseline scalar profile is
not a performance promise; SIMD/GPU, actual latency, memory and energy remain
part of the original product requirements.

Candidate metadata declares the shared media, firmware, disc-state and saved-
file protocols, matching source/runtime versions and adapter hashes. It is
emitted only by explicit build mode after a real binary exists, with
`distribution_approved: false` and `verification: not_run`. No missing binary or
prepared source entry is inserted into the installed catalog. Development
catalogs therefore remain empty until real core builds/admission occur.

## Memory-card barrier

The upstream asynchronous card-writer implementation is replaced in the adapted
tree with a synchronous callback implementation and compile-time no-thread
guards. No eventcount, native writer thread or atomic pending-card queue remains
in that translation unit. The ordinary upstream periodic path can still hide
short-write errors; it does not constitute the capture barrier.

The appended core-specific `_om_web_flush_save_io` reads every active nonempty
card device. The first card remains libretro SRAM under the upstream default
method, so OPRM0001 captures it alongside files. File-managed cards are written
in complete 128-KiB images to private temporary names, checked for full write,
flush and close, then renamed. Invalid device sizes/indices, colliding active
card indices, wrong save roots and file failures reject the barrier. Dirty/save-
delay bookkeeping is reset only for successfully written cards. A failed
barrier does not authorize a new committed save; it is not a multi-file power-
loss transaction or a kernel storage quota.

The shared OPWH0001/OPWF0001 envelopes then include all linked files plus SRAM/
RTC or the core/disc state. Restore uses the shared closed-handle guest restart
and prior-files/state rollback. Source staging and temporary-file rename do
not themselves establish actual cache/hash/stdio/restart correctness; final
qualification must prove those behaviors with the exact SDK/core build.

Upstream second-card enablement, controller selection and per-game defaults
remain upstream choices. This recipe does not silently enable a second card
for games known to require it disabled. Finite per-game boot/region/second-card choice UI is now authored; see
`GAME_CORE_OPTIONS.md`. Actual choice/controller/mobile/game evidence remains due.

## Firmware, licensing and explicit later build

The manifest policy requires a user-selected regional PS1 BIOS group through
the existing vault. No manufacturer firmware or game asset is supplied.
The selected BIOS must still match the actual game/region; group admission
alone cannot establish that. Upstream original MIT PCSX-Redux OpenBIOS remains
separately identified in source, together with its notice. The full adapted
source and root GPL notice are retained; the compiled dependency graph, exact
notices, corresponding-source distribution and permission review remain due.

After all product authoring is complete, the consolidated pass may invoke the
recipe with `--build-prepared` and explicit Make/Emscripten compiler/archive
paths. That path requires the exact retained source inventory and frontend
export contract before building. Preparation never invokes those tools.
Exact SDK/toolchain identity and repeatable artifact evidence remain required;
an authoring receipt cannot substitute for a verified build.

The final pass must cover the actual scalar wasm32 build, strict media/firmware
and region handling, CUE/M3U/CHD/ISO/PBP/large seeks, disc/tray/state changes,
both card methods and every active card/index, periodic/barrier failures,
temporary rename/open-unlinked/cache behavior, unload/reinitialize/rollback,
per-game compatibility, memory/CPU/input/audio/video/lifecycle and real desktop/
Android/iOS/browser/store evidence. All other original full-product scope stays
active. See `WEB_SAVED_FILES.md`, `WEB_DISC_STATE.md` and `WEB_FIRMWARE.md`.

Shared source adaptation now also makes card bytes unconditional in snapshots, so direct frontend SRAM restoration cannot be omitted when the private data-used flag is still false. Native and Web recipes retain the change; no core state or parser was executed. The Web prepared source/receipt includes its exact modified file.
