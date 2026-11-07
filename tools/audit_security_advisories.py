"""Audit Cargo.lock with RustSec and compare the Chromium pin with Windows Stable."""
from __future__ import annotations

import argparse
from datetime import datetime, timedelta, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import tomllib
from urllib.request import urlopen

REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_CARGO_LOCK = REPO_ROOT / "Cargo.lock"
DEFAULT_UPSTREAM_LOCK = REPO_ROOT / "browser" / "upstream.lock.json"
RUSTSEC_URL = "https://github.com/RustSec/advisory-db.git"
CHROMIUM_DASH = "https://chromiumdash.appspot.com/fetch_releases?channel=Stable&platform=Windows&num=1"
MAX_ADVISORY_AGE = timedelta(days=14)
GIT_HASH = re.compile(r"[0-9a-f]{40}")
CHROMIUM_VERSION = re.compile(r"[0-9]{1,8}(?:\.[0-9]{1,8}){3}")


def checked_lock(raw: bytes) -> list[dict]:
    lock = tomllib.loads(raw.decode("utf-8"))
    packages = lock.get("package")
    if not isinstance(packages, list) or not packages:
        raise ValueError("Cargo.lock contains zero package records")
    for package in packages:
        if not isinstance(package, dict) or not all(
            isinstance(package.get(field), str) and package[field].strip() for field in ("name", "version")
        ):
            raise ValueError("Cargo.lock contains a malformed package record")
    return packages


def inspect_rustsec_snapshot(checkout: Path, revision: str, now: datetime) -> dict:
    if not GIT_HASH.fullmatch(revision):
        raise ValueError("Provide the independently selected RustSec commit SHA")
    env = {**os.environ, "GIT_NO_REPLACE_OBJECTS": "1", "GIT_OPTIONAL_LOCKS": "0"}

    def git(*args: str) -> str:
        result = subprocess.run(
            ["git", "-C", str(checkout), *args], check=True, capture_output=True,
            text=True, timeout=30, env=env,
        )
        return result.stdout.strip()

    if git("remote", "get-url", "origin").lower() != RUSTSEC_URL.lower():
        raise ValueError("RustSec snapshot origin does not match the official database")
    if git("rev-parse", "HEAD") != revision or git("status", "--porcelain=v1", "--untracked-files=all", "--ignored"):
        raise ValueError("RustSec snapshot must be clean and match the selected commit SHA")
    tree, timestamp = git("show", "-s", "--format=%T %ct", revision).split()
    committed = datetime.fromtimestamp(int(timestamp), timezone.utc)
    if not -timedelta(minutes=5) <= now - committed <= MAX_ADVISORY_AGE:
        raise ValueError("RustSec snapshot is stale or has a future commit timestamp")
    return {"source": RUSTSEC_URL, "revision": revision, "tree": tree, "committed_at": committed.isoformat()}


def run_cargo_audit(raw: bytes, checkout: Path, executable: Path, expected_count: int) -> dict:
    # Run outside the workspace so a project-local cargo-audit config cannot suppress findings.
    with tempfile.TemporaryDirectory() as temp:
        temp_dir = Path(temp)
        lock = temp_dir / "Cargo.lock"
        lock.write_bytes(raw)
        result = subprocess.run(
            [str(executable), "audit", "--db", str(checkout), "--no-fetch", "--json", "--file", str(lock)],
            cwd=temp_dir, capture_output=True, timeout=120,
        )
    if not result.stdout or result.returncode not in (0, 1):
        raise ValueError(f"cargo-audit could not scan the lockfile (exit {result.returncode})")
    data = json.loads(result.stdout)
    settings = data["settings"]
    findings = data["vulnerabilities"]
    if (
        data["lockfile"]["dependency-count"] != expected_count
        or data["database"]["advisory-count"] <= 0
        or settings["ignore"] or settings["target_arch"] or settings["target_os"] or settings["severity"] is not None
        or findings["count"] != len(findings["list"])
        or findings["found"] != bool(findings["list"])
        or result.returncode != int(bool(findings["list"]))
    ):
        raise ValueError("cargo-audit returned an incomplete, filtered or inconsistent report")
    return data


def fetch_chromium_stable() -> dict:
    with urlopen(CHROMIUM_DASH, timeout=20) as response:
        raw = response.read(65537)
    if len(raw) > 65536:
        raise ValueError("Chromium Dash response exceeds the size limit")
    releases = json.loads(raw)
    if not isinstance(releases, list) or len(releases) != 1:
        raise ValueError("Chromium Dash did not return one Windows Stable release")
    release = releases[0]
    if release["platform"] != "Windows" or release["channel"] != "Stable":
        raise ValueError("Chromium Dash result is not Windows Stable")
    return {
        "version": release["version"],
        "revision": release["hashes"]["chromium"],
        "released_at": datetime.fromtimestamp(release["time"] / 1000, timezone.utc).isoformat(),
        "response_sha256": hashlib.sha256(raw).hexdigest(),
    }


def chromium_freshness(lock_path: Path) -> dict:
    pin = json.loads(lock_path.read_bytes())
    stable = fetch_chromium_stable()
    for release in (pin, stable):
        if not CHROMIUM_VERSION.fullmatch(release["version"]) or not GIT_HASH.fullmatch(release["revision"]):
            raise ValueError("Invalid Chromium version or revision")
    if pin["channel"] != "Stable" or pin["platform"] != "Windows":
        raise ValueError("Expected a Windows Stable upstream lock")
    pinned_version = tuple(map(int, pin["version"].split(".")))
    stable_version = tuple(map(int, stable["version"].split(".")))
    status = "UNVERIFIED"
    if pinned_version < stable_version:
        status = "BEHIND_STABLE"
    elif pinned_version == stable_version and pin["revision"] == stable["revision"]:
        status = "COMPLIANT"
    return {
        "status": status, "source": CHROMIUM_DASH, "pinned_version": pin["version"],
        "pinned_revision": pin["revision"], "stable": stable,
    }


def audit_dependencies(
    cargo_lock_path: Path = DEFAULT_CARGO_LOCK, *, upstream_lock_path: Path = DEFAULT_UPSTREAM_LOCK,
    advisory_db_path: Path | None = None, advisory_revision: str | None = None,
    cargo_audit: Path | None = None,
) -> dict:
    now = datetime.now(timezone.utc)
    report = {
        "schema_version": 2, "generated_at_utc": now.isoformat(), "status": "UNVERIFIED",
        "inspected_packages": 0, "advisory_database": None, "cargo_audit": None,
        "flagged_vulnerabilities": [], "chromium_freshness": {"status": "UNVERIFIED"}, "errors": [],
    }
    errors = report["errors"]
    try:
        raw = cargo_lock_path.read_bytes()
        packages = checked_lock(raw)
        report["cargo_lock_sha256"] = hashlib.sha256(raw).hexdigest()
        if advisory_db_path is None or advisory_revision is None:
            raise ValueError("Provide --advisory-db and --advisory-revision for the official RustSec snapshot")
        checkout = advisory_db_path.absolute()
        report["advisory_database"] = inspect_rustsec_snapshot(checkout, advisory_revision, now)
        resolved_audit = cargo_audit or shutil.which("cargo-audit")
        if resolved_audit is None:
            raise ValueError("Install cargo-audit or specify --cargo-audit")
        audit = run_cargo_audit(raw, checkout, Path(resolved_audit).absolute(), len(packages))
        report["inspected_packages"] = len(packages)
        report["cargo_audit"] = audit
        report["flagged_vulnerabilities"] = audit["vulnerabilities"]["list"]
    except (OSError, ValueError, KeyError, TypeError, AttributeError, subprocess.SubprocessError) as exc:
        errors.append(f"RustSec audit: {exc}")
    try:
        report["chromium_freshness"] = chromium_freshness(upstream_lock_path)
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as exc:
        errors.append(f"Chromium freshness: {exc}")

    freshness = report["chromium_freshness"]["status"]
    if report["flagged_vulnerabilities"]:
        report["status"] = "VULNERABLE"
    elif not errors and freshness == "BEHIND_STABLE":
        report["status"] = "OUTDATED"
    elif not errors and freshness == "COMPLIANT":
        report["status"] = "CLEAN"
    return report


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cargo-lock", type=Path, default=DEFAULT_CARGO_LOCK)
    parser.add_argument("--upstream-lock", type=Path, default=DEFAULT_UPSTREAM_LOCK)
    parser.add_argument("--advisory-db", type=Path, help="Clean checkout of the official RustSec advisory database")
    parser.add_argument("--advisory-revision", help="Independently selected RustSec advisory commit")
    parser.add_argument("--cargo-audit", type=Path, help="Path to the official cargo-audit executable")
    parser.add_argument("--output", type=Path, default=REPO_ROOT / "dist" / "security-cve-audit.json")
    args = parser.parse_args(argv)
    report = audit_dependencies(
        args.cargo_lock, upstream_lock_path=args.upstream_lock, advisory_db_path=args.advisory_db,
        advisory_revision=args.advisory_revision, cargo_audit=args.cargo_audit,
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"Security audit {report['status']} ({report['inspected_packages']} packages): {args.output}")
    return 0 if report["status"] == "CLEAN" else 1


if __name__ == "__main__":
    raise SystemExit(main())
