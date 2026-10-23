# Owned Linux bootstrap launch source

This implements lower-level broker-side admission and gated launch of the Linux
actor bootstrap and trusted GUI pair (`LINUX_PRESENTATION_PAIR.md`). The explicit
LinuxAdapter/service factory composes these sources; approval still requires
the actual authenticated catalog and target behavior. Current item-3 compiler
and test evidence is in `LINUX_ITEM3_CLOSURE.md`; historical authoring notes below
do not supersede it. A compiler or userland socket pass alone establishes no
spawn/cgroup/core/device qualification or product completion percentage.

## Executable and resource ownership

The bootstrap descriptor must be an ordinary readonly executable owned by root
or the current user, with no writable/setuid/setgid permission bits and at most
64 MiB. The
future broker supplies its hash/length from the authenticated outer package. The
admitter hashes actual bytes cooperatively in 64 KiB blocks, checks stable
descriptor metadata, and requires x86-64 little-endian ELF static PIE with bounded
program/dynamic headers, an executable-file-backed entry and a non-executable
stack. It refuses PT_INTERP, DT_NEEDED, RPATH/RUNPATH and W+X LOAD segments. This
is a finite shape/identity check, not a complete ELF parser or a security audit of
the binary. The readonly held inode cannot be replaced by a pathname rename;
host-account ownership is not claimed to be fs-verity or protection against an
already authorized host writer changing permissions. See the primary
[ELF format reference](https://man7.org/linux/man-pages/man5/elf.5.html).

A delegated resource leaf is admitted by its owned 0700 O_PATH directory, actual
cgroup-v2 filesystem, exact `omniplay-<session>` name, empty domain/task/subtree
shape and unfrozen state. The admitted object exposes no arbitrary path-based
kill operation. The future owner must create a fresh leaf under its legitimate
delegation; these functions do not enable controllers or grant host permissions.
The same directory object is retained for tree kill and empty-tree observations.

The launch spec owns the reviewed executable, root directory, acquired session
work lease, admitted resource leaf, immutable image owners and staged game files.
The lease is an owned private single-link regular file and is locked nonblocking.
The future caller must bind that lease and all staged file identities to this
exact session/profile. The spec does not authenticate renderer-selected inputs
or implement signed catalog/cache/game staging on behalf of that caller.

## Atomic pidfd spawn and descriptor map

The GNU host path resolves the already loaded libc's version and optional spawn/
file-action symbols. It requires GNU libc 2.40 or later and its pidfd_spawn ABI;
there is no handwritten Rust post-fork closure, process-ID-number kill fallback,
PATH-based executable search or inherited loader environment. Older/unsupported
hosts refuse this optional native path. The floor avoids the earlier unpatched
spawn exec-error pidfd leak; actual SDK/libc provenance still needs qualification.
See the primary [GNU spawn implementation](https://raw.githubusercontent.com/bminor/glibc/glibc-2.40/sysdeps/unix/sysv/linux/spawni.c)
and [upstream exec-error fix](https://sourceware.org/pipermail/glibc-cvs/2024q2/085497.html).

All source descriptors are duplicated CLOEXEC above the low mapped range before
libc spawn file actions. Every low descriptor is filled contiguously: control
read/write at 0/1, NUL stderr at 2, media read/write at 3/4, sealed protocol-3 plan
at 5, start socket at 6, reviewed executable at 7, root/cgroup at 8/9 and declared
image/input files from 10 onward. The child changes to a trusted root directory,
then closes every descriptor above that exact range before fd-based bootstrap
exec. Existing low descriptors are overwritten, preventing holes from retaining
unselected broker handles. Parent temporary source/gate/control/media copies close
after spawn. The bootstrap later drops setup handles before the actor policy.

Creation is dispatched through a bounded eight-job process-lifetime launch lane.
The lane stays alive after a temporary preparation worker returns its adapter:
parent-death signals follow the creating thread, so creating a persistent game
from a short-lived background job would otherwise terminate it too early. The
caller retains its local durable-record callback and releases the gate only after
the lane returns an owned process. Cancellation joins the queued operation rather
than abandoning a possibly running spawn. A per-job unwind is contained; no
untrusted core executes on the lane. Kernel/file operations can still delay actual
cancellation completion. See the primary
[parent-death thread semantics](https://man7.org/linux/man-pages/man2/PR_SET_PDEATHSIG.2const.html).

Before accepting jobs, that lane sets and reads back no-new-privileges for its
own spawning task. Children inherit it across libc spawn; bootstrap main requires
it already set. This prevents privilege gain from executable set-ID/capability
metadata before the reviewed bootstrap reaches its later policy setup. The
source neither grants a privilege nor changes sibling broker tasks' spawn policy.
The lane readiness handshake fails if this restriction cannot be established.
See the primary [no-new-privileges task and exec semantics](https://docs.kernel.org/userspace-api/no_new_privs.html).

The libc interface returns the child pidfd from process creation. Subsequent
signals and waits use that live handle. A numeric PID observed in fdinfo is only
for a bounded journal identity (PID, start ticks, boot UUID); live pidfd checks
surround proc observations. A delayed pidfd_open of a recycled process number is
not substituted for atomic spawn. See [pidfd lifetime semantics](https://man7.org/linux/man-pages/man2/pidfd_open.2.html).

## Durable start gate and cleanup

The static bootstrap arms its parent-death chain and waits at most ten seconds
on a private UNIX seqpacket socket. It validates socket domain/type and the
creator's peer PID/UID before accepting exactly one byte 1 and actual peer EOF.
This gate closes before cgroup/user-namespace/root setup or any guest/core load.
The seqpacket socketpair's creator credentials follow the reviewed
[Linux UNIX-socket implementation](https://raw.githubusercontent.com/torvalds/linux/v6.12/net/unix/af_unix.c).
Only after an owning reaper and actual process identity exist does the launch
callback durably record that identity. Cancellation or callback failure closes
the gate and terminates the owned tree. The authorization send uses NOSIGNAL/
DONTWAIT, so a closed bootstrap gate does not alter global signal policy or send
SIGPIPE through the broker. No such socket survives into the untrusted actor.

The guard/reaper retains the work lease, executable, source files and immutable
budget reservations through real exit. Stop closes an unreleased gate, signals
the exact pidfd and kills the exact admitted cgroup tree. Completion needs both
actual pidfd death and an empty owned resource tree, followed by exact-child wait
(or already-reaped ECHILD after that death proof). An outer supervisor's death
alone is not treated as proof that every descendant ended. Cleanup attempts wait
briefly; uncertain kernel/process/tree state leaves the reaper holding resources.
If the reaper thread itself cannot be created, startup stops before guest release
and keeps an exceptional unconfirmed owner until broker exit. This conservative
retention is not automatic recovery of every failed launch or a hard preemption
guarantee for a blocked kernel operation. See [cgroup tree semantics](https://docs.kernel.org/admin-guide/cgroup-v2.html).

An independent abandoned-work query validates the recorded boot/PID/start identity
and never signals a process from disk. Different proven boot, a dead process or
a reused live PID may establish that the recorded process ended; read/permission
uncertainty is retained. The caller still needs owned cgroup/root/work records and
empty-tree checks before deleting storage. No native work journal integration is
claimed by this lower-level API.

`LINUX_PROFILE_DELEGATION.md` now adds optional managed profile/leaf reservations
and retirement ownership. `LINUX_WORK_JOURNAL.md` supplies separate authored
immutable work/group/launch/process acknowledgements and a wrapper that commits
each observed process record before its start gate. These are not selected by
the product adapter, and actual provisioning, crash recovery and full package/
cache/game/save staging remain required.

## Remaining work

The NativeRegistry/adapter and signed Linux catalog, authenticated complete ELF
worker/loader/dependency package, shared profile cache and private game staging,
delegation provisioning/leaf removal and durable uncertain-exit journals,
product GUI-pair/activation and final-save/control-channel integration remain
required. The process API has not launched even a bootstrap. Actual GNU ABI, ELF/
FD-map/collision/exec-error behavior, gate/record/cancel/parent-death races, reaping/
cgroup quiescence and blocked I/O, whole-profile/resource quotas, real core/driver/
input/performance and dependency/SDK/static-link/license evidence belong to the
single final pass after all original product authoring is finished. All other
browser/native/mobile/streaming/sync/security/store/release work remains in scope.
