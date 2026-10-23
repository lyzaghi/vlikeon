# Portable backup and retained-data recovery

Authored source, not executed acceptance. No build, test, real profile migration,
backup export, restore or cleanup was run while authoring these changes.

## Multi-save portable archive

Native desktop/mobile and Web/PWA expose `save_backup_protocol:1` with
`save_backup_export` and `save_backup_restore`. History offers export of the
currently loaded page (at most 50 records) and import of a selected archive.
This is a bounded group operation, not a one-click full-profile/database backup.
Other history pages must be exported separately. Existing single-save exports
remain available for entries too large for a group.

The format is `OPBK0001`, a little-endian u32 JSON length, UTF-8 JSON manifest,
concatenated payloads in manifest order and a 32-byte SHA-256 trailer. Hash input
is `Vlikeon portable saves archive v1\0` followed by all preceding archive bytes.
Limits: 64 distinct saves, 128 MiB total, 4 MiB manifest, 16 prior archived origin
records per entry and 256 KiB retained reference per imported entry. Hitting a
limit refuses the operation; historical origin data is not silently truncated.

Each manifest entry preserves local metadata, the complete current origin,
whether it was a head and flattened prior archived metadata/origin/head records.
Repeated export of a restored branch carries its earlier references forward.
These references are descriptive and never upgrade imported content into an
authenticated remote save. Neither game binaries, paths, credentials, firmware
nor executable launch specifications are included.

Export captures metadata/origin/head/revision identities, verifies actual payload
bytes and rechecks the captured identities. Native output starts with an invalid
magic and publishes the real magic only after the remainder is flushed. PWA
uses its existing file writer or visible browser download; download initiation
is not a durable-file acknowledgement. User originals are not deleted.

Restore validates the complete archive and every payload before its first write.
It matches current library content by hash and creates separate imported
branches without selecting their heads. Full archived origin references commit
with each new generation. Each generation is its own transaction; partial
success is reported per source ID and must not be represented as whole-archive
atomicity. Active game leases/sessions prevent new branch publication.

Imported IDs derive from archive fingerprint, source UUID and a little-endian
attempt counter under `Vlikeon backup imported branch v1\0`. Retry reuses a
previously restored intact generation and verifies its retained reference.
An explicit later import after a local purge selects a new derived UUID instead
of removing suppression markers or reviving the intentionally removed UUID.
The maximum derived attempt count is 128. A damaged existing restored generation
returns a recovery error; it is not overwritten through this import path.

## Retained library content

`library_recovery_protocol:1` adds a Settings inventory and reviewed reimport.
Native candidates are ordinary content-hash files and canonical UUID
`.pending-*` snapshots in the private assets directory. Unknown names, links,
special files, zero/oversized files remain retained. Native inventory and open
hold the same asset lease used by import/collection. Reimport keeps an owned
descriptor, releases that lease and enters the normal import copy/publication
path; the old source is never moved or removed. Existing content identities
must pass their actual checksum before reimport. A changed review is refused.

Web recovery inventories immutable asset Blobs, verifies the selected actual
checksum and uses the existing cross-tab import lease and normal publication.
Abort cancels import preparation; a commit already admitted can still finish.
Reimport deduplicates by normal content identity. A page shows at most 64 valid
candidates. Unknown counts are observations of that inventory pass, not a claim
that all origin storage has been scanned.

The inventory may include content still owned by a library entry: reimport
returns the existing entry. It does not infer a multi-file disc layout, recover
missing track names or declare raw bytes a valid save without metadata.

## Native orphan save payloads

`save_orphan_protocol:1` exposes preview/recover from each save-history entry.
Only an existing validated local save supplies metadata, compatibility, lineage
and the required payload hash/length. Candidate names are canonical UUID `.bin`
leaves in that game's legacy folder or the flat repair directory. The payload
lease and an immediate transaction serialize inventory with known ownership.
Known live/staged/retired files, intentional-removal markers and queued legacy
retirements are excluded. A candidate must match actual length and SHA-256.

Each page admits at most 64 inspected leaves and 128 MiB of matching-length
hash reads, under a cooperative 30-second budget. Directory inventory caps at
20,000 entries. Continuation preserves an unprocessed byte-budget candidate.
Recovery checks the reviewed revision, then calls the existing fenced repair
store to journal a fresh immutable copy. It preserves the orphan original,
metadata, current heads and authenticated origins. The same existing active
session/metadata/location checks govern publication.

An unknown file with no trustworthy metadata cannot reconstruct its lost game,
runtime compatibility or lineage. It remains retained for manual analysis;
there is no automatic guessing, provenance promotion or deletion. Web/PWA has
no native loose-file namespace and does not advertise this native scanner.

Sequential item 50 now supplies a separate manual inventory, inspection,
unverified recovery package and extraction tool for these retained bytes,
including unknown filenames and Web Blobs with invalid/missing metadata.
See `MANUAL_DATA_RECOVERY.md` for its executed acceptance and limits. This does
not upgrade unknown bytes into a compatible or authenticated save.

## Acceptance still required

Four additional Rust fixtures cover archive roundtrip/retry/head preservation,
corrupt-before-write rejection, active-session refusal and exact orphan-copy
recovery. They are authored, not compiled or executed. Later execution must
also cover native/PWA interchange, origin-history limits, explicit reimport
after purge, concurrent restoration, full disk, partial completion, source
replacement, cursor budgets, denied/link/reparse roots and actual device storage.
Full-profile/database repair and physical storage quota controls remain open.
