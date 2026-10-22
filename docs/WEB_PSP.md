# PSP Web pthread/software candidate: source authoring

The Web producer uses PPSSPP parent
`a5275067b62433375bbde12921e96329a795dff0`, twelve selected parent dependency
gitlinks plus the explicit FFmpeg 9.0.2 override in `PSP_DECODER_MAINTENANCE.md`,
and Emscripten 6.0.13 source
`e13ee988ce3d87a738dd8f3661d02223308c412a`. Its separate identity is
`ppsspp-web`, version `1.20.4+omniplay.websw3.a5275067b624`. Source preparation
and an explicit later build recipe are separate operations.

No compiler/SDK installation, configure script, makefile, compiled core, browser,
game, firmware, operating-system API, mobile device or test has been run. A
prepared source inventory is not an artifact approval or executable-compatibility
claim. The installed core catalogs remain empty development catalogs.

The historical `websw1` source assembly remains at `out/web-cores/ppsspp` with its
original inventories and partial-input history. Current product source uses
`out/web-cores/ppsspp-v3-committed-atlas`, the current decoder identity and its retained source/
host/recipe inventories. The current build command requires a separately compiled
current decoder receipt. Historical source-only receipts cannot establish an
eligible decoder binary. No failed archive is treated as a compiler/test result.

## Immutable inputs and build recipe

`tools/build_ppsspp_web.py --prepare-only` materializes selected committed source
through the bounded Git-archive helper. It does not copy arbitrary current
checkout modifications. It excludes upstream platform frontends, manufacturer
content and prebuilt Vulkan/FFmpeg platform libraries from the selected source.
The old FFmpeg fork contains large prebuilt platform trees; only its explicit
source/build/notice directories are selected. All selected gitlink dependencies
are separately materialized at their locked revisions. No submodule updater,
configure script or package manager runs during preparation.

The prepared project retains per-archive hashes, adapted source-file hashes,
exact SDK worker/callback source hashes, the complete shared host source snapshot,
adapter/recipe/policy/lock inventories and original notices. The worker source
contracts are retained from the exact pinned Emscripten tree. This does not
install or qualify its LLVM, Binaryen, runtime cache, Node/Python or other binary
toolchain dependencies. `tools/fetch_emscripten_sources.py` provides the explicit
source-only fetch path and preserves a different existing checkout. It never
bootstraps or installs a binary SDK. The contract snapshot reads exact committed
Git objects individually, so an unrelated missing blob in a sparse SDK cache
does not silently widen the source selection or trigger a full SDK archive.

The explicit `--build-prepared` path requires actual make, shell, Emscripten C/C++/
archive/ranlib/nm tool paths and a generated open-font atlas receipt. It requires
the selected compiler's SDK checkout/relevant source files to match the pinned
source contract, and refuses changed prepared source, recipes, host snapshot or
policies. It builds in a fresh copy, retaining its complete adapted build source.
The source recipe does not download a compiler or codec during compilation.

The multimedia producer in `PSP_DECODER_MAINTENANCE.md` builds finite decoder,
demuxer/parser and supporting libraries from the locked official FFmpeg 9.0.2
source. Its separately compiled receipt is required by the core producer. Network/
protocols, encoders, hardware codecs, program builds and external autodetection
are disabled in that authored policy. One-thread decoder contexts have explicit
pixel/sample/probe bounds, padded packets and custom-I/O failure ownership. None
of these source declarations proves actual codec compatibility, safety, patent/
license clearance or performance. The old parent decoder gitlink remains in its
original source graph for provenance and does not select the current product's
decoder implementation.

## Execution and ownership

The selected CPU is the IR interpreter, with no native CPU or vertex JIT.
Rendering uses the software context at one-times internal resolution. The factory
cannot select an OpenGL/Vulkan/native context. Other upstream graphics source is
still a build input and remains part of the compiled-inventory review.

The profile explicitly prewarms sixteen SDK pthread Workers and requests strict
pool exhaustion. The upstream task manager is fixed to one compute and two I/O
workers; loader, savedata, memory-info and remaining core tasks share that finite
SDK pool. Unplanned/excess thread creation is a failure, not a silent increase.
The root actor and actual UI-owned thread handles use `WEB_THREADS.md`. A child
pre-JavaScript hook also denies nested Worker construction through its global
surface. These source restrictions still need real SDK fault/termination evidence
and do not constitute a sandbox against arbitrary malicious trusted glue.

The current pinned SDK uses integer control command IDs 1 through 9. The relay's
bounded payload contract admits those actual IDs in addition to its older bounded
string controls. It still rejects arbitrary platform handles and oversized data.
The statically compiled typed callback frontend supplies environment, video,
audio, input and rumble addresses, avoiding per-instance JavaScript-function-table
closure assumptions across pthread instances.

Source flags select a shared 256-MiB initial memory with a 1-GiB declared maximum,
4-MiB actor C stack and 2-MiB pthread stacks, disable dynamic JavaScript evaluation,
and avoid Asyncify. The host independently reads the shared-memory import bounds
before instantiation and fences callback faults. These are guest declarations,
not a measured total RSS, OS physical-memory cap or a recommendation for every
phone. The source catalog requests at least 2 GiB of device memory, but actual
capability and thermal/performance admission still require measurements.

## Boot, content and settings

Readiness follows synchronous upstream boot completion with active guest memory
and GPU state, before executing a guest frame. The independent UI preparation
deadline can retire the pool if a blocking loader/OS/guest operation fails to
return. Failed or deferred snapshot restoration is not acknowledged as success.
Snapshots leave guest resumption to the next frontend run operation.

The candidate requires verified file-backed content and declares ISO, CSO, PBP
and CHD. Existing private native-range or OPFS source policies continue to govern
actual loading; no game, disc image or firmware is included. PSP peripheral,
disc compatibility, malformed-media behavior and all formats still need the
final pass. A declared format is not a compatibility result.

The first finite user option policy is empty. CPU/backend/thread/network/JIT,
resolution and system paths are fixed profile choices. Network/UPnP/adhoc,
achievements, plugins, cheats, automatic firmware changes, rewind, recent-game
tracking and full-ISO RAM caching are disabled by the adapted boot configuration.
Configuration writes/internal data use private scratch; game content and system
assets remain read-only. Real network/host capability containment, unexpected
background work and all configuration transitions remain unqualified.

## Bundled system data

PPSSPP HLE does not require a manufacturer BIOS for this profile. Its exact
compatibility file, generated open-font atlas/meta and selected replacement PGF
files are separate bundled system data. The build requires the explicit generated
atlas receipt documented in `PSP_OPEN_FONT_ATLAS.md`; it cannot silently package
the upstream atlas instead. Exact Noto/FreeType inputs, PGF provenance, compiled
notices and complete corresponding source still require release review.

The new `system_assets_protocol: 1` inventories at most 32 files, 32 MiB per file
and 64 MiB combined. ASCII paths have at most four components; duplicate,
file/directory-colliding, reserved, traversal and out-of-core resource paths are
rejected. Each resource carries its exact length/hash in both source assembly and
the catalog. User firmware and these system assets cannot claim the same mount.

The UI loads bounded same-origin assets, verifies their complete length/hash and
includes their ordered path/length/hash identities in save compatibility. The
worker re-verifies transferred bytes before creating a nested read-only `/system`
tree. Reads are positional and bounded; create/write/rename/link/truncate changes
are denied by the existing immutable filesystem backend. Files never become user
firmware vault entries, executable modules or remote downloadable runtime content.

## Threaded persistent files and state

The profile declares `pthread_joined_barrier_v1`, separately from the old
single-threaded barrier. All core filesystem operations use the SDK's existing
main-runtime JavaScript FS proxy; the bounded `/saves` tree is owned by the actor.
It does not use WasmFS or claim that JavaScript FS objects are shared across
independent thread globals.

At a save barrier, the software guest has returned from its frontend run call.
The adapted hook rejects an active libretro emulation thread, flushes pending
software drawing, joins savedata-dialog I/O, synchronizes sceIo manager work and
flushes C stdio. These are separate producer boundaries. Capture/restore reuse
the existing bounded OPSF0002 file tree and OPWH0001/OPWF0001 envelope, exact
core/platform/configuration identity, complete-archive preflight, stopped-guest
directory replacement, restart and rollback-or-terminal-failure rules.

The entire sixteen-worker pool remains UI-owned through restart. Threaded guest
allocations survive until actor retirement. Runtime file/heap quotas, joins,
concurrent callback/FS behavior, boot/restart failures, decoder/state semantics,
actual lost-work cleanup and shutdown races must all be qualified. No portable
snapshot or native/Web PSP progress-conversion relation is inferred. Existing
immutable history/export/sync behavior is retained.

## Remaining full-product evidence

The explicit build emits an unapproved manifest only after producing bounded
module/Wasm files; it never promotes distribution approval. The module worker,
assets, all retained notices, source/SDK/atlas receipts and actual compiled source
are inventoried for the later release pipeline. Security review, transitive SBOM,
patents/content licenses and actual compiled-component inventory remain open.

The consolidated final pass must include real compilation/imports/exports and
multimedia completeness; pool/proxy/FS/barrier/state/boot fault injection; malformed
media/saves, allocation/quota/full-disk and cancellation; actual input/audio/video,
save/export/sync/rollback and device memory/latency/energy. GPU/JIT enhancement,
all remaining desktop/mobile/iOS runtime paths, Chromium builds, PC streaming,
store/release/security obligations and the original overall scope remain due.

Primary source inputs: [PPSSPP pinned parent](https://github.com/hrydgard/ppsspp/tree/a5275067b62433375bbde12921e96329a795dff0),
[pinned Emscripten worker contract](https://github.com/emscripten-core/emscripten/blob/e13ee988ce3d87a738dd8f3661d02223308c412a/src/lib/libpthread.js),
and [SDK release mapping](https://github.com/emscripten-core/emsdk/blob/main/emscripten-releases-tags.json).
