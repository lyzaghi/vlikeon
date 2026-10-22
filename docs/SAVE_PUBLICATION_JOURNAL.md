# Journaled new-save publication: authored source

New native captures, imported branches and newly ingested cloud generations now
use the same immutable payload journal as recovery files. This closes the authored
new-writer window where a final UUID payload could be published before its SQL
row and then be left without a journal after interruption. No save, SQL, file API,
compiler or test has been executed; actual crash durability remains unqualified.

## Before a payload exists

`SaveStore::commit_origin` validates the input identity, byte length/hash and
expected parent shape, then obtains a staged-payload owner. That owner retains
the stable profile payload lease through the later metadata commit. It checks
the game identity, rejects an existing/intentionally removed UUID and reserves
storage in an immediate transaction. A generated file UUID and full expected
payload identity are committed to `save_repair_files` in `staged` state before
file creation. The historical journal/table/directory names are retained for
compatibility; they now cover both new payloads and replacements.

The native host exclusively creates the private UUID file, writes bounded 64-KiB
chunks, synchronizes it, re-reads/verifies its exact bytes and synchronizes the
directory on Unix. It does not publish an old `saves/<game>/<save>.bin` path or
replace any existing file. A failed creation/write/sync or process termination
leaves only a known journal reservation and possibly an unreferenced file.

## One metadata publication transaction

After staging, the save transaction rechecks game membership, local suppression,
parent domain, current head and greatest generation. It assigns local order and
conflict state at this actual commit boundary. Canonical-origin graph admission
still precedes insertion of visible metadata. Concurrent parent/head changes
retain the existing branch policy instead of being decided by an earlier I/O
snapshot or wall-clock time.

The transaction inserts the immutable save/origin, verifies the staged identity,
marks its file live and inserts its committed location. It removes the temporary
legacy-original reservation inserted by the fallback trigger, rechecks final
capacity, updates the head when eligible and records the save audit event. All
of these SQL effects share the same commit. Until it commits, readers have no
visible save/location pointing at the staged bytes. After it commits, the
existing verified payload resolver supplies the owned descriptor for native
restore, Web transfers, synchronization and exports.

Rejection during parent/origin/admission or failure during SQL commit leaves
the staged row/file for collection. Dropping the staged owner releases the actual
OS lease; it does not guess whether a fallible SQL commit succeeded and delete
a possibly live file. The bounded collector independently requires an
unreferenced staged/retired row. It cannot collect live committed locations.

## Schema 13 and storage reservation

Native schema 13 adds `save_legacy_payloads`. Migration reserves the expected
original UUID payload of every existing save conservatively, including legacy
generations with a separate repaired location. An insert trigger preserves this
reservation for an original-path writer. A current journaled writer removes
only its own fallback reservation in its publication transaction.

Byte admission sums known legacy originals, pending legacy cleanup and every
staged/live/retired journaled file. The entry count is logical saves plus pending
legacy cleanup and staged/retired journal debt. A normal newly captured file
therefore does not consume two logical-save entries or two byte reservations
after publication. A retained damaged legacy original and its healthy replacement
both remain reserved, and repeated replacement debt remains counted until
collected. The existing 8-GiB/20,000-entry policy is application accounting;
unknown files, filesystem overhead, memory, temporary transfers and kernel disk
quotas are separate. It is not a measured physical-size or constant-RSS claim.

History purge queues only reserved legacy originals in the legacy queue. Deleting
a committed location retires its journaled file in the same transaction, and
the existing bounded collector reclaims it afterward. Logical removal counts
are still saved-generation metadata counts. Unknown files are never interpreted
as permission for recursive deletion.

## Bounded recovery and limitations

Profile open attempts at most 32 existing cleanup records per queue; a new staged
capture attempts eight journal records before reserving more space. Explicit
cleanup retains its existing bound. The same payload lease prevents a collector
from touching an active staged writer; cleanup failures/busy files retain durable
debt and do not make an otherwise readable profile fail to open. No background
thread or unbounded filesystem traversal is started by this path.

Existing canonical metadata, generation/head semantics, OPSV wire bytes, save
compatibility and the PWA's strict atomic Blob transactions remain unchanged.
Native schema/updater floor advance to 13; service schema stays 2 and PWA stays 6.
Older app processes already open across an upgrade, all arbitrary pre-journal
orphans, unjournaled import/transfer assets and database corruption require
separate compatibility/recovery treatment. No bytes/metadata are invented for
unknown historical files.

The final consolidated pass must inject faults before/after journal commit,
every write/sync/hash, save/origin/location/head/audit insertion and final commit;
exercise process death, uncertain commit outcomes, lock/collector races, full
disk/quota, migration/fallback writers, repeated repair, purge, read/export/sync,
real directory durability/permissions and platform backup/uninstall behavior.
Windows directory flushing, adversarial ancestor replacement and physical quotas
are not qualified by these source helpers. All original runtime/platform/
streaming/security/store/release requirements and verification remain due.
