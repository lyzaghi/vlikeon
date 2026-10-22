# Explicit encrypted-cloud payload recovery: source authored

Native download handoff and decryption now use an owned private ciphertext file
and one in-place zeroizing AEAD buffer. See `SYNC_MEMORY_OWNERSHIP.md`. Per-step
guards do not make an individual blocking OS/AEAD call preemptible. Historical
limitations below still apply unless explicitly superseded by that source note
or `SAVE_PUBLICATION_JOURNAL.md`; no verification has been executed.

An old acknowledged receipt no longer prevents an explicit request from checking
a saved generation. Native desktop/mobile hosts expose
`save_cloud_recovery_protocol: 1`, with request, per-game status and cancellation
commands. The PWA does not advertise this protocol because its account adapter
remains disabled. No service, request, cipher, device or test has been executed.

The shared history UI offers recovery for a connected, registered, enabled
account on a capable host. The action checks the selected generation and downloads
its cloud copy only when needed. It queues the request and starts ordinary
foreground synchronization, preserving the request if a concurrent job wins the
start race. Refresh shows queued/downloading/waiting/completed/intact/failed/
cancelled status. Global sync status shows transfer/recovery counts. Pending
requests have a cancellation action. No key or plaintext payload crosses this UI.

## Bounded account-scoped requests

Individual JSON records use generated UUID keys in the private preferences
table. The profile is now schema 13 for the separate publication journal; the
service remains schema 2 and PWA storage version 6. See `SAVE_PUBLICATION_JOURNAL.md`.
Each record binds a fresh request UUID, current account, game/save UUID,
captured local-metadata fingerprint and any known canonical fingerprint. At most
64 records of at most 4096 bytes are admitted. Capacity eviction selects only an
older terminal record for the current account; pending work is not silently
removed. These records are request status, not capture attestations or save history.

Creation requires enabled registered-account consent, native credential access
and the existing account/transport serialization. The remote object UUID is the
existing local save UUID. Intentional local removal is respected. No receipt is
deleted or global list cursor rewound. Disconnect clears these requests through
the existing credential/transfer cleanup path. No account key is stored here.
Local history purge removes matching request records in its transaction; the
transport's next bounded cleanup also retires partial downloads for intentionally
removed saves. Purging a request never removes its remote cloud object.

## Guarded execution

The regular reconciliation worker uses its authenticated client, device
certificate, account cipher and lifecycle policy. At most eight requests are
attempted per run. A retained download is prioritized; other requests rotate by
last attempt time. Existing foreground and separately opted-in mobile background
jobs can continue queued work. OS execution is not guaranteed.

The native payload store checks actual local health inside its lease and
transaction. An intact payload completes without a download or replacement.
Missing/damaged bytes require a stopped game and unchanged metadata/origin.
The worker fetches the exact object through the existing signed, resumable,
hash-checked encrypted transport regardless of its old acknowledged receipt.
Header/account/object validation, authenticated decryption and all existing
runtime/content/origin/length/hash rules still apply. Temporary plaintext has a
zeroizing owner and is passed to the immutable repair store. Missing or hostile
remote objects cannot publish replacement bytes or select another head.

An active game/busy repair store defers a request. A request already owning the
single resumable download slot yields the remainder of that run to preserve its
prefix; a busy request without a retained download allows other work to continue.
Network interruptions retain pending state/prefix. Other failures require an
explicit retry. Failed immediate cleanup does not change a successful repair;
terminal requests trigger bounded transport cleanup on a later run and status
reports cleanup debt separately. This is not a physical disk/RSS quota claim.

## Cancellation and publication ordering

Every status update compares the prior complete record. Work must still observe
the same request UUID in a nonterminal state. Cancellation uses its own short
transaction while the worker owns the account lease and can cancel pending work
even after sync consent is disabled. It does not start another account mutation.

Request guards run around header I/O, on both sides of each chunk, during prefix
hashing and before returning ciphertext. Repair hooks also check cancellation,
credential/lifecycle access and current stored consent before staging, between
64-KiB writes and inside publication. An already-blocking HTTP/OS I/O/hash/crypto
operation is not forcibly interrupted; strict asynchronous abort remains due.

Request completion is committed in the same immediate SQLite transaction as the
replacement pointer. An intact observation completes in its guarded transaction.
Cancellation that commits first prevents publication. Cancellation after committed
recovery returns its completed state and cannot pretend to undo repaired bytes.
Lost UI replies do not erase the committed result. Source hooks reuse caller
transactions and do not open nested SQLite transactions.

## Required final evidence

No SQL/CAS/file lease/partial download, HTTP/service, AEAD/hash/credential API,
foreground/background job, browser/UI/device, compiler or test has run. Final
verification must cover cancellation/publication/lost-reply races, per-account
disconnect/revocation, changed/purged metadata, full disk/quota, missing/hostile
objects, interrupted prefixes/cleanup, request rotation, OS visibility/deadlines,
stale UI and actual privacy/timing/memory/platform/release behavior. Automatic
full-history scanning, ordinary-save orphan recovery, broader database/backup
repair, PWA account synchronization and all other original runtime/platform/
streaming/security/store/release requirements remain due.
