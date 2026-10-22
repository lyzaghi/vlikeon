# Explicit browser cloud recovery source

This extends `WEB_SYNC_RECONCILIATION.md` with queued per-generation cloud repair.
No migration, database/Blob/worker/crypto/hash/network/account/browser/device
operation, compiler or test was executed. The actual crypto artifact and service
policies remain disabled, with no executable or HTTPS audience admitted. The
generated Web account UI and encrypted_sync capability therefore remain off.
This source is not browser, interoperability, crash-recovery or release evidence.

## Request ownership and capacity

Standalone IndexedDB schema 9 creates sync_recovery, keyed by save UUID, with
account and account/game indexes. Each exact bounded record contains a fresh
request UUID, account/device/game identities, the full local metadata fingerprint,
an optional already known canonical-origin fingerprint, a public state/timestamp/
sanitized error and an optional authenticated completion proof. The proof binds
the final canonical metadata hash to the opened ciphertext hash. Record size is
at most 4 KiB and total retained request count is 64. New requests may replace an
existing request for that save; admitting a new save at capacity can retire the
oldest terminal status for the same account. Live requests are not silently evicted.

The request command requires an admitted product capability, foreground user
action and current enabled account consent. It captures checked metadata/origin
and payload revision ownership without requiring intact payload bytes. A missing,
wrong-sized or damaged Blob remains recoverable if its immutable generation
metadata, origin and revision are valid. Missing or corrupt metadata/revision is
not guessed into a new generation.

List is local and returns the existing shared history UI's bounded status shape.
Cancel needs the connected account/device identity but does not need enabled
consent, a crypto worker or network access. It atomically makes a nonterminal
request cancelled. Request identity is compared in later state/publication
transactions, so a replaced/cancelled request cannot overwrite the replacement
or publish a late local repair.

Purging a game's saved history removes its request records in the existing
purge/accounting/tombstone transaction. Disconnect clears requests together with
encrypted cache/receipts and the credential/consent tombstones. Local saves and
other settings remain independent of the account.

## Separate ciphertext purpose

Schema 9 adds recovery_request_id to retained slots/inbox objects. Existing schema
7/8 objects migrate to null, preserving their headers, bytes, offsets, parts,
quota and cursor. The migration has finite two-slot/32-inbox row bounds and does
not treat an unexpected preexisting purpose field as an admitted old record.
Upload slots always have null purpose; a recovery download has a request UUID
and null list sequence. Ordinary listed transfers keep their existing cursor
contract. The crypto/account/resumable protocol 1 wire identities are unchanged.

A recovery transfer uses the existing one-download slot, actual part/prefix hash
resume and retained encrypted inbox. Its request must still be current before
download resume, part checkpoint and inbox publication. Scope checks between
network chunks also compare the request and local generation. A request cancelled
while an HTTP read is already in flight can leave that read unwinding; it cannot
publish later saved progress under the cancelled request.

Ordinary inbox draining and immutable-upload comparison never borrow a copy tagged
to explicit recovery. A tagged copy alone cannot advance the normal list cursor.
If an independent checked receipt or removal tombstone already covers that object,
their existing cursor shortcut still applies. A listed unacknowledged object held
by recovery yields the current opportunity with remaining work rather than
discarding the request copy or skipping the object permanently.

Retrying the same live request retains its encrypted bytes and checkpoint.
An explicit replacement request may claim the retained copy under its new UUID;
this changes only local operation ownership, not ciphertext, metadata authority
or canonical ancestry. Final authenticated checks must still match the newly
captured generation. Exact terminal/stale recovery copies can be retired under
their request UUID. Ordinary inbox objects and upload stages are not swept by
this cleanup. A cancellation observed during recovery ends that opportunity
after owned cleanup; future user-requested synchronization can perform its normal
replication work under the still-current account consent.

## Health, authenticated repair and completion

Each foreground run attempts at most eight pending requests, prioritizing the
one owning a retained partial download. It checks current consent, account/device,
request identity, local metadata and any known canonical hash. It hashes actual
local payload bytes in a worker before deciding whether a network copy is needed.
A healthy generation completes already_intact in the same strict transaction
that rechecks the captured payload revision and metadata/origin fingerprints.
It does not download the remote save for that health check.

A damaged generation with an active game lease becomes waiting_for_game_close.
An existing partial remains retained and yields the remaining opportunity until
the game is closed. Otherwise the request becomes downloading and claims or
resumes the tagged ciphertext transfer. Network/deadline failures queue a retry
while preserving the partial. Other failures retain a sanitized terminal status;
owned temporary recovery copies may be cleaned up, without deleting local saved
progress or another transfer's bytes.

The existing native Rust cipher must open the exact object and validate its
authenticated metadata and actual payload. The save UUID, runtime/content tuple,
local projection and already known canonical origin must agree. The internal
save repair checks request/consent again inside its write transaction, rechecks
the local payload revision and metadata, refuses a different canonical origin,
and checks the active game lease again. It preserves the generation identity,
heads, branch metadata, other saved copies and accounting; only bad payload bytes
get a fresh payload revision. A legacy origin can gain authenticated canonical
metadata under the existing graph/projection rules.

That same payload/origin transaction publishes recovered or already_intact plus
the canonical-metadata/ciphertext completion proof. Cancellation and completion
therefore have a transaction order: a cancellation committed first prevents
repair; completion committed first is terminal and a later cancellation reports
the finished status. No success is published before the local payload commit.

Receipt/inbox/quota acknowledgement follows the payload commit using the existing
journal's actual local byte hash and revision/origin recheck. A crash between
those commits retains the tagged inbox and completion proof. The next run checks
that proof, canonical local origin and actual local bytes before completing the
acknowledgement. An uncertain journal write poisons its handle and ends that run;
later work must reopen committed state. A completed request's acknowledgement
failure does not cause cleanup to discard its recovery copy in the error path.

An authenticated legacy-origin upgrade can precede an old noncanonical receipt's
upgrade across such a crash. Receipt reconciliation requires its existing reduced
projection fingerprint to agree with the checked canonical origin, hashes actual
local payload bytes and rechecks the same payload revision/origin/receipt before
upgrading that receipt. It preserves the service's acknowledged header and never
converts a genuinely different fingerprint or lost canonical origin into success.
Ordinary acknowledged payloads are not continuously rehashed on every run.

## Controls, summaries and remaining evidence

WebPlatform now implements sync_save_recovery_request/list/cancel and advertises
save_cloud_recovery_protocol 1. The existing shared history UI can queue repair,
start a foreground job, show waiting/recovered/intact/failed/cancelled status and
cancel a pending request when the actual encrypted-sync capability is admitted.
The sync UI reports per-run recovered/intact/failed/cleanup and current pending
counts, including requests arriving after the initial eight-request snapshot.
Pending, deferred or failed recovery is described as an ended opportunity rather
than claiming every save has been synchronized. No root key, payload, service
body or account identifier is added to diagnostics.

The fresh source-only candidate is out/web-sync/crypto-v5-explicit-cloud-recovery:
188 source files / 1,231,346 bytes. Earlier candidates retain their own inventories.
All six Web/desktop Chromium/Android Chromium/iOS resource and integration source
assemblies completed with the same disabled artifact/service policy and empty
development core catalogs.
These are source inventories, not built binaries or runtime acceptance receipts.

The one final consolidated pass must cover upgrades from each earlier IndexedDB
version; exact-purpose migration; full 64-request/32-inbox limits; healthy, missing,
short and silently corrupt payloads; malformed/missing metadata and origins;
active-game races; all request cancel/replace/purge/disconnect/consent-generation
orders around hash/decrypt/part/publication; completed-payload/failed-ack crashes;
proof and receipt mismatch; resumed/mutated ciphertext; network and quota failure;
new requests arriving mid-run; cursor independence and actual browser/device
memory, physical storage and power-loss behavior. None was run. Existing-generation
local upload-copy repair and canonical rebinding are now authored in
`WEB_SYNC_UPLOAD_RECOVERY.md`. Unexplained missing-record staging/orphan recovery,
physical quota/GC and all
other original browser/runtime/platform/streaming/security/licensing/store/release
requirements remain active.
