from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys


REPO_ROOT = Path(__file__).resolve().parents[1]
LOCK_PATH = REPO_ROOT / "browser" / "upstream.lock.json"
CLT_PYTHON_FALLBACK = Path(
    "/Library/Developer/CommandLineTools/Library/Frameworks/Python3.framework/Versions/3.9/bin/python3"
)


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def read_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, payload: dict[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def resolve_macos_python3() -> str:
    """Resolve concrete Xcode/CLT python3 instead of /usr/bin/python3 stub so TMPDIR on external disk is preserved."""
    if platform.system() == "Darwin":
        completed = subprocess.run(
            ["xcrun", "--find", "python3"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )
        candidate = completed.stdout.strip()
        if completed.returncode == 0 and candidate and candidate != "/usr/bin/python3" and Path(candidate).is_file():
            return str(Path(candidate).resolve())
        if CLT_PYTHON_FALLBACK.is_file():
            return str(CLT_PYTHON_FALLBACK.resolve())
    return sys.executable


def inspect_macos_sdk() -> dict[str, object]:
    if platform.system() != "Darwin":
        return {"supported_host": False, "sdk_path": None, "sdk_version": None, "darwin_foundation_split_modulemaps": []}
    sdk_path_res = subprocess.run(
        ["xcrun", "--sdk", "macosx", "--show-sdk-path"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )
    sdk_ver_res = subprocess.run(
        ["xcrun", "--sdk", "macosx", "--show-sdk-version"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )
    sdk_path = sdk_path_res.stdout.strip() if sdk_path_res.returncode == 0 else ""
    sdk_ver = sdk_ver_res.stdout.strip() if sdk_ver_res.returncode == 0 else ""
    split_maps: list[str] = []
    if sdk_path:
        modules_dir = Path(sdk_path) / "usr" / "include"
        for name in ("DarwinFoundation1.modulemap", "DarwinFoundation2.modulemap", "DarwinFoundation3.modulemap"):
            candidate = modules_dir / name
            if candidate.is_file():
                split_maps.append(str(candidate))
    return {
        "supported_host": True,
        "sdk_path": sdk_path or None,
        "sdk_version": sdk_ver or None,
        "darwin_foundation_split_modulemaps": split_maps,
    }


def build_macos_env(depot_tools: Path | None, work_root: Path, concrete_python: str) -> dict[str, str]:
    env = dict(os.environ)
    tmp_dir = (work_root / "tmp").resolve()
    cache_dir = (work_root / "cache").resolve()
    cipd_cache_dir = (work_root / "cipd-cache").resolve()
    for directory in (tmp_dir, cache_dir, cipd_cache_dir):
        directory.mkdir(parents=True, exist_ok=True)

    python_bin_dir = str(Path(concrete_python).parent)
    path_entries = [ entry for entry in env.get("PATH", "").split(os.pathsep) if entry ]
    if python_bin_dir not in path_entries:
        path_entries.insert(0, python_bin_dir)
    if depot_tools is not None:
        depot_str = str(depot_tools.resolve())
        if depot_str not in path_entries:
            path_entries.insert(0, depot_str)
    env["PATH"] = os.pathsep.join(path_entries)
    env["TMPDIR"] = str(tmp_dir)
    env["TMP"] = str(tmp_dir)
    env["TEMP"] = str(tmp_dir)
    env["XDG_CACHE_HOME"] = str(cache_dir)
    env["CIPD_CACHE_DIR"] = str(cipd_cache_dir)
    env["DEPOT_TOOLS_UPDATE"] = "0"
    env["DEPOT_TOOLS_METRICS"] = "0"
    env["GCLIENT_SUPPRESS_GIT_VERSION_WARNINGS"] = "1"
    env["GIT_CONFIG_COUNT"] = "2"
    env["GIT_CONFIG_KEY_0"] = "core.autocrlf"
    env["GIT_CONFIG_VALUE_0"] = "false"
    env["GIT_CONFIG_KEY_1"] = "core.longpaths"
    env["GIT_CONFIG_VALUE_1"] = "true"
    return env


def detect_active_jobs(checkout_parent: Path) -> list[dict[str, object]]:
    if platform.system() == "Windows":
        return []
    completed = subprocess.run(
        ["ps", "-axo", "pid=,ppid=,pgid=,stat=,etime=,comm=,args="],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )
    if completed.returncode != 0:
        return []
    jobs: list[dict[str, object]] = []
    checkout_str = str(checkout_parent)
    tokens = ("gclient", "fetch", "git-remote-http", "ninja", "siso", "build_desktop_browser.py")
    for line in completed.stdout.splitlines():
        parts = line.strip().split(None, 6)
        if len(parts) < 7:
            continue
        pid, ppid, pgid, stat, etime, comm, args = parts
        if int(pid) == os.getpid():
            continue
        if any(tok in comm or tok in args for tok in tokens) and (
            checkout_str in args or "chromium" in args or comm in ("gclient", "ninja", "siso")
        ):
            jobs.append(
                {
                    "pid": int(pid),
                    "ppid": int(ppid),
                    "pgid": int(pgid),
                    "state": stat,
                    "elapsed": etime,
                    "command": comm,
                    "args": args,
                }
            )
    return jobs


def inspect_checkout_state(checkout_parent: Path, work_root: Path) -> dict[str, object]:
    src_dir = checkout_parent / "src"
    out_dir = src_dir / "out" / "OmniPlayRelease"
    probe = checkout_parent
    while not probe.exists() and probe.parent != probe:
        probe = probe.parent
    usage = shutil.disk_usage(probe)
    root_usage = shutil.disk_usage(Path("/")) if platform.system() != "Windows" else usage
    return {
        "checkout_parent": str(checkout_parent),
        "work_root": str(work_root),
        "gclient_config_exists": (checkout_parent / ".gclient").is_file(),
        "src_exists": src_dir.is_dir(),
        "gn_args_exists": (out_dir / "args.gn").is_file(),
        "app_bundle_exists": (out_dir / "OmniPlay.app" / "Contents" / "MacOS" / "OmniPlay").is_file(),
        "work_volume_free_bytes": usage.free,
        "work_volume_free_gib": round(usage.free / (1024 ** 3), 2),
        "root_volume_free_bytes": root_usage.free,
        "root_volume_free_gib": round(root_usage.free / (1024 ** 3), 2),
        "active_jobs": detect_active_jobs(checkout_parent),
    }


def run_logged_stage(
    stage_name: str,
    argv: list[str],
    cwd: Path,
    env: dict[str, str],
    log_dir: Path,
) -> dict[str, object]:
    log_dir.mkdir(parents=True, exist_ok=True)
    log_path = log_dir / f"{stage_name}.log"
    caffeinate = shutil.which("caffeinate") if platform.system() == "Darwin" else None
    full_argv = [caffeinate, "-dimsu", *argv] if caffeinate else list(argv)
    started = utc_now()
    with log_path.open("w", encoding="utf-8") as log_file:
        log_file.write(f"# stage={stage_name} started_at={started}\n")
        log_file.write(f"# cwd={cwd}\n")
        log_file.write(f"# argv={json.dumps(full_argv)}\n")
        log_file.write(f"# TMPDIR={env.get('TMPDIR')}\n\n")
        log_file.flush()
        proc = subprocess.Popen(
            full_argv,
            cwd=str(cwd),
            env=env,
            stdout=log_file,
            stderr=subprocess.STDOUT,
            text=True,
            start_new_session=(os.name != "nt"),
        )
        returncode = proc.wait()
    finished = utc_now()
    return {
        "stage": stage_name,
        "argv": full_argv,
        "cwd": str(cwd),
        "pid": proc.pid,
        "returncode": returncode,
        "started_at_utc": started,
        "finished_at_utc": finished,
        "log_path": str(log_path),
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Supervise macOS ARM64 Chromium preparation, source staging, build, and acceptance on external build disk."
    )
    parser.add_argument(
        "--mode",
        choices=("inspect", "prepare", "stage", "build", "accept"),
        default="inspect",
        help="Execution phase to run.",
    )
    parser.add_argument(
        "--work-root",
        type=Path,
        default=Path("/Volumes/BuildDisk/omniplay-macos-005"),
        help="External disk root holding checkout, tmp, caches, and receipts.",
    )
    parser.add_argument(
        "--checkout-parent",
        type=Path,
        default=None,
        help="Parent directory containing .gclient and src/ (defaults to <work-root>/chromium).",
    )
    parser.add_argument(
        "--depot-tools",
        type=Path,
        default=None,
        help="Path to depot_tools directory.",
    )
    parser.add_argument(
        "--broker-binary",
        type=Path,
        default=None,
        help="Optional packaged macOS broker binary for final acceptance.",
    )
    parser.add_argument(
        "--jobs",
        type=int,
        default=None,
        help="Optional parallel job limit passed to build_desktop_browser.py.",
    )
    args = parser.parse_args(argv)

    work_root = args.work_root.resolve()
    checkout_parent = (args.checkout_parent or (work_root / "chromium")).resolve()
    prep_out = work_root / "preparation"
    staged_root = work_root / "staged"
    package_out = work_root / "package"
    log_dir = work_root / "logs"
    receipt_path = work_root / "macos-supervisor-receipt.json"

    depot_candidates = [
        p
        for p in (
            args.depot_tools,
            work_root / "depot_tools",
            work_root.parent / "depot_tools",
            Path.home() / "depot_tools",
        )
        if p is not None
    ]
    depot_tools = next((p.resolve() for p in depot_candidates if (p / "gclient").is_file()), None)
    concrete_python = resolve_macos_python3()
    sdk_info = inspect_macos_sdk()
    state = inspect_checkout_state(checkout_parent, work_root)
    lock_data = read_json(LOCK_PATH)

    payload: dict[str, object] = {
        "schema_version": 1,
        "item_id": 5,
        "target": "macos-aarch64",
        "mode": args.mode,
        "generated_at_utc": utc_now(),
        "pinned_release_tag": lock_data.get("release_tag"),
        "pinned_revision": lock_data.get("revision"),
        "concrete_python3": concrete_python,
        "depot_tools": str(depot_tools) if depot_tools else None,
        "sdk_info": sdk_info,
        "state": state,
        "stages": [],
    }

    if args.mode == "inspect":
        write_json(receipt_path, payload)
        print(json.dumps(payload, indent=2, sort_keys=True))
        return 0

    if state["active_jobs"]:
        payload["error"] = "Active macOS Chromium checkout/build process detected; refusing concurrent mutation."
        write_json(receipt_path, payload)
        print(json.dumps(payload, indent=2, sort_keys=True), file=sys.stderr)
        return 2

    env = build_macos_env(depot_tools, work_root, concrete_python)
    stages: list[dict[str, object]] = []

    if args.mode in ("prepare", "stage", "build", "accept"):
        prep_cmd = [
            concrete_python,
            str(REPO_ROOT / "tools" / "prepare_desktop_browser.py"),
            "--target",
            "macos-aarch64",
            "--output-dir",
            str(prep_out),
            "--checkout-root",
            str(checkout_parent / "src"),
        ]
        stage_res = run_logged_stage("01-prepare-desktop-browser", prep_cmd, REPO_ROOT, env, log_dir)
        stages.append(stage_res)
        if stage_res["returncode"] != 0:
            payload["stages"] = stages
            write_json(receipt_path, payload)
            return int(stage_res["returncode"])

    if args.mode in ("stage", "build", "accept"):
        build_cmd = [
            concrete_python,
            str(REPO_ROOT / "tools" / "build_desktop_browser.py"),
            "--target",
            "macos-aarch64",
            "--chromium-src",
            str(checkout_parent / "src"),
            "--work-dir",
            str(staged_root),
            "--output-dir",
            str(package_out),
        ]
        if args.broker_binary is not None:
            build_cmd.extend(["--broker-binary", str(args.broker_binary.resolve())])
        if args.jobs is not None:
            build_cmd.extend(["--jobs", str(args.jobs)])
        if args.mode in ("build", "accept"):
            build_cmd.append("--execute-build")
        stage_name = "02-build-desktop-browser" if args.mode in ("build", "accept") else "02-stage-desktop-browser"
        stage_res = run_logged_stage(stage_name, build_cmd, REPO_ROOT, env, log_dir)
        stages.append(stage_res)
        if stage_res["returncode"] != 0:
            payload["stages"] = stages
            write_json(receipt_path, payload)
            return int(stage_res["returncode"])

    if args.mode == "accept":
        if args.broker_binary is None:
            payload["error"] = "--broker-binary is required for --mode accept"
            payload["stages"] = stages
            write_json(receipt_path, payload)
            return 2
        accept_cmd = [
            concrete_python,
            str(REPO_ROOT / "qa" / "macos" / "accept-chromium-build.py"),
            "--chromium-src",
            str(checkout_parent / "src"),
            "--work-dir",
            str(staged_root),
            "--output-dir",
            str(package_out),
            "--broker-binary",
            str(args.broker_binary.resolve()),
        ]
        if args.jobs is not None:
            accept_cmd.extend(["--jobs", str(args.jobs)])
        stage_res = run_logged_stage("03-accept-chromium-build", accept_cmd, REPO_ROOT, env, log_dir)
        stages.append(stage_res)
        if stage_res["returncode"] != 0:
            payload["stages"] = stages
            write_json(receipt_path, payload)
            return int(stage_res["returncode"])

    payload["stages"] = stages
    payload["state_after"] = inspect_checkout_state(checkout_parent, work_root)
    write_json(receipt_path, payload)
    print(json.dumps(payload, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
