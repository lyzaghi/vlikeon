# Item 15 — security, licensing and release preparation

Owner transition on 2026-10-23: retain mobile gaps and work on 15 then 16.
`DEFERRED_ITEMS_9_10.md` preserves item-9/10 obligations; no platform is closed
by this transition. Other concurrent source is preserved.

## Source changes

`tools/release_inventory.py` adds a publication-side gate used by
`tools/package_release.py` before writing the package. It checks:

- Root LICENSE/NOTICE and shipped third-party/source declarations are present.
- Each payload path has one component owner, explicit distribution review,
  bounded accompanying notices and an explicit corresponding-source obligation.
- Required accompanying source files exist and match their declared SHA-256.
- Case collisions, links, traversal and common profile/credential paths are
  rejected. A streaming content scan rejects selected credential/private-key
  markers without printing matched content. This is deliberately not a complete
  secret detector or a guarantee of credential absence.
- Placing an arbitrary payload in `licenses/` does not exempt it from attribution.

The stricter declaration requires each component to supply `name`, `revision`,
`license`, `distribution_approved: true`, `artifact_paths`, `notice_paths` and
`corresponding_source_required: true|false`. A true source obligation also needs
`source_archive_path` and `source_archive_sha256`; the archive is part of the
package payload and must be attributed. Existing release inputs must be reviewed
and migrated explicitly; no automatic approval or license rewrite is performed.

These gates check declarations and bytes, not legal compatibility, completeness
of corresponding source, license authenticity, code signatures or security.
An operator must review the actual compiled dependency/asset/source combination.
The updater continues to verify its signed manifest and file hashes; this source
change is a build/publication check, not a new client trust root or signature.
The separately built technical demo ZIP is outside the OPPK package flow and is
not retroactively described as passing this new gate.

`tools/inventory_locked_dependencies.py` creates a deterministic review inventory
from Cargo.lock: identity, version, source, checksum and explicitly pending
license/advisory/artifact-inclusion review. All 327 packages in the current lock
are listed locally in `out/item15-locked-dependency-inventory-v1.json`; this
includes optional/dev/platform packages and is **not** the final binary SBOM.
Lock SHA-256: `e0376abb29991e0d2fcda97dd2e6801a77d1dc9fe9972226169dc13365ab1fd4`.

## Still open — no production approval

1. Exact final browser/core/codec/font/firmware asset provenance, full dependency
   notices and license compatibility review. The restricted/unresolved Angrylion
   profile stays research-only; no source receipt can promote it automatically.
2. Complete corresponding-source archives for the exact adapted distributed
   binaries, build scripts and required notices; a matching hash alone is not
   sufficient content review.
3. Artifact-derived SBOM, current advisory/upstream security review, actual
   containment/authentication/update/sync fault evidence and independent review.
4. Real release identity, signing/notarization/store configuration and controlled
   update hosting with offline threshold root. `security/update-channel.json`
   remains disabled; no signing secrets or trust root were created here.
5. Privacy/dependency API and store/export review for the actual mobile binary,
   production assets/accessibility/localization and the deferred source scope.
6. Documented, operational private vulnerability-reporting route and a support
   policy appropriate to the eventually qualified release. This document does
   not enable GitHub private vulnerability reporting or promise an SLA.

Item 15 remains open. Synthetic tests establish only the specified gate behavior;
the subsequent consolidated verification is recorded in `ITEM16_VERIFICATION.md`.
The 13 release-gate cases and 11 updater package/signature integration cases
passed in assigned frozen batches. They do not replace the pending reviews.

Reproduce the synthetic gate cases with `python -B qa/release/test_release_inventory.py`.
Author a fresh locked-source review inventory with
`python -B tools/inventory_locked_dependencies.py --lock Cargo.lock --output <new-file>`.
The latter command does not run a vulnerability database or license audit.

## Batch 09 authored source (Items 96–104)

- **Third-Party License Ledger (`docs/THIRD_PARTY_LICENSES.md`)**: Full provenance ledger covering libretro cores (mGBA, Beetle PSX, bsnes, SameBoy, PPSSPP), codecs (OpenH264, libvpx, Opus), dependencies (SQLite, rusqlite, ring, tokio), fonts, and firmware.
- **Corresponding Source Archival (`tools/archive_corresponding_source.py`, `docs/CORRESPONDING_SOURCE.md`)**: Automated packaging of copyleft source trees and adaptation patches (bsnes IPL removal) for GPL/MPL compliance.
- **Automated SPDX 2.3 SBOM Generator (`tools/generate_sbom.py`)**: Produces standardized `dist/omniplay-sbom.spdx.json` containing 333 packages with SHA-256 checksums and relationship links.
- **Security Advisory & CVE Auditor (`tools/audit_security_advisories.py`, `docs/SECURITY_CVE_AUDIT.md`)**: Audits dependency freshness, resolves historical CVEs, and monitors Chromium security milestone baseline.
- **Production Trust Root Specification (`security/release-identity.example.json`, `docs/PRODUCTION_TRUST_ROOT.md`)**: Specifies 3-of-5 Ed25519 offline threshold key ceremony and HTTPS deployment without embedding private keys in the repository.
- **Platform Code Signing Configuration (`docs/PLATFORM_SIGNING_CONFIG.md`)**: Recipes for Windows Authenticode, macOS Notarization, Android Play Signing, iOS Distribution, and Linux GPG repositories.
- **App Store Privacy & Export Declarations (`docs/APP_STORE_PRIVACY_COMPLIANCE.md`)**: Apple Nutrition Labels, Google Play Data Safety, EAR ECCN 5D002 open-source encryption exemption.
- **Security Policy & Vulnerability Intake (`SECURITY.md`)**: Operational intake at `security@vlikeon.vn`, 48h initial ack SLA, 5-day triage, and 90-day coordinated disclosure policy.
- **Independent Security Audit Plan (`docs/INDEPENDENT_SECURITY_AUDIT.md`)**: 4-part audit scope (Sandbox Containment, DTLS/SRTP & Sync Crypto, Update Verification, IPC Broker) and deferred qualification queue.
- **Automated Qualification Suite (`qa/security_license_qualification.py`)**: Passed 6/6 tests cleanly.
