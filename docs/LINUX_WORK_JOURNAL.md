# Linux pair work journal source

`platform/native-runtime/src/linux_work.rs` now authors a private session work
owner and immutable acknowledgements around the lower-level GNU/Linux pair.
It is not selected by NativeRegistry/NativeAdapter and has never created a
directory, acquired a lease, hashed/admitted a product binary, launched a child
or exercised a product API. Linux native availability remains false.

## Durable ownership chain

The source creates a fresh canonical UUID beneath the profile's private 0700
native-work directory. Existing links, different directory identity or ownership/
mode are refused. It retains the observed O_PATH directory identity, writes a
Linux work intent, creates independently locked private 0600 owner/actor/UI
leases, creates private empty root and presentation-tmp directories and writes
the owned acknowledgement. It syncs directory creation and the lease files.
Partial creation failure leaves conservative unclaimed storage; it does not
silently delete a directory that may need recovery.

Every journal body is closed-schema JSON of at most 4096 bytes, published as a
new immutable record with the existing PendingFile file-sync/hard-link/directory-
sync flow. Existing records are never overwritten. Records bind profile/work,
the observed work-directory device/inode and the literal Linux target. The
work path is checked against its held directory before each publication/read.
Record reads use O_NOFOLLOW/CLOEXEC/nonblocking open, admit only an actual
current-user single-link 0600 regular file, then bound the read. This avoids
opening a replaced FIFO as a blocking journal read. The private host work tree
is outside the core's immutable namespace root; these records are not renderer
or actor commands. Separately authorized host-account writers are not treated
as hostile-sandboxed principals.

The delegation's before-create callback writes `linux-pair-intent.json` with
the exact profile/parent identity and session. Per-leaf callbacks write distinct
actor/UI group acknowledgements with parent/leaf device/inode, role and charged
memory ceiling. Only the same profile and UUID session can be acknowledged.
The intended composition uses `Delegation::create_pair` with these two methods;
the actual provisioning/product factory still must supply that composition.

`LinuxWork::launch_pair` requires managed groups matching those acknowledgements,
the exact actor/UI lease identities, the held private root and the exact UI
temporary directory/path. It writes both launch intents, including held reviewed
executable hashes/lengths, before starting either child. The private launcher
then returns each live observed boot/PID/start-tick identity to this work owner
and commits a distinct process acknowledgement before its corresponding gate
can release. An inconsistent, failed or interrupted record refuses release and
the owned launch guards perform their normal stop/retention flow.

Actor and UI lease clones enter their respective existing pidfd/cgroup resource
owners. Dropping the work-journal object does not release those child-held
broker leases prematurely. There is no automatic work-directory deletion in
this source owner. The generic abandoned-work collector refuses both Linux
intent and owned records so it cannot treat them as an unrecorded Windows
staging directory. The current Linux collector remains unavailable.

## Remaining integration and recovery

This is authored durable-write/launch composition, not a complete crash recovery
backend. A future collector must acquire all applicable work leases, authenticate
the private profile/work and delegation/leaf identities, prove exact process and
resource-tree exit and asynchronous charge retirement, and retain unknown
creation/launch acknowledgements. A missing process acknowledgement cannot be
treated as proof that no child ever existed. Disk PIDs must never authorize
signalling. Reclamation must use confined bounded traversal and include staged
content/cache/save accounting before deleting storage or releasing reservations.

The signed complete ELF package, immutable profile cache, game/firmware/save
staging, legit host delegation provisioning and migration, NativeRegistry/
NativeAdapter, compositor activation and final-save/control-channel flow remain
required. No catalog, platform, capability or release approval was enabled.
The single final verification must cover real filesystem/fsync/lease semantics,
record/gate/cancel/crash ordering, pair startup failure, identity reuse, uncertain
exit, storage retention/recovery and actual product/driver/core behavior after
all original source authoring. All original product/release work stays active.

`LINUX_IMAGE_PACKAGE.md` now supplies separate authenticated image-package and
shared sealed-cache source for future composition with this journal. It does
not connect the product adapter or implement game/save staging and crash
recovery. Those requirements remain outstanding.

`NATIVE_WORK_STORAGE.md` now adds durable schema-14 copy reservations before
work-directory creation. `LINUX_INPUT_STAGING.md` authors the checked game and
user-firmware copies, immutable bounded receipt and mandatory staged-ledger/
inode binding in launch_pair. The product adapter/control/save path, actual
provisioning, existing-tree crash recovery and confirmed work reclamation remain
unfinished. No database/migration/process/source-input operation was run.
