from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys


REPO_ROOT = Path(__file__).resolve().parents[2]
if str(REPO_ROOT / "tools") not in sys.path:
    sys.path.insert(0, str(REPO_ROOT / "tools"))

import windows_actor_imports  # noqa: E402


WINDOWS_LIFECYCLE_GATES: tuple[tuple[str, str, str], ...] = (
    (
        "creation_time_job_list_association",
        "platform/sandbox/src/windows.rs",
        "PROC_THREAD_ATTRIBUTE_JOB_LIST",
    ),
    (
        "pre_resume_job_membership_check",
        "platform/sandbox/src/windows.rs",
        "IsProcessInJob",
    ),
    (
        "suspended_actor_token_verification",
        "platform/sandbox/src/windows/storage.rs",
        "verify_actor_token",
    ),
    (
        "private_registry_sealing_before_resume",
        "platform/sandbox/src/windows/registry_storage.rs",
        "pub(super) fn seal(",
    ),
    (
        "default_filesystem_no_write_up_sealing",
        "platform/sandbox/src/windows/storage.rs",
        "seal_default_files",
    ),
    (
        "win32k_syscall_lockdown_for_presented_actor",
        "platform/sandbox/src/windows.rs",
        "if presentation.is_some(){mitigations|=1<<28;}",
    ),
    (
        "write_quota_bounds_for_core_helper_cache_profile",
        "platform/sandbox/src/windows/write_quota.rs",
        "pub fn enforce_work_write_quotas(",
    ),
    (
        "legacy_unowned_session_quarantine",
        "platform/native-runtime/src/work.rs",
        "pub fn quarantine_unowned_windows_work(",
    ),
    (
        "fixed_16_slot_ipc_worker_retirement",
        "platform/native-runtime/src/control_workers.rs",
        "const LIMIT:usize=16;",
    ),
    (
        "presentation_helper_isolated_job_and_pipes",
        "platform/sandbox/src/windows/presentation.rs",
        "--stdio-presentation-v2",
    ),
)

IGNORED_WINDOWS_ENV_VARS: tuple[str, ...] = (
    "OMNIPLAY_WINDOWS_STORAGE_PROBE",
    "OMNIPLAY_WINDOWS_READONLY_STORAGE_PROBE",
    "OMNIPLAY_WINDOWS_LOADER_PROBE",
    "OMNIPLAY_WINDOWS_PRESENTATION_FIXTURE",
    "OMNIPLAY_WINDOWS_DEVICE_FREE_WORKER",
    "OMNIPLAY_WINDOWS_DEVICE_FREE_CORE",
)


def resolve_cargo() -> str:
    env_cargo = os.environ.get("CARGO")
    if env_cargo and Path(env_cargo).is_file():
        return str(Path(env_cargo).resolve())
    found = shutil.which("cargo")
    if found:
        return found
    local_rustup = REPO_ROOT.parent / ".rustup-local" / "toolchains" / "stable-x86_64-pc-windows-msvc" / "bin" / "cargo.exe"
    if local_rustup.is_file():
        return str(local_rustup)
    return "cargo"


def build_acceptance_queue(
    cargo_bin: str,
    python_bin: str,
    lifecycle_worker: Path,
    lifecycle_core: Path,
    lifecycle_void_core: Path,
    run_root: Path,
) -> list[dict[str, object]]:
    queue: list[dict[str, object]] = [
        {
            "id": "sandbox_windows_unit_and_storage_suite",
            "command": [cargo_bin, "test", "--locked", "--offline", "-j", "2", "-p", "omniplay-sandbox", "--", "--nocapture"],
            "covers_items": [8, 10, 11],
        },
        {
            "id": "native_runtime_windows_work_and_ipc_suite",
            "command": [cargo_bin, "test", "--locked", "--offline", "-j", "2", "-p", "omniplay-native-runtime", "--", "--nocapture"],
            "covers_items": [8, 9, 11],
        },
        {
            "id": "broker_windows_recovery_jobs_suite",
            "command": [cargo_bin, "test", "--locked", "--offline", "-j", "2", "-p", "omniplay-broker", "native_work_jobs::tests", "--", "--nocapture"],
            "covers_items": [9, 11],
        },
    ]
    if all(os.environ.get(var) for var in (
        "OMNIPLAY_WINDOWS_STORAGE_PROBE",
        "OMNIPLAY_WINDOWS_READONLY_STORAGE_PROBE",
        "OMNIPLAY_WINDOWS_PRESENTATION_FIXTURE",
        "OMNIPLAY_WINDOWS_DEVICE_FREE_WORKER",
        "OMNIPLAY_WINDOWS_DEVICE_FREE_CORE",
    )):
        queue.append(
            {
                "id": "sandbox_windows_ignored_os_probes_suite",
                "command": [
                    cargo_bin,
                    "test",
                    "--locked",
                    "--offline",
                    "-j",
                    "2",
                    "-p",
                    "omniplay-sandbox",
                    "--",
                    "--ignored",
                    "--skip",
                    "actual_actor_reports_staged_system_dll_initialization",
                    "--nocapture",
                ],
                "covers_items": [8, 10, 11, 105],
            }
        )
    if lifecycle_worker.is_file() and lifecycle_core.is_file():
        queue.append(
            {
                "id": "native_control_wire_cases",
                "command": [
                    python_bin,
                    "qa/native-lifecycle/wire_cases.py",
                    "--worker",
                    str(lifecycle_worker.resolve()),
                    "--core",
                    str(lifecycle_core.resolve()),
                    "--output",
                    str((run_root / "wire-cases").resolve()),
                ],
                "covers_items": [10, 11],
            }
        )
        queue.append(
            {
                "id": "native_temporary_directory_cases",
                "command": [
                    python_bin,
                    "qa/native-lifecycle/temporary_directory_cases.py",
                    "--worker",
                    str(lifecycle_worker.resolve()),
                    "--core",
                    str(lifecycle_core.resolve()),
                    "--output",
                    str((run_root / "temp-dir-cases").resolve()),
                ],
                "covers_items": [8, 11],
            }
        )
    if lifecycle_worker.is_file() and lifecycle_core.is_file() and lifecycle_void_core.is_file():
        queue.append(
            {
                "id": "native_lifecycle_run_cases",
                "command": [
                    python_bin,
                    "qa/native-lifecycle/run_cases.py",
                    "--worker",
                    str(lifecycle_worker.resolve()),
                    "--core",
                    str(lifecycle_core.resolve()),
                    "--void-core",
                    str(lifecycle_void_core.resolve()),
                    "--output",
                    str((run_root / "lifecycle-run-cases").resolve()),
                ],
                "covers_items": [10, 11],
            }
        )
    return queue


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify_source_gates() -> list[dict[str, object]]:
    results: list[dict[str, object]] = []
    for gate_id, rel_path, anchor in WINDOWS_LIFECYCLE_GATES:
        target = REPO_ROOT / rel_path
        exists = target.is_file()
        content = target.read_text(encoding="utf-8") if exists else ""
        passed = exists and (anchor in content)
        results.append(
            {
                "id": gate_id,
                "path": rel_path,
                "anchor": anchor,
                "passed": passed,
                "sha256": sha256_file(target) if exists else None,
            }
        )
    return results


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Verify Windows AppContainer, Job Object, write quota, legacy recovery, isolated presentation, and IPC lifecycle."
    )
    parser.add_argument(
        "--execute",
        action="store_true",
        help="Execute the Cargo and Python runtime acceptance suites (leave off when execution is deferred).",
    )
    parser.add_argument(
        "--worker-binary",
        type=Path,
        default=None,
        help="Optional compiled omniplay-native-worker.exe to verify PE import isolation.",
    )
    parser.add_argument(
        "--core-dll",
        type=Path,
        default=None,
        help="Optional compiled core DLL to verify PE import isolation.",
    )
    parser.add_argument(
        "--helper-binary",
        type=Path,
        default=None,
        help="Optional compiled omniplay-presentation-host.exe to verify PE import isolation.",
    )
    parser.add_argument(
        "--lifecycle-worker",
        type=Path,
        default=REPO_ROOT / "out" / "qualification" / "worker-direct" / "omniplay-native-worker.exe",
        help="Direct qualification worker binary for wire_cases.py / run_cases.py / temporary_directory_cases.py.",
    )
    parser.add_argument(
        "--lifecycle-core",
        type=Path,
        default=REPO_ROOT / "out" / "qualification" / "lifecycle-cores" / "omniplay-lifecycle-fixture.dll",
        help="Qualification lifecycle fixture core DLL.",
    )
    parser.add_argument(
        "--lifecycle-void-core",
        type=Path,
        default=REPO_ROOT / "out" / "qualification" / "lifecycle-cores" / "omniplay-lifecycle-void-fixture.dll",
        help="Qualification void-only lifecycle fixture core DLL.",
    )
    parser.add_argument(
        "--run-output-dir",
        type=Path,
        default=None,
        help="Fresh directory root for child qualification suite evidence.",
    )
    parser.add_argument(
        "--output-receipt",
        type=Path,
        default=None,
        help="Optional path to write windows-lifecycle-qualification-receipt.json.",
    )
    args = parser.parse_args(argv)

    gates = verify_source_gates()
    all_gates_passed = all(bool(g["passed"]) for g in gates)

    pe_audit = None
    if args.worker_binary and args.core_dll:
        pe_audit = windows_actor_imports.verify_windows_isolated_deployment(
            args.worker_binary.resolve(),
            args.core_dll.resolve(),
            args.helper_binary.resolve() if args.helper_binary else None,
        )

    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    run_root = (
        args.run_output_dir.resolve()
        if args.run_output_dir is not None
        else (REPO_ROOT / "out" / "qualification" / "lifecycle-runs" / stamp).resolve()
    )
    acceptance_queue = build_acceptance_queue(
        cargo_bin=resolve_cargo(),
        python_bin=sys.executable,
        lifecycle_worker=args.lifecycle_worker,
        lifecycle_core=args.lifecycle_core,
        lifecycle_void_core=args.lifecycle_void_core,
        run_root=run_root,
    )

    executed_suites: list[dict[str, object]] = []
    if args.execute:
        run_root.mkdir(parents=True, exist_ok=True)
        for entry in acceptance_queue:
            cmd = [str(x) for x in entry["command"]]
            completed = subprocess.run(
                cmd,
                cwd=str(REPO_ROOT),
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                check=False,
            )
            suite_log = run_root / f"{entry['id']}.log"
            suite_log.write_text(completed.stdout, encoding="utf-8")
            executed_suites.append(
                {
                    "id": entry["id"],
                    "command": cmd,
                    "returncode": completed.returncode,
                    "log_path": str(suite_log),
                    "output_tail": completed.stdout[-4000:],
                }
            )

    receipt: dict[str, object] = {
        "schema_version": 1,
        "items": [8, 9, 10, 11, 105],
        "generated_at_utc": utc_now(),
        "all_source_gates_passed": all_gates_passed,
        "source_gates": gates,
        "pe_isolation_audit": pe_audit,
        "execution_mode": "executed" if args.execute else "deferred_queue_ready",
        "acceptance_queue": acceptance_queue,
        "executed_suites": executed_suites,
    }

    if args.output_receipt is not None:
        args.output_receipt.parent.mkdir(parents=True, exist_ok=True)
        args.output_receipt.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    print(json.dumps(receipt, indent=2, sort_keys=True))
    if not all_gates_passed:
        return 1
    if args.execute and any(int(s["returncode"]) != 0 for s in executed_suites):
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
