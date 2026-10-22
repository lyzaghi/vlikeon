# SNES software WASM candidate — source authored, execution pending

The SNES path now has a dedicated retained-source recipe at
`tools/build_bsnes_web.py`, pinned to bsnes revision
`63ee22f7af8f5d1ada295c2c98fd7bf261fa62a7`. `--prepare-only` materializes
committed source and the adaptations without running a compiler, configure
step, application, core, browser or test. No SNES Web binary is installed or
approved by this authoring step. All original product requirements stay active.

## Cooperative core execution

The upstream default libco fallback is not treated as a working WASM backend.
The new `bsnes-fibers.c` uses the Emscripten fiber API with Asyncify in one
worker. Each guest context owns an aligned C stack and a separate continuation
buffer; the root context owns its continuation buffer too. There are at most
64 guest contexts, a 256-KiB minimum/2-MiB maximum C stack and a 512-KiB
continuation buffer per guest. The root buffer is 1 MiB. Invalid handles,
allocation exhaustion, deleting/re-deriving the active context and returning
from a guest entry point terminate execution. Re-derivation resets a known
owned context, matching bsnes's power/reload pattern. These are source bounds,
not measured stack sufficiency or device-memory evidence.

The backend returns `co_serializable() == 0`, so bsnes synchronizes chip state
instead of saving host continuation pointers or stack bytes. The candidate
rejects unsynchronized state headers and has a distinct save-format identity.
Adapted serializer primitives/bulk byte arrays check capacity and report an
invalid decode; state version comparison uses all sixteen fixed bytes. Exact
length, synchronized flag and ROM-region bounds are checked. This does not
establish a complete hostile-state parser audit: chip-specific allocations,
semantic invariants, run-to-save termination and recovery still need the final
fault/security qualification.

Emscripten documents separate C/Asyncify stacks, the required Asyncify link
mode, incomplete root-context initialization and the prohibition on moving an
active fiber between threads. These constraints informed this adapter.
[Emscripten fiber API](https://emscripten.org/docs/api_reference/fiber.h.html).

## Explicit return completion

`libretro_result_protocol: 1` opts in to an additional void outer export and an
eight-byte result/completion slot. Load, state-size, serialize, unserialize and
the private save barrier write their final value and then mark completion.
The shared runtime reads the slot after the cooperative call returns, using
fresh heap views after memory growth. It rejects missing completion, re-entry
or malformed boolean results, stops further guest calls and retains affected
allocations until worker retirement. The renderer/worker handshake and bundle
admission require the declared protocol; legacy cores keep their direct calls.

This is an inference from the current upstream trampoline implementation:
fiber rewind occurs in the outer wrapper's unwind cleanup, and the trampoline
does not forward a rewound function's return value. The authored bridge avoids
depending on that discarded value. No SDK behavior was executed or qualified.
The selected release SDK must still prove synchronous completion; external
Promise/event-loop suspension is not admitted by this protocol.
[Emscripten Asyncify/fiber implementation](https://github.com/emscripten-core/emscripten/blob/main/src/lib/libasync.js).

## Persistent RAM and RTC

Upstream exposes neither SRAM nor RTC through the libretro memory interface.
This candidate keeps that behavior and opts into the shared private-file
envelopes. Its VFS adaptation routes canonical single-cartridge `.ram` and
`.rtc` names into `/saves`, including enhancement-chip RAM and real-time-clock
files. Names are lower-case ASCII components, at most 96 bytes, with no path
separators or parent references. Optional absence is allowed; other read,
length, write, flush, close and rename failures reject the capture barrier.

Each save handle has a 16-MiB logical buffer limit; all handles together have
a 32-MiB logical buffer limit and a 32-handle admission limit. Vector capacity,
WASM heap, JavaScript filesystem blocks and archive copies have additional
memory costs; these limits are not constant-RSS or kernel quota claims.
The existing `/saves` mount independently bounds materialized files/bytes.

The explicit barrier visits all upstream persistent-memory/RTC writers,
rejects duplicate canonical output names, and requires zero retained handles.
Writes close a private temporary file before checked rename. The shared
OPWH0001 progress envelope and OPWF0001 state envelope include every linked
save file. Restart restore reloads the same content/firmware/options and uses
the prior files/core state for recovery. A file publication failure cannot
authorize a new committed save, but publication within the live mount is not
an atomic multi-file transaction. Actual failure/retry/restart/rollback,
calendar/time behavior and snapshot boundaries remain unqualified.

## Content, firmware and policy

The embedded manufacturer SPC IPL array is removed from adapted corresponding
source. Its raw upstream archive is hashed and then removed from this output;
it is not a release input. A user-selected exact 64-byte `snes_spc_ipl` vault
item is mounted read-only at `/system/ipl.rom`. Missing required core resources
and failed loads are reported as failures rather than accepted sessions.
No manufacturer firmware or game content is supplied.

The current candidate accepts one `.sfc`/`.smc` image, checks the actual source
length before upstream full-vector loading (32 KiB through 64 MiB), and checks
each heuristic region before copying. Multi-cart, Super Game Boy, BS Memory
and Sufami Turbo slot configurations are explicitly rejected by this candidate.
The original complete scope still requires dedicated adapters for those modes,
sidecars and additional firmware-dependent enhancement chips; no compatibility
claim is inferred from their retained upstream source.

CPU rates, accurate PPU/DSP, one-times HD scale, no runahead, no delayed
coprocessor synchronization and the selected upstream HLE path are fixed inside
the core environment adapter. They cannot be overridden by the renderer's
finite-choice map. The shared per-game editor offers aspect ratio, image
blending, overscan and existing game hotfix choices through the retained
`bsnes-options.json` policy. Actual timing/game behavior and alternate optimized
profiles still require qualification.

## Final build and qualification

After all source authoring, `--build-prepared` requires the exact retained source
inventory, source list, exports, runtime methods, fixed choices and option policy,
and explicit Emscripten C/C++ tool paths. Only an actual build can emit candidate
module/WASM metadata, and it remains `distribution_approved: false` with
`verification: not_run`. No prepared source is inserted into the installed
catalog. Exact SDK identity, compiled dependency notices, complete corresponding
source, GPL-3.0-only/ISC review, artifact-content review and release admission
remain required.

The final consolidated phase must cover the actual compiler/linker/ABI and
trampoline behavior, stack exhaustion/limits, reload/re-power/controller
lifecycle, synchronized/corrupt state and recovery, SRAM/RTC/enhancement chips,
firmware, malformed ROM geometry/heuristics, checked file failures and device
CPU/memory/audio/input/video performance on all intended browser/mobile paths.
No test, compiler, core, browser, firmware, save operation or device API has run.

See `WEB_SAVED_FILES.md`, `WEB_FIRMWARE.md`, `GAME_CORE_OPTIONS.md` and the full
requirements ledger.
