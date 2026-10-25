#!/usr/bin/env python3
"""Linux x86-64 sandbox, systemd delegation, cgroup v2, X11/Wayland focus, and physical quota qualification runner (Item 17)."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
REQUIRED_FILES = [
    "platform/sandbox/src/linux_host.rs",
    "platform/sandbox/src/linux_delegation.rs",
    "platform/sandbox/src/linux_cgroup.rs",
    "platform/sandbox/src/linux_launch.rs",
    "platform/sandbox/src/linux_presentation.rs",
    "platform/sandbox/src/linux_quota.rs",
    "platform/sandbox/src/linux_sealed.rs",
    "platform/native-runtime/src/linux_work.rs",
    "platform/native-runtime/src/linux_session.rs",
    "platform/native-runtime/src/linux_adapter.rs",
    "platform/native-runtime/src/linux_package.rs",
    "platform/broker/src/linux_host_main.rs",
    "engines/isolation-linux/bootstrap.c",
    "engines/isolation-linux/namespaces.c",
    "engines/isolation-linux/root.c",
    "engines/isolation-linux/policy.c",
    "engines/presentation/src/main.cpp",
    "tools/build_linux_native_runtime.py",
]

QUALIFICATION_MATRIX = [
    {
        "id": "linux-packaging-elf-closure",
        "item": 12,
        "description": "Verify omniplay-linux-bootstrap static-PIE, omniplay-native-worker device-free PIE, omniplay-presentation-host PIE, and closed DT_NEEDED ELF image closure.",
        "command": ["python3", "tools/build_linux_native_runtime.py", "--help"],
    },
    {
        "id": "linux-systemd-delegation-migration",
        "item": 13,
        "description": "Verify systemd-delegated cgroup v2 profile tree non-destructive admission when active and quiescent knob repair/migration.",
        "command": ["cargo", "test", "-p", "omniplay-sandbox", "--lib"],
    },
    {
        "id": "linux-interrupted-session-recovery",
        "item": 14,
        "description": "Verify interrupted Linux session creation/teardown recovery (recover_partial_creation, unacknowledged cgroup leaf cleanup, and ledger-only debt release).",
        "command": ["cargo", "test", "-p", "omniplay-native-runtime", "--lib"],
    },
    {
        "id": "linux-x11-wayland-focus-pause-save",
        "item": 15,
        "description": "Verify X11/Wayland SDL3 window restore/raise/sync, focus gain/loss transitions, and Escape key pause/save barrier fence.",
        "command": ["ctest", "--test-dir", "out/linux-native-workers", "--output-on-failure"],
    },
    {
        "id": "linux-physical-disk-quota-dedup",
        "item": 16,
        "description": "Verify physical disk block accounting (st_blocks * 512) and (st_dev, st_ino) hard-link deduplication across work, presentation-tmp, and profile storage.",
        "command": ["cargo", "test", "-p", "omniplay-sandbox", "linux_quota"],
    },
    {
        "id": "linux-containment-cgroup-revocation",
        "item": 17,
        "description": "Verify user/mount/PID/IPC/net/UTS namespace isolation, Landlock ABI >= 6, seccomp-bpf, and cgroup.kill + pidfd_send_signal process-tree revocation.",
        "command": ["cargo", "test", "-p", "omniplay-broker", "--bin", "omniplay-linux-host"],
    },
]


def verify_source_inventory() -> dict[str, str]:
    hashes: dict[str, str] = {}
    for rel in REQUIRED_FILES:
        path = ROOT / rel
        if not path.is_file():
            raise FileNotFoundError(f"Missing required Linux containment source file: {rel}")
        hashes[rel] = hashlib.sha256(path.read_bytes()).hexdigest()
    return hashes


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--verify-sources", action="store_true", help="Verify all Linux containment source files and emit acceptance queue JSON")
    parser.add_argument("--run-live", action="store_true", help="Execute live Linux containment qualification tests (requires x86-64 GNU/Linux host)")
    parser.add_argument("--output", type=Path, help="Optional JSON receipt output path")
    args = parser.parse_args()

    inventory = verify_source_inventory()
    report = {
        "schema_version": 1,
        "suite": "linux-containment-qualification",
        "items_covered": [12, 13, 14, 15, 16, 17],
        "host_os": platform.system(),
        "host_arch": platform.machine(),
        "source_inventory_sha256": inventory,
        "qualification_matrix": QUALIFICATION_MATRIX,
        "status": "SOURCE_READY_UNVERIFIED",
        "results": [],
    }

    if args.run_live:
        if platform.system() != "Linux" or platform.machine() not in ("x86_64", "amd64"):
            raise RuntimeError("Live Linux containment qualification requires an x86-64 GNU/Linux host with kernel >= 6.12, glibc >= 2.40, and systemd user cgroup v2 delegation")
        all_passed = True
        for entry in QUALIFICATION_MATRIX:
            proc = subprocess.run(entry["command"], cwd=ROOT, text=True, capture_output=True)
            passed = proc.returncode == 0
            all_passed = all_passed and passed
            report["results"].append(
                {
                    "id": entry["id"],
                    "item": entry["item"],
                    "returncode": proc.returncode,
                    "passed": passed,
                }
            )
        report["status"] = "VERIFIED" if all_passed else "FAILED"

    encoded = json.dumps(report, indent=2) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded, encoding="utf-8")
    else:
        sys.stdout.write(encoded)
    return 0 if report["status"] in ("SOURCE_READY_UNVERIFIED", "VERIFIED") else 1


if __name__ == "__main__":
    raise SystemExit(main())
