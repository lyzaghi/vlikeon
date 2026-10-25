# OmniPlay Independent Security Audit Plan & Qualification Queue (Item 104)

This document establishes the audit scope, security threat model, engagement requirements, and deferred qualification queue for external third-party security audits of the OmniPlay platform.

---

## 1. Independent Audit Scope & Target Subsystems

To achieve production release readiness, an accredited external cybersecurity firm will evaluate the following core components:

### Scope 1: Process Sandboxing & OS Containment
- **Windows**: AppContainer token isolation, Job Object memory/process limits, restricted token mitigations (blocking child process creation and arbitrary network egress).
- **Linux**: Unprivileged user namespaces, seccomp-bpf syscall filters, cgroups v2 memory limits, Landlock filesystem restrictions.
- **macOS**: App Sandbox containment, Hardened Runtime validation, JIT restriction enforcement (`allow-unsigned-executable-memory: false`).

### Scope 2: Cryptographic Protocols & Network Transport
- **Remote Play Streaming**:
  - DTLS 1.2 handshake and ChaCha20-Poly1305 / AES-GCM cipher suite negotiation.
  - SRTP key derivation via RFC 5764 DTLS-SRTP exporter.
  - Replay window protection (64-packet bitmask) and replay attack resistance.
  - Host/client pairing credential fencing and immediate revocation reconciliation.
- **Cloud Save Synchronization**:
  - Client-side XChaCha20-Poly1305 envelope encryption.
  - 44-byte immutable Additional Authenticated Data (AAD) structure preventing payload swapping.
  - Zero-knowledge server model (server stores only ciphertext, transfer state, and quota bytes).

### Scope 3: Update Verification & Software Supply Chain
- **Threshold Root**: 3-of-5 Ed25519 signature verification on release metadata.
- **Downgrade Protection**: Monotonic security epoch enforcement and schema floor fencing.
- **Package Extraction**: Zip-slip path traversal prevention, link/reparse point refusal, and SHA-256 integrity verification before activation.

### Scope 4: IPC Broker & Permission Delegation
- **Permission Broker**: Grant leasing, token scoping, path canonicalization, and exclusion of sensitive OS directories.
- **Shared Memory**: Double-free prevention, ring buffer bounds checking, and synchronization locks.

---

## 2. Pre-Audit Self-Assessment & Clean Gates

Prior to external engagement, the following internal automated suites must pass cleanly:
1. `qa/release/test_release_inventory.py` (checks zero credential leaks, correct attribution, matching hashes).
2. `qa/sync_qualification.py` (checks cryptographic envelopes, chunk memory ceilings, revocation fencing).
3. `qa/installer_qualification.py` (checks repair, rollback, staging sweep, and uninstallation).
4. `qa/benchmark_qualification.py` (checks presentation timing monotonicity and router selection).
5. `qa/security_license_qualification.py` (checks SPDX SBOM, CVE audit, license ledger, and secret detection).

---

## 3. Deferred Audit Qualification Queue

| Scope | Audit Component | Target Firm Capability | Status |
|:---|:---|:---|:---|
| **Scope 1** | OS Sandbox Containment | Specialized OS internals & escape research | `WAITING_PERMISSION_OR_RESOURCE` |
| **Scope 2** | DTLS/SRTP & Sync Crypto | Applied cryptography & protocol security | `WAITING_PERMISSION_OR_RESOURCE` |
| **Scope 3** | Update Verification & Supply Chain | Software update & code-signing specialists | `WAITING_PERMISSION_OR_RESOURCE` |
| **Scope 4** | IPC Broker & Memory Safety | Native systems & Rust concurrency auditors | `WAITING_PERMISSION_OR_RESOURCE` |
