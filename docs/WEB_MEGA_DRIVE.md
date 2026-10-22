# Mega Drive WASM and checked persistence — source only

`tools/build_ares_md_web.py --prepare-only` materializes the pinned ares
`9408cb43d4948fc3ea6e152a307a34348df3fe04` candidate and the pinned libretro
interface in a fresh owned subtree. Git archive selects explicit committed
paths. The recipe retains selected/adapted source, analysis/controller inputs,
source inventories, adapter/runtime/recipe/policy hashes and the original input
archive hash, then retires the temporary input archive/tree. It copies no
resource aggregator, UI/media assets, embedded BIOS image or automatic SVP
fallback into the selected product source. The new native preparation shares
this committed-input path. No compiler, core, browser or test was started.

## Execution and configuration

The candidate adds explicit wasm32 platform/architecture/ABI/endian detection;
it does not pretend to be an x86 host. Nall header-only definitions are selected.
The software 68000/Z80, PSG/FM, mapper and controller path retains the native
adapter's bounded video/audio behavior. Shared SH2 types have their existing
interpreter/no-recompiler adaptation and non-executable allocator. Only Mega
Drive models are loaded; retained 32X/CD shared types do not establish support
for those systems. TMSS is disabled. SVP content requires the optional selected
2-KiB `mega_drive_svp` vault file when actually needed; no file is supplied.

Cooperative libco execution uses the existing bounded single-worker Asyncify
fiber adapter, including stable handle re-derivation. Pending upstream thread
entrypoints are removed on replace/destroy to avoid stale callbacks. The shared
explicit result protocol uses a void outer export and a completion slot for
load, state-size, serialization, restoration and file barriers. Guest contexts
stay on their owner worker. Native threading, JIT and mandatory WASM SIMD are
not selected by this candidate. Actual SDK/ABI/exception/fiber behavior remains
unqualified. Emscripten documents separate continuation/C stacks and the need
to enable Asyncify; its C++ exception support must also be enabled explicitly
for this adapter's exception boundaries.
[Fiber API](https://emscripten.org/docs/api_reference/fiber.h.html),
[C++ exceptions](https://emscripten.org/docs/porting/exceptions.html).

Finite per-game region and automatic/three-/six-button controller choices are
shared by the native and Web candidate policies. Automatic controller choice
retains the pinned upstream incompatibility list. TMSS, execution backend,
threading and scale are not renderer options. The selected option values enter
existing save configuration identities. The Web candidate reports
`0.3.0+ares.9408cb43d494.omweb1`; the native candidate reports
`0.3.0+ares.9408cb43d494`. No earlier source preparation is an admitted binary.

## Canonical SRAM and EEPROM files

Both current candidates use one checked file path for progress. Upstream Pak
images keep the board's canonical SRAM byte ordering and EEPROM bytes. The
standard, linear and banked board save helpers now check writable Pak presence,
mode, exact memory/file size and complete resulting offsets, and propagate a
failed write to the frontend barrier. All declared persistent images are
captured; the current file profile exposes zero direct libretro SRAM/RTC bytes.
The native-only direct board accessors remain source for other explicit profiles.

Only `save.ram` and `save.eeprom` are admitted from the adapter's own declared
memory definitions, at most 1 MiB each. Existing files must have the exact
expected size and a complete read/close result before cartridge connection;
only absent optional files are accepted as a fresh game. No ROM-controlled
filename or arbitrary host path is used. The barrier checks the actual save
root, asks the guest to save into its Pak, verifies every declared image and
performs complete temporary writes with checked flush/close/publication. Failure
rejects a new capture. Source bounds do not prove device memory, physical quota,
power-loss durability or an atomic transaction across multiple live files.

The Web root is `/saves`, inside the shared bounded filesystem. The current
0.3 profile emits direct canonical OPSF0002 progress; OPWF0001 snapshots keep
the full core/file envelope. The native root is exactly `save/MD`, bound to its
captured directory; native progress uses the same explicit canonical profile
and snapshots retain OPNS0002. Restore stops/reloads the same game/firmware/
options and uses prior files/state for recovery. Games without persistent
memory do not create empty progress records. `MEGA_DRIVE_PORTABLE_SAVES.md`
specifies loaded-game descriptors, exact file-set preflight, one-way 0.2 readers
and cross-target admission. Portability is authored but not qualified; no actual
native/WASM bytes, game or device have been compared.

Windows publication uses a helper that accepts only the two fixed private
temporary/destination pairs and checks `MoveFileExW` with replacement enabled.
It does not remove the old destination before attempting replacement. Windows
documents replacement subject to ACL permissions and the write-through flag's
move/copy behavior; that alone is not a qualified filesystem crash guarantee.
[MoveFileExW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw).

## Native fibers and admission

The former upstream Windows fiber backend cannot derive a context. The new
backend retains an opaque handle while replacing its inactive underlying
Windows fiber. It validates owner thread/current fiber and known handles,
rejects deleting/re-deriving an active context, bounds guest count at 64 and
stack reservation requests at 256 KiB through 4 MiB, and requests floating-point
state switching. A returned guest entry terminates the process. The root stays
owned by the worker thread; contexts cannot migrate. These are authored checks,
not executed Windows/AppContainer evidence. The same backend is wired into the
native SNES recipe. Microsoft documents separate commit/reserve parameters,
explicit floating-point switching and converting a thread before scheduling
fibers.
[CreateFiberEx](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createfiberex),
[ConvertThreadToFiberEx](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-convertthreadtofiberex).

Native MD packaging now requires its matching checked-file/fiber adapter and
policy hashes, version, library hash and I/O/root declaration. Its actual pause
and checked-flush exports are required by the existing worker. No asynchronous
guest writer is selected; the pause boundary is between cooperative core calls.
The selected new native file/state identities reject incompatible old formats.
This does not qualify current Windows containment, reparse handling or storage
pressure. Other desktop/mobile native backends remain original requirements.

## State, later build and evidence

Both selected MD projects force synchronized chip state for non-serializable
fiber backends. Raw coroutine stack states are rejected before loading. Primitive
reads cannot grow past input capacity, serializer writes are bounded at 64 MiB,
version comparison uses the full fixed sixteen-byte field and invalid reads are
reported to the public adapter. This still requires a chip-by-chip semantic,
allocation, scheduler-termination and rollback audit with the actual build.

The explicit `--build-prepared` Web path requires matching retained source,
runtime, recipe, policy and export inventories plus Emscripten C/C++ tool paths.
Only a real build can emit module/WASM metadata. Candidate distribution remains
false and verification remains not run. No prepared-source record is inserted
into an installed catalog. Exact toolchain identity, compiled dependency notices,
artifact content, full corresponding source and release review remain required.

The final consolidated phase must cover compiler/linker/ABI, fiber count/stack
exhaustion/re-derivation and stale entrypoints, exceptions/result completion,
ROM analysis and mapper geometry, all RAM modes/EEPROM/SVP/regions/controllers,
corrupt/truncated/unsynchronized states, every I/O/reload/recovery failure,
native roots/containment/reparse/quota/crash behavior, history/export/sync and
actual desktop/Android/iOS/browser/device CPU/memory/audio/video/input evidence.
No decoder, filesystem/core/firmware/Windows API, compiler/build, browser/device
or test was executed. All original remaining product requirements stay active.

See `MEGA_DRIVE_RUNTIME.md`, `WEB_SNES.md`, `WEB_SAVED_FILES.md` and
`GAME_CORE_OPTIONS.md`.
