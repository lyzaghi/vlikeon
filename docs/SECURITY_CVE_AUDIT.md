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

## 3. Chromium Security Freshness

OmniPlay's browser runtime integrates with Chromium:
- **Baseline Milestone**: Chromium 128+
- **Security Floor**: Requires active V8 memory sandbox and site isolation.
- **Rollback Policy**: Update state records schema floors; downgrading to an insecure schema or older security epoch is rejected fail-closed.
