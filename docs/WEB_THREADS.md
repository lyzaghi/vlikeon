# UI-owned Web pthread source foundation

The shared player now has a separate, explicit execution contract for a bounded
Emscripten pthread pool. The UI owns the actual thread Worker handles, while the
emulator actor sees a relayed Worker surface. This allows the existing outer
request deadline, failure, stop and page-hide paths to retire thread handles even
when the emulator actor is blocked inside guest code.

This is source foundation, not a qualified runtime. The later pinned PSP Web
software producer, selected SDK source and joined persistent-file extension are
documented in `WEB_PSP.md`. No executable, game, firmware or distribution approval
has been added.
No JavaScript/C compiler, browser/Worker/Atomics/API, emulator, device or test has
run. Actual PSP producer/multimedia/assets/file-barrier behavior, a threaded
hardware renderer, all platform paths and measured performance remain required
original scope.

## Admission and resources

A threaded core must declare `wasm_threads: true`, `thread_callbacks_protocol: 1`
and `libretro_result_protocol: 2`. Its strict `thread_pool` object contains:

| Field | Required value or bound |
| --- | --- |
| `schema_version` | 1 |
| `api` | `emscripten_pthreads_ui_v1` |
| `maximum_workers` | Integer 1 through 16 |
| `maximum_memory_bytes` | 64 MiB through 1 GiB, in 64-KiB pages |
| `worker_module` | One `.js` resource beside the core's own loader |
| `worker_type` | `module` |

The manifest also records exact `pthread_worker_sha256` and
`pthread_worker_bytes` (1 byte through 32 MiB). Source-bundle admission checks the
ordinary local resource and copies it into the product resource inventory. The
loader itself can be the worker resource when the selected SDK emits that shape.
Arbitrary origins, blob URLs, query strings, classic workers and extra resources
are not admitted by this protocol. Thread metadata on an unthreaded core fails
admission. Existing release approval and resource-integrity rules still apply.

The catalog/player requires actual cross-origin isolation, shared memory, Atomics,
Worker and WebAssembly support before ranking a threaded candidate as available.
The PWA catalog applies the same declaration rules. Preparation validates the
policy again in the UI and emulator actor and requires an explicit Ready
`threadProtocol: 1` acknowledgement. An unthreaded session acknowledges zero.
An unsupported browser remains unavailable; no capability is fabricated.

Hardware-video and portable-progress combinations remain unadmitted. Persistent
files require the later explicit `pthread_joined_barrier_v1` extension described
in `WEB_PSP.md`; they cannot inherit the old single-threaded barrier by implication.

## Shared-memory admission

Before module instantiation, `thread-wasm.js` reads the bounded binary header and
import/memory sections. It requires one imported `env.memory`, the shared flag,
an explicit 32-bit maximum and limits within the declared page budget; an
additional defined memory or memory64 is rejected. The engine still validates
the complete module itself. The small inspector does not execute the module.

The inspector accepts valid bounded padded unsigned LEB encodings and rejects
truncation, overflow, duplicate standard sections, unsupported declarations and
excessive section/import/name work. It caps section traversal at 256, imports at
4,096, each name at 1 KiB and aggregate import-name bytes at 1 MiB. This keeps
the UI preflight bounded without decoding code or data bodies.

After instantiation, typed callback checks require the actual heap to be shared,
at least the binary's imported minimum and no larger than the policy maximum.
This bounds the admitted guest memory declaration. It is not an OS physical-RAM,
Worker-stack, table, JavaScript allocation, CPU or total-process RSS quota.

## Owned worker relay

The emulator actor installs `RemoteThreadPool` before importing the admitted SDK
factory. That surface accepts only the declared worker URL and module options.
It emits bounded create/post/terminate control messages to `UiThreadRelay`; only
the UI constructs actual Worker objects. Monotonic IDs cannot be reused, and no
more than the declared number of Worker objects may be created over an actor's
entire lifetime. A producer must prewarm/reuse its pool, not churn replacement
workers within the actor. Losing a necessary worker requires fresh preparation.

The pinned Emscripten 6.0.13 source uses integer command IDs 1 through 9; the relay
admits those exact IDs and its older bounded string controls. SDK messages allow
bounded plain structured data, compiled-module handles and
shared memory. Ordinary buffers are limited to eight and 1 MiB in aggregate;
depth, node, string, array and shared-handle bounds also apply. MessagePorts and
arbitrary platform objects are rejected. Explicit outbound transfer ownership is
preserved on the actor-to-UI hop; UI-owned received copies may be transferred
onward. A malformed message, child error or clone failure fails the entire actor.

UI replies bypass the guest-command promise queue so SDK pool startup/control
cannot wait behind the very guest operation it serves. Late replies to a retired
thread are ignored. Root Worker event handlers are fenced by their actual owner,
so an old actor cannot send events into a later session's relay.

Stop, page hide, an operation deadline and runtime failure close the UI-owned
handles independently of guest cooperation. Normal deinitialization gets its
existing bounded stop opportunity first. The actor retains threaded guest heap
allocations until its complete Worker retirement, and the retired remote Worker
factory remains closed; a late SDK task cannot fall back to spawning an untracked
nested Worker. Ordinary unthreaded preparation also disables nested Worker
construction through that global surface.

Browser termination/reclamation is asynchronous and must be qualified. This is
not a kernel hard-realtime or physical thread-count claim. The admitted JavaScript
glue and SDK are trusted, inventoried product source. The relay is not a sandbox
against arbitrary hostile JavaScript, captured constructors, SharedWorker or
other worker-producing APIs; a producer must exclude/audit such bypasses and all
network/evaluation capabilities as part of artifact approval.

## Typed callbacks

`adapters/threaded-frontend.c` supplies seven compiled libretro callback addresses:
environment, video, scalar/batch audio, input poll/state and rumble. Each address
belongs to the module's static function table. It does not invoke a JavaScript
`addFunction` closure from a different Wasm instance's function table.

The C adapters use the SDK's synchronous main-runtime proxy macros. That runtime
is the emulator Worker, not the UI. `ThreadCallbacks` provides its bounded host
callbacks, checks the shared heap and latches any callback fault. `LibretroCalls`
checks the latch before and after every guest lifecycle/result operation;
preparation and command completion also check it. Faulted callbacks return a
neutral value until the enclosing call retires or the independent UI deadline
ends the blocked actor. The host does not acknowledge a completed healthy call
after observing that fault.

A producer must retain the frontend's accompanying MIT notice, compile/link the
typed adapter and result bridge, export both thread-ABI/address functions, use a
fixed strict prewarmed pthread pool, disable dynamic replacement/evaluation and
provide the exact inventoried module-worker glue for its selected pinned SDK.
The later PSP recipe retains the selected SDK source contracts and authors these
flags. No compiled SDK/core or device has established the actual contract.
Asyncify/thread/GPU combinations are not implied by result protocol 2 alone.

## Final evidence still required

The final consolidated pass must cover actual SDK output/imports/exports, typed
callback ABI and proxying; valid/malformed/padded memory sections; module/message
cloning; pool exhaustion, lost workers, transfer ownership and denied URLs;
blocking core calls, deadlines, failure/page-hide/stop and old-session messages;
thread joins, heap growth, late callbacks, state/IO/audio/input/rumble and resource
retirement. It must also establish browser/mobile compatibility, real RSS/energy/
latency, isolation/permissions/CSP, source/license inventories and release policy.
No test or completion evidence is recorded for these requirements.

Primary design contracts: [Emscripten pthread startup/proxying](https://emscripten.org/docs/porting/pthreads.html),
[strict prewarmed pool settings](https://emscripten.org/docs/tools_reference/settings_reference.html#pthread-pool-size-strict),
[main-runtime callback macros](https://emscripten.org/docs/api_reference/emscripten.h.html#main-thread-em-asm),
and the [HTML Worker lifecycle](https://html.spec.whatwg.org/multipage/workers.html).
These documents do not substitute for a pinned SDK or actual device evidence.
