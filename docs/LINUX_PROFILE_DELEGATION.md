# Explicit Linux profile delegation owner source

`platform/sandbox/src/linux_delegation.rs` authors the owner of a supplied,
already provisioned profile cgroup-v2 subtree. No kernel, process, filesystem
lease or API operation has been run. This is not an installed delegation service,
Linux NativeRegistry integration or native availability evidence.

## Admission and allocation

The future host/service supplies an O_PATH directory named
`omniplay-profile-<profile-uuid>` and the exact private profile lease file. The
caller must authenticate their profile association; a filename is not authority.
The owner requires non-root current-user ownership, mode 0700 on the directory,
an ordinary single-link 0600 lease, cgroup-v2 filesystem, domain topology, no
direct tasks, no visible or dying descendants and an unfrozen subtree. The lease
is locked nonblocking and retained for the complete delegation-owner lifetime.
An in-process weak registry refuses a second live owner of the same device/inode;
clones share the original counters and lease. It bounds simultaneous profile
owners at 64. No arbitrary path or environment setting creates delegation.

The existing subtree must already enable cpu/memory/pids for its children and
have finite memory.max at most the caller's admitted policy (512 MiB through
64 GiB), zero swap, finite CPU quota of at most eight cores, pids.max exactly
128 times the session limit, maximum depth one and maximum descendants twice
the session limit. The session limit is one through eight. The source only
reads these parent controls; it never grants permissions, enables controllers,
migrates the broker or widens an ancestor's limits. A legitimately delegated
subtree must also permit migration from the launch lane's actual resource
domain; kernel refusal has no alternate unbounded launch path. The primary
[kernel delegation model](https://docs.kernel.org/admin-guide/cgroup-v2.html#delegation)
defines those constraints.

Each reservation includes actor memory, immutable tmpfs root, scratch, save,
128 MiB actor overhead and the separate UI memory ceiling. The bounds match the
bootstrap and presentation policies, are page-aligned and summed with overflow
checks. A mutex reserves one complete pair and its bytes against the current
finite parent memory ceiling before either leaf is created. It rechecks parent
shape/limits for every pair. This is conservative reserved capacity, not a new
measurement of actual total browser/profile RSS or GPU allocations.

Before any mkdir, the caller's durable intent callback must succeed. New leaves
have fixed `omniplay-<session>` and `omniplay-ui-<session>` names and mode 0700;
existing names are refused and never adopted. Each opens relative to the held
parent with O_PATH/O_DIRECTORY/O_NOFOLLOW/CLOEXEC. A separate acknowledgement
callback receives the observed profile/session/role, parent and leaf device/inode,
and charged-memory ceiling before a leaf can be returned. The caller still must
write and fsync the actual work journal; these callbacks are not that journal.
The launch source refuses a managed leaf when its actual actor/UI declared
ceiling differs from this reservation. Raw externally admitted lower-level
leaves remain the external caller's accounting responsibility.

`LINUX_WORK_JOURNAL.md` now authors the corresponding immutable work intent,
leaf acknowledgement and pre-gate process-record methods. The product factory
still must connect them to the legitimately provisioned delegation and package/
staging flow. That source wrapper does not supply existing-tree crash recovery.

## Ownership through exit and removal

A managed leaf attaches its lifetime owner to `DelegatedGroup`, so the existing
pidfd reaper retains the profile lease and reservation through true process and
tree exit. Merely dropping an active process guard cannot release that capacity.
Failed pair creation drops only newly owned guards; unknown existing groups are
never killed or removed. An open/admission failure after mkdir conservatively
retains the reservation for later recorded recovery.

Leaf retirement requires observed zero populated/tasks/descendants, zero dying
descendants, and a fresh fd-relative directory-identity check against its owned
name. It removes only that empty leaf with unlinkat(AT_REMOVEDIR). Metadata and
unlink are not an atomic defence against a separately authorized host writer;
the private parent and trusted broker are part of this ownership boundary.
Unconfirmed shape, read, permission or removal leaves the descriptor, parent
lease and reservation retained until broker exit. No PID from a record is used
to signal a process.

After successful unlink, the leaf descriptor closes and a bounded-by-reservations
retirement owner waits for the parent's actual nr_dying_descendants to reach
zero. Kernel cgroup destruction is asynchronous; unlink alone does not prove all
charged resources disappeared. The watcher retains only the parent and pair
reservation, not the removed leaf descriptor that could itself hold kernfs
references. Other live sibling groups do not by themselves block this check.
Watcher creation/read failure keeps capacity reserved. Both leaves must finish
retirement before the shared pair reservation is released. The parent itself
belongs to the external provisioning owner and is never deleted here. See the
[kernel cgroup.stat lifecycle](https://docs.kernel.org/admin-guide/cgroup-v2.html#core-interface-files).

## Remaining product work

Actual trusted host/service provisioning, the correct broker migration domain,
durable profile/work/leaf/launch/uncertain-exit journals, crash recovery and
bounded storage reclamation remain required. Recovery must authenticate the
same profile/parent/leaf identities and prove process/tree exit; unknown records
or resource state must stay retained. The current admitter intentionally refuses
an existing nonempty or dying subtree. It does not pretend that refusal is a
recovery implementation. Signed full ELF package/cache/game staging, the product
adapter, compositor activation, final-save flow and every original platform/
product/release requirement also remain active.

Actual kernel delegation/controller ownership, cgroup migration/spawn/removal,
asynchronous resource charge retirement, counter/lease/cancellation/error races,
process death and crash recovery belong to the single final verification after
all original source authoring. No compiler, resolver, process, kernel API,
product hash/admission, helper/core/browser/device or test has run.

`LINUX_ABANDONED_RECOVERY.md` now authors a separate RecoveryDelegation mode and
explicit bounded work collector. It permits existing descendants only for
read-only process/tree proof and exact empty-leaf retirement, with no new launch
or signalling authority. Conversion back to normal delegation requires a wholly
empty, non-dying subtree. Partial or missing ownership records remain retained;
this does not complete every interrupted creation or resumable work deletion.
