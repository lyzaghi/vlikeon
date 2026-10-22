# Contained desktop runtime source

The native runtime now has an actual source path from Game Hub through the Rust broker, approved catalog, process container and C++ libretro frontend. It has not been compiled or executed. No native core binary is installed and no sandbox or compatibility result is claimed. Windows x86-64 is the first authored container backend. macOS and Linux currently return unavailable until their own real adapters are implemented.

`NATIVE_PRESENTATION_HOST.md` authors a separate bounded raw-media/input helper.
The optional native software frontend and Windows broker-owned duplex pair,
signed helper inventory, process guards and recoverable own-window focus path
are now authored in `NATIVE_PRESENTATION_FRONTEND.md` and
`NATIVE_PRESENTATION_LAUNCH.md`. Direct presentation remains the selected default.
No binary/catalog/platform was enabled and no helper or SDK/compiler was run.
Real POSIX isolation/lifecycle, actual package/OS admission and hardware paths
remain required; avoiding GUI calls alone does not contain a hostile core.

Linux now also has unbuilt actor policy and static bootstrap source with a closed
sealed-FD plan, private namespaces/init/reaping, an immutable minimal root and
mandatory delegated cgroup leaf controls. `LINUX_ISOLATION_BOOTSTRAP.md` describes
the source boundary and remaining broker/package/cache/GUI/delegation lifecycle.
No Linux syscall/compiler/helper was run and no Linux native availability changed.

Lower-level GNU launch source now includes reviewed static-PIE admission, a
protocol-3 durable start gate, contiguous descriptor maps, atomic pidfd spawn,
a process-lifetime launch lane, actual process-plus-resource-tree exit and a
resource-retaining reaper. `LINUX_RUNTIME_LAUNCH.md` separates that foundation from
the remaining catalog/adapter, full package/cache/staging and delegated
resource/work journal integration. It has never launched a process or enabled a
Linux runtime.

`LINUX_PRESENTATION_PAIR.md` now adds lower-level source for a trusted normal-user
SDL helper, distinct atomic cgroup placement, gated durable pair creation,
whitelisted desktop environment and a separate owner-only own-window focus
channel. Its actual focus is still established by fresh input epochs. This
source does not enable Linux registry/adapter support or qualify system/driver
dependencies. Unix session directories/lease files are authored with private
0700/0600 creation modes; complete staging/journal/resource ownership remains due.

The unselected Linux source additionally has explicit profile delegation
admission, shared pair reservations, empty-leaf/dying-charge retirement and an
immutable work/leaf/launch/process journal wrapper. See
`LINUX_PROFILE_DELEGATION.md` and `LINUX_WORK_JOURNAL.md`. Actual host provisioning,
product factory/adapter, authenticated package/cache/staging, crash recovery and
full device/SDK qualification remain due; no Linux runtime was enabled.

`LINUX_IMAGE_PACKAGE.md` additionally authors the Linux catalog image policy,
authenticated outer-inventory/catalog selection, held fd-relative ordinary
files, a finite immutable ELF DT_NEEDED/SONAME closure and profile-shared sealed
cache with cancellation, de-duplicated preparation, inactive-entry eviction and
reaper-retained owners. The Unix unpack recipe authors readonly final file modes
and the worker CMake recipe authors Linux PIE/RELRO/NOW/NX-stack without RPATH.
NativeRegistry execution is still unavailable on Linux; the product adapter,
private staging, legitimate provisioning/recovery, whole-profile accounting and
actual build/SDK/core/device/license qualification remain due.

`NATIVE_WORK_STORAGE.md` advances unexecuted database source to schema 15 and
adds durable bounded copy debt, pre-copy reservation and confirmed-retirement
release to Windows preparation/cleanup. `LINUX_INPUT_STAGING.md` authors checked
canonical game/user-firmware copies, a bounded immutable receipt and pre-gate
ledger/inode admission for the unselected Linux pair. Product Linux registry/
adapter, control/save/final-save, provisioning/recovery and full physical
storage/platform/device/release qualification remain required.

`LINUX_SESSION_COMPOSITION.md` now composes the typed package/cache/input/work/
delegation owners into a gated actor/UI session and shared control channel, with
hello admission, signed-option core preparation, bounded lifecycle/save/load/
status calls and compatible save identity. This remains an internal unselected
source path. Full RuntimeFactory/RuntimeAdapter final-save/metrics/disc/close,
legitimate host provisioning, confirmed crash recovery/reclamation and actual
platform/SDK/core/device qualification remain due.

`LINUX_RUNTIME_ADAPTER.md` now adds the shared RuntimeAdapter implementation and
an explicit installed_linux factory constructor requiring an already admitted
profile delegation and authenticated installed release. It connects normal
adoption/save/compatibility/metrics/disc/close interfaces in source. Default
broker startup still supplies no delegation and keeps Linux native unavailable.
Actual host provisioning/selection, confirmed crash recovery/reclamation,
compositor activation and complete platform/SDK/device qualification remain due.

`LINUX_ABANDONED_RECOVERY.md` now authors explicit recovery-parent admission and
a bounded collector for fully acknowledged ended pairs. It independently locks
all work leases, confirms recorded process and empty resource identity, writes
retirement intents and releases copy debt only after confirmed removal and
parent sync. It never kills a disk PID. Partial creation/launch or interrupted
work-deletion records remain retained; resumable partial recovery, actual host
startup selection and full qualification are still required.

`LINUX_RESUMABLE_RETIREMENT.md` now authors an external immutable confirmed-pair
receipt, crash-resumable work removal/debt release, nonblocking audit serialization
and bounded rotating receipt/work cursors. It preserves incomplete original
ownership/launch and live/unknown state; actual host startup, partial-origin
recovery and full target filesystem/kernel/device qualification remain due.

`LINUX_CREATOR_RECOVERY.md` now authors schema-2 Linux ownership binding to the
observed creator broker. A valid but unacknowledged launch may use proven exact
creator exit only together with independent leases and empty owned resource
trees; old schema-1 work keeps its stricter observed-child proof. Resumable
receipts distinguish this case from a role that never reached launch. No PID
is signalled; missing group/ownership proof and live/unknown state stay retained.

`NATIVE_PRESENTATION_CLIENT.md` adds the bounded producer-side library and owned
private-pipe admission. Its frontend and optional Windows launcher are connected
in source; the library alone does not change native runtime availability.

The optional software frontend is now connected in source in
`NATIVE_PRESENTATION_FRONTEND.md`, including protocol-2 aspect/input/lifecycle
handling and a frozen source candidate. The default broker path still selects
direct mode. The private Windows launcher/activation/package source has no actual
binary/OS qualification; real POSIX containment is still required. Brokered
measurement is deliberately not advertised as qualified.

Writable session data has separate `scratch` and optional `save` subtrees. Only the specific game container receives write/delete access there, with a low mandatory integrity label so the DACL grant is effective for its low-integrity token. ROM/firmware directories receive read access and directory-only traversal; installed runtime files receive read/execute access. Temporary environment paths target `scratch`. Game/save files do not gain file-execute access from the application's data grant, and process image/dynamic-code mitigations remain enabled. This ACL/label authoring requires the final real Windows containment pass; no effectiveness result is claimed.

The production catalog must reside in the running broker's signed immutable version directory. Its own inventory entry and every worker, core and support library are checked against the outer release inventory and their runtime hashes. Windows retains read-only file handles that prevent replacement while the worker is alive. Release builds have no arbitrary catalog environment override. Debug builds permit an explicit developer root. A runtime still requires an explicit distribution decision; the build recipes default that decision to false.

The Windows FFI is isolated in `platform/sandbox`. Each prepared native session creates a fresh UUID-derived AppContainer identity; existing profiles are never adopted. Launch supplies no network or sensor capabilities, only an explicit inherited stdin/stdout/stderr handle list and a minimal environment. The worker begins suspended, enters a kill-on-close job with one-process, memory and CPU limits, and resumes only after assignment. The software path prohibits executable heap code, external extension points, remote/low-label images and clipboard/display/desktop changes. The initial CPU ceiling is 50 percent of system capacity, a bounded policy that still requires real-device performance evaluation. Failure to establish the required isolation fails closed.

Native code directories contain public program files and receive read/execute access for AppContainers. Private ROM/firmware files are copied, hashed and published in a new UUID directory under the owning browser profile; only the specific game container receives read access. Game code receives a bounded filename, not an arbitrary renderer path. The sandbox’s own Windows-managed profile remains a separate OS-provided storage area. Normal retirement and conservative journal recovery now have source, while kernel disk quotas, ambiguous crash windows and complete recovery controls remain required. The worker cannot write the broker's library or saves. Owner lease handles keep a new broker from reclaiming an active work directory. Startup reclaims only validated abandoned UUID work directories after their lease can be opened exclusively and any acknowledged session-container/process conditions permit retirement. See `NATIVE_CONTAINER_RETIREMENT.md` for exact behavior and remaining gaps.

The C++ frontend binds software and explicitly declared OpenGL core libretro profiles, uses a separate native window, bounds CPU/GPU targets, handles four standard controllers plus keyboard/analog input, and paces the core at its declared nominal refresh. Battery mode limits presentation to 30 Hz without changing the core clock. A bounded single-producer/single-consumer audio ring supplies SDL's actual audio demand and records starvation only after priming. Unknown GPU, thermal, hardware latency and driver measurements stay unknown. Additional GPU/JIT/platform adapters, native capture/streaming surfaces and controlled native benchmarks remain outstanding.

Control messages use independent length-prefixed JSON and bounded binary frames. The broker validates correlation and ABI identity. Ordinary unsupported/incompatible operations do not terminate a healthy core. Malformed frames, crashes and response timeouts terminate the contained job. Native state loads take a rollback snapshot before attempting restoration. Save RAM includes RTC bytes in a versioned envelope; native format identifiers do not assert cross-platform portability. State compatibility incorporates exact core artifact, options, firmware hashes and platform. Native playtime comes from a monotonic worker counter and omits large unconfirmed suspension gaps.

Native window focus loss pauses playback. Closing its window requests save-and-close in Game Hub. Declining an unsuccessful save clears the close request and leaves the game paused. Closing the control page triggers a broker-owned best-effort recovery save and job shutdown. With no surviving UI causal head, recovery preserves a conflict branch instead of replacing newer progress. Browser/OS process termination can still end the native host before a final recovery write completes; previously committed autosaves are retained. Full shutdown and fault-injection verification remain required.

`tools/build_native_core.py` authors explicit mGBA and bsnes build paths. The bsnes adaptation removes its upstream embedded 64-byte SPC IPL array, reads user-selected `ipl.rom` instead, and selects Windows fibers instead of executable-heap libco trampolines. These transformations are authored and not yet exercised. The firmware vault imports only a user-picked file, checks role/length/hash, stores immutable versions locally with a quota, allows role selection/deletion and never uploads firmware. These checks establish neither provenance nor compatibility. No manufacturer firmware file is supplied by the application. Corresponding-source and artifact-content verification remain release gates.

Native preparation now has a separate background-job path. It validates and stages content away from the broker's control loop, supports cooperative cancellation between copy operations and IPC receive polls, and transfers the prepared adapter into the session manager only when its authenticated client collects the result. Closing the client cancels uncollected preparations. Dependency integrity hashing and filesystem calls can still delay a cancellation acknowledgement until their current operation returns. The synchronous compatibility command remains available to older clients; the current Game Hub uses the asynchronous path.

Multi-file CUE/M3U media now follows an explicit folder grant, generated local asset names and immutable content hashes. Native disc controls use libretro callbacks and bound the visible disc count to eight. The OPNS0001 state envelope records the selected disc/eject state alongside core serialization, so restoring a state can restore its disc first. Failed restoration attempts recover the previous disc and state when possible, otherwise the contained runtime is terminated. The software Beetle PSX source and build recipe are pinned; its JIT, GPU and direct optical-drive backends are disabled for this adapter. The default catalog constrains memory cards to the libretro memory buffer so a second file-backed card is not silently lost. Region-specific user firmware and upstream original MIT OpenBIOS have separate provenance; the shipped BIOS policy is still subject to final inventory review.

The authored NDS integration uses melonDS DS 1.4.0 with locked offline dependency sources, user-selected system files, software video and no JIT/network backend. Its worker supports a normalized mouse/single-touch stylus and core-option v2 defaults. A minimum system-RAM guard rejects the upstream error-message display as playable content. See `NDS_RUNTIME.md` for scope, provenance, source preparation and outstanding verification. No NDS binary has been built or admitted.

The authored ares Mega Drive adapter adds bounded software presentation/audio, mapper analysis, three/six-button input, live SRAM/EEPROM, snapshots and optional selected SVP firmware. Its standalone source excludes embedded resource/firmware loaders and dynamic-code backends. See `MEGA_DRIVE_RUNTIME.md` for source provenance, limits and remaining evidence. It has not been compiled or admitted into the installed runtime registry.

The N64 software/interpreter source recipe and its native identity are authored; its state interfaces, reset-after-persistent-memory policy and retained provenance review are described in `N64_RUNTIME.md`. The PSP source recipe now includes immutable dependencies, software/interpreter configuration, synchronous load acknowledgement, readonly inventoried system assets, async-I/O savedata capture and a snapshot wrapper carrying both guest state and persistent files. `PSP_RUNTIME.md` describes the new bounded directory archive and rollback through guest/VFS reinitialization. Save-size resource-limit replies preserve a healthy channel; an incomplete response remains terminal. No N64/PSP runtime binary has been built or admitted.

AppContainer profile cleanup and live disk quotas, platform installers/uninstallers, ACL/inheritance tests, actual window/audio/controller behavior, shared-surface presentation, advanced GPU/JIT/mobile paths and device-lab verification remain incomplete. No existing Chromium sandbox policy has been relaxed to make this path work.

Primary API references: [Microsoft AppContainer launch](https://learn.microsoft.com/en-us/windows/win32/secauthz/implementing-an-appcontainer), [AppContainer isolation](https://learn.microsoft.com/en-us/windows/win32/secauthz/appcontainer-isolation), [SDL audio stream](https://wiki.libsdl.org/SDL3/SDL_OpenAudioDeviceStream), [SDL audio data](https://wiki.libsdl.org/SDL3/SDL_PutAudioStreamData). Pinned local SDK and dependency headers were used while authoring; no product test has run.

An additional OpenGL core adapter is now authored for the separate N64 GLideN64/HLE profile. Compiled worker/backend identity, bounded offscreen targets, robust context negotiation/reset checks, aspect-correct native presentation and graphics-aware teardown are explicit in source. The process mitigations remain enabled. N64/PSP input keeps physical analog and D-pad signals separate and supplies keyboard analog controls with localized Game Hub help. N64 byte-order formats and PSP CHD declarations now align with their media contracts. `N64_OPENGL_RUNTIME.md` describes the candidate and unverified limits. No graphics driver, native frame or sandbox behavior has been exercised.

The PSP OpenGL/IR profile adds explicit post-context boot completion and a guest pause barrier, queued audio/geometry changes and file/snapshot restoration with synchronous restart/rollback. `PSP_OPENGL_RUNTIME.md` and `PSP_SAVE_FORMAT.md` describe its source lifecycle and v2 file archive. The software profile shares the pause/file behavior. No native CPU JIT is enabled by either profile.

The native performance collector now has an explicit worker handshake/capability, bounded monotonic samples, broker-owned identity/origin, one-time local commit and Game Hub controls/history/export. Pausing, hiding/minimizing or otherwise interrupting playback cancels unfinished capture. Raw payload size is specific to the worker command, including zero binary bytes for ordinary control responses. `PERFORMANCE_MEASUREMENTS.md` defines the frontend-tick semantics and outstanding controlled device-lab work. Neither collector nor runtime has been executed.

Paused native sessions now expose a direct live-save export command whose selected-file capability, capture compatibility metadata and OPSV0001 envelope bypass local history admission. It does not create a save/head/sync record or automatically resume the guest. Capture and file I/O remain synchronous source requiring final platform/latency/failure qualification. See `LIVE_SAVE_EXPORT.md`; no native capture, file picker, export or test has run.

Per-game keyboard/standard-gamepad input profiles now load from local broker preferences and require worker input-profile protocol 3. The contained frontend validates exact key/button schemas before preparation; input remapping stays outside emulated-state compatibility hashes. Existing workers must be rebuilt and admitted by signed inventory. See `GAME_INPUT_PROFILES.md`; no input/runtime/build/test has run.

Optional local rumble now has a separate worker handshake, admitted runtime capability and per-game opt-in. Core callbacks only update short mutex-protected motor state; SDL device operations remain on the frontend thread, outside those locks. Latched libretro strong/weak values renew bounded effects while the frontend progresses, and pause/removal/shutdown request zero effects. This is a source contract, not evidence of physical stop deadlines, driver support or platform coverage. See `GAMEPAD_HAPTICS.md`; no haptics API or worker has run.

Both native sticks now use a strict per-game 0–50% radial deadzone (15% default), direction-preserving circular saturation and symmetric signed conversion shared with the authored Web transport. Native input polling snapshots the two sticks once per port instead of rereading axes for every button query. Digital-only left-stick fallback uses the same processed axes; N64/PSP analog remains separate from D-pad. See `ANALOG_INPUT.md`; no SDL poll, core, input behavior or test has run.
