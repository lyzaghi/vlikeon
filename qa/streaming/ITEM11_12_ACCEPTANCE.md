# Items 11/12 — future execution scope

Authored only; not a released execution batch. No commands are authorized by
this file. Antigravity retains test-only execution once the owner resumes it.

1. Corrupt/truncate an inventoried cached app/core resource. Offline serving must
   refuse it; online serving must verify a replacement and rebuild policy headers.
   Unknown URLs and game/save/account objects must never be adopted into cache.
2. Request the same resource concurrently and many distinct inventoried resources.
   Responses must remain independent, and failed jobs must release admission.
3. Prepare a complete core with auxiliary resources; exercise progress, quota,
   missing files, invalid inventory, timeout, cancellation, closed clients and
   worker restart. Preserve valid partial cache and inspect actual offline use.
4. Status with a missing module/auxiliary file must show incomplete despite a
   present WASM. Presence is not checksum evidence. Validate message client,
   scope, query, UUID, cross-client cancel and malformed progress handling.
5. Hold account/sync startup at each policy, consent and credential await. Blur,
   hide, Stop and Close must prevent late job publication. A second Start refuses;
   repeated/reentrant Close returns the same promise and waits for owned work.
6. Hold crypto creation/unlock/generation checks and vault encryption/writes.
   Lock, disconnect, close or cancel; late results must not revive unlocked
   state. Guard failure inside the transaction must abort publication. Distinguish
   transactions committed before cancellation from uncommitted refusals.
7. Reject provisioning worker execution before its result. Retire temporary
   command/result references; do not log recovery keys. Cover create, recover,
   renewal, registration, retained account results and uncertain remote commits.
8. Finish native/mobile credential, cross-tab, encrypted transfer/reconciliation,
   revoked-device and real service/device qualification under the original scope.
