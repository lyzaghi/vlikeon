# Local save removal and retirement source

This is authored source only. No SQLite/IndexedDB migration, file deletion, quota
operation, synchronization, application, compiler or test has been executed.

The game detail dialog now exposes explicit removal of **all local saved progress
for that game**, including current heads. The application first obtains the actual
record count, logical byte count and a revision bound to the game's identity,
saved metadata and current heads. Its confirmation states the scope and asks the
player to export anything they want to keep. Backend removal rechecks the revision
under a write transaction; a concurrent save, promotion or other relevant change
requires a fresh preview/choice. A live game session prevents removal. The count covers all stored history. The separate bounded paging view can now
reach records beyond the old 500-row window; see `SAVE_HISTORY_PAGING.md`. Bulk
backup remains required work and is not implied by this removal control.

## Native transaction and owned-file queue

Schema 0009 adds `locally_removed_saves` and `save_payload_gc`. The immediate
SQLite transaction records removed save IDs, queues their exact generated
game-UUID/save-UUID payload names, drops associated local outbox/receipt/head rows,
deletes that game's save records and writes an audit event. It does not delete the
game, ROM, firmware, installed-game directory or any remote service object.
Foreign-key restrictions still reject an inconsistent cross-game lineage.

The durable queue separates committed logical removal from filesystem cleanup.
Each cleanup call considers at most 128 files (the core accepts no more than 256).
It requires a matching local-removal marker and absence of a live save record,
parses UUIDs, derives paths inside the profile's save root and refuses nonordinary
or linked/reparse roots, directories and leaves. It removes only the exact known
`.bin` leaf; no recursive traversal or deletion occurs. Empty game directories
remain so cleanup cannot race a new save's directory-creation sequence. Failures
keep queue entries, and bounded attempt ordering lets other queued files proceed.
Unknown/orphan/pending files are not adopted or erased by this feature.

After metadata removal, the command attempts one cleanup batch and returns actual
remaining queue counts. Settings exposes status/resume/stop-after-current-batch.
A user-authorized removal can continue bounded cleanup in the background, with a
finite batch budget and a visible retry path. Closing the UI does not revoke the
already-authorized retirement of those exact files; unfinished work stays queued.
No pathname is returned to the UI or written into free-form error text.

These path checks are not a handle-relative filesystem proof against an external
same-user attacker swapping ancestors. Contained runtimes are not granted write
access to this broker-owned save tree. Directory synchronization uses the existing
platform abstraction: Unix requests directory fsync; Windows currently lacks the
equivalent implementation/evidence. Filesystem/power-loss/locking behavior remains
required final work. Counts describe logical records/queued bytes, not guaranteed
physical free-space reclamation, secure erasure or deletion of SSD/cloud backups.

## Synchronization behavior

Deletion is local. UUID/content-hash markers persist after the save/game record is
removed, so an already-known encrypted cloud object does not silently resurrect
that intentionally removed save. Reconciliation skips those object IDs, discards
staged matching inbox items and checks again when an import races removal. The
save commit transaction also rejects reuse of a locally removed UUID. An already
in-flight network operation may finish or fail after removal; no server-side delete
is sent. Other devices and later, newly identified progress keep their own state.
An intentional manual import creates a new save UUID and remains available for
restoring a previously exported file. Profile reset/recovery and a fuller user
interface for reversing suppression still require product work.

## Web storage and accounting

The original origin/database name upgrades to IndexedDB version 3, adding local
removed-ID records while retaining version-2 favorite metadata/indexes. Preview
hashes metadata incrementally through an indexed cursor without reading payload
bytes. A strict write transaction checks cross-tab live leases and the revision,
records removed IDs, deletes records/heads and adjusts accounting together.
Transaction abort retains the old state. Blob physical reclamation is controlled
by the browser; this backend has no native retired-file queue.

Native save admission now mirrors the existing Web logical limits: at most 20,000
live/retired records and 8 GiB of live/retired payload bytes per profile. SQLite
admission happens under the save write transaction; queued files remain charged
until cleanup succeeds. This is a logical quota. It does not bound unidentified
orphans, pending writes, native scratch/AppContainer storage, firmware, database
overhead or all physical disk writes. Those storage controls remain required.

Direct paused progress export now has source that bypasses logical history quota;
see `LIVE_SAVE_EXPORT.md`. Bulk backup, orphan recovery,
provider/physical-storage handling and the full remaining storage/platform scope
are still required before implementation is considered complete.
No release readiness or protection against all disk-full failures is claimed.

The current profile schema and new-install updater floor are 10. Existing updater
stores retain the opened-schema high-water path; signed rollback metadata must
include the actual observed schema. The final consolidated pass must cover fresh
and interrupted upgrades, foreign keys/CAS, active sessions and multiple tabs,
read/upload/download/removal races, local suppression and new remote progress,
stalled/malformed cleanup entries, missing/locked/reparse paths, ID reuse, resource
limits and disk-full/power-loss behavior, with no data loss outside chosen scope.
