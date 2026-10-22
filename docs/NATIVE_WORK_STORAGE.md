# Durable native session copy reservations

Core database source now advances to schema 15. Schema 14 introduced the
native_work_storage ledger; schema 15 adds an immutable Windows creation marker.
No database, migration, transaction, quota query, process, runtime, compiler or
test has been run. This is source authoring; it does not migrate a user profile
or approve a release's data-schema range.

## Reserved copies and crash debt

Before a new private UUID work directory can receive game/firmware copies, the
native path reserves its declared bytes and file slots in an immediate database
transaction. The declaration includes the game, the maximum admitted firmware
shape and 8 MiB of private metadata allowance. The per-work limits are 8 GiB and
144 input slots; the profile ledger admits at most 64 retained works, 32 GiB and
4096 slots in total. SQL constraints/triggers and the transaction both enforce
the finite policy. A bounded scan refuses unregistered, linked or malformed
existing work entries instead of pretending their disk usage is zero. An entry
for the newly generated UUID can be bound by its work owner. In normal new
creation, the committed reservation precedes the directory, so concurrent
registered creators do not depend on a partially observed directory scan.

New Windows source holds an external creation lease across reservation,
directory creation and per-work owner-lease acquisition. The reservation commits
`windows_creation_protocol=1` atomically with its debt; schema-14 rows and other
creation routes remain zero and cannot be promoted later. The Windows recovery
inventory now includes bounded ledger-only debts. With the creation lease held,
only protocol-1 reserved rows may recover a missing directory or an ordinary
pre-owner partial directory without conflicting launch/ownership journals.
Staged or legacy missing ownership remains retained. See
`WINDOWS_PRELAUNCH_RECOVERY.md`; no lease or migration has run.

The immutable row identity holds profile/work UUID, reserved bytes/slots and
creation time. Phases move only reserved to staged or retiring, and staged to
retiring. Staged requires a lowercase SHA-256 receipt. A checked copy/manifest
must be durably published before the staged transition; new launch paths must
not execute a core before that acknowledgement. Reserved slots can conservatively
exceed actually selected optional firmware. Counters track retained copy
reservations, including uncertain or interrupted work, across process restart.

This ledger covers private disk copies and their declared metadata allowance.
It does not claim to be an OS filesystem quota, all-profile physical allocated
blocks, original imported assets/saves/sync cache, arbitrary Windows core scratch
writes or all kernel/driver memory. Linux's separate immutable root/scratch/save
tmpfs and owned memory/task/CPU limits retain their independent kernel policy.
Complete physical/whole-profile resource accounting remains product work.

New Windows source seals the exact fresh AppContainer default filesystem tree
and checks the real actor token before resume; see `WINDOWS_DEFAULT_STORAGE.md`.
The same suspended launch now seals its exact private registry namespace via a
single-use checked-token lookup, as described in `WINDOWS_REGISTRY_STORAGE.md`.
These changes close those selected source-level write surfaces without claiming
hard disk quota for permitted scratch/save subtrees or helper caches, or complete
externally granted registry isolation. Actual sealing and token behavior remain
unverified.

The Windows MD source now selects frontend-owned bounded memory files and omits
actor OS write grants for work save/scratch in that declared mode. The core,
worker, catalog and prepare acknowledgement must agree; common save capture and
staged replacement retain existing envelopes. This does not change input-copy
debt or add physical allocated-byte status. Other cores and trusted helper
caches remain separate work. See `NATIVE_MEMORY_FILES.md`; no route has run.

## Retirement and release

Only the native process/work owner may acknowledge retiring, after proving the
owned processes/tree/profile are retired. The database transition never kills a
process or removes files. After confined work removal actually completes,
release_removed verifies that the literal generated work path is absent,
including refusing an existing dangling link, before deleting its retiring row.
It does not free reserved/staged rows merely because a directory disappeared.
Profile open retries at most 32 already-retiring, already-removed debts; read,
permission or database uncertainty retains them. Missing-directory reserved debt
has a separate pre-launch release: new code cannot launch before staged, and
that branch does not reinterpret a staged or unknown launch as uncreated.

The Windows NativeAdapter now reserves before creating/copying work, publishes
an immutable game/firmware staging summary and acknowledges staged before
container creation. Cleanup reacquires the exclusive work lease and rechecks
durable actor/helper identities and ownership through the existing abandoned-work
path, then acknowledges retiring before removal. Missing ownership/launch
acknowledgements and uncertain process/profile state retain both work and debt.
Its own already-retired container profile can be checked again because Windows
documents successful deletion for an absent profile; see
[DeleteAppContainerProfile](https://learn.microsoft.com/en-us/windows/win32/api/userenv/nf-userenv-deleteappcontainerprofile).
No Windows API was called during source authoring.

Confirmed Windows retirement now publishes an external proof under
`native-retirement` before work-tree deletion, and requires the exact retiring
ledger row whenever a directory still exists. A separate exclusive collector
lease and immutable profile/work proof preserve retry authority after partial
removal erases the internal lease or journals. The ledger is released only after
actual absence, then the external proof is removed. A bounded rotating cursor
gives retained/live entries and later proofs independent recovery turns. See
`WINDOWS_NATIVE_COMPLETION.md` for the remaining ambiguous-origin, resource and
power-loss boundaries. These paths have not executed.

Linux work creation now requires an explicit pre-copy reservation. Its separate
checked input stager and pre-gate manifest/ledger binding are described in
`LINUX_INPUT_STAGING.md`. Linux confirmed crash recovery and the product adapter
remain unfinished; no generic Windows cleanup may erase its ownership records.

The desktop Windows maintenance source now exposes reservation/phase totals and
fixed limits through one profile-checked readonly aggregate. It keeps physical
allocated bytes unknown. Startup/manual recovery runs on one owned background
worker with status/cancel controls in Settings and independent per-document job
capabilities. See `WINDOWS_NATIVE_MAINTENANCE.md`; no status query, worker or UI
has executed.

## Remaining evidence

Final verification, after all original source authoring, must cover migration
and immutable/phase/global quota triggers, writer contention, crash ordering,
partial copy and ENOSPC, identity/lease/process uncertainty, interrupted
retirement/removal, registered/unregistered legacy work and bounded debt retry.
Real physical storage/accounting, complete Linux/macOS integration, all original
product/release requirements and the single consolidated verification remain
pending. No verified behavior is claimed from this source or its copy inventory.
