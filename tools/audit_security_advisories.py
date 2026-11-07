#!/usr/bin/env python3
"""
OmniPlay Security Advisory & CVE Freshness Auditor
Addresses Work Item 99.

Scans exact crate versions parsed from Cargo.lock against the versioned
RustSec advisory dataset (security/advisory-database.json) and verifies
the pinned Chromium release/revision in browser/upstream.lock.json against
official security baseline thresholds. Fails closed when lockfiles or
advisory data are missing/unreadable or when any vulnerability is found.
"""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import sys
import tomllib
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_CARGO_LOCK = REPO_ROOT / "Cargo.lock"
DEFAULT_ADVISORY_DB = REPO_ROOT / "security" / "advisory-database.json"
DEFAULT_UPSTREAM_LOCK = REPO_ROOT / "browser" / "upstream.lock.json"


def _utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def _parse_version_tuple(raw: str) -> tuple[int, ...]:
    """Parse a dotted numeric or semver string into an integer tuple (ignoring pre-release/build metadata for numeric comparison)."""
    core = raw.strip().split("-", 1)[0].split("+", 1)[0]
    if not core:
        raise ValueError(f"Empty version string: {raw!r}")
    parts = core.split(".")
    result: list[int] = []
    for p in parts:
        if not p.isdigit():
            raise ValueError(f"Non-numeric version segment {p!r} in {raw!r}")
        result.append(int(p))
    return tuple(result)


def _compare_versions(lhs: str, rhs: str) -> int:
    l_tup = _parse_version_tuple(lhs)
    r_tup = _parse_version_tuple(rhs)
    max_len = max(len(l_tup), len(r_tup))
    l_pad = l_tup + (0,) * (max_len - len(l_tup))
    r_pad = r_tup + (0,) * (max_len - len(r_tup))
    if l_pad < r_pad:
        return -1
    if l_pad > r_pad:
        return 1
    return 0


_CLAUSE_RE = re.compile(r"^(<=|>=|<|>|==|=)\s*([0-9][0-9A-Za-z.\-+]*)$")


def _matches_clause(version: str, clause: str) -> bool:
    m = _CLAUSE_RE.match(clause.strip())
    if not m:
        raise ValueError(f"Unsupported version range clause: {clause!r}")
    op, target = m.group(1), m.group(2)
    cmp_val = _compare_versions(version, target)
    if op == "<":
        return cmp_val < 0
    if op == "<=":
        return cmp_val <= 0
    if op == ">":
        return cmp_val > 0
    if op == ">=":
        return cmp_val >= 0
    if op in ("=", "=="):
        return cmp_val == 0
    raise ValueError(f"Unknown operator {op!r}")


def version_matches_range_expr(version: str, expr: str) -> bool:
    """Return True if `version` satisfies all comma-separated clauses in `expr`."""
    clauses = [c.strip() for c in expr.split(",") if c.strip()]
    if not clauses:
        raise ValueError(f"Empty version range expression: {expr!r}")
    return all(_matches_clause(version, c) for c in clauses)


def audit_dependencies(
    cargo_lock_path: Path = DEFAULT_CARGO_LOCK,
    *,
    advisory_db_path: Path = DEFAULT_ADVISORY_DB,
    upstream_lock_path: Path = DEFAULT_UPSTREAM_LOCK,
) -> dict[str, Any]:
    print("[+] Auditing dependencies for known security advisories and CVEs...")
    errors: list[str] = []
    flagged_vulnerabilities: list[dict[str, Any]] = []

    report: dict[str, Any] = {
        "schema_version": 1,
        "generated_at_utc": _utc_now(),
        "status": "UNVERIFIED",
        "cargo_lock_present": False,
        "cargo_lock_sha256": None,
        "inspected_packages": 0,
        "advisory_database": {
            "present": False,
            "path": str(advisory_db_path),
            "dataset_id": None,
            "dataset_version": None,
            "dataset_date": None,
            "sha256": None,
            "rustsec_rules_count": 0,
        },
        "resolved_advisories_checked": 0,
        "flagged_vulnerabilities": flagged_vulnerabilities,
        "chromium_freshness": {
            "upstream_lock_present": False,
            "pinned_version": None,
            "pinned_revision": None,
            "minimum_version": None,
            "target_milestone": None,
            "covered_cves_verified": 0,
            "status": "UNVERIFIED",
        },
        "errors": errors,
    }

    # 1. Load and validate advisory database
    if not advisory_db_path.is_file():
        errors.append(f"Advisory database file is missing: {advisory_db_path}")
        print(f"[-] Error: {errors[-1]}")
        return report

    try:
        db_bytes = advisory_db_path.read_bytes()
        db_sha256 = hashlib.sha256(db_bytes).hexdigest()
        db_data = json.loads(db_bytes.decode("utf-8"))
        if not isinstance(db_data, dict):
            raise ValueError("Advisory database root must be a JSON object")
        dataset_version = str(db_data.get("dataset_version") or "").strip()
        dataset_date = str(db_data.get("dataset_date") or "").strip()
        rustsec_advisories = db_data.get("rustsec_advisories")
        chromium_baseline = db_data.get("chromium_security_baseline")
        if (
            not dataset_version
            or not dataset_date
            or not isinstance(rustsec_advisories, list)
            or not rustsec_advisories
            or not isinstance(chromium_baseline, dict)
        ):
            raise ValueError("Advisory database is missing required version/date/rules fields")
    except Exception as exc:
        errors.append(f"Failed to parse advisory database {advisory_db_path}: {exc}")
        print(f"[-] Error: {errors[-1]}")
        return report

    report["advisory_database"] = {
        "present": True,
        "path": str(advisory_db_path),
        "dataset_id": db_data.get("dataset_id"),
        "dataset_version": dataset_version,
        "dataset_date": dataset_date,
        "sha256": db_sha256,
        "rustsec_rules_count": len(rustsec_advisories),
        "sources": db_data.get("sources", {}),
    }

    # 2. Load and parse Cargo.lock
    if not cargo_lock_path.is_file():
        errors.append(f"Cargo.lock is missing: {cargo_lock_path}")
        print(f"[-] Error: {errors[-1]}")
        return report

    try:
        lock_bytes = cargo_lock_path.read_bytes()
        lock_sha256 = hashlib.sha256(lock_bytes).hexdigest()
        lock_data = tomllib.loads(lock_bytes.decode("utf-8"))
        packages = lock_data.get("package", [])
        if not isinstance(packages, list) or len(packages) == 0:
            raise ValueError("Cargo.lock contains 0 [[package]] entries")
    except Exception as exc:
        errors.append(f"Failed to parse Cargo.lock ({cargo_lock_path}): {exc}")
        print(f"[-] Error: {errors[-1]}")
        return report

    report["cargo_lock_present"] = True
    report["cargo_lock_sha256"] = lock_sha256

    crates_by_name: dict[str, list[dict[str, Any]]] = {}
    inspected_count = 0
    for pkg in packages:
        if not isinstance(pkg, dict):
            continue
        name = pkg.get("name")
        version = pkg.get("version")
        if not isinstance(name, str) or not name or not isinstance(version, str) or not version:
            errors.append(f"Malformed package entry in Cargo.lock: {pkg!r}")
            return report
        try:
            _parse_version_tuple(version)
        except ValueError as exc:
            errors.append(f"Invalid crate version in Cargo.lock for {name}: {version} ({exc})")
            return report
        crates_by_name.setdefault(name, []).append(pkg)
        inspected_count += 1

    report["inspected_packages"] = inspected_count

    # 3. Evaluate all RustSec advisories against locked crate versions
    checked_rules = 0
    for adv in rustsec_advisories:
        if not isinstance(adv, dict):
            errors.append("Malformed advisory entry in rustsec_advisories")
            return report
        adv_id = str(adv.get("id") or "")
        pkg_name = str(adv.get("package") or "")
        affected_ranges = adv.get("affected_ranges", [])
        if not adv_id or not pkg_name or not isinstance(affected_ranges, list) or not affected_ranges:
            errors.append(f"Incomplete advisory rule in database: {adv!r}")
            return report

        checked_rules += 1
        matching_crates = crates_by_name.get(pkg_name, [])
        for crate_pkg in matching_crates:
            crate_ver = str(crate_pkg["version"])
            try:
                is_vulnerable = any(
                    version_matches_range_expr(crate_ver, str(rng)) for rng in affected_ranges
                )
            except ValueError as exc:
                errors.append(f"Invalid range in advisory {adv_id}: {exc}")
                return report

            if is_vulnerable:
                flagged_vulnerabilities.append(
                    {
                        "id": adv_id,
                        "cve": adv.get("cve"),
                        "component": pkg_name,
                        "locked_version": crate_ver,
                        "affected_ranges": affected_ranges,
                        "patched_versions": adv.get("patched_versions", []),
                        "severity": adv.get("severity", "HIGH"),
                        "description": adv.get("description", ""),
                    }
                )

    # 4. Evaluate Chromium security baseline against browser/upstream.lock.json
    if not upstream_lock_path.is_file():
        errors.append(f"Chromium upstream lockfile is missing: {upstream_lock_path}")
        print(f"[-] Error: {errors[-1]}")
        return report

    try:
        upstream_data = json.loads(upstream_lock_path.read_text(encoding="utf-8"))
        pinned_version = str(upstream_data.get("version") or "").strip()
        pinned_revision = str(upstream_data.get("revision") or "").strip()
        min_chromium_version = str(chromium_baseline.get("minimum_version") or "").strip()
        approved_revisions = chromium_baseline.get("approved_revisions", [])
        covered_cves = chromium_baseline.get("covered_cves", [])
        if not pinned_version or not pinned_revision or not min_chromium_version:
            raise ValueError("Missing version or revision in upstream.lock.json or baseline")
        pinned_ver_tuple = _parse_version_tuple(pinned_version)
        min_ver_tuple = _parse_version_tuple(min_chromium_version)
    except Exception as exc:
        errors.append(f"Failed to validate Chromium lockfile {upstream_lock_path}: {exc}")
        print(f"[-] Error: {errors[-1]}")
        return report

    chromium_status = "COMPLIANT"
    if _compare_versions(pinned_version, min_chromium_version) < 0:
        chromium_status = "OUTDATED"
        flagged_vulnerabilities.append(
            {
                "id": "CHROMIUM-BASELINE-STALE",
                "cve": None,
                "component": "Chromium",
                "locked_version": pinned_version,
                "required_minimum_version": min_chromium_version,
                "severity": "HIGH",
                "description": (
                    f"Pinned Chromium version {pinned_version} is older than security baseline {min_chromium_version}"
                ),
            }
        )

    if isinstance(approved_revisions, list) and approved_revisions:
        if pinned_revision not in approved_revisions and _compare_versions(pinned_version, min_chromium_version) == 0:
            chromium_status = "UNAPPROVED_REVISION"
            flagged_vulnerabilities.append(
                {
                    "id": "CHROMIUM-REVISION-MISMATCH",
                    "cve": None,
                    "component": "Chromium",
                    "locked_version": pinned_version,
                    "locked_revision": pinned_revision,
                    "severity": "HIGH",
                    "description": f"Pinned Chromium revision {pinned_revision} is not in approved_revisions",
                }
            )

    verified_cves = 0
    if isinstance(covered_cves, list):
        for cve_entry in covered_cves:
            if not isinstance(cve_entry, dict):
                continue
            cve_id = str(cve_entry.get("id") or "")
            fixed_in = str(cve_entry.get("fixed_in_chromium") or "")
            if not cve_id or not fixed_in:
                continue
            checked_rules += 1
            verified_cves += 1
            if _compare_versions(pinned_version, fixed_in) < 0:
                chromium_status = "VULNERABLE"
                flagged_vulnerabilities.append(
                    {
                        "id": cve_id,
                        "cve": cve_id,
                        "component": cve_entry.get("component", "Chromium"),
                        "locked_version": pinned_version,
                        "fixed_in_chromium": fixed_in,
                        "severity": cve_entry.get("severity", "HIGH"),
                        "description": cve_entry.get("description", ""),
                    }
                )

    report["resolved_advisories_checked"] = checked_rules
    report["chromium_freshness"] = {
        "upstream_lock_present": True,
        "pinned_version": pinned_version,
        "pinned_revision": pinned_revision,
        "minimum_version": min_chromium_version,
        "target_milestone": min_ver_tuple[0] if min_ver_tuple else None,
        "pinned_milestone": pinned_ver_tuple[0] if pinned_ver_tuple else None,
        "covered_cves_verified": verified_cves,
        "status": chromium_status,
    }

    if errors:
        report["status"] = "UNVERIFIED"
    elif flagged_vulnerabilities:
        report["status"] = "VULNERABLE"
    else:
        report["status"] = "CLEAN"

    print(
        f"[+] Dataset {dataset_version} ({dataset_date}): inspected {inspected_count} Cargo.lock packages "
        f"and Chromium {pinned_version} ({pinned_revision[:12]}). "
        f"Status={report['status']}, findings={len(flagged_vulnerabilities)}."
    )
    return report


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Audit Cargo.lock and browser/upstream.lock.json against security/advisory-database.json."
    )
    parser.add_argument(
        "--cargo-lock",
        type=Path,
        default=DEFAULT_CARGO_LOCK,
        help="Path to Cargo.lock (defaults to workspace Cargo.lock).",
    )
    parser.add_argument(
        "--advisory-db",
        type=Path,
        default=DEFAULT_ADVISORY_DB,
        help="Path to security/advisory-database.json.",
    )
    parser.add_argument(
        "--upstream-lock",
        type=Path,
        default=DEFAULT_UPSTREAM_LOCK,
        help="Path to browser/upstream.lock.json.",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=REPO_ROOT / "dist" / "security-cve-audit.json",
        help="Path to write JSON audit report.",
    )
    args = parser.parse_args(argv)

    report = audit_dependencies(
        args.cargo_lock,
        advisory_db_path=args.advisory_db,
        upstream_lock_path=args.upstream_lock,
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"[+] Audit report saved to {args.output}")
    return 0 if report["status"] == "CLEAN" else 1


if __name__ == "__main__":
    raise SystemExit(main())
