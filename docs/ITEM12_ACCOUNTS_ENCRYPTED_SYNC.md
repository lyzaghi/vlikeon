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
