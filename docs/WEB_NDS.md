# Nintendo DS software WASM candidate: authored source, execution pending

`tools/build_melonds_web.py --prepare-only` materializes melonDS DS 1.4.0
commit `f394adbacb5722ee97c1b37c8064da9a25818310` and all ten dependencies in
`security/melonds-source.lock.json` through committed Git archives. The melonDS
backend is separately pinned to `16f127dbb587a73371827ce79689c5d237c4b59b`.
Working-tree changes and untracked source are not build inputs. Preparation
retains adapted trees, source/adapter/runtime/recipe/policy inventories and the
source-lock copy in `out/web-cores/melonds-ds`. Temporary upstream archives are
hashed and retired. This step does not configure, compile, link or execute an
emulator, application, browser or test.

The separate `--build-prepared` path requires explicit CMake, Emscripten C/C++
compiler and toolchain paths, intact prepared inputs and a fresh build directory.
Its candidate version is `1.4.0+omniplay.web1`. It produces neither a release
approval nor compatibility evidence. The candidate manifest keeps
`distribution_approved: false` and `verification: not_run`; executable catalogs
remain empty until actual build/admission work is completed.

## One synchronous Worker

The frontend, interpreter and software renderer execute in one dedicated
Worker. JIT, GPU rendering, pthreads, dynamic loading, networking/libslirp,
Tracy, debugger and upstream test targets are disabled. The frontend's existing
task queue is explicitly initialized in cooperative mode. The adaptation removes
the otherwise mandatory thread discovery and does not compile libretro-common's
thread implementation. Software rendering is fixed to its synchronous branch.

The frontend's local lock class checks ownership on this single execution
sequence. Recursive acquisition, unmatched release or destruction while held
terminates the guest. Its semaphore implementation has bounded counters;
nonblocking/timed attempts return the current availability, and a blocking wait
without a possible synchronous producer terminates the guest. These adapters
do not implement background threads or silently report a successful wait.
Upstream chip-specific synchronization, stack sizes, callbacks, exceptions and
shutdown still need actual SDK and fault qualification.

The shared void outer-call/result-completion protocol is used for content load
and snapshots. No Asyncify fibers are needed by this candidate. The shared
worker rejects the missing completion of a declared result protocol and fences
further guest calls through the existing terminal-state handling.

## Cartridge ownership and file-backed media

The candidate requires file-backed content even below 256 MiB. Player selection,
runtime acknowledgement and actual `need_fullpath` observation enforce that
requirement. PWA content uses the existing verified OPFS staging session;
mobile/native-backed media uses its bounded leased range reader. Both mount the
same read-only `/game` filesystem. There is one declared DS cartridge, with no
subsystem/slot-2 boot input. The input reader accepts 4 KiB through 512 MiB,
checks the exact length, reads at most 64 KiB at a time and rejects a short or
changed file. No whole-ROM JavaScript view is requested by the core adapter.

The adapted `GameInfo` transfers its correctly typed allocation into upstream's
owning cartridge parser. It retains a 4-KiB aligned header for later frontend
metadata access. Upstream console reset destroys and reconstructs the console;
the adapter reopens the same verified media lease when a new cartridge is
needed. A stale lease/read failure therefore rejects reconstruction. Trimmed
ROMs can still cause upstream's power-of-two padding allocation. WASM heap,
padding, chip memory, graphics/audio and save copies have additional costs;
this is not a one-allocation or constant-RSS claim. Actual iOS/Android memory
limits and largest-ROM behavior remain unqualified.

ARM7/ARM9 input regions are checked with subtraction before parsing, including
the ARM9 words read by the frontend's secure-area probe. These finite guards do
not replace a complete decoder/security audit of the pinned emulator.

## Firmware, controls and readiness

DS mode, direct boot, selected native BIOS/firmware, one screen layout, pointer
input and software execution are fixed inside the adapted configuration reader.
The fixed values cannot be overridden by per-game renderer configuration.
ARM7 BIOS, ARM9 BIOS and firmware are selected through the existing private
firmware vault and mounted read-only in `/system`. Failure to read selected
system files throws instead of silently substituting FreeBIOS/generated
firmware. Firmware flush tasks are omitted because the selected system inputs
are immutable; session firmware changes remain in memory and are not advertised
as durable cartridge progress. No manufacturer BIOS or firmware is bundled.

The source has standard Retropad controls and the existing pointer protocol 3
stylus path. Localized per-game choices expose four arrangements of both
screens and the core's five audio interpolation choices. Microphone capture,
networking, host-folder/SD mirroring, DSi/NAND, GBA slot-2 content and expanded
peripherals are not claimed by this candidate. They remain separate adapters or
qualification work wherever the original product specification requires them.

A manifest can now declare `minimum_system_ram_bytes` up to 64 MiB. Bundle
admission validates the value; renderer, runtime and worker pass and acknowledge
it. After every initial load or internal reload, the worker verifies the observed
system-memory pointer/length against the current WASM heap and its declared
floor. This NDS candidate requires 4 MiB. The upstream error display reports zero
system RAM and cannot be admitted as a ready game session. This observation is
not proof that a title boots, runs correctly or meets a performance target.

## Progress and snapshots

Cartridge save memory uses the upstream libretro SRAM interface. Adapted save
updates bound allocation at 32 MiB and validate offsets with subtractive wrap
handling. Save capture remains immutable through the existing common history,
hash, compatibility and import/export paths. The initial Web progress format
is explicitly distinct from the existing native NDS memory envelope; no
cross-platform conversion or native/Web compatibility is invented.

Snapshot input/output is bounded at 64 MiB. Primitive copies use subtractive
capacity checks; growth uses a wider intermediate and a bounded allocation.
Section discovery checks a complete chain of at most 4096 sections, minimum
header/length bounds, forward progress and duplicate requested identifiers.
Version reads tolerate rejected short buffers, and section-header writes check
their range. Chip-specific semantic fields, variable allocations, mutation on
failure, snapshot completion and actual rollback/restore behavior still need
the final consolidated fault/security/compatibility verification. These source
guards are not a qualified hostile-state parser result.

## Distribution and remaining evidence

The pinned upstream GPL notices, original BSD FreeBIOS replacement notice,
third-party source and Open Sans notices are retained. The generated combined
upstream attribution must accompany a compiled candidate. Final SPDX aggregation,
transitive content/SBOM review, artifact inventories, system SDK choices and
distribution approval remain required.

No SDK configure/compiler/linker, NDS core, cartridge/state/firmware API, browser,
Worker, pointer/audio/device, benchmark, store/release operation or test has
been executed for this candidate. Compatibility, recovery, memory/energy/timing,
largest-ROM handling, input geometry and the complete original product scope
remain pending. See `NDS_RUNTIME.md`, `WEB_FIRMWARE.md`, `GAME_CORE_OPTIONS.md`
and `requirements.json`.
