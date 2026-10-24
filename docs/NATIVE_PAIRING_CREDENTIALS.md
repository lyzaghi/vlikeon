# Native Remote Play credential lifecycle — item 58

Source authoring only. No vault, signing, native/mobile API, build or test has
been executed for this checkpoint. Item 58 remains incomplete. Existing browser
Remote Play still uses its IndexedDB/WebCrypto store; this module does not
migrate those non-extractable keys or enable a native credential capability.

## Secret material source

`platform/streaming/src/native_credentials.rs` defines a Remote Play provider
separate from encrypted-save synchronization. Desktop keyring service
`org.vlikeon.remote-play.credentials.v1` addresses one profile/device/kind/id/
generation. Identity and pair addresses have different kinds, each new candidate
gets a fresh UUID generation, and all UUIDs must be nonzero. Deserialized
addresses are data, not lifecycle authorization.

The provider's binary record is exactly 137 bytes: eight-byte version magic,
one kind byte, four UUIDs (profile, device, item, generation), a 32-byte binding
and 32 secret bytes. Loads check all address fields and the expected binding;
another generation/profile/device/kind is rejected. Records and temporary secret
buffers use zeroizing ownership. Secret identity/pair types do not implement
Serialize, Clone or Debug and expose no secret export method. This does not
claim the OS/backend makes no transient copies or that software keyring storage
is hardware-backed/non-exportable.

Signing identities use OS randomness and Ed25519. Reloading verifies that the
seed produces the pinned public half. Signing accepts only the existing four
Remote Play domain constants, with their existing body-size bounds and JSON
object parsing. This primitive still needs typed protocol and document/session
authorization in native orchestration; it is not a renderer signing endpoint.
The parser ignores JSON values without retaining a second object tree.

Pair records are bound to the complete signed grant hash and the pinned local
public key. Before load/store, the existing grant verifier checks signature,
audience and lifetime, and the local public key must be a participant. The pair
UUID must equal the address item UUID. A monotonic elapsed-time adjustment
rechecks expiry after synchronous vault work, so slow I/O cannot publish a pair
key using only the earlier timestamp. The caller must still supply native
current time, its own pinned identity and durable active-generation authority.
`from_derived` is only an internal/native material wrapper: it is not proof of
an authenticated ECDH exchange, which remains to be implemented/integrated.

Candidate storage first refuses an already present address, writes, then reads
back the exact record before returning. The keyring is not a CAS store; a
durable journal must serialize generation operations before using these calls.
A failed write/read-back/expiry recheck can leave an orphan candidate and must
retain cleanup debt. The module deliberately does not retry by inventing a new
identity or deleting an uncertain record automatically.

Desktop providers select the same pinned keyring backends already used for
save-sync (Windows native, Apple native, Linux persistent native keyring), with
a separate service/account namespace and binary secret APIs. Only NoEntry maps
to absence. Locked, denied, malformed-length and other failures remain errors;
no plaintext-file or renderer fallback exists. Android/iOS default desktop
provider calls refuse until a scoped native mobile provider is supplied.

## Durable metadata journal source

`platform/core/src/stream_credentials.rs` now authors a profile/device-bound
journal in the existing SQLite preferences table, with no schema migration or
key bytes in SQL. Each transition runs inside an immediate transaction, compares
the complete journal revision and commits a bounded replacement. Read admission
checks the SQL byte length before fetching JSON, its version/bindings, unique
generations, public keys, states and active-pair identity invariant. Limits are
128 retained generations, 192 KiB and ten million revisions; exhaustion refuses
new work. Tombstones are not discarded by age or silently reused.

Reserved -> Writing is committed before any vault writer may start. The trusted
native owner may record MaterialReady only after exact vault read-back and
actual writer retirement, then Active after native opt-in and, for pairs,
authenticated relay approval. These are native metadata APIs, not implementations
of those external operations or verifiable receipts. Pair reservation verifies
the signed grant and pins its hash/expiry and the active local identity. The
same pair UUID cannot be reserved again, even after retirement. Grant expiry
is rechecked after SQL-lock waiting on reservation/activation; orchestration
must still recheck authority after commit before publishing or using a key.

Revocation commits Revoking before side effects. Revoking an identity atomically
fences all its pairs. Writing generations keep their writer debt; repeating a
revocation does not clear previous cleanup observations. An already retired
identity cannot be used to revoke a later identity generation. Separate native
observations track writer retirement, material absence, local session retirement
and relay retirement. Material absence is refused while a writer is unretired.
All four are required for Retired, and identity retirement additionally waits
for every pair tombstone to retire. No rollback to Active exists. Uncertain SQL
commit requires reloading the authoritative revision rather than repeating
side effects under an old state.

This journal has no current product caller, startup recovery worker or vault/
network orchestration. Loading metadata is not a live authority lease. Existing
in-memory key objects are not yet fenced by journal changes. A native owner must
serialize side effects, retire all old writers before declaring cleanup, keep
the grant required for relay revocation and enforce current epochs across
processes. The journal now also retains the exact signed grant and its canonical
HTTPS relay audience as non-secret native recovery metadata, in addition to the
hash/expiry. Every journal admission verifies the signature, original grant
shape, UUID, local participant, audience, hash and expiry bindings. Historical
validation uses the grant's signed creation time to retain expired revocation
metadata; this does not authorize use now. Native operations/activation still
require current time and live authority. A pair record missing that metadata is
refused for recovery, never filled from an untrusted renderer or silently reset.
Metadata consumes the existing 192-KiB bound, so large grants can exhaust the
journal before its 128-generation ceiling. No private seed, pair AES key or
invitation secret is written to the journal.

`Journal::recovery_plan` is now a read-only bounded plan for each retained
generation. Incomplete candidate states and expired active pairs first request
a durable fence; valid active records request revalidation before use, not
automatic activation. Revoking records request actual writer retirement, local
session retirement, relay revocation, material removal and finalization in that
order. An identity waits for every pair to retire before its material cleanup;
acknowledging identity material absence while pair debt remains is refused.
`finalize_retirement` can finish an identity after its earlier receipts and pair
cleanup are complete, without inventing new evidence. The caller must reload
after every transition/side effect: a plan is a snapshot, not an authority lease
or executor. Vault lock, unknown writer and failed relay requests keep debt;
elapsed time alone does not acknowledge writer or session retirement.

## Required durable ordering and integration

The journal authors the state transitions below. Their external effects and
product/recovery integration remain required work:

1. Persist a reserved generation and expected public/grant binding in a durable
   profile/device journal before writing material. Bound pair count and serialize
   profile/device writers; record candidate and cleanup debt through crash.
2. Publish active state only after matching vault read-back and successful
   authenticated pairing/relay acceptance. Retain approval uncertainty rather
   than claiming a pair active after an ambiguous network reply.
3. Before each native operation, consult the original generation/state and
   current document/pair/session authority. Loaded key objects alone cannot
   authorize use after revocation; close all in-flight producers and leases.
4. Commit a permanent local revocation fence before removing key material, keep
   a bounded relay-revocation tombstone, and require actual local retirement and
   remote acknowledgment/expiry before clearing the corresponding debt.
5. Reconcile reserved/orphan/cleanup generations after restart without reviving
   deleted or revoked credentials. Keyring deletion and SQL commits are not one
   atomic transaction; handle both crash windows explicitly.

`delete_material` removes only the exact addressed generation, then checks
absence. Its success is a material-cleanup observation, not confirmation of
relay revocation, active session retirement or absence of concurrent writers.
The caller must fence and retire writers first. Failed cleanup keeps debt.

## Remaining implementation and qualification

- Journal product integration, authority epochs, actual startup recovery execution,
  rotation/re-pair and local/relay revocation orchestration.
- Native authenticated ECDH/HKDF pairing and encrypted signaling sequence/replay
  lifecycle compatible with the current signed protocol; no secret IPC export.
- Broker operations and document-scoped native capabilities, with explicit
  native opt-in and locking/background/expiry behavior.
- Separate Android Keystore/iOS Keychain provider namespace and scoped callback
  ABI. Existing save-sync callbacks are not reused as pairing storage.
- Device-lock, protected-data, backup/restore/reinstall, profile deletion,
  missing vault and lost-key behavior. Native identity recovery must not silently
  replace the identity pinned by remote peers.
- UI/transport migration: non-extractable browser keys cannot be read into this
  module. A fresh native identity requires explicit re-pairing or a separately
  specified authenticated migration, without falsely preserving peer trust.
- Source review and actual build/test/platform qualification of all providers,
  wrong bindings, corrupt/missing/locked records, slow I/O, candidate conflicts,
  concurrent writes, failed deletion, crashes and revocation races.

All earlier incomplete items and the original 51–109 scope remain open. The
existing streaming policy is unchanged and disabled.
