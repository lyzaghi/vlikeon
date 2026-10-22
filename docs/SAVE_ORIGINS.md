# Canonical save origin source

Native core schema 11 and browser IndexedDB version 5 introduced semantic save
origins separately from local history projections. This is authored source. No
migration, SQL/IndexedDB transaction, save hash, synchronization, export, UI,
browser/device, compiler or test has run. The sync-service schema remains 2.

## Records and meaning

| Record | Purpose |
|---|---|
| Local save projection | Current library game UUID, local ordered generation, local parent availability and conflict/head display state |
| Canonical metadata | Complete normalized SaveMetadata values established at a capture or authenticated receive boundary; retained without projection rewrites |
| Imported reference | Metadata read from an imported OPSV file before assigning its new local identity/branch |
| Legacy projection marker | Explicit lack of canonical evidence for data that already existed before this schema |

`captured`, `authenticated_remote` and `imported` describe the boundary that
established a record. They are not signed device attestations. In particular,
authenticated remote metadata can itself have been produced by an older sender
from a local projection. Retaining those received values does not reconstruct or
prove an earlier historical capture. Legacy migration never fabricates missing
remote game/generation/parent values.

Each known record carries a SHA-256 consistency fingerprint over the bytes
`OmniPlay save origin v1\0` followed by UTF-8 JSON of its normalized SaveMetadata
with sorted field names. All nineteen metadata fields participate, including
explicit null/default values, original game UUID, generation, parent, conflict,
runtime/configuration identity, payload hash/length and capture metadata.
Integers retain the existing safe bounds; strings must be valid Unicode and core
versions valid semantic versions. This is a specified metadata encoding, not a
general-purpose JSON canonicalization standard or an authentication substitute.
The Rust contract and the shared synchronous JavaScript helper implement it;
their cross-language equality still requires the final fixture evidence.

## Native capture, import and reconciliation

Migration `0011.sql` creates `save_origins`, its parent index and cascading
save-row ownership. Existing rows receive `legacy_projection` with no canonical
metadata/hash. An insert trigger also gives a still-open older writer an honest
legacy marker. New writers upgrade that marker in the same transaction as their
save, rather than allowing a committed row with a missing origin.

Capture assigns local generation/conflict state, prepares the canonical record
and ancestry admission, publishes payload bytes, and commits the save/origin/head
updates together. Invalid origin metadata/ancestry is rejected before payload
publication. Existing file-publication/orphan recovery still needs its full
qualification; adding a metadata row does not establish power-loss durability.
Promotion changes only the local projection/head and leaves canonical metadata
and its fingerprint intact. Purge cascades origin/reference removal with the
logical save deletion and keeps the existing local suppression/payload-GC rules.

Authenticated ingestion retains the incoming metadata before mapping its game UUID,
assigning local order or omitting an unavailable local parent. The canonical parent
UUID stays present even when its save has not arrived or was locally removed.
Late parent arrival changes the read-only availability view without silently
rewriting lineage or promoting a different head. Parent and child admission checks
game content, save kind and slot; self/cyclic ancestry is rejected. A parent-index
probe skips ancestry walks for an unreferenced new UUID, avoiding a full-history
walk for ordinary capture. Referenced arrivals use a deduplicating ancestry query.
That query has a history-derived hard row bound and fails on truncation instead
of treating an incomplete walk as proof of acyclicity.

An already-known origin must match the full canonical fingerprint. A previously
unknown legacy row can acquire a matching authenticated wire origin; its existing
local payload is verified first. A corrupt local payload keeps reconciliation
from acknowledging a healthy local copy; automatic payload-file repair is not
implemented by this milestone. Missing-content, locally removed IDs and preserved
CAS branches retain their existing policies.

Outgoing reconciliation reads the canonical record when available. Its encrypted
staging manifest records whether the fingerprint uses canonical identity or the
older projection comparison. A change of identity retires its owned unfinished
upload before resealing. Known canonical conflicts compare every field, while
legacy retries retain the existing projection relation until an authenticated
matching object establishes a record. An incoming duplicate/lost acknowledgement
can attach that record without rewriting the save's local game/order/head.

The existing cipher and OPSV0001 structure stay unchanged. Export uses canonical
metadata when known, and an honest projection otherwise. Import still allocates
a new UUID and independent branch; its pre-import metadata becomes the local
imported reference. Source kind, imported-reference records and proof/confidence
classification are not carried as an additional envelope by format 1. A future
versioned envelope/backup design is still needed to carry that entire record
across repeated imports or account transport. No historical-fidelity claim is
made where the old representation had already lost information.

## Web/PWA and shared history

IndexedDB version 5 adds `save_origins` with a canonical-parent index. Its bounded
upgrade labels existing saves as legacy instead of guessing hashes. New capture,
import and origin publication use the same strict transaction as the save/head/
usage rows. Local purge removes origin rows explicitly because IndexedDB has no
foreign-key cascade. Synchronous bounded metadata hashing avoids awaiting file,
network, timer or WebCrypto work inside that transaction.

Web metadata/ancestry admission mirrors the native semantic rules, including
missing parents, referenced-arrival cycle checks and immutable known records.
`ingestRemote` is a trusted-adapter source entry point, with payload verification
and no renderer write command. It does not enable standalone PWA account sync or
claim that such a crypto/credential adapter already exists. The platform continues
to advertise `encrypted_sync:false`.

Desktop, native mobile and PWA hello expose `save_origin_protocol:1`. The bounded
read-only `save_origin_get` response includes source classification, known canonical
metadata/fingerprint, an optional imported reference and current parent-presence
status. It exposes neither a filesystem path nor a secret. There is no renderer
command to write or replace a canonical record.

The localized history view offers an Origin action only when that protocol is
advertised. It shows the source boundary, original/local order when different,
retained missing-parent links and a comparison fingerprint. Response ownership
checks keep a late result from opening a dialog for a different history page.
Values enter text nodes, and origin consistency is checked before display. The
view is not a fresh payload-health check or a manufacturer/core/device attestation.

## Schema and required final evidence

The current native schema and fresh updater data-schema floor are now 13 for the
separate publication journal, with PWA storage version 6; see
`SAVE_PUBLICATION_JOURNAL.md` and `SAVE_PAYLOAD_RECOVERY.md`. Existing
update state records opened schemas through the existing monotonic floor API;
an older signed release must still declare support before rollback can proceed.
No signed release, profile migration or installer has actually been executed.

The single consolidated pass must cover SQL and IndexedDB upgrade atomicity,
legacy fallback writers, schema/rollback floors, missing/invalid origin rows,
all JSON/Unicode/null/integer/SemVer/hash cross-language fixtures, capture/import/
promotion/purge transactions, duplicate/changed canonical metadata, payload faults,
missing/late/cyclic/cross-domain parents, concurrency and interrupted publication,
legacy-to-known retry staging, all exact state and portable-save formats, OPSV
round trips, PWA adapter boundaries, stale UI responses, long histories, memory/
latency/energy, backup/uninstall/recovery and privacy/security/device evidence.
Payload-recovery source is described in `SAVE_PAYLOAD_RECOVERY.md`; it remains
unqualified. Complete versioned provenance envelopes and broader
filesystem/recovery/crypto/runtime/platform/streaming/release work remain required.
No original v1 requirement has been removed or marked qualified by this source.

Primary storage contracts: [SQLite recursive query and UNION behavior](https://www.sqlite.org/lang_with.html),
[IndexedDB key paths and transactions](https://www.w3.org/TR/IndexedDB-3/).
