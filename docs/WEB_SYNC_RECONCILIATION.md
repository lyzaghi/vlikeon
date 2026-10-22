# Browser authenticated save reconciliation and foreground runs

This is source authoring, not browser/service or release qualification. No
migration, compiler, browser, IndexedDB/Blob/Web Lock, worker, hash, cryptographic,
Fetch/account operation or test was executed. The actual crypto artifact and
service policies remain disabled, with no admitted executable or service origin.
Consequently the generated product still advertises encrypted_sync false and
hides the account UI. The source now derives that capability from admitted crypto,
finite service policy and the required secure browser facilities; it no longer
hardcodes an unfinished reconciler as the reason to disable it.

## Authenticated publication and acknowledgement

`WebSyncReconciler` composes the existing Rust envelope open operation, retained
ciphertext journal and `WebSaves.ingestRemote`. Only the authenticated envelope's
metadata and actual payload are submitted to local publication; object UUID,
declared payload length and content integrity must agree. The local game UUID
comes from its content hash. Canonical remote metadata remains unchanged in the
origin ledger even when local generation ordering, game UUID, conflict projection
or missing-parent projection differs. Existing branches and heads follow the
existing causal save-store rules, without clock-based overwrites.

Existing-generation recovery validates the canonical origin before modifying
payload bytes. A known canonical UUID cannot acquire another origin. A legacy
projection can acquire an authenticated canonical origin only if the checked
local projection agrees. Changed or missing payload bytes are replaced under a
fresh payload revision, and an active game lease defers damaged-payload recovery.
Healthy bytes, other metadata, heads and accounting remain retained. Hash work in
the authenticated path uses dedicated workers outside transactions.

Save publication commits before inbox acknowledgement. The acknowledgement then
captures the checked local canonical origin, payload revision and immutable Blob,
hashes actual local bytes, and rechecks the same revision/origin in its strict
write transaction. That transaction publishes a canonical receipt, releases a
matching upload slot if present, removes the exact inbox object and adjusts quota
and inbox count. A crash between the two commits leaves an encrypted replay copy;
replay checks the already present generation instead of duplicating it. No
acknowledgement claims filesystem durability beyond the browser's contract.

An immutable upload conflict is not accepted from HTTP status, ciphertext size or
an equal plaintext hash alone. The remote encrypted object must first be retained
and opened with the account cipher, then pass save projection/canonical-origin
publication and actual local payload checks. Canonical uploads require the full
origin fingerprint. Legacy upload receipts use their explicitly reduced
projection fingerprint until authenticated canonical information is available.
Different payloads or origins leave local progress and both encrypted copies
retained. Retiring a matching local upload slot after equivalence does not assert
that an unrelated/expired server partial has been cleaned up; service expiry
still governs retained remote partials.

## Cursor, removal intent and bounded traversal

Complete inbox publication still precedes cursor advancement in the transfer
transaction. Known receipts, durable inbox copies and checked local removal
tombstones can advance a listed sequence without downloading again. Their exact
UUID/sequence/size bounds and the expected prior cursor are checked; unknown
entries cannot take this shortcut. A comparison download's null sequence can be
attached to its first actual list sequence without inventing a new download.

A valid local removed-save tombstone suppresses an inbox copy without decrypting
it. A matching partial download is retired only after rechecking that removal
intent in its retirement transaction. A matching outgoing partial is aborted by
its exact owner before its local ciphertext slot is released. Unknown or
unexplained missing-source staging is preserved for recovery rather than deleted.
Ordinary receipt skipping does not continuously rehash local payloads or promise
to detect silent local corruption; explicit cloud repair remains separate work.

Standalone IndexedDB version 8 adds the global compound save ordering index
created_at_ms/generation/UUID. Initial traversal requires index and store counts
to agree, with a 20,000-generation bound. Pages read at most 101 keys and emit at
most 100 checked identity/fingerprint/revision summaries; actual payload hashing
occurs when a generation needs an upload. Each run attempts at most 500 uploads
and 20 remote pages of 100 entries. Reaching the outgoing upload budget still
permits incoming work. The summary distinguishes remaining work and retained
unknown-game or active-game-deferred copies from a completed transfer.

Unknown-game ciphertext stays in the 32-object inbox. Full cache, corrupt input,
inconsistent staging or immutable identity conflict stops further mutation and
does not silently evict a recovery copy. These bounds do not guarantee that every
remote account can be drained in one opportunity.

## Explicit consent and operation lifetime

The config consent record binds profile, account and device to a fresh epoch.
Enabling requires a foreground user action and compares both the credential
generation and previously displayed consent epoch inside the settings transaction.
Create, Register, Recover and Renew do not automatically grant cloud consent.
Disconnect atomically tombstones credentials and consent and clears only encrypted
sync cache/receipts, preserving the local library and saves.

Disabling is available while an operation is running. It commits a fresh disabled
epoch first, clears the cloud preference and requests cancellation. It requires
neither an account unlock nor an available crypto/network artifact. General
settings writes can preserve an existing valid true consent, but cannot create
false-to-true authority. They can withdraw it. An outdated enable request cannot
restore a later withdrawal merely because it started earlier.

A run captures its consent/account/device/generation before asynchronous product
policy work and carries that exact context through the shared profile lease.
Every journal mutation and every authenticated local save publication checks that
context in the same transaction as its write. HTTP identity checks also recheck
it before signing/sending and before accepting a response. Another page's disable,
disconnect or generation change prevents later local publication even if a
response was already in flight. Cancellation cannot retract a service mutation
already accepted by the remote server or revoke arbitrary trusted-origin script.

`WebSyncRun` owns one foreground job, start/status/cancel controls and a five-minute
opportunity deadline. Account actions and runs exclude each other locally and use
the same nonqueued profile Web Lock across pages. The lock follows the actual
operation promise; stop requests do not advertise completion or release ownership
while work is still unwinding. Focus loss, hidden/page hide, explicit cancellation,
timeout and shutdown retire HTTP/hash/crypto workers and fence local writes.
Shutdown awaits owned work before closing the database. Account actions also
retire their private crypto worker after completion, rather than retaining a
grown WASM heap indefinitely.

The shared UI uses the epoch-aware Web enable command, keeps withdrawal available
while running, disables conflicting account actions until completion, polls the
actual job and describes bounded or deferred opportunities without claiming all
saves are synchronized. Native enable/settings behavior retains its existing
protocol. No account identifier, recovery key, ciphertext or payload is added to
the diagnostic export.

## Artifacts and remaining work

The new frozen source candidate is
`out/web-sync/crypto-v4-authenticated-reconciliation`: 185 files / 1,187,294 bytes.
Earlier candidates retain
their own inventories and are not rewritten. Its producer name does not change
the native/browser crypto, account or resumable protocol 1 wire identities.
All six source assemblers completed Web, desktop Chromium, Android Chromium and
iOS resource/integration outputs without cores; none is a built or admitted product.

Explicit queued cloud recovery and cancellation/CAS completion are now authored
in `WEB_SYNC_RECOVERY.md`, including schema 9 purpose and request storage.
Existing-generation upload-copy repair/rebinding is authored in
`WEB_SYNC_UPLOAD_RECOVERY.md`. Unexplained missing-record staging/orphan recovery,
physical quota/garbage collection and the complete browser/SDK/artifact/source-license
qualification remain due.
In particular, a local corrupt save behind an already acknowledged receipt uses
explicit recovery rather than resetting the remote cursor.

The single final verification must exercise actual schema migrations, canonical
and legacy cases, different local game UUIDs, out-of-order ancestors, conflict
branches, late game import, active leases, malformed/changed envelopes, equivalent
nonce conflicts, actual local payload corruption and repair, purges, cursor replay,
crashes between both publication commits, uncertain strict writes, full quotas,
credential/consent ABA races across pages, worker/network/lock lifetime, the real
service and supported browser/device memory/storage behavior. No such evidence
has been produced. Every other original browser/runtime/platform/streaming/
security/licensing/store/release requirement remains active.
