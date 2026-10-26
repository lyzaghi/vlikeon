#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="${1:-$(pwd)}"
WORK_ROOT="${2:-$HOME/omniplay-linux-browser-build}"
CHECKOUT_PARENT="${3:-$WORK_ROOT/chromium}"
OUTPUT_PATH="${4:-$WORK_ROOT/build-host-preflight.json}"

python3 - "$REPO_ROOT" "$WORK_ROOT" "$CHECKOUT_PARENT" "$OUTPUT_PATH" <<'PY'
import json
import os
import platform
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

repo_root = Path(sys.argv[1]).resolve()
work_root = Path(sys.argv[2]).resolve()
checkout_parent = Path(sys.argv[3]).resolve()
output_path = Path(sys.argv[4]).resolve()


def run_cmd(argv: list[str], cwd: Path | None = None) -> dict[str, object]:
    try:
        completed = subprocess.run(
            argv,
            cwd=str(cwd) if cwd else None,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=15,
            check=False,
        )
        return {
            "argv": argv,
            "returncode": completed.returncode,
            "stdout": completed.stdout.strip(),
            "stderr": completed.stderr.strip(),
        }
    except Exception as exc:
        return {
            "argv": argv,
            "returncode": -1,
            "stdout": "",
            "stderr": f"{type(exc).__name__}: {exc}",
        }


def disk_info(target: Path) -> dict[str, object]:
    probe = target
    while not probe.exists() and probe.parent != probe:
        probe = probe.parent
    usage = shutil.disk_usage(probe)
    return {
        "requested_path": str(target),
        "existing_probe_path": str(probe),
        "total_bytes": usage.total,
        "used_bytes": usage.used,
        "free_bytes": usage.free,
        "free_gib": round(usage.free / (1024 ** 3), 2),
    }


def read_meminfo() -> dict[str, int]:
    result: dict[str, int] = {}
    meminfo = Path("/proc/meminfo")
    if not meminfo.is_file():
        return result
    for line in meminfo.read_text(encoding="utf-8", errors="replace").splitlines():
        if ":" not in line:
            continue
        key, rest = line.split(":", 1)
        parts = rest.strip().split()
        if parts and parts[0].isdigit():
            result[key.strip()] = int(parts[0]) * 1024
    return result


depot_tools_candidates = [
    work_root.parent / "depot_tools",
    work_root / "depot_tools",
    Path.home() / "depot_tools",
]
resolved_depot_tools = next((p for p in depot_tools_candidates if (p / "gclient").is_file()), None)

tools = {}
for name in [
    "git",
    "python3",
    "ninja",
    "gn",
    "gclient",
    "fetch",
    "clang",
    "clang++",
    "ld.lld",
    "pkg-config",
    "bison",
    "flex",
    "gperf",
    "curl",
    "xz",
]:
    found = shutil.which(name)
    if not found and resolved_depot_tools and (resolved_depot_tools / name).is_file():
        found = str(resolved_depot_tools / name)
    tools[name] = found

pkg_checks = {}
if tools["pkg-config"]:
    for pkg in [
        "glib-2.0",
        "gtk+-3.0",
        "nss",
        "atk",
        "ATK-bridge-2.0",
        "drm",
        "gbm",
        "xkbcommon",
        "alsa",
        "libpulse",
        "libudev",
        "wayland-client",
    ]:
        probe = run_cmd([tools["pkg-config"], "--exists", pkg.lower()])
        pkg_checks[pkg.lower()] = probe["returncode"] == 0

mem = read_meminfo()
work_disk = disk_info(work_root)
checkout_disk = disk_info(checkout_parent)

src_dir = checkout_parent / "src"
gclient_file = checkout_parent / ".gclient"
active_procs = run_cmd(["ps", "-eo", "pid=,ppid=,pgid=,stat=,etimes=,comm=,args="])
chromium_jobs = []
if active_procs["returncode"] == 0:
    for line in str(active_procs["stdout"]).splitlines():
        if any(tok in line for tok in ("gclient", "fetch", "git-remote-http", "ninja", "siso", "prepare_desktop_browser", "build_desktop_browser")):
            chromium_jobs.append(line.strip())

blockers = []
if platform.system() != "Linux" or platform.machine() not in ("x86_64", "amd64"):
    blockers.append(f"Expected Linux x86_64 host, got {platform.system()} {platform.machine()}")
if checkout_disk["free_bytes"] < 100 * (1024 ** 3):
    blockers.append(f"Checkout volume has {checkout_disk['free_gib']} GiB free (< 100 GiB minimum)")
if not tools["git"] or not tools["python3"]:
    blockers.append("Missing required host git or python3")
if not tools["gclient"] or not tools["fetch"]:
    blockers.append("Missing depot_tools (gclient/fetch)")

summary = {
    "schema_version": 1,
    "item_id": 4,
    "target": "linux-x86_64",
    "generated_at_utc": datetime.now(timezone.utc).isoformat(),
    "host": {
        "system": platform.system(),
        "release": platform.release(),
        "machine": platform.machine(),
        "cpu_count": os.cpu_count(),
        "mem_total_gib": round(mem.get("MemTotal", 0) / (1024 ** 3), 2),
        "mem_available_gib": round(mem.get("MemAvailable", 0) / (1024 ** 3), 2),
        "swap_total_gib": round(mem.get("SwapTotal", 0) / (1024 ** 3), 2),
    },
    "paths": {
        "repo_root": str(repo_root),
        "work_root": str(work_root),
        "checkout_parent": str(checkout_parent),
        "depot_tools": str(resolved_depot_tools) if resolved_depot_tools else None,
        "gclient_config_exists": gclient_file.is_file(),
        "src_exists": src_dir.is_dir(),
        "src_args_gn_exists": (src_dir / "out" / "OmniPlayRelease" / "args.gn").is_file(),
    },
    "disk": {
        "work_root": work_disk,
        "checkout_parent": checkout_disk,
    },
    "tools": tools,
    "pkg_config_packages": pkg_checks,
    "active_chromium_processes": chromium_jobs,
    "ready_for_checkout_or_build": len(blockers) == 0,
    "blockers": blockers,
}

output_path.parent.mkdir(parents=True, exist_ok=True)
output_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
print(json.dumps(summary, indent=2, sort_keys=True))
PY
