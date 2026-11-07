# OmniPlay Security Advisory & CVE Audit (Item 99)

This document records the vulnerability auditing methodology, dependency security freshness thresholds, and CVE remediation status for OmniPlay components.

---

## 1. Audit Methodology & Sources

Dependency components and submodules are continuously monitored against:
1. **RustSec Advisory Database**: Rust crates ecosystem vulnerabilities.
2. **Chromium Security Bulletins & Stable Channel Updates**: Pinned browser runtime security advisories.
3. **National Vulnerability Database (NVD)**: C/C++ native engine dependencies (libvpx, OpenH264, Opus, SQLite).

---

## 2. Pinned Engine & Media Dependency Status

| Advisory ID | Affected Component | Vulnerability Summary | Remediation in OmniPlay |
|:---|:---|:---|:---|
| **CVE-2023-4863** | `libwebp` | Heap buffer overflow in WebP lossless decoding | Updated to libwebp >= 1.3.2 |
| **CVE-2023-5217** | `libvpx` | Heap buffer overflow in VP8 encoding | Updated to libvpx >= 1.13.1 |
| **CVE-2024-0553** | `sqlite` | Integer overflow in window functions | Pinned SQLite >= 3.45.1 |
| **RUSTSEC-2020-0071** | `time` crate | Segfault in multithreaded environment | Upgraded to time >= 0.3.36 |

---

## 3. Chromium Security Freshness & Automated Lockfile Verification

OmniPlay's browser runtime integrates with Chromium and enforces automated lockfile and advisory verification via [`tools/audit_security_advisories.py`](../tools/audit_security_advisories.py) and [`security/advisory-database.json`](../security/advisory-database.json):
- **Dataset Versioning**: `dataset_version = "2026-11-08.1"`, `dataset_date = "2026-11-08"` (`vlikeon-rustsec-and-chromium-advisory-db`).
- **RustSec Lockfile Scan**: Parses all 327 `[[package]]` entries from `Cargo.lock` via `tomllib` and evaluates each crate's exact version against RustSec affected ranges.
- **Chromium Security Baseline**: Compares the pinned Chromium version and commit revision in `browser/upstream.lock.json` (`155.0.8059.85` / `0e66b94549a8bd85f03fbdf189b752d6e7b63c32`) against the minimum baseline (`155.0.8059.85`) and historical CVE fix milestones.
- **Fail-Closed Gate**: When `Cargo.lock`, `browser/upstream.lock.json`, or `security/advisory-database.json` is missing/unreadable or yields 0 packages, the auditor reports `status: "UNVERIFIED"` and exits with code `1`. When any vulnerable crate or outdated Chromium revision is detected, it reports `status: "VULNERABLE"` and exits with code `1`.
- **Rollback Policy**: Update state records schema floors; downgrading to an insecure schema or older security epoch is rejected fail-closed.
