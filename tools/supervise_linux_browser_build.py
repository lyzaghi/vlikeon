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


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def read_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, payload: dict[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def build_process_env(depot_tools: Path | None, scratch_tmp: Path) -> dict[str, str]:
    env = dict(os.environ)
    path_entries = [ entry for entry in env.get("PATH", "").split(os.pathsep) if entry ]
    if depot_tools is not None:
        depot_str = str(depot_tools.resolve())
        if depot_str not in path_entries:
            path_entries.insert(0, depot_str)
    env["PATH"] = os.pathsep.join(path_entries)
    env["DEPOT_TOOLS_UPDATE"] = "0"
    env["DEPOT_TOOLS_METRICS"] = "0"
    env["CHROME_DEVEL_SANDBOX"] = ""
    scratch_tmp.mkdir(parents=True, exist_ok=True)
    env["TMPDIR"] = str(scratch_tmp.resolve())
    env["TMP"] = str(scratch_tmp.resolve())
    env["TEMP"] = str(scratch_tmp.resolve())
    # Process-local git LF and longpaths configuration without mutating host ~/.gitconfig
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
        ["ps", "-eo", "pid=,ppid=,pgid=,stat=,etimes=,comm=,args="],
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
        pid, ppid, pgid, stat, etimes, comm, args = parts
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
                    "elapsed_seconds": int(etimes) if etimes.isdigit() else 0,
                    "command": comm,
                    "args": args,
                }
            )
    return jobs


def inspect_checkout_state(checkout_parent: Path, work_root: Path) -> dict[str, object]:
    src_dir = checkout_parent / "src"
    git_dir = src_dir / ".git"
    index_lock = git_dir / "index.lock"
    out_dir = src_dir / "out" / "OmniPlayRelease"
    probe = checkout_parent
    while not probe.exists() and probe.parent != probe:
        probe = probe.parent
    usage = shutil.disk_usage(probe)
    return {
        "checkout_parent": str(checkout_parent),
        "work_root": str(work_root),
        "gclient_config_exists": (checkout_parent / ".gclient").is_file(),
        "src_exists": src_dir.is_dir(),
        "git_dir_exists": git_dir.is_dir(),
        "git_index_lock_exists": index_lock.exists(),
        "gn_args_exists": (out_dir / "args.gn").is_file(),
        "chrome_binary_exists": (out_dir / "chrome").is_file(),
        "free_bytes": usage.free,
        "free_gib": round(usage.free / (1024 ** 3), 2),
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
    started = utc_now()
    with log_path.open("w", encoding="utf-8") as log_file:
        log_file.write(f"# stage={stage_name} started_at={started}\n")
        log_file.write(f"# cwd={cwd}\n")
        log_file.write(f"# argv={json.dumps(argv)}\n\n")
        log_file.flush()
        proc = subprocess.Popen(
            argv,
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
        "argv": argv,
        "cwd": str(cwd),
        "pid": proc.pid,
        "returncode": returncode,
        "started_at_utc": started,
        "finished_at_utc": finished,
        "log_path": str(log_path),
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Supervise Linux x86_64 Chromium preparation, source staging, build, and acceptance without colliding with active jobs."
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
        default=Path.home() / "omniplay-linux-browser-build",
        help="Root directory for preparation outputs, logs, and staged receipts.",
    )
    parser.add_argument(
        "--checkout-parent",
        type=Path,
        default=None,
        help="Parent directory holding .gclient and src/ (defaults to <work-root>/chromium).",
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
        help="Optional packaged Linux broker binary for final acceptance.",
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
    scratch_tmp = work_root / "tmp"
    receipt_path = work_root / "linux-supervisor-receipt.json"

    depot_candidates = [
        p
        for p in (
            args.depot_tools,
            work_root.parent / "depot_tools",
            work_root / "depot_tools",
            Path.home() / "depot_tools",
        )
        if p is not None
    ]
    depot_tools = next((p.resolve() for p in depot_candidates if (p / "gclient").is_file()), None)
    state = inspect_checkout_state(checkout_parent, work_root)
    lock_data = read_json(LOCK_PATH)

    payload: dict[str, object] = {
        "schema_version": 1,
        "item_id": 4,
        "target": "linux-x86_64",
        "mode": args.mode,
        "generated_at_utc": utc_now(),
        "pinned_release_tag": lock_data.get("release_tag"),
        "pinned_revision": lock_data.get("revision"),
        "depot_tools": str(depot_tools) if depot_tools else None,
        "state": state,
        "stages": [],
    }

    if args.mode == "inspect":
        write_json(receipt_path, payload)
        print(json.dumps(payload, indent=2, sort_keys=True))
        return 0

    if state["active_jobs"]:
        payload["error"] = "Active Chromium checkout/build process detected; refusing concurrent mutation."
        write_json(receipt_path, payload)
        print(json.dumps(payload, indent=2, sort_keys=True), file=sys.stderr)
        return 2

    env = build_process_env(depot_tools, scratch_tmp)
    stages: list[dict[str, object]] = []
    python_exe = sys.executable

    if args.mode in ("prepare", "stage", "build", "accept"):
        prep_cmd = [
            python_exe,
            str(REPO_ROOT / "tools" / "prepare_desktop_browser.py"),
            "--target",
            "linux-x86_64",
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
            python_exe,
            str(REPO_ROOT / "tools" / "build_desktop_browser.py"),
            "--target",
            "linux-x86_64",
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
            python_exe,
            str(REPO_ROOT / "qa" / "linux" / "accept-chromium-build.py"),
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
