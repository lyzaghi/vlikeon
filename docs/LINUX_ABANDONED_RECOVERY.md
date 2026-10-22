# Explicit abandoned Linux pair recovery source

The sandbox now authors RecoveryDelegation and native-runtime exports
recover_linux_work plus a count-only LinuxRecoverySummary. These functions have
not been called. No host directory, lease, resource group, process, database,
removal, compiler, helper/core, kernel/API or test operation ran. Default broker
startup still does not provision a Linux native context or invoke this collector.

## Recovery resource authority

The trusted provisioning owner supplies the existing profile subtree descriptor,
profile UUID, exact locked profile lease and finite resource policy. Admission
checks the same non-root ownership, 0700 cgroup-v2 shape, fixed profile name,
domain/no-direct-task state, enabled child controllers and finite parent limits
as normal delegation. It permits existing descendants for this audit only. The
in-process parent-identity registry and independently acquired profile lease
prevent a concurrent admitted launch owner. RecoveryDelegation has no create-
pair or process-signalling API. No disk filename or recorded PID grants it a
host permission.

The admitted parent remains held through the audit. A recorded leaf must match
the same profile/parent device/inode, literal session/actor-or-UI name and leaf
identity. Existing leaves are opened fd-relative with O_PATH/O_DIRECTORY/
O_NOFOLLOW/CLOEXEC and must be an owned domain with no tasks, processes, visible
or dying descendants. Occupied, different-inode, linked, inaccessible or unknown
leaves fail. The recovery API removes only the exact empty owned leaf using
unlinkat(AT_REMOVEDIR), after a second name/inode observation. It neither writes
cgroup.kill nor signals a process. The caller must durably acknowledge the
retirement intent before requesting removal.

Missing names can represent the earlier managed owner already removing its
confirmed empty leaf or an interrupted recovery retry. They are accepted only
within this authenticated parent/session/group ownership chain, after the work
owner proves recorded process exit and records retirement. Arbitrary renames or
resource migration by a separately authorized host writer are outside the
private broker ownership boundary. A parent read/shape failure remains uncertain.
After removing leaves, actual parent nr_dying_descendants must reach zero before
work cleanup. The collector does not sleep waiting for charge retirement: the
work remains for a later explicit retry. New launch delegation conversion
requires the entire subtree to be empty with no visible or dying descendants;
unknown leftovers prevent RecoveryDelegation::finish from returning it.

## Work ownership and process proof

The explicit collector validates the private canonical profile/native-work
directories and inspects at most 64 entries per call. Each UUID work directory
must have both immutable Linux intent and owned records, each binding the exact
profile/work/target and observed directory device/inode. It independently opens
and nonblocking-locks all owner/actor/UI lease files through the no-follow
private regular-file admitter. Duplicated file descriptions from a live session
are not reused as proof of abandonment. The primary
[flock lifetime contract](https://man7.org/linux/man-pages/man2/flock.2.html)
explains why independent acquisition and all descriptor owners matter.

Both pair-group acknowledgements and their common pair intent must agree with
the supplied recovery parent and distinct actor/UI inodes. Each launch record
must have a matching process acknowledgement with the same group, bounded
executable identity and a nonzero boot UUID. The existing read-only pidfd query
must prove the exact boot/PID/start-tick process ended or its identity belongs
to a different process/boot. No stored PID is ever killed. A launch intent
without observed process acknowledgement remains retained; missing gate
acknowledgement cannot be interpreted as proof that no child existed. If both
launch-intent and process records are absent, source ordering establishes that
that role never reached spawn, but the recorded resource tree must still be
observably empty.

Only after both process/tree proofs does the collector publish new immutable
per-role retirement intents. Existing intents must exactly match. It removes
the confirmed empty leaves, checks asynchronous charge retirement and rechecks
the current work directory against its held identity. It acknowledges copy debt
as retiring, invokes the existing confined link-refusing/depth/entry-bounded
work removal, syncs the native-work parent and releases debt only after actual
path absence. Unix lease descriptors remain held through that sequence. Failure
keeps work/debt for a later explicit audit; the public summary reveals only
inspected/removed/retained counts and no host paths or child output.

## Limits and remaining recovery work

This first source path reclaims completely acknowledged pairs whose exact
processes and resource trees are already ended. It intentionally retains work
with partial creation, absent ownership/group/process acknowledgements, unknown
old delegation identities, unconfirmed charge state or live processes. It is
not a service that kills abandoned live processes, reconstructs every interrupted
creation, repairs corrupted journals or adopts arbitrary old groups. A crash
during generic recursive work deletion may remove an acknowledgement before all
files disappear; such an incomplete tree is conservatively retained and still
needs a durable resumable work-deletion protocol. The bounded scan can also need
cursor/fairness integration for profiles with many retained entries. No full
crash-recovery completion is claimed.

The newer source in `LINUX_RESUMABLE_RETIREMENT.md` now writes an immutable
confirmed proof outside the work tree before deletion and resumes that deletion
even if inner acknowledgements/leases disappeared. It adds serialized audits
and bounded rotating cursors. The earlier limitation above describes the first
collector checkpoint; missing original ownership/launch proofs and unconfirmed
live resource states remain retained by the newer source too.

Normal broker provisioning/startup must bind this audit to the legitimate host
resource domain, preserve unsafe/unknown records and select the explicit runtime
factory only after admissible cleanup. Resumable partial-work retirement,
physical whole-profile accounting, compositor activation and compiled SDK/core/
driver/license/device qualification remain required, together with every
original browser/native/macOS/mobile/remote-play/sync/security/store/release
requirement. The one final verification after all source authoring must cover
actual flock/pidfd/cgroup identities, missing/replayed records, parent/leaf name
races, process/tree exit, asynchronous charge retirement, partial removal/fsync,
quota debt and bounded collector behavior. No result exists yet.
