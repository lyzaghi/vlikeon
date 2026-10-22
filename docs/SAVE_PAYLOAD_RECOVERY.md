# Saved progress payload recovery: authored source

New native captures now also use the immutable journal and location resolver.
Current native schema/updater floor is 13; see `SAVE_PUBLICATION_JOURNAL.md` for
the original-payload reservation, capacity and publication rules. The sections
below describe the recovery path introduced by schema 12; PWA storage remains 6.

Existing generations now have an explicit recovery path for missing, truncated
or hash-mismatched payloads. A healthy payload remains unchanged. Recovery
accepts only bytes whose exact length and SHA-256 match the existing local
metadata, with the same save UUID, game content, save type/slot and runtime
compatibility fields. It does not create a new generation, select a head,
change a branch, relabel progress or make incompatible snapshots compatible.
All platform and release qualification remains pending.

## User-selected recovery

The shared history view advertises recovery only when Hello reports
`save_repair_protocol: 1`. Its localized action explains that the game must be
stopped and the previously exported backup must belong to this exact generation.
The action obtains an ordinary one-use import capability and passes its opaque
grant and save UUID to `save_repair`. It revokes the grant in a finally block;
stale history/container replies cannot update a new view. Successful responses
distinguish actual recovery from an already intact generation.

Native desktop and mobile hosts decode the bounded existing OPSV0001 envelope,
check its metadata/payload, compare the selected identity with the existing
generation and call the payload recovery store. An imported backup supplies
bytes only. It cannot replace canonical metadata, attach authenticated-remote
provenance, restore an intentionally purged identity or redirect a path. Ordinary
save import still creates a new preserved branch with a fresh UUID.

The PWA uses the same envelope and finite identity checks through its existing
browser file capability. Account synchronization remains disabled in the PWA.

## Native pointer publication

Core/profile schema 12 adds `save_repair_files` and `save_payload_locations`.
Existing generations continue to resolve to `saves/<game UUID>/<save UUID>.bin`
until a committed location exists. New recovery files live in a separate flat
`repaired-saves-v1/<file UUID>.bin` namespace; all IDs are generated and canonical.
No path from renderer metadata or imported content is used as a filename.

A stable zero-length profile lease serializes recovery and its collector across
processes. Committed-location readers retain that lease through lookup, open
and streaming integrity verification, then return an owned file descriptor at
offset zero. This prevents a collection race between selecting and opening a
retired replacement. Windows readers deny write/delete sharing; Unix descriptors
continue to own opened inodes after unlink. This is not protection against an
adversarial owner of the profile or an ancestor-directory replacement attack.

Recovery first validates the local row and any authenticated canonical metadata,
observes the actual existing payload and rejects active game sessions when a
repair is needed. It reserves a staged-file journal row in a committed immediate
transaction before creating a file. It then exclusively creates the generated
file, writes and synchronizes it, verifies its actual contents and synchronizes
the directory on Unix. A second immediate transaction rechecks local metadata,
the selected payload location and session state. Only that transaction publishes
the new location, marks its file live, retires any previous replacement and
records the repair audit event.

The potentially damaged original file is not overwritten or moved. Failed or
interrupted writes cannot publish a location. A crash after journal creation or
file synchronization leaves a known unreferenced staged file. A crash at the
pointer transaction is governed by the existing SQLite FULL/WAL transaction
boundary. Power-loss behavior and directory durability still require actual
fault evidence; the shared Windows directory helper does not claim directory
flush support.

All save reads, uploads, native/Web transfer downloads and exports resolve the
committed location and validate actual payload length/hash. User history and
canonical-origin records remain semantically unchanged. Concurrent metadata
changes, removal, promotion or a new session can reject publication; the staged
file remains journaled for collection. Save commit quotas conservatively include
all repair journal entries in addition to existing logical saves and pending
legacy cleanup. This can double-count an original generation and its replacement;
the 8-GiB/20,000-entry admission policy is not a physical filesystem quota or a
claim about memory/temporary-copy usage.

## Bounded cleanup and removal

Recovery cleanup requires the same lease and visits at most its explicit limit
(eight before a repair, up to 256 for a collector call). It selects only staged
or retired journal rows and rejects a referenced file. Exact UUID `.bin` entries
are removed only when they are ordinary regular files; unknown files, links,
reparse entries and wrong types are not traversed or recursively deleted.
Missing generated files are idempotent. Failures retain journal rows and bounded
attempt counters. The ordinary cleanup status includes staged/retired recovery
debt. Active work can defer collection without losing that debt.

Local history removal still suppresses cloud resurrection and retires original
legacy payloads through its prior queue. Cascading deletion of a committed
location marks the replacement retired in the same transaction. Subsequent
bounded cleanup reclaims it after it is unreferenced. Original and replacement
files therefore follow their own explicit queues. The older ordinary-save
pre-commit orphan window, arbitrary unknown-file recovery and broader profile
backup/database corruption repair remain separate outstanding work.

## PWA atomic replacement

IndexedDB version 6 adds a separate per-save payload revision store, with bounded
backfill and the prior blocked/version-change behavior. New saves create a
revision and local purge removes it. Recovery hashes its input and observed
payload outside the storage transaction. Its strict read/write transaction
compares both metadata and the captured payload revision, checks active leases,
and atomically stores the verified replacement Blob and a new revision. Healthy
payloads are not replaced. A competing repair or metadata change produces a
conflict instead of a stale overwrite. Logical usage, canonical origin, save
generation and heads do not change. Actual browser quota/disk failure and
transaction-abort behavior still need fault qualification.

PWA history-removal previews use declared generation metadata and logical usage;
they do not require a healthy Blob. A damaged payload therefore cannot prevent
the user from explicitly removing that local history to free storage.

## Authenticated duplicate ingestion

Native and PWA trusted ingestion can now recover damaged bytes when they receive
an authenticated/decrypted duplicate of the same generation. Full known
canonical metadata must match; legacy canonical observations use the existing
origin rules. A user-selected backup never takes this trusted path.

Native synchronization retains a busy recovery object in its durable encrypted
inbox, records no receipt and continues other entries; status reports the
deferred count and tells the user to close the game and sync again. This is
recovery of an object actually being ingested. Existing acknowledged cloud
receipts do not trigger an automatic all-history integrity scan. A separate
explicit cloud request/queue path is now authored in `CLOUD_SAVE_RECOVERY.md`;
comprehensive automatic background integrity/recovery policy remains due.

## Schema, evidence and remaining work

The native schema and fresh updater data-schema floor advance to 12; the service
schema remains 2. Existing updater state records newly opened schemas through
its monotonic floor API. PWA storage advances to 6. No migration, SQL/IndexedDB
transaction, file lease, I/O/collector, hash/cipher/sync, UI/device/compiler or
test has run. No original requirement is marked complete by authored source.

The final consolidated pass must cover healthy/missing/truncated/wrong-hash
payloads; wrong UUID/runtime/content/input; conflict/legacy origins; every
journal/write/sync/publication boundary; repeated recovery; old/new readers and
file locks; quota/full disk; active/stale sessions; concurrent repair/promotion/
purge; interrupted mobile/background work; deferred sync receipts; file grant
cancellation and stale UI; cross-platform OPSV behavior and actual security/
backup/uninstall/store/release evidence. Broader physical quotas, ancestor-handle
containment, complete provenance envelopes, all original runtimes/platforms,
streaming and release work remain required.
