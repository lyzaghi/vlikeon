# OmniPlay Production Trust Root & Update Infrastructure Specification (Item 100)

This document specifies the operational design, threshold key ceremony, and deployment standards for the production release update system.

---

## 1. Security Model & Separation of Duties

The OmniPlay update infrastructure enforces an offline, threshold-based trust model:
1. **Threshold Root Keys**: 3-of-5 Ed25519 signing quorum required to rotate the root or authorize new release-signing keys.
2. **Air-Gapped Operation**: Root private keys must reside on hardware security modules (YubiKey / Nitrokey HSM) stored in geographically distributed secure safes. Private keys are **never** present on build servers, CI runners, or repository files.
3. **Delegated Release Signer**: An online signing key with limited validity period (max 30 days) signed by the threshold root signs daily/weekly release metadata.
4. **Fail-Closed Client Verification**: The client updater (`omniplay-update`) verifies the signed metadata envelope, rejects version rollbacks below security epochs, and validates package SHA-256 before extraction.

---

## 2. Production HTTPS Endpoint Requirements

- **TLS Version**: Strict TLS 1.3 only; legacy TLS 1.0, 1.1, and 1.2 disabled.
- **Certificate**: EV (Extended Validation) TLS certificate issued by a trusted public CA with OCSP stapling enabled.
- **Log Hygiene**: Access logs retain IP and User-Agent for rate-limiting, but payload URLs contain no user credentials or profile tokens.
