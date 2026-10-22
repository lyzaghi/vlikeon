# Recovery of retained local upload copies

This is an authored extension of `WEB_SYNC_RECOVERY.md`, not executed or qualified
browser behavior. No IndexedDB/Blob/hash/worker/crypto/network/service/browser
operation, compiler or test ran. Actual crypto/service policies remain disabled;
current generated products still have encrypted_sync false and no admitted sync
artifact or service audience. No schema or wire version is changed by this step.

## Recover the local source before resuming an upload

The foreground reconciler now reads an owned upload's retained ciphertext even
when its local payload is missing, damaged or has a changed payload revision.
The journal still validates the exact slot shape/account/device, declared bounds
and actual ciphertext hash. Reading for recovery does not bypass consent,
credential-generation, profile lease or journal-revision checks, and does not
authorize network upload of an unchecked source.

The existing Rust cipher opens that exact ciphertext. The save UUID, payload
length, original staged canonical-or-legacy fingerprint and complete stable save
projection must agree. A canonical stage requires the same known canonical local
origin; losing or changing that ledger entry stops automatic recovery. A legacy
stage must match the same reduced projection fingerprint and can never establish
canonical ancestry merely because the browser itself encrypted it.

The internal staged repair uses the existing-generation payload repair with
authenticated-origin promotion disabled. Worker hashing checks incoming and
current bytes. Guarded read/write transactions compare the current consent,
account/device/generation, exact upload owner/header/fingerprint and unchanged
local metadata/origin. The normal payload-revision compare-and-swap and active-game
lease checks still apply. Healthy payloads remain unchanged; damaged payloads get
the verified bytes and a fresh payload revision. Local generation, heads, branch
metadata, origins and accounting stay retained.

The resulting local source then passes the normal actual-byte forSync hash and
revision/fingerprint recheck. Only after that can the journal rebind a changed
payload revision or newly known matching canonical fingerprint. Rebinding preserves
the exact transfer UUID, nonce, header and ciphertext Blob. Every later upload
source check now also compares the bound payload revision, including chunk sends,
transfer-ID renewal and final receipt publication. A later repair or purge cannot
silently acknowledge a different revision through the old binding.

## When a legacy envelope differs from the known canonical envelope

If the newly known canonical metadata exactly equals the metadata already inside
the old legacy ciphertext, the same immutable bytes can be rebound as canonical.
No new nonce or re-encryption is needed.

If only the reduced stable projection agrees, the local payload must first be
recovered and verified against the current known canonical origin. The transport
then aborts only its own remote partial, or recognizes that partial as absent.
The server abort path checks account/device ownership and removes upload staging;
it does not delete an immutable published object. Local slot retirement rechecks
the same current source fingerprint/canonical flag/payload revision in its strict
transaction. Failure or a changed source retains the local ciphertext. The normal
upload path can subsequently stage the current canonical envelope. A real
immutable-object mismatch still goes through authenticated remote equivalence or
remains a conflict; this step does not overwrite it.

Ciphertext/metadata mismatch, missing local generation metadata, invalid origin or
revision, different projection, active-game repair, lost consent/generation and
unexplained staging remain preserved as an error or deferred work. In particular,
this helper does not recreate a missing save row from guessed metadata, sweep an
orphan, export plaintext automatically, or turn a locally sealed wrapper into a
remote-origin claim. Purged copies continue to follow explicit removal intent.

Plaintext exists only in private transferred buffers and a temporary immutable
repair Blob, with buffers wiped or transferred and the credential worker retired
at opportunity end. Physical memory/GC, WASM peak usage and browser storage
behavior remain unqualified. The shared UI reports a count of local saves
recovered from pending copies without exposing account identities or technical
stage details in the product flow.

## Source and outstanding qualification

The new source-only candidate is
out/web-sync/crypto-v6-local-upload-recovery: 189 files / 1,238,997 bytes. Earlier
source candidates retain their own inventories. All six Web/desktop Chromium/
Android Chromium/iOS resource and integration source assemblies completed with
disabled policies and empty development core catalogs. None is a compiled or
admitted product.

The single final verification must cover good/missing/short/corrupt local bytes,
valid/invalid canonical and legacy stage metadata, a changed payload revision,
exact-byte rebinding versus canonical replacement, lost begin/commit responses,
remote partial ownership/expiry/abort failures, immutable publication conflicts,
active-game/purge/consent/generation races around each transaction and network
boundary, crashes after payload repair or remote abort, actual buffer retirement,
cache quota and real browser/device memory/storage. No such pass ran. Explicit
recovery/export for unexplained missing-record staging, physical orphan/quota/GC
policy and every remaining original browser/runtime/platform/streaming/security/
licensing/store/release requirement remain active.
