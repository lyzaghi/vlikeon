# OmniPlay App Store Privacy, API Declarations & Export Compliance (Item 102)

This document provides the authoritative declarations for Apple App Store, Google Play Store, and US Export Administration Regulations (EAR).

---

## 1. Apple App Store Privacy Details (Nutrition Labels)

### 1.1 Data Used to Track You
- **None**: OmniPlay does not collect IDFA, advertising IDs, or cross-app tracking data.

### 1.2 Data Linked to You
- **None**: No personal accounts or identity markers are required for local play.

### 1.3 Data Not Linked to You
- **Diagnostics / Crash Data**: Optional, strictly local performance runs. Sent only upon explicit manual user export.
- **User Content (Saved Games / Screenshots)**: When cloud sync is opted-in, saves are end-to-end encrypted with XChaCha20-Poly1305 on the device before transmission; the sync server has zero visibility into plaintext content.

---

## 2. Google Play Data Safety Declarations

- **Data Collected**: None by default.
- **Data Shared**: None with third parties or advertising networks.
- **Security Practices**:
  - Data encrypted in transit (TLS 1.3).
  - Data encrypted at rest (AES-GCM / XChaCha20-Poly1305).
  - Account deletion request mechanism supported in settings.

---

## 3. Encryption Export Compliance (US EAR / ERN)

OmniPlay utilizes cryptographic algorithms (ChaCha20-Poly1305, Ed25519, DTLS 1.2, SRTP) for authenticated peer-to-peer streaming and end-to-end encrypted save synchronization.

- **Export Control Classification Number (ECCN)**: `5D002.c.1`
- **Exemption Basis**:
  - **Open Source Exemption**: Qualifies under EAR § 740.13(e) (publicly available source code and standard implementations without proprietary encryption modifications).
  - **Mass Market Exemption**: Eligible for License Exception ENC § 740.17(b)(1).
- **Apple App Store Connect Declaration**: Non-exempt encryption: **NO** (app qualifies for standard exemptions; no formal CCATS approval required).
