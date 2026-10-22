# Owned Linux presentation pair source

The lower-level GNU/Linux launcher now authors a trusted normal-user SDL host
beside the isolated software actor. This source has not been built or run. The
Linux NativeRegistry/adapter, approved catalog and product availability remain
unchanged. No GUI, audio, controller, process, ELF admission, resource group,
kernel API, compiler or test has been exercised.

## Two distinct authorities

The actor still receives only its private media endpoints, immutable minimal
root and the namespace/Landlock/seccomp/resource restrictions described in
`LINUX_ISOLATION_BOOTSTRAP.md`. It receives no desktop socket or owner-control
endpoint. The separate presentation helper loads no core, ROM or actor-selected
library. Media protocol 2 accepts bounded pixels/PCM/state/haptics and returns
its own window's bounded input snapshots; it carries no command or filename.

The presentation host is trusted code with the current user's desktop/audio
authority, including system window, audio and graphics drivers. Its cgroup and
no-new-privileges restriction do not make it a second hostile-code sandbox.
Selected SDL/system/driver dependencies still require their own provenance,
licensing and compatibility evidence. A compromised trusted UI process is not
claimed to be contained like a core.

`DesktopEnvironment` copies a finite whitelist of existing current-user display,
runtime, authentication, session and audio variables. Values are bounded and
control characters refused. PATH and locale are fixed. Loader injection variables,
SDL dynamic API overrides and arbitrary inherited environment do not pass through.
TMPDIR/TMP/TEMP refer to the broker's admitted private 0700 directory. This is
environment hygiene, not complete desktop isolation or a credential sandbox.

## Executable, group and lifetime admission

`PresentationExecutable` hashes the actual held readonly executable against the
future caller's authenticated outer inventory. It requires an ordinary root- or
user-owned non-writable non-set-ID ELF64 x86-64 PIE of at most 256 MiB, a
file-backed executable entry, non-executable stack and no writable/executable
LOAD segment. It admits exactly `/lib64/ld-linux-x86-64.so.2`, refuses embedded
RPATH/RUNPATH/TEXTREL, bounds the dynamic/string tables and accepts only finite
basename-shaped DT_NEEDED strings. This checks shape and identity, not every ELF
semantic or the transitive system loader/library/driver closure. The build recipe
authors PIE/RELRO/NOW/NX-stack and omits build/install RPATH for this target.

The UI needs a distinct fresh delegated cgroup-v2 leaf named
`omniplay-ui-<session>`, owned 0700, empty, unfrozen and with no child groups. Its
actual held device/inode identity must differ from the actor's leaf. Before
spawn, it sets and reads back memory.max (256 MiB through 2 GiB), zero swap,
group OOM, 64 tasks, one through eight CPU cores and no child groups. Its pixel
budget must fit the declared dimensions plus fixed overhead. GNU libc 2.40's
POSIX_SPAWN_SETCGROUP path places it atomically into this leaf; unavailable
delegation or atomic spawn has no unbounded fallback. No ancestor controller,
permission or resource delegation is changed by these functions. Charged memory
does not claim to measure every shared driver allocation.

Both children use the permanent launch lane, inherited no-new-privileges and
live pidfd/resource owners described in `LINUX_RUNTIME_LAUNCH.md`. The UI's
private start gate must be released only after its exact boot/PID/start-tick
identity is durably recorded. Before SDL initialization the helper checks the
gate and owner socket's Unix seqpacket type and broker peer credentials, equal
non-root real/effective/saved IDs, inherited no-new-privileges and actual parent.
It holds a parent pidfd and establishes/rechecks the parent-death signal. It
requires a single grant byte followed by peer closure within ten seconds. It
then drops setup descriptors and lowers core-dump, locked-memory and realtime
priority limits. The creating thread must stay alive for the process lifetime.

The child's descriptor map is contiguous: media input/output at 0/1, NUL stderr
at 2, private start gate at 3, held executable at 4 and trusted owner control at 5.
Sources are duplicated above the selected range before spawn, cwd is the held
trusted package directory, and every unselected descriptor closes in libc file
actions. The helper closes 3/4 after admission and moves owner control to a
CLOEXEC descriptor before SDL. The actor uses its separate bootstrap map.

`presentation_channels` creates two distinct anonymous blocking pipes with exact
read/write roles. `spawn_presented` verifies their cross-end identities, matching
session and distinct resource leaves, starts the host, then the actor. An actor
launch failure drops the owned host guard. Each child retains its own work lease,
reviewed executable and applicable resources until actual pidfd death and an
empty admitted cgroup tree. Distinct independently acquired lease files are
required when the caller locks actor and UI separately. Unknown exit retains
ownership and prevents premature storage retirement. Numeric PIDs from disk
never authorize signalling.

## Trusted owner focus channel

The separate broker-to-helper Unix seqpacket channel has one fixed 40-byte
message: OPOWNER1 magic (8), session UUID (16), strictly increasing little-endian
u64 sequence in 1 through 2^63-1, u32 action (1 focus, 2 stop), and zero u32.
It accepts neither variable body nor path, input-injection command or native
handle. Malformed/session/replay/direction/size/EOF conditions stop the host.
The actor never receives this endpoint. The reader owns no SDL objects and only
sets bounded atomic flags; the main SDL thread handles its own window.

`LinuxPresentationHost::activate` queues a nonblocking request for this helper's
own window. A successful send proves only queue admission. The helper calls
SDL_RaiseWindow on its main thread and the producer must still observe fresh
actual focused input before running/resuming. A compositor can deny focus.
No focus-stealing workaround, foreign-window operation or invented Wayland
activation token is authored. A complete Chromium/user-gesture activation-token
handoff remains required where the compositor demands it. See SDL's
[raise-window contract](https://wiki.libsdl.org/SDL3/SDL_RaiseWindow).

## Remaining integration and final evidence

This API is not connected to the product NativeRegistry/NativeAdapter. The
signed full ELF worker/loader/dependency inventory, approved SDK/toolchain and
compiled dependency closure, immutable shared profile cache, private game/save
staging, legitimate delegated-parent provisioning/leaf removal, whole-profile
quotas, durable pair/work/uncertain-exit journals and final-save/control-channel
adapter remain required. The product must not select this source as a usable
Linux runtime until that integration and qualification exist.

`LINUX_PROFILE_DELEGATION.md` additionally authors admission of an explicitly
provisioned private profile subtree, shared complete-pair reservations, durable
creation callbacks and empty-leaf/asynchronous-charge retirement. It supplies no
host delegation service, durable work journal, existing-tree recovery or product
adapter. Those integration duties remain outstanding.

`LINUX_WORK_JOURNAL.md` now authors immutable pair/work/group/launch/process
records around these callbacks. Product factory/package/staging integration,
legitimate host delegation provisioning and confirmed crash recovery are still
required before enabling the Linux runtime.

The single final verification, after all original product source authoring,
must cover actual GNU ABI/SDK/kernel/delegation behavior, dynamic loader and
driver dependencies, cross-FD/role/gate/cancellation/parent-death races, pair
failure and tree-exit/storage retention, OS/compositor focus, input epochs,
SDL/audio/controller failures and real core/device latency/memory/performance.
GPU/JIT/mobile/macOS and every other original browser/runtime/streaming/sync/
security/licensing/store/release requirement remain active. No build or runtime
qualification follows from a copied source inventory.
