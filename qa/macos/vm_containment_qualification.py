#!/usr/bin/env python3
"""macOS VM boot/game/video/audio/save (Item 20) and crash/lost-owner/reboot/resource-abuse (Item 21) acceptance queue.

Default mode verifies source inventory and emits the queue as SOURCE_READY_UNVERIFIED.
--execute requires Apple Silicon macOS 13+, a signed broker app and an operator-supplied
case driver; each case result must be a driver exit status. Nothing is ever marked
passed from source inspection.
"""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
SOURCES = [
    "engines/macos-owner/src/virtual_machine.mm", "engines/macos-owner/src/boot.mm", "engines/macos-owner/src/session.mm",
    "engines/macos-owner/src/process.mm", "engines/macos-guest/init.c", "engines/macos-guest/kernel.config",
    "platform/native-runtime/src/mac_work.rs", "platform/native-runtime/src/mac_guest.rs", "platform/native-runtime/src/mac_package.rs",
    "platform/native-runtime/src/mac_session.rs", "platform/sandbox/src/macos.rs",
    "tools/build_macos_guest_image.py", "tools/finalize_macos_catalog.py", "docs/MACOS_BOOT_RECOVERY.md",
]
CASES = [
    (20, "vm-boot-hello", "Signed broker launches VZ guest; hello matches runtime_id/library/protocol"),
    (20, "vm-game-run", "Representative game runs for >= 10 minutes with stable frame pacing"),
    (20, "vm-video-presentation", "OMPRES02 video reaches the helper window; resize/fullscreen/focus fence behave"),
    (20, "vm-audio-presentation", "Audio is audible, bounded, cleared on pause/focus loss, no underrun storm"),
    (20, "vm-save-restore", "Save RAM/state/in-game export, stop, relaunch, load round-trips byte-exact"),
    (20, "vm-no-network-no-write", "Guest has no network/writable host share/devices; scratch tmpfs bound enforced"),
    (21, "owner-crash", "Kill XPC owner mid-game: VM and helper end, slots/debt retained until confirmation"),
    (21, "broker-crash-restart", "Kill broker mid-session: restart recovers via previous-boot journal, no stale PID signal"),
    (21, "host-reboot", "Reboot Mac mid-session: previous-boot recovery releases debt, work dir removed"),
    (21, "guest-memory-abuse", "Guest allocates past RAM/scratch: guest dies, host stays within declared budget"),
    (21, "guest-fork-cpu-abuse", "Guest fork bomb/CPU spin cannot starve owner or exceed vCPU count"),
    (21, "startup-cancel", "Cancel during stage/launch leaves no live VM and no orphaned staged directory"),
    (21, "legacy-data-retention", "Schema-1/lease-less legacy data is retained, never deleted or adopted"),
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--driver", type=Path, help="Executable invoked as: driver <case-id>")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    inventory = {}
    for name in SOURCES:
        path = ROOT / name
        if not path.is_file():
            raise FileNotFoundError(name)
        inventory[name] = hashlib.sha256(path.read_bytes()).hexdigest()
    report = {"schema_version": 1, "suite": "macos-vm-qualification", "items": [20, 21], "source_inventory_sha256": inventory,
              "cases": [{"item": i, "id": c, "requirement": d, "result": "queued"} for i, c, d in CASES],
              "status": "SOURCE_READY_UNVERIFIED"}
    if args.execute:
        if platform.system() != "Darwin" or platform.machine() != "arm64" or not args.driver or not args.driver.is_file():
            raise RuntimeError("Execution requires Apple Silicon macOS and an explicit case driver")
        for entry in report["cases"]:
            code = subprocess.run([str(args.driver), entry["id"]], cwd=ROOT).returncode
            entry["result"] = "passed" if code == 0 else f"failed:{code}"
        report["status"] = "VERIFIED" if all(c["result"] == "passed" for c in report["cases"]) else "FAILED"
    text = json.dumps(report, indent=2) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(text, encoding="utf-8")
    else:
        sys.stdout.write(text)
    return 0 if report["status"] != "FAILED" else 1


if __name__ == "__main__":
    raise SystemExit(main())
