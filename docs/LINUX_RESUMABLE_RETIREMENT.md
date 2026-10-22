# Resumable confirmed Linux work retirement source

The ended-pair collector now authors an external retirement receipt so confirmed
work deletion can resume after a crash removes the original inner journals.
The source also authors bounded rotating recovery cursors. None of these
functions have run; no filesystem/database/cgroup/process/removal operation,
compiler, runtime or test has been exercised. Default startup remains unchanged.

After validating the complete original work/group/launch/process chain,
independently locking its three leases and proving the exact actor/UI processes
and resource trees ended, the collector writes a new immutable receipt beneath
the profile's private 0700 native-retirements directory. Each canonical
<work-uuid>.json is a no-follow admitted private 0600 single-link record, bounded
at 4096 bytes. It includes the observed work directory identity, both group
identities and each role's observed process identity, or an explicit absence
where the original complete journal proved that role never reached spawn.
No receipt is created from an incomplete ownership or launch chain.

The receipt is published and synced outside the work tree before any recursive
work deletion starts. It is not overwritten and remains through work removal,
parent directory sync and copy-debt release. Only then is the receipt removed
and its parent synced. A crash at any point can retry the same immutable proof;
absence of an inner journal after this point is not confused with an unknown
original launch. The profile/resource ownership boundary assumes trusted broker
code and excludes a separately authorized host writer forging local journals.

On every retry, the receipt's profile/work/target/parent/leaf identities are
validated again, recorded processes are queried read-only, matching resource
leaves must be empty or already absent and asynchronous dying charges must be
retired. Nothing signals a disk PID. A remaining work directory must match the
receipt's actual device/inode. Any still-present lease files are independently
locked; missing ones can be a valid consequence of the earlier confirmed partial
deletion. This relaxation applies only under the durable external proof. A
different directory reusing the name, live process/tree, mismatched group or
uncertain read refuses continuation.

The collector marks debt retiring before the confined link-refusing/depth/entry-
bounded deletion and only releases debt after actual work-path absence. If the
work already disappeared, it still revalidates process/resource proof before
finishing the debt and receipt. Database profile-open cleanup may already have
released a retiring absent path; the explicit receipt retry remains idempotent.
If deletion fails partway, the external receipt survives. No deletion, debt
release or recovery action is executed during source authoring.

RecoveryDelegation now provides a nonblocking per-instance audit guard in
addition to the independently locked profile delegation lease and parent-
identity registry. Concurrent collector calls cannot race the same profile
receipt or work tree. Each explicit pass inventories at most 256 candidate
names in each private directory, refuses excess entries, sorts them and visits
at most 32 receipt names plus 32 work names. It rotates lexically after two
separate bounded cursors persisted in the existing profile preferences table.
Retained early entries therefore do not permanently hide later valid work.
Receipt UUIDs already visited in that pass are not counted again as work.
Failed passes can safely repeat from the previous cursor. Unknown or malformed
entries remain retained; the source does not adopt their names as authority.

This extends the fully acknowledged ended-pair path. It does not reconstruct
partial original creation or missing process acknowledgements, force-stop live
or unconfirmed processes, repair corrupted proofs, discover a changed old
delegation, delete unknown temporary receipts or grant host resource authority.
Normal provisioning/migration/startup selection, partial-origin recovery,
whole-profile physical accounting, compositor activation and complete compiled
SDK/core/driver/license/device evidence remain required. Every original
product/platform/release requirement stays active.

The final single verification after all source authoring must cover receipt
publication and every crash boundary before/during/after recursive deletion,
lease disappearance and independent reacquisition, identity/name reuse, process/
tree/charge retirement, database debt/receipt retry, parent fsync, malformed
receipts, cursor wraparound/fairness and candidate limits on actual filesystems
and target kernels. No such evidence exists yet.

`LINUX_CREATOR_RECOVERY.md` now extends new schema-2 ownership and receipts for
a valid launch intent lacking its child acknowledgement. Such a retry must
prove the exact recorded creator ended as well as all existing lease/resource
conditions; it is explicitly distinct from the never-launched role. Schema-1
records cannot use that fallback. Partial original group/ownership identities
remain unclaimed and retained.
