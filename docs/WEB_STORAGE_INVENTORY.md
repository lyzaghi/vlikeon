# Browser stored-data inspection source

This source adds an explicit readonly `storage_inspect` command and a localized
button in browser Settings. It is independent of cloud consent, service policy,
account connection, journal health and cryptographic artifact approval. The
existing quick storage status remains separate, so opening Settings does not
automatically walk the complete content inventory.

The inspection opens one readonly transaction over config, games, firmware
metadata and the six known binary-bearing stores: assets, saves, firmware data,
sync slots, download parts and encrypted inbox. It reads cursor values one at a
time, adds immutable Blob.size values and returns only aggregate counts/sizes.
It never opens payload ArrayBuffers, hashes/decrypts content, reconstructs a
save, imports/exports data, fetches the network or changes any record. Object
IDs, content names, account/device identities, keys and game paths are absent
from the response. Download-slot reservations without payloads are not counted
as stored content. Unexpected download-slot payloads are counted and flagged.

Every scan checks request cancellation and has finite row admission: 200,000
assets, 20,000 saves, 128 firmware data objects, two sync slots, 1,025 download
parts and 32 inbox entries. The cursor work shares a 15-second monotonic sampling
budget. This is a cooperative budget, not a guarantee on browser scheduling,
transaction admission or storage latency. The existing bridge deadline and
transaction abort remain the outer cancellation boundary. A platform instance
admits one inspection at a time and releases admission in finally on failure.

Hitting a row/time bound returns `complete: false` for that store, with observed
values as lower bounds. The aggregate result is complete only when all six
stores ended naturally. No truncated scan can establish an accounting match.
Missing Blob values, zero/oversized values and disagreement with declared asset,
save or ciphertext length are counted separately. Aggregate asset/save payload
bytes, save/game counts and firmware metadata/data counts are compared against
the existing declared accounting in the same transaction. Missing/invalid
accounting is reported as unknown rather than repaired or normalized to zero.

The UI identifies partial results, displays per-store content sizes and sample
time, and separates browser-origin usage/quota estimates from content sizes.
It advises exporting important saves when actual size/accounting discrepancies
are observed. Locale changes reuse the snapshot without rerunning inspection.
The browser-origin estimate is the separately sampled quick status and can
include caches, database metadata and storage overhead. Blob backing data may
be shared between references, so their sum is explicitly not physical disk
allocation. It is not a checksum result, an orphan-ownership proof or permission
to delete any object. Matching byte/count totals do not prove record identity.

Inspection can coexist with other tabs. Its readonly transaction serializes
against overlapping writes while active; results are a historical snapshot,
not a durable reservation or authority over later changes. Full physical quota
enforcement, owned orphan cleanup, ancestor/backup accounting and the remaining
original product/platform/release requirements remain incomplete.

No browser, IndexedDB, Blob inspector, service, compiler, build or test has been
run for this source. After all original authoring, the one consolidated final
verification must cover empty/exact/over-limit stores, corrupt/missing payloads,
partial and cancelled scans, timeout and close, byte/count mismatch, retained
encrypted data with a corrupt journal, unapproved crypto/missing service policy,
readonly behavior, concurrent writes, instance admission, locale changes and
real browser storage/memory/transaction latency. These sources do not provide
runtime or cryptographic success evidence.
