# Item 16 — consolidated verification

The owner explicitly moved this chat to 15 then 16 on 2026-10-23. Exact frozen
verification batches are now authorized; source-only timing does not prevent
these batches. Antigravity executes the assigned commands only. Codex reviews
raw logs, diagnoses, changes source and accepts or rejects the results.

## Source identity

The repository contains substantial concurrent uncommitted work. Verification
uses fresh isolated copies, including required untracked source, with full file
SHA-256 manifests. The base commit alone is not their identity. Existing demo
evidence qualifies only its frozen inputs, not the current workspace.

Batch 049: 198-file frozen working-tree snapshot, source manifest SHA-256
`2d72f817142aaf0da04b8db73ad64ba07e08bf49959195cf4585f6a8ff36768f`.
Eleven synthetic release inventory cases and thirteen browser context/ownership
unit cases passed. Cargo workspace library tests exited 101 before compilation:
the offline cache lacked `axum 0.8.4`. No Rust test result exists for this batch.
Source hashes remained unchanged. The original failed result is retained.

Batch 050 used a new snapshot with stricter release attribution and packager
integration cases. All 13 release cases and 13 browser cases passed. Locked
dependency downloads completed, then Rust compilation stopped because the
snapshot omitted the public `security/update-channel.json` compile-time input.
This was snapshot preparation failure, not a passing Rust build/test. Source
hashes stayed unchanged. Source-manifest SHA-256:
`1998b3bc63cbe2e3c6dd58eb7ba9a03459f99ecad037f3e18febb3d93a833846`.

Batch 051 includes public security configuration and removes any compile-time
update-channel environment override in its process, preserving the disabled
default. It reuses only the completed batch-050 dependency compilation cache.
Its exact assignment permits ordinary locked dependency downloads, retains
`--locked`, and does not edit source, lockfiles or toolchains. Codex reviewed
the raw results: 13 release cases, 13 browser cases and 41 Rust library cases
passed (14 core/storage and 27 native-runtime cases). Four explicit Windows
actor/storage fixture tests were ignored and remain open. Several other crates
reported zero library tests, so compilation is not described as coverage there.
Compiler warnings remain; the successful exit is not a warning-free build or a
full security review. Source-manifest SHA-256:
`600ceaff0aac6bd5e337ed522f9d9c5f665c2273a87895c94e6f7fd1bb615615`.

Batch 052 extended the same frozen snapshot to `cargo test --workspace
--all-targets --locked --offline -- --test-threads=1`, including executable and
integration targets omitted by the library-only command. Existing caches only;
new evidence directory and explicit deadline. It exited 0 with unchanged source
hashes: 42 passed, 4 ignored. This repeats 41 library cases and adds one broker
diagnostic-redaction case; it is not 42 additional distinct tests. Available
executable targets compiled; the other executable targets reported zero own
tests. No existing integration target was present in that snapshot.

Batch 053 adds `platform/updater/tests/release_security.rs` to a new frozen copy
of the exact batch-052 source. It exercises actual package unpacking and
signature-policy code with synthetic content and fake test-only keys: exact
package, truncation, corruption, trailing bytes, existing-tree refusal, path
collisions, signed identity, real distinct threshold signers, expired/duplicate
keys, metadata replay, host and data-schema constraints. It executes only this
new integration target. Codex reviewed exit 0 and all 11 passing cases, with
unchanged source hashes. Source-manifest SHA-256:
`afe1670ff686b8de0ffd7e8f613f4c79ab8f4df40774fc75e6022baf7da47040`.
No product key, trusted root or live update service was involved.

Distinct executed cases across these scopes: **79 passed** (13 Python, 13
JavaScript, 42 existing Rust and 11 new Rust integration cases), **4 ignored**
Windows fixture cases retained as open. Repeated Rust library cases in batch
052 are not counted twice. This is initial Windows/shared-source verification,
not completion of item 16. Sanitized evidence indexes with raw-log digests and
source manifests are under `../qa/evidence/item16/`; original raw logs/snapshots
remain in the local assigned QA workspace. No local credentials/tool paths are
needed in the published indexes.

## Required remaining evidence

| Area | Qualification still required |
| --- | --- |
| Rust/shared services | Workspace library tests, all-target build/check, executable/integration cases and Windows-specific lifecycle/fault coverage |
| Native cores | Actual pinned full core builds and original-media compatibility/save/audio/graphics/input/fault tests; fixture evidence is separate |
| Desktop browser | Real Chromium builds, profiles/private tabs, native bridge ownership, permission/download/capture/update/install/repair behavior |
| Windows | Actual production containment/core/presentation/device/save/storage matrix, including deferred item 2 |
| Linux/macOS | Corresponding real OS/compiler/containment/guest/device builds and fault matrix; Windows cannot qualify these |
| Android/iOS | Deferred source plus actual platform SDK/device/provider/lifecycle/store matrix; see mobile records |
| Web/PWA and sync | Actual browser/install/offline/storage/crypto/account/recovery/network fault matrix on exact staged assets |
| Remote Play | Owned capture/input, actual devices/networks/NAT/TURN/permissions/revoke/reconnect and latency evidence |
| Performance | Controlled device scenes and measurements; no rank or performance guarantee inferred from inventory/unit tests |
| Release/security | Item-15 inventory/provenance/SBOM/advisory/signature/privacy/update and independently reviewed fault/security gates |

No production binary is approved and no release gate is waived. A failed,
blocked, skipped, unavailable or unrun case stays open. Outstanding source is
retained in its original item, not renamed as only a test or moved to item 17.
