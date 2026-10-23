# Linux actor policy source component

This Linux x86-64 actor policy has a separate namespace/root and static bootstrap
target described in `LINUX_ISOLATION_BOOTSTRAP.md`. Exact Ubuntu compiler batches
are now assigned during item 3; `LINUX_ITEM3_CLOSURE.md` records their outcomes.
They do not establish actual kernel, process-tree, core/helper or device
acceptance. `omniplay_sandbox::supported()` still returns false on Linux; the
explicit service-backed factory admits authority independently. No overall
completion percentage or distribution approval is claimed.

## Caller boundary

The bootstrap must create the private user/mount/PID/network/IPC root,
own its init/reaper and parent-death chain, remove capabilities, bind only approved
readonly runtime/loader/system inputs and game data, provide bounded private
scratch/save storage, verify the private proc mount, and admit only its fixed
control/media/program descriptors. All other inherited authority must be closed.
These are required caller work, separate from this policy component. Their new
source implementation still needs actual broker/OS qualification. No core or
untrusted initializer may run before the policy, and the trusted caller must
still be single threaded. A private `/proc/self/task` count is an extra admission
check; it cannot stop an adversarial concurrent thread creator by itself.

The policy also requires empty effective/permitted/inheritable capability sets
and the caller's already locked no-root/no-setuid-fixup/no-ambient-raise state.
The future bootstrap must establish that state while it still owns its setup
capabilities, then drop them; the policy never tries to regain them.

The API accepts at most 24 unique O_PATH/CLOEXEC file descriptors, 3..127,
categorized as readonly file/directory, private writable directory, executable
file or the exact character device 1:3 for `/dev/null`. No other device receives
that rule, and the null rule grants no device IOCTL. The signed namespace plan
must distinguish those roles. The worker and
exact ELF loader need explicit executable-file entries; whole library directories
receive read permission. No path strings or renderer-supplied permissions are
interpreted by this component. Policy FDs are consumed before exec except the
worker FD, which closes on the successful ELF exec. Already open I/O rights must
have been removed by the bootstrap; Landlock is not an inherited-FD scrubber.

## Authored kernel policy

Landlock requires ABI 6 or later, handles filesystem rights through device IOCTL,
denies TCP port actions and scopes signals/abstract UNIX sockets. No reduced
older-ABI policy is selected. Filesystem metadata has limitations, which is why
the private mount/PID root remains mandatory. See the official
[Landlock documentation](https://docs.kernel.org/userspace-api/landlock.html).

The seccomp filter checks native x86-64 and rejects x32, then defaults to EPERM.
Its short closed blocks use their actual array lengths for syscall-match skips.
Thread-only clone requires the shared thread flags and permits only explicit
thread/TLS/TID bits; clone3 returns ENOSYS for a qualified libc fallback. Other
process creation, sockets, network/IPC/device/control/namespace/mount/kernel,
ptrace/perf/BPF and scheduler-changing operations receive no allow rule.
The own-group tgkill, finite prctl/fcntl operations, current-process readonly
prlimit query and worker-FD/AT_EMPTY_PATH execveat have argument checks. File I/O,
memory, timing, ordinary thread synchronization and pipe polling are enumerated.
No filter-log mode or runtime-discovered syscall grant exists. Seccomp alone is
not a complete sandbox. See the official
[seccomp filter documentation](https://docs.kernel.org/userspace-api/seccomp_filter.html).

File-backed RX mappings support the approved ELF loader. W+X and anonymous
executable mmap requests are refused, and mprotect cannot add execution. This is
not a claim of complete Windows ACG equivalence, native-code integrity, a complete
W^X proof or compatibility with every libc/core. Loading attacker-controlled
machine code cannot be the security boundary for an already untrusted native
core; the namespace, Landlock, syscall and process boundaries still must hold.

The source bounds address space to 256 MiB..8 GiB, individual output files to at
most 256 MiB, descriptors to 128, memory locking/core dumps/realtime priority to
zero, and sets no-new-privileges before Landlock and a synchronized seccomp filter.
These policy-local ceilings do not establish complete RSS/CPU/thread/disk-tree
accounting. The separate bootstrap now has mandatory delegated leaf-control
source, described in `LINUX_ISOLATION_BOOTSTRAP.md`; actual broker/host accounting
still needs integration. Any missing required operation exits 125; exec receives
only the caller's admitted argv/environment.

## Remaining work

Namespace/bootstrap/init/reaper and the immutable root now have source in
`LINUX_ISOLATION_BOOTSTRAP.md`; broker FD/error-pipe/process accounting,
parent-death and uncertain-exit journals, trusted GUI pair/activation, complete
delegated-cgroup lifecycle/accounting and signed SDK/helper/package receipts remain
required. The bootstrap's new leaf-controller source does not qualify those
broker/host behaviors.
A qualified kernel/UAPI/libc baseline, normal software core needs,
denied-call/ELF/clone/focus/resource/lifecycle behavior and attack resistance belong
to the single final verification after all product authoring. GPU/JIT and other
primary/secondary platforms keep their original requirements. Source existence
does not enable a Linux runtime or satisfy that final evidence.
