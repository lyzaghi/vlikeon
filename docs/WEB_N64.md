# N64 WebGL2/interpreter candidate: source authored

`tools/build_mupen64plus_web.py --prepare-only` materializes selected committed
paths from Mupen64Plus-Next revision
`12edd2c74a517ff86dfa8cfc71ad75e4c10486d5`. Its root Angrylion, CXD4 and parallel
backend directories are absent from the selected archive. GLideN64, HLE RSP,
the core, common/frontend code and pinned local zlib/minizip/libpng/xxHash
inputs are retained. This does not clear every remaining dependency's license
or artifact content. The separate research Angrylion profile remains prohibited
from release approval.

The retained source recipe fixes cached CPU interpretation, GLideN64/HLE,
synchronous rendering, a 640 by 480 target, 4:3 display, no multisampling/FXAA,
no high-resolution texture input or shader-cache storage, a fixed texture-cache setting and
four memory paks. Per-game UI exposes only the core's two image-filter choices.
CPU dynarec and parallel/LLE backends are excluded by explicit make variables;
the source's cached instruction storage is non-executable. Compiler fast-math
is overridden. Scratch configuration is private to the module's `/scratch`.
No installed native game or emulator is launched by preparation.

The owned single-Worker Asyncify backend uses the shared bounded libco adapter,
with an 8-MiB per-fiber C-stack ceiling for this core's 4-MiB request. On normal
CPU termination, the coroutine returns to its owner and remains suspended until
the root frees it. A never-started CPU is not accidentally started by shutdown.
Started CPU shutdown requires an actual completion flag within a bounded number
of cooperative returns; failure terminates the guest instead of freeing a live
context. The core's void lifecycle uses result protocol 2. The shared WebGL2
frontend owns reset/presentation/destroy and the graphics context through core
deinitialization. See `WEB_HARDWARE_VIDEO.md`.

Cartridges require a non-null input, at least the boot-header region and at most
the shared 256-MiB direct-ROM bound; copy allocation failure is reported. The
catalog declares native z64/v64/n64 byte orders and the common media filesystem
protocol. GPU activation precedes the post-load 8-MiB system-memory observation.
These checks do not prove that any title boots or plays correctly.

Progress uses the upstream cartridge/pak save-memory image with a distinct Web
format. Snapshots require the exact upstream candidate size. Their magic,
current version and loaded-ROM MD5 field are checked before queueing a load;
metadata additionally binds the actual content/artifact/options identity.
The core's actual state callback determines load success, and the shared worker
retains its previous snapshot for rejected-load recovery. Pre-Start jobs may
initialize the core's coroutine. Chip-specific state parsing, allocations,
event queues, partial mutation, recovery and coroutine termination still need
the final fault/security/compatibility qualification. Native/Web progress
conversion is not claimed.

Preparation retains adapted corresponding source, selected-archive hash,
recipe/auxiliary/runtime inventories and a finite options policy under
`out/web-cores/mupen64plus-web`. The raw temporary archive is retired. The
explicit `--build-prepared` path requires actual make, Emscripten C/C++ and
archive tool paths, intact prepared input and fresh build output. It compiles a
copy of that source, retains the compiled-object inventory and creates a
candidate manifest with `distribution_approved: false`, `verification: not_run`
and core version `3.0.0+omniplay.webgl1.12edd2c74a51`. It never starts a browser
or runs a game/test. This is an authored recipe; no compiler has been invoked.

No compiled N64 module is installed. Exact SDK/link/ABI and GLES/WebGL shader
compatibility, backend/microcode coverage, companion assets/peripherals,
snapshot faults and recovery, browser context loss, controller/display/audio,
actual memory/timing/energy, complete source/content/license/SBOM review and
all required mobile/store/security/release evidence remain due. All original
v1 platform and runtime requirements remain active.
