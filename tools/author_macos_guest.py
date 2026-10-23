"""Author a deterministic diskless ARM64 guest; never build or execute inputs.

The signed release must include the kernel's corresponding source, build config,
license and any required core/dependency source offers. Inputs are explicit;
there is no host-library discovery, archive extraction or dependency download.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import stat
import struct

MAX_IMAGE = 256 * 1024 * 1024
MAX_FILES = 512


def ordinary(path, maximum):
    path = path.absolute()
    for parent in (path, *path.parents):
        if parent.is_symlink() or getattr(parent, "is_junction", lambda: False)():
            raise ValueError("Guest inputs cannot contain links")
    shape = path.stat()
    if not stat.S_ISREG(shape.st_mode) or shape.st_nlink != 1 or not 0 < shape.st_size <= maximum:
        raise ValueError("Guest inputs must be bounded ordinary single-link files")
    return path.read_bytes()


def elf(raw, kind, static_init=False):
    if len(raw) < 64 or raw[:6] != b"\x7fELF\x02\x01" or struct.unpack_from("<H", raw, 18)[0] != 183:
        raise ValueError("Guest code must be little-endian ARM64 ELF")
    if struct.unpack_from("<H", raw, 16)[0] not in kind:
        raise ValueError("Guest ELF has an unexpected type")
    offset = struct.unpack_from("<Q", raw, 32)[0]
    size, count = struct.unpack_from("<HH", raw, 54)
    if size != 56 or count > 1024 or offset + size * count > len(raw):
        raise ValueError("Guest ELF program headers are invalid")
    for index in range(count):
        header = offset + size * index
        ptype, flags = struct.unpack_from("<II", raw, header)
        if ptype == 0x6474E551 and flags & 1:
            raise ValueError("Executable guest stacks are refused")
        if static_init and ptype == 3:
            raise ValueError("PID 1 must be statically linked")


def name(value):
    p = PurePosixPath(value)
    if not value.isascii() or len(value) > 200 or p.is_absolute() or any(
            part in (".", "..") or not part or not all(c.isalnum() or c in "_+.-" for c in part)
            for part in value.split("/")):
        raise ValueError("Invalid guest archive path")
    return value


def record(output, path, mode, data, inode):
    encoded = path.encode("ascii") + b"\0"
    values = (inode, mode, 0, 0, 1, 0, len(data), 0, 0, 0, 0, len(encoded), 0)
    output.write(b"070701" + b"".join(f"{value:08x}".encode() for value in values))
    output.write(encoded)
    output.write(b"\0" * (-(110 + len(encoded)) % 4))
    output.write(data)
    output.write(b"\0" * (-len(data) % 4))
    if output.tell() > MAX_IMAGE:
        raise ValueError("Guest initramfs exceeds its bound")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package-root", required=True, type=Path)
    parser.add_argument("--catalog", required=True, type=Path)
    parser.add_argument("--runtime", required=True)
    parser.add_argument("--init", required=True, type=Path)
    parser.add_argument("--kernel", required=True, type=Path)
    parser.add_argument("--dependencies", required=True, type=Path,
                        help="JSON array of path/source/sha256/bytes for explicit /lib guest dependencies")
    parser.add_argument("--kernel-provenance", required=True, type=Path,
                        help="JSON source_sha256/config_sha256/source_url/source_commit/license")
    parser.add_argument("--output", required=True, type=Path, help="New output directory outside package inputs")
    args = parser.parse_args()
    base = args.package_root.resolve(strict=True)
    catalog = json.loads(ordinary(args.catalog, 1024 * 1024))
    selected = [r for r in catalog["runtimes"] if r["manifest"]["id"] == args.runtime]
    if len(selected) != 1:
        raise ValueError("Select exactly one runtime")
    runtime = selected[0]
    prefix = f"native-runtimes/{args.runtime}/"
    files = {}
    binding = {}
    worker, core = runtime["worker"], runtime["core_library"]
    helper = runtime["presentation"]["executable"]
    mac = runtime["macos"]
    excluded = {helper, mac["guest_kernel"], mac["guest_initrd"]}
    for entry in runtime["files"]:
        path = entry["path"]
        name(path)
        if not path.startswith(prefix):
            raise ValueError("Runtime inputs must stay inside their inventory prefix")
        if path in excluded:
            continue
        raw = ordinary(base / path, MAX_IMAGE)
        digest = hashlib.sha256(raw).hexdigest()
        if len(raw) != entry["bytes"] or digest != entry["sha256"]:
            raise ValueError("Guest input differs from its declared package bytes")
        relative = name(path[len(prefix):])
        target = "opt/vlike/runtime/" + ("omniplay-native-worker" if path == worker else relative)
        if target in files:
            raise ValueError("Duplicate guest destination")
        if path == worker:
            elf(raw, (2, 3))
        elif path == core:
            elf(raw, (3,))
        files[target] = (0o100755 if path == worker else 0o100644, raw)
        binding[path] = {"sha256": digest, "bytes": len(raw)}
    if worker not in binding or core not in binding or "/" in core[len(prefix):]:
        raise ValueError("A guest needs its exact worker and adjacent core")
    dependencies = json.loads(ordinary(args.dependencies, 1024 * 1024))
    if not isinstance(dependencies, list) or len(dependencies) > 128:
        raise ValueError("Guest dependencies must be explicit and bounded")
    for entry in dependencies:
        if set(entry) != {"path", "source", "sha256", "bytes"}:
            raise ValueError("Invalid dependency identity")
        target = name(entry["path"])
        if not target.startswith("lib/") or target in files:
            raise ValueError("Dependencies must be unique files below /lib")
        raw = ordinary(Path(entry["source"]), 64 * 1024 * 1024)
        if len(raw) != entry["bytes"] or hashlib.sha256(raw).hexdigest() != entry["sha256"]:
            raise ValueError("Dependency bytes do not match")
        elf(raw, (3,))
        files[target] = (0o100755, raw)
    init = ordinary(args.init, 16 * 1024 * 1024)
    elf(init, (2, 3), static_init=True)
    files["init"] = (0o100755, init)
    kernel = ordinary(args.kernel, 128 * 1024 * 1024)
    if len(kernel) < 64 or kernel[56:60] != b"ARM\x64":
        raise ValueError("An uncompressed ARM64 Linux Image is required")
    provenance = json.loads(ordinary(args.kernel_provenance, 65536))
    if set(provenance) != {"source_sha256", "config_sha256", "source_url", "source_commit", "license"}:
        raise ValueError("Explicit kernel source and configuration provenance is required")
    for key in ("source_sha256", "config_sha256"):
        if len(provenance[key]) != 64 or any(c not in "0123456789abcdef" for c in provenance[key]):
            raise ValueError("Invalid kernel provenance hash")
    if provenance["license"] != "GPL-2.0-only" or not provenance["source_url"].startswith("https://") or not provenance["source_commit"]:
        raise ValueError("Kernel provenance is incomplete")
    receipt = {"schema_version": 1, "guest_protocol": 1, "runtime": args.runtime,
               "runtime_files": binding, "kernel_provenance": provenance,
               "kernel_sha256": hashlib.sha256(kernel).hexdigest(),
               "init_sha256": hashlib.sha256(init).hexdigest(),
               "dependencies": [{k: v for k, v in entry.items() if k != "source"} for entry in dependencies]}
    files["vlike-image.json"] = (0o100644, (json.dumps(receipt, sort_keys=True, separators=(",", ":")) + "\n").encode())
    directories = {"proc", "sys", "dev", "inputs", "scratch", "work", "tmp", "lib"}
    for path in files:
        directories.update(str(p) for p in PurePosixPath(path).parents if str(p) != ".")
    if len(files) + len(directories) > MAX_FILES or sum(len(raw) for _, raw in files.values()) > MAX_IMAGE - 1024 * 1024:
        raise ValueError("Guest archive inventory exceeds its bound")
    if directories.intersection(files):
        raise ValueError("Guest file/directory collision")
    output = args.output.absolute()
    if output.resolve().is_relative_to(base) or output.exists():
        raise ValueError("Choose a new output directory outside package inputs")
    output.mkdir(parents=True, exist_ok=False)
    image = output / "guest-initrd.cpio"
    with image.open("xb") as target:
        for index, directory in enumerate(sorted(directories, key=lambda p: (p.count("/"), p)), 1):
            record(target, directory, 0o040755, b"", index)
        for index, (path, (mode, raw)) in enumerate(sorted(files.items()), len(directories) + 1):
            record(target, path, mode, raw, index)
        record(target, "TRAILER!!!", 0, b"", len(files) + len(directories) + 1)
        target.write(b"\0" * (-target.tell() % 512))
        target.flush()
        os.fsync(target.fileno())
    (output / "guest-kernel").write_bytes(kernel)
    receipt.update({"initrd_sha256": hashlib.sha256(image.read_bytes()).hexdigest(),
                    "initrd_bytes": image.stat().st_size, "kernel_bytes": len(kernel)})
    (output / "guest-receipt.json").write_text(json.dumps(receipt, sort_keys=True, indent=2) + "\n", encoding="utf-8")
    print("Authored guest bytes and provenance. No build, execution or test was performed.")


if __name__ == "__main__":
    main()
