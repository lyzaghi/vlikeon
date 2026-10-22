# Explicit unused library content collection source

Browser library removal previously retained its content-addressed media Blobs,
which also remain charged to the library's 56 GiB logical asset limit. This
source adds a separate user-reviewed collection flow in browser Settings.
Deleting a library entry does not automatically run collection. Nothing runs
on opening Settings, periodic scheduling, cloud consent changes or startup.

`storage_unused_preview` requires exactly `after_hash` (null or a content SHA-256
cursor). It selects at most 64 asset records in key order and provides a next
cursor when further records exist. Every selected asset must be an ordinary
closed three-field record with matching key/hash, finite declared length,
actual Blob and exact Blob size. Bad records stop collection; unknown state
does not become permission to delete data. The preview covers one batch, not
the complete asset store. Skipping to the next batch makes no deletion claim.

Before declaring any candidate unused, it scans the complete game ownership set
(at most 50,000 games), normalized per-game media manifests (at most 128 assets
per game), saves (at most 20,000) and lifecycle records (at most 64). Legacy
single-ROM game declarations use the existing media normalization rules.
Any candidate referenced by a game or a save content identity is retained.
Shared media length disagreements or malformed owners stop the scan. A save
with no library game blocks collection because its missing media ownership
cannot be reconstructed safely. An unexpired ready/running/paused game lease
also blocks collection. Close games and restore/manage orphan progress first.
Firmware, encrypted sync data, save payloads and origin records are never
collection candidates. The original ROM files outside browser storage are
never accessed or deleted.

The complete ownership scan is required even for a small candidate batch.
Cursor events check cancellation and a shared 15-second cooperative scan
budget. Incomplete reference scans fail, preserving all assets; unlike the
readonly size inventory, they cannot return partially authorized deletion.
This budget does not guarantee browser scheduling/storage latency. The bridge
deadline and transaction abort remain the outer cancellation boundary.

A streaming framed SHA-256 preview revision binds the local profile, input
cursor, asset identities/lengths, current declared asset bytes, complete scanned
game/media and save ownership, resulting candidate retention decisions, counts
and whether another asset page exists. It is a change fence, not a secret,
cryptographic content attestation or capability to delete unrelated records.
No payload is hashed or opened, and no root/account key is involved.

`storage_unused_remove` requires exactly the reviewed `after_hash` and
`revision`. It repeats the entire snapshot inside one strict readwrite
transaction containing config/assets/games/saves/leases. The revision must
still match. Deleting only the resulting unused asset keys and decrementing
the exact declared asset byte charge commit atomically. Any ownership error,
scan bound, cancellation, mismatch or database failure aborts the transaction;
no partial batch deletion or partial accounting adjustment is intended. No
automatic accounting repair is inferred from discrepancies. Import publication,
library mutation, saved-progress publication and session lease changes already
overlap these stores and therefore serialize with this final ownership check.
New imports retain copied snapshots and publish their assets in that same
atomic library commit; a later import can recreate its required content.

Each platform collector admits one scan/removal at a time and releases the
admission in finally. Multiple tabs rely on IndexedDB transaction serialization
and the revision fence, not a JavaScript singleton. No long-lived preview lock
is held while a user considers the choice. Replaying an already deleted batch
changes the preview and cannot erase a different group through the old revision.

The Vietnamese/English UI displays the batch's candidate count and content size,
lets the user inspect further groups, and requires a concrete confirmation
before removing any group. It explains that removed games require original
files to be imported again. Success reports removed Blob-reference bytes;
physical disk reclamation is controlled by the browser and is not promised.
After removal it can inspect the next group, but never removes another group
without a separate choice. A stale preview requires refresh and review again.

This is collection of owned, unreferenced library asset records only. It is not
whole-origin physical quota enforcement, recovery of malformed metadata,
orphan-save import, firmware/sync cache deletion, ancestor/backup accounting or
proof of a complete original product. Every remaining original requirement
stays active. No IndexedDB/browser, product API, compiler/build or test was run.

The single final verification after all original authoring must cover shared
single/disc media, valid legacy media, removed entries, no-library saves,
malformed owners/Blobs, accounting underflow, exact/over-limit scans, active and
expired leases, empty/end/multiple pages, skipped batches, stale/replayed
previews, import/save/session races across tabs, cancellation/deadline/close,
strict transaction rollback, localization/confirmation and actual browser disk
and memory behavior. Source inventories are not runtime or deletion evidence.
