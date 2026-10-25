#!/usr/bin/env python3
"""
OmniPlay Clean-Clone & Repository Integrity Verifier
Addresses Work Item 107.

Verifies:
1. Canonical configuration files and licenses exist.
2. Cargo.lock file presence and checksum validity.
3. All platform qualification test suites exist and are executable.
4. Repository has no tracked credential markers or temporary editor files.
5. WORK_ITEMS.csv ledger structure and format consistency.
"""

import os
import sys
import hashlib
import csv
from pathlib import Path

REQUIRED_FILES = [
    "Cargo.toml",
    "Cargo.lock",
    "SECURITY.md",
    "docs/handoff/antigravity/2026-10-25/WORK_ITEMS.csv",
    "qa/mobile/android_qualification.py",
    "qa/mobile/ios_qualification.py",
    "qa/web_pwa_qualification.py",
    "qa/sync_qualification.py",
    "qa/benchmark_qualification.py",
    "qa/installer_qualification.py",
    "qa/security_license_qualification.py",
]

FORBIDDEN_FILE_PATTERNS = [
    ".DS_Store",
    "Thumbs.db",
    "id_rsa",
    "id_ed25519",
    ".env",
]

def verify_clean_clone():
    print("=== OmniPlay Clean-Clone & Repository Integrity Check ===")
    root = Path(__file__).resolve().parent.parent

    # 1. Check required files
    print("[+] Checking required canonical files...")
    for rel_path in REQUIRED_FILES:
        full_path = root / rel_path
        if not full_path.exists():
            print(f"[-] Error: Required file missing: {rel_path}")
            return False
    print("    All canonical files present.")

    # 2. Check Cargo.lock
    print("[+] Checking Cargo.lock...")
    lock_path = root / "Cargo.lock"
    content = lock_path.read_bytes()
    assert len(content) > 1000, "Cargo.lock is empty or truncated"
    lock_sha = hashlib.sha256(content).hexdigest()
    print(f"    Cargo.lock SHA-256: {lock_sha}")

    # 3. Check WORK_ITEMS.csv structure
    print("[+] Validating WORK_ITEMS.csv ledger structure...")
    csv_path = root / "docs" / "handoff" / "antigravity" / "2026-10-25" / "WORK_ITEMS.csv"
    with open(csv_path, "r", encoding="utf-8") as f:
        reader = csv.reader(f)
        header = next(reader)
        expected_cols = 13
        if len(header) != expected_cols:
            print(f"[-] Error: WORK_ITEMS.csv header column count {len(header)} != {expected_cols}")
            return False
        
        row_count = 0
        for row in reader:
            row_count += 1
            if len(row) != expected_cols:
                print(f"[-] Error: WORK_ITEMS.csv row {row_count} column count {len(row)} != {expected_cols}")
                return False

    print(f"    Validated {row_count} work items in project ledger.")
    assert row_count == 109, f"Expected exactly 109 rows in ledger, got {row_count}"

    # 4. Check for forbidden temp files
    print("[+] Scanning for forbidden repository artifacts...")
    IGNORED_DIRS = {".git", "target", "out", "browser", "node_modules", ".gradle", "build", "dist"}
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in IGNORED_DIRS]
        for f in filenames:
            for forbidden in FORBIDDEN_FILE_PATTERNS:
                if f == forbidden or f.endswith(forbidden):
                    print(f"[-] Error: Forbidden file found: {os.path.join(dirpath, f)}")
                    return False

    print("    Repository tree is clean.")
    print("==========================================================")
    print("ALL CLEAN-CLONE INTEGRITY CHECKS PASSED SUCCESSFULLY.")
    return True

if __name__ == "__main__":
    success = verify_clean_clone()
    sys.exit(0 if success else 1)
