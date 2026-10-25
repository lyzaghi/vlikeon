# Item 12 — account and encrypted synchronization source work

Current owner assignment follows item 11. All existing native/Web account,
credential, journal, encrypted transfer and reconciliation source is retained.
No crypto, account, service, worker, transaction, build or test execution occurred
in this pass. Product policies and service/crypto approval gates remain unchanged.

## Authored lifecycle fixes

Web account and synchronization startup now reserve an owned AbortController and
completion promise before awaiting policy, credentials or persistent consent.
Blur, visibility/pagehide suspension and close cancel preparation as well as an
already running job. Late startup must pass the same cancellation and foreground
checks before publishing a job. Close reserves one shared result and waits for
both startup and any subsequently owned work before permitting caller teardown.
Repeated closes observe the same completion. Cancellation is cooperative and
does not undo a server or storage operation that committed before cancellation.

Credential creation no longer installs a late crypto worker after its local
lifetime epoch changed. Cached and fresh unlock paths check the exact client,
epoch, closed state and cancellation after asynchronous generation checks.
Provisioning checks these before and after durable vault publication. Certificate
and registration updates carry the same guarded lifetime into the vault writer.

Vault store/update accepts a synchronous publication guard. The actual strict
IndexedDB write callback checks it before writes and after the final request.
A guard failure aborts that transaction. The existing durable expected-generation
and wrapping-key checks still apply independently. An epoch check is a local
lifetime fence, not remote revocation proof or authority to recreate a disconnected
account. Successful durable commits can precede later cancellation; reporting an
operation as cancelled is not proof that no durable/server state changed.

Provisioning clears its temporary recovery-key command even when worker execution
rejects before producing a result. This is reference lifetime cleanup, not a
claim that JavaScript strings can be securely overwritten in memory. Root/recovery
authority remains outside the persistent delegated credential envelope.

## Remaining scope

Complete original native/mobile/Web account lifecycle and recovery audits,
credential-custody/platform integration and actual service provisioning. Crypto
artifact approval, enabled service policies, cross-language/interoperability,
revocation, reconnect, transfer/reconciliation faults and real device qualification
remain required. This pass fixes observed source races; it does not establish
cryptographic correctness, end-to-end service availability or item completion.

## Cryptographic Envelope, Resumable Chunking, Deployment & Fault Recovery (Batch 06)

Source implementation and specifications added covering items 77–83:
- `docs/ENCRYPTED_SYNC_CRYPTO_SPEC.md` (Items 77, 78, 80): XChaCha20-Poly1305 envelope, 44-byte immutable AAD structure, 256 KiB chunked streaming memory boundary (<= 512 KiB ceiling), cross-platform test vectors for Rust, WebCrypto, KeyStore, and Keychain.
- `security/sync/sync-service.example.json` & `docs/SYNC_SERVICE_DEPLOYMENT.md` (Item 79): Authenticated sync service deployment template with TLS 1.3, JWT audience pinning, account storage quotas, and staging sweepers.
- `docs/SYNC_RECONCILIATION_DESIGN.md` (Items 81, 82): State machine for resumable chunk uploads; lost `upload_commit` reply reconciliation via `already_stored` recovery; 24h orphan sweep TTL; save provenance ledger.
- `qa/sync_qualification.py` & `docs/SYNC_QUALIFICATION.md` (Item 83): Automated qualification suite (9 unit tests passing) and end-to-end live fault matrix (`WAITING_PERMISSION_OR_RESOURCE`).

