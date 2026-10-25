"""Finalize the macOS native catalog and owner identity from actual artifacts (Item 19).

Replaces the provisional guest kernel/initrd inventory entries with the exact bytes
produced by build_macos_guest_image.py, binds entitlement hashes from the real
entitlement files, verifies every inventory file on disk, and emits both the native
catalog and the schema-2 owner identity consumed by author_macos_runtime_catalog.py.
It never signs, runs, or invents a hash/identity; inconsistencies are hard errors.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import stat

ROOT = Path(__file__).resolve().parents[1]
HEX = re.compile(r"[0-9a-f]{64}\Z")


def ordinary(path, maximum=1024**3):
    path = path.absolute()
    for parent in (path, *path.parents):
        if parent.is_symlink():
            raise ValueError("Artifact ancestry cannot contain links")
    shape = path.stat()
    if not stat.S_ISREG(shape.st_mode) or shape.st_nlink != 1 or not 0 < shape.st_size <= maximum:
        raise ValueError(f"{path.name} must be a bounded ordinary single-link file")
    return path.read_bytes()


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package-root", required=True, type=Path, help="Release inventory root containing native-runtimes/")
    parser.add_argument("--provisional-catalog", required=True, type=Path)
    parser.add_argument("--runtime", required=True)
    parser.add_argument("--guest-output", required=True, type=Path, help="Output of build_macos_guest_image.py")
    parser.add_argument("--owner-entitlements", required=True, type=Path)
    parser.add_argument("--presentation-entitlements", required=True, type=Path)
    parser.add_argument("--owner-identity-output", required=True, type=Path)
    parser.add_argument("--catalog-output", required=True, type=Path)
    args = parser.parse_args()
    base = args.package_root.resolve(strict=True)
    guest = args.guest_output.resolve(strict=True)
    catalog = json.loads(ordinary(args.provisional_catalog, 1024 * 1024))
    matches = [r for r in catalog["runtimes"] if r["manifest"]["id"] == args.runtime]
    if len(matches) != 1:
        raise ValueError("Select exactly one runtime")
    runtime, mac = matches[0], matches[0]["macos"]
    build = json.loads(ordinary(guest / "build-recipe-receipt.json", 65536))
    kernel = ordinary(guest / "guest-kernel", 128 * 1024**2)
    initrd = ordinary(guest / "guest-initrd.cpio", 256 * 1024**2)
    if digest(kernel) != build["kernel_sha256"] or digest(initrd) != build["initrd_sha256"] or build["runtime"] != args.runtime:
        raise ValueError("Guest artifacts differ from their build receipt")
    by_path = {entry["path"]: entry for entry in runtime["files"]}
    for path, raw in ((mac["guest_kernel"], kernel), (mac["guest_initrd"], initrd)):
        if path not in by_path:
            raise ValueError("The provisional catalog does not declare a guest boot image")
        by_path[path].update({"sha256": digest(raw), "bytes": len(raw)})
        target = base / path
        target.parent.mkdir(parents=True, exist_ok=True)
        if target.exists() and ordinary(target) != raw:
            raise ValueError("A different boot image already occupies the package path")
        if not target.exists():
            target.write_bytes(raw)
    mac["owner_entitlements_sha256"] = digest(ordinary(args.owner_entitlements, 65536))
    mac["presentation_entitlements_sha256"] = digest(ordinary(args.presentation_entitlements, 65536))
    for entry in runtime["files"]:
        raw = ordinary(base / entry["path"], 4 * 1024**3)
        if len(raw) != entry["bytes"] or digest(raw) != entry["sha256"] or not HEX.fullmatch(entry["sha256"]):
            raise ValueError(f"{entry['path']} differs from its final inventory entry")
    manifest = runtime["manifest"]
    manifest["executable_sha256"] = by_path[runtime["worker"]]["sha256"]
    helper = runtime["presentation"]
    identity = {"id": args.runtime, "team_identifier": mac["team_identifier"], "presentation": Path(helper["executable"]).name,
                "presentation_identifier": mac["presentation_signing_identifier"],
                "presentation_memory_mb": helper["memory_limit_mb"],
                "kernel": Path(mac["guest_kernel"]).name, "kernel_sha256": by_path[mac["guest_kernel"]]["sha256"],
                "kernel_bytes": by_path[mac["guest_kernel"]]["bytes"],
                "initrd": Path(mac["guest_initrd"]).name, "initrd_sha256": by_path[mac["guest_initrd"]]["sha256"],
                "initrd_bytes": by_path[mac["guest_initrd"]]["bytes"], "guest_memory_mb": runtime["memory_limit_mb"],
                "guest_cpu_count": mac["guest_cpu_count"], "scratch_mb": mac["scratch_memory_mb"]}
    for output in (args.catalog_output.resolve(), args.owner_identity_output.resolve()):
        if output.exists():
            raise ValueError("Choose new catalog output paths")
    args.catalog_output.parent.mkdir(parents=True, exist_ok=True)
    args.owner_identity_output.parent.mkdir(parents=True, exist_ok=True)
    args.catalog_output.write_text(json.dumps(catalog, indent=2) + "\n", encoding="utf-8")
    args.owner_identity_output.write_text(json.dumps({"schema_version": 2, "runtimes": [identity]}, indent=2) + "\n", encoding="utf-8")
    print("Final macOS catalog and owner identity authored from exact artifacts. Signing/notarization not performed.")


if __name__ == "__main__":
    main()
