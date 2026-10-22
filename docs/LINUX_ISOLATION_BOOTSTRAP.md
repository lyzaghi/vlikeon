# Linux namespace and immutable-root bootstrap source

This authors a static-PIE Linux x86-64 bootstrap target beside the actor policy.
No CMake/configuration/SDK/compiler, ELF executable, Linux syscall, namespace,
process/pipe/mount/FD/capability, helper/core/device or test ran. There is no Linux
integrated native-registry launcher or approved package and Linux runtime
availability stays false. The new lower-level owned launch source is described
in `LINUX_RUNTIME_LAUNCH.md`.
It is not a qualified sandbox. Overall progress remains 88%; all original
requirements and the single final verification remain active.

## Fixed entry point and plan

The bootstrap accepts exactly `--stdio-isolation-v3 <canonical-session-uuid>`.
It receives independent blocking FIFO endpoints at 0/1 for actor control and 3/4
for presentation, write-only `/dev/null` at 2, and a readonly fully sealed plan
memfd at 5. FD 6 is a private one-shot start socket; FD 7 is the reviewed
bootstrap executable inherited for fd-based exec and closed immediately on entry.
It never opens a renderer-provided host path or selects an executable
command from the plan. The four FIFO identities must differ. Source/root FDs are
8..1023; they become CLOEXEC after inherited arrival. The plan closes after
decoding. Unmentioned descriptors are closed before namespace setup, and the
actor later closes every descriptor above 4 before reopening only its policy
files. Only the five fixed I/O descriptors survive worker exec.

The little-endian plan has a 96-byte header and at most 256 536-byte entries.
Header: `OPNSBX03` magic (8), version u32=3, entry count u32, expected parent PID
u32, empty staging-directory FD u32, memory/file/root/scratch/save limits (five
u64), 16 bytes of session identity, cgroup-directory FD u32, exact task bound
u32=64, admitted CPU cores u32 (1..8), and zero reserved u32. Its exact size is
`96 + count*536`. The cgroup FD differs from the stage and every image/input FD.
No older plan is silently accepted without the mandatory resource group and
pre-guest durable process-registration gate. Header layout stays 96 bytes.
Each entry is role u32 (image=1, game input=2), FD u32, byte length u64,
executable boolean u32, path length u16, zero reserved u16 and 512 path bytes.
The used path bytes contain no NUL; all remaining bytes are zero. Paths must be
relative safe ASCII components, at most 127 bytes per component and eight levels.
Duplicate FDs/paths, noncanonical session, unknown roles/flags, wrong file shape,
unsealed/mutable plan, trailing bytes or resource-policy overflow exit 125.

The future broker must authenticate the actual bootstrap/package and exact
plan/image bytes and construct these descriptors itself. A memfd seal prevents
mutation; it does not authenticate who produced the bytes. The plan is never an
approval token from a renderer. The bootstrap's own static-PIE build and the
broker's sanitized spawn environment still need actual toolchain/package evidence.

`platform/sandbox/src/linux_plan.rs` now authors the matching pure Rust encoder,
with the same canonical session, field/role/path/FD, executable identity, image/
input totals, exact padding and finite resource bounds. It does not query or
authenticate an OS descriptor, create a memfd, spawn a process or enable Linux
support. The C parser and Rust producer still require real cross-language wire
qualification in the one final pass; source similarity is not that evidence.

## Immutable descriptor preparation source

The Linux-only `linux_sealed` module authors budgeted preparation of image and
plan memfds. It creates them non-executable/CLOEXEC, copies image bytes in 64 KiB
blocks while hashing against the caller's authenticated inventory, supports
cooperative cancellation between blocks, and publishes only a readonly reopened
descriptor after write/grow/shrink/seal/execute seals and exact inode/size/mode/
descriptor readback. A failed copy/hash/seal/cancellation drops the temporary file
and reservation. Plan bytes come only from the closed encoder. There is no
executable or mutable-memfd fallback. The C plan/image admission requires the
same non-executable sealed shape; executable roles apply to the new root copy.
See [non-executable memfd behavior](https://docs.kernel.org/userspace-api/mfd_noexec.html)
and [file seal semantics](https://man7.org/linux/man-pages/man2/F_ADD_SEALS.2const.html).

One shared budget admits 16 MiB..1 GiB and at most 256 owned objects. Cloning an
object retains the same inode/reservation until the final owner is dropped. This
is a per-budget bound on owned descriptor bytes, not a deduplicating cache, hard
RSS guarantee or complete profile-wide cache manager. The future broker must
share one profile budget across preparations, retain owners through every child
descriptor lifetime, and separately account root copies, package/game storage and
other in-flight work. The public raw descriptor is for that trusted launcher;
its existence does not authenticate package approval or spawn an actor. Neither
the encoder nor this Linux OS module has been compiled or executed.

## Namespace and lifetime source

The entry path requires a normal unprivileged single-threaded caller with equal
real/effective/saved IDs, empty capabilities and the expected live parent. It
creates a process group, opens parent/self pidfds, installs parent-death SIGKILL
with before/after liveness checks, resets SIGCHLD, maps only the current user and
group through a new user namespace with setgroups denied, and creates private
mount/PID/network/IPC/UTS namespaces. Mount propagation becomes private before
the root is constructed. No setuid helper, broader ID map or host network is used.
These mechanisms follow the primary [user namespace reference](https://man7.org/linux/man-pages/man7/user_namespaces.7.html).

An outer trusted supervisor forks namespace PID 1, which constructs the root,
locks no-root/no-setuid-fixup/no-ambient-raise, clears ambient and every supported
bounding capability, and drops effective/permitted/inheritable capabilities before
forking the actor. The outer supervisor also drops its setup capabilities. Each
parent retains only the exact child pidfd while waiting/reaping its exit and
propagates a finite exit status. No PID read from a file is killed. Parent-death
checks are repeated across credential/setup changes. The actor's parent is the
private init; namespace-init death also terminates its processes. The actual
broker must own the outer process and helper until both are observed ended.
See [pidfd ownership and wait semantics](https://man7.org/linux/man-pages/man2/pidfd_open.2.html),
[parent-death signal](https://man7.org/linux/man-pages/man2/PR_SET_PDEATHSIG.2const.html)
and [PID namespace init behavior](https://man7.org/linux/man-pages/man7/pid_namespaces.7.html).

## Immutable root source

The root plan names a pre-existing empty 0700 directory by O_PATH FD. A new
bounded tmpfs is created by fd-based mount operations and attached only in the
private mount namespace. It contains no host-root bind, home directory, ambient
desktop/audio socket, network configuration, account data or general device tree.
The source uses [fd-based filesystem construction](https://docs.kernel.org/filesystems/mount_api.html),
[mount attachment](https://man7.org/linux/man-pages/man2/move_mount.2.html)
and [mount attributes](https://man7.org/linux/man-pages/man2/mount_setattr.2.html).

Runtime/core/asset inputs are readonly fully sealed regular files, each at most
256 MiB. Their broker-authenticated bytes are copied in 64 KiB chunks into the
new root, with at least 2 MiB reserved from its 16 MiB..1 GiB budget. Only
`runtime/omniplay-native-worker` and `lib64/ld-linux-x86-64.so.2` may have the
executable-file role. Other runtime files/assets remain under `runtime/`.
Executable-file Landlock rights are separately restricted by the actor policy;
readonly libraries may still have legitimate ELF RX mappings.

Game/firmware/media inputs are the broker's private immutable staging copies:
readonly ordinary single-link files owned by the mapped user, without group/other
write bits, at most 4 GiB each and 32 GiB total. They get individual readonly,
nosuid/nodev/noexec file mounts under flat `work/` names, excluding scratch/save.
The broker must keep their backing bytes stable. A readonly bind is not a claim
that another already authorized host-side writer cannot change the backing inode.
No user-picked host directory or writable media backing tree is passed to the core.

Scratch and save use separate 4..512 MiB noexec/nosuid/nodev tmpfs mounts, each
with at most 8192 inodes. Work itself is readonly/noexec. Proc is mounted for the
private PID namespace and readonly/nosuid/nodev/noexec; init checks that its own
proc identity is 1. Only character device 1:3 is mounted as `/dev/null`. The new
root is pivoted in, the old root detached and removed, and the root made readonly.
The actor closes all remaining host/root/setup FDs, reopens only its policy inputs,
enforces Landlock/seccomp/resource limits, and execs the fixed worker FD with
literal control/presentation arguments. The admitted loader and libraries are
inside the new root; fixed `LD_LIBRARY_PATH=/runtime` does not name a host tree.
See [pivot_root](https://man7.org/linux/man-pages/man2/pivot_root.2.html).

## Mandatory delegated resource group

Before unsharing or forking, the single bootstrap joins a fresh empty cgroup-v2
domain leaf under the broker's existing legitimate delegation. Its directory FD
must be O_PATH, owned 0700 by the current user, named `omniplay-<session-uuid>`,
have no tasks/descendants, be unfrozen and have no enabled subtree controllers.
The bootstrap changes only that leaf's own controls. It neither creates/enables
an ancestor delegation nor relaxes a permission when the host rejects it.

It writes and reads back memory.max (admitted actor address-space budget plus
root/scratch/save budgets and 128 MiB), memory.swap.max=0, memory.oom.group=1,
pids.max=64, and cpu.max with 100 ms period and the admitted 1..8 CPU quota.
Only its own process enters; descendants inherit that membership. Its own OOM
adjustment becomes zero to avoid inheriting a protected launcher exception. It
requires exactly one task on entry, disallows subgroup depth/descendants and
closes the cgroup FD before namespace/core startup. Missing controllers or a
refused write/readback exits 125; there is no unbounded fallback.

These control the cgroup's charged memory, tasks and CPU bandwidth. They are not
a claim that every shared/precharged page or device allocation is accounted as
private RSS. The broker still must bound its sealed-image cache, own the leaf,
kill/observe descendants and remove an empty leaf on completion or recovery.
The primary [cgroup-v2 documentation](https://docs.kernel.org/admin-guide/cgroup-v2.html)
defines delegation, controller and group-termination semantics. Actual delegated
service setup and broker lifecycle remain unimplemented and unqualified.

## Remaining integration and evidence

Actual signed ELF/loader/dependency admission, shared sealed-image cache admission,
private game
staging, root-directory lifecycle, broker spawn/FD map and exec-error channel,
outer pidfd/creation identity and uncertain-exit work journals, trusted GUI helper
launch/activation/final-save behavior and actual delegated-cgroup provisioning,
cache/whole-profile accounting and lifecycle still need implementation/integration.
No optional fallback may remove namespaces,
Landlock, private mounts or syscall policy when the kernel/libc cannot support it.

The single final pass must cover actual static and worker/helper builds, ELF
interpreter/loader behavior, old-root/FD/namespace escape attempts, PID reuse and
parent-thread/process death, partial startup/reaping, mount/user-namespace policy
restrictions, file mutation and quota failures, legitimate core/thread/file I/O,
focus/pipe/guest-save lifecycle, memory/latency/driver behavior and complete source/
dependency/license/SDK provenance. GPU/JIT, macOS and all remaining browser/mobile/
streaming/sync/security/store/release work retain their original scope.

The source producer copies this component and the selected Rust contract/manifests
with SHA-256 inventories and a future build recipe. That Rust selection is not a
standalone complete Cargo workspace. No recipe is run during source preparation.
The future static libc/SDK distribution needs its actual license/notice and
relink/corresponding-source materials as applicable; a source receipt alone is not
that evidence or distribution approval.
