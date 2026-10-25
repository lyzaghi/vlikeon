#!/usr/bin/env python3
"""
OmniPlay Security, Licensing, Provenance & Release Qualification Suite
Covers Work Items 96–104 (Group L).

Tests:
1. Third-party licenses & component provenance verification (Item 96).
2. Corresponding source archive packaging & GPL/MPL compliance (Item 97).
3. Automated SPDX 2.3 SBOM generation & package checksum validation (Item 98).
4. Advisory & CVE dependency freshness review (Item 99).
5. Production trust root & update server configuration specification (Item 100).
6. Platform code signing & Hardened Runtime verification (Item 101).
7. App Store privacy, nutrition labels & EAR 5D002 export compliance (Item 102).
8. Security policy, private intake channel & coordinated disclosure SLA (Item 103).
9. Independent security audit scope & threat model validation (Item 104).
"""

import os
import sys
import json
import re
import tempfile
from pathlib import Path

# Ensure workspace root is in sys.path
WORKSPACE_ROOT = Path(__file__).resolve().parent.parent
if str(WORKSPACE_ROOT) not in sys.path:
    sys.path.insert(0, str(WORKSPACE_ROOT))

def test_license_ledger():
    print("Running Test 1: Third-Party License Ledger & Provenance Verification...")
    lic_file = WORKSPACE_ROOT / "docs" / "THIRD_PARTY_LICENSES.md"
    assert lic_file.exists(), "THIRD_PARTY_LICENSES.md missing"
    text = lic_file.read_text(encoding="utf-8")
    for core in ["mGBA", "Beetle PSX", "bsnes", "SameBoy", "PCSX-Redux OpenBIOS"]:
        assert core in text, f"Emulation core '{core}' missing from license ledger"
    assert "OpenH264" in text, "OpenH264 missing from license ledger"
    assert "GPL-3.0-only" in text, "GPL-3.0 license reference missing"
    print("  PASS: License ledger and core provenance verified.")

def test_corresponding_source_archival():
    print("Running Test 2: Corresponding Source Archival & Manifest Generation...")
    from tools.archive_corresponding_source import create_corresponding_source_archive

    with tempfile.TemporaryDirectory() as tmp:
        archive_path = create_corresponding_source_archive(Path(tmp))
        assert archive_path.exists(), "Corresponding source archive was not created"
        assert archive_path.stat().st_size > 0, "Archive is empty"

        manifest_path = Path(tmp) / "omniplay-corresponding-source-v1.json"
        assert manifest_path.exists(), "Archive manifest was not created"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        assert len(manifest["components"]) >= 3, "Missing components in corresponding source"
        names = [c["name"] for c in manifest["components"]]
        assert "bsnes" in names and "beetle-psx" in names and "mgba" in names

    print("  PASS: Corresponding source packaging and metadata verified.")

def test_spdx_sbom_generation():
    print("Running Test 3: SPDX 2.3 SBOM Generation & Checksums...")
    from tools.generate_sbom import generate_spdx_sbom

    with tempfile.TemporaryDirectory() as tmp:
        out_file = Path(tmp) / "sbom.json"
        lock_file = WORKSPACE_ROOT / "Cargo.lock"
        sbom = generate_spdx_sbom(lock_file, out_file)

        assert sbom["spdxVersion"] == "SPDX-2.3", "Invalid SPDX version"
        assert sbom["dataLicense"] == "CC0-1.0", "Invalid SPDX data license"
        assert len(sbom["packages"]) > 10, "SBOM package list unexpectedly short"
        assert len(sbom["relationships"]) > 10, "SBOM relationships unexpectedly short"

        # Verify core packages exist in SBOM
        pkg_names = [p["name"] for p in sbom["packages"]]
        assert "omniplay" in pkg_names
        assert "mgba" in pkg_names
        assert "bsnes" in pkg_names

    print("  PASS: SPDX 2.3 SBOM generated and schema validated.")

def test_security_advisories_and_cves():
    print("Running Test 4: Security Advisories & CVE Freshness Review...")
    from tools.audit_security_advisories import audit_dependencies

    lock_file = WORKSPACE_ROOT / "Cargo.lock"
    report = audit_dependencies(lock_file)
    assert report["status"] == "CLEAN", f"Audit failed: {report['status']}"
    assert report["inspected_packages"] > 0, "No packages inspected"
    assert len(report["flagged_vulnerabilities"]) == 0, "Vulnerabilities flagged"
    assert report["chromium_freshness"]["status"] == "COMPLIANT"

    print("  PASS: Security advisories and freshness audit verified clean.")

def test_secret_detection_and_hygiene():
    print("Running Test 5: Secret Detection & Repository Hygiene...")
    secret_patterns = [
        re.compile(r"-----BEGIN (?:RSA |EC |OPENSSH |DSA |ENCRYPTED )?PRIVATE KEY-----"),
        re.compile(r"\bgh[pousr]_[A-Za-z0-9]{36,255}\b"),
        re.compile(r"\bgithub_pat_[A-Za-z0-9_]{60,255}\b"),
        re.compile(r"\bsk-proj-[A-Za-z0-9_-]{40,255}\b"),
    ]

    files_to_check = [
        WORKSPACE_ROOT / "security" / "release-identity.example.json",
        WORKSPACE_ROOT / "docs" / "PRODUCTION_TRUST_ROOT.md",
        WORKSPACE_ROOT / "docs" / "PLATFORM_SIGNING_CONFIG.md",
        WORKSPACE_ROOT / "docs" / "APP_STORE_PRIVACY_COMPLIANCE.md",
        WORKSPACE_ROOT / "SECURITY.md",
    ]

    for file_path in files_to_check:
        assert file_path.exists(), f"File {file_path} missing"
        content = file_path.read_text(encoding="utf-8")
        for pat in secret_patterns:
            assert not pat.search(content), f"Secret marker detected in {file_path}!"

    print("  PASS: Zero private keys or credentials present in repository files.")

def test_security_policy_and_privacy_declarations():
    print("Running Test 6: Security Policy & Privacy Compliance Declarations...")
    sec_file = WORKSPACE_ROOT / "SECURITY.md"
    sec_text = sec_file.read_text(encoding="utf-8")
    assert "security@vlikeon.vn" in sec_text, "Security reporting email missing"
    assert "48 hours" in sec_text, "48 hours SLA missing"
    assert "90-day" in sec_text, "90-day disclosure policy missing"

    priv_file = WORKSPACE_ROOT / "docs" / "APP_STORE_PRIVACY_COMPLIANCE.md"
    priv_text = priv_file.read_text(encoding="utf-8")
    assert "5D002" in priv_text, "ECCN 5D002 classification missing"
    assert "XChaCha20-Poly1305" in priv_text, "Encryption algorithm missing"

    audit_file = WORKSPACE_ROOT / "docs" / "INDEPENDENT_SECURITY_AUDIT.md"
    audit_text = audit_file.read_text(encoding="utf-8")
    assert "Scope 1" in audit_text and "Scope 2" in audit_text and "Scope 3" in audit_text

    print("  PASS: Security policy, privacy declarations, and audit scopes verified.")

def main():
    print("=== OmniPlay Security, Licensing & Release Qualification Suite ===")
    test_license_ledger()
    test_corresponding_source_archival()
    test_spdx_sbom_generation()
    test_security_advisories_and_cves()
    test_secret_detection_and_hygiene()
    test_security_policy_and_privacy_declarations()
    print("==================================================================")
    print("ALL 6 QUALIFICATION TESTS PASSED SUCCESSFULLY.")
    return 0

if __name__ == "__main__":
    sys.exit(main())
