# Standalone browser firmware vault

Authored standalone Web/PWA source now supplies the firmware commands already
used by the shared settings/player. The native desktop/mobile vault stays its
own implementation. No browser, IndexedDB migration/transaction, file picker,
Blob/hash worker, core, UI, compiler, server or test has run. This is source
progress, not an admitted advanced core or qualified browser release.

## Origin storage and upgrade

The original `omniplay-local-v1` database upgrades to IndexedDB version 4 with
separate firmware metadata, Blob and role-selection stores. Existing games,
saves, heads, preferences, tombstones and leases are retained. The selection
store has an exact content-hash index for deletion. Metadata enumeration never
reads every firmware Blob. Existing upgrade blocking/abandonment and connection
version-change handling remain in force; live multi-tab upgrade/device recovery
still needs the final pass.

The vault has at most 128 objects of at most 512 KiB each (64 MiB logical maximum)
and exactly the eight supported SNES/MD/PS1/DS roles. All role/size combinations
reuse the shared WASM policy and match the native vault's existing sizes.
Metadata/selection count, schema, hash, revision, timestamp, reference and size
admission is bounded. These are logical application limits; browser quota,
eviction, user clearing of site data and filesystem durability remain separate.

Publication, reselection and deletion require the existing exposed strict
transaction policy and await completion before acknowledgement. Quota admission
and metadata/Blob/selection changes share a read-write transaction, including
cross-tab concurrency. Firmware accounting stays separate from content/save
accounting and is reported in storage status. No existing save-usage schema is
rewritten to introduce firmware counters.

## User selection, integrity and ownership

`firmware_import_selected` validates the requested role before opening the
existing local file picker. Its single-use selected-file grant is consumed in
the same request. Only an exact allowed file size enters the vault. It copies
at most 512 KiB into an immutable Blob, hashes that same snapshot in a dedicated
worker, then publishes it under its SHA-256 identity and selects the role.
The object retains no original filename, path or external handle. No firmware
URL/download, manufacturer BIOS or game data is supplied by this implementation.

Reimporting identical content can repair a missing/corrupt stored Blob using
newly verified bytes with the same content identity. Its revision changes so
concurrent stale admissions must retry. Existing creation time is retained.
Ordinary reselection verifies actual Blob bytes outside the transaction, then
checks the same object revision again in the publication transaction. Missing,
wrong-size or checksum-invalid content is rejected; size/hash checks do not
assert ownership/provenance or game compatibility.

There is one firmware mutation and at most four game-admission hash operations
per page, plus existing platform request admission/deadlines. These operations
do not create uncapped import jobs. Abort/deadline/close retires hashing and
prevents later transactions; a small already-started Blob read may finish before
its next cancellation observation. No hard OS read preemption is claimed.
Lost acknowledgements can be reconciled by refreshing the idempotent inventory;
a deadline is not evidence that an earlier committed transaction rolled back.

## Player transfer boundary

`firmware_read_start` takes a library game UUID, role and expected selected hash.
The backend derives the system from the stored game and requires the current
selection/allowed size. It hashes the actual stored Blob, then rechecks the game,
selection, object revision and data presence in another transaction. Changes
during verification reject admission. It returns the existing bounded Blob
transfer shape; the transfer verifies its final hash and the worker repeats
policy/checksum admission before mounting `/system` read-only.

No arbitrary historical hash or renderer filename bypasses the selected role.
The existing 48-KiB sequential chunks, four-download limit, transfer expiry and
abort rules still apply. A committed deletion removes the object/metadata and
all indexed selections atomically. Already admitted transfers or running games
can retain their immutable copy; deletion is not a claim of revoking every
in-flight Blob/core byte. Future game openings must select/import again.

Firmware identities remain part of the existing configuration hash and therefore
snapshot compatibility. There is no new portable-save declaration. Actual
BIOS/region/core matching remains required. Mutable system calibration/config
and complete PSP runtime assets still need their separate product paths.

## Shared UI and privacy

Standalone hello now advertises the local vault, so the existing Firmware panel
can import, select and delete through this backend. Shared UI inventory bounds,
mutation ownership, stale-refresh rejection, selected-role retention across
refreshes, readable role labels and count/byte summaries are authored. The
standalone storage panel reports firmware bytes separately. Empty or corrupted
metadata is never invented as a valid zero-byte firmware asset.

User firmware lives only in this origin's local database. Service-worker caches
contain inventoried application resources only; the new `web/firmware.js` is
application code, not firmware payload. Firmware Blob bytes, selected hashes and
roles are not added to diagnostics, sync, telemetry or remote requests. This
does not establish protection from a compromised same-origin application or
prove browser retention. All original security/deployment requirements remain.

## Remaining consolidated evidence

The single final pass must exercise fresh/version-1/2/3 upgrades, blocked/closed
connections, strict-transaction absence, quotas/eviction and concurrent tabs;
role/key/schema/hash/count/size/timestamp/revision boundaries and corrupted
records; picker activation/cancel/deadline, mutable provider reads, exact
immutable snapshots, import dedup/repair, selection races and delete/transfer
ownership; hash-worker retirement, bounded chunks/final checksums and read-only
core mount; retained game/save/config data, firmware-dependent snapshot
compatibility, Settings/accessibility/localization and real desktop/Android/iOS
browser memory/latency/storage/device evidence. No advanced core binary or
firmware is supplied. Every other original product/release requirement stays due.
