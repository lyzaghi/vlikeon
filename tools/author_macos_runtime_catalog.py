"""Author the closed runtime catalog embedded in the signed macOS XPC owner."""
import argparse
import json
import os
from pathlib import Path
import re
import stat
import tempfile

RUNTIME = re.compile(r"[A-Za-z0-9_-]{1,64}\Z")
BASENAME = re.compile(r"[A-Za-z0-9_+.-]{1,128}\Z")
IDENTIFIER = re.compile(r"[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+){2,}\Z")
TEAM = re.compile(r"[A-Z0-9]{10}\Z")
FIELDS = {"id", "team_identifier", "presentation", "presentation_identifier",
          "presentation_memory_mb", "kernel", "kernel_sha256", "kernel_bytes",
          "initrd", "initrd_sha256", "initrd_bytes", "guest_memory_mb",
          "guest_cpu_count", "scratch_mb"}
NUMERIC = {"presentation_memory_mb", "kernel_bytes", "initrd_bytes",
           "guest_memory_mb", "guest_cpu_count", "scratch_mb"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--identity", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    supplied_shape = args.identity.lstat()
    source = args.identity.resolve(strict=True)
    shape = source.lstat()
    if not source.is_file() or stat.S_ISLNK(supplied_shape.st_mode) or shape.st_nlink != 1 or shape.st_size > 1024 * 1024:
        raise ValueError("The macOS runtime identity must be a bounded ordinary file")
    value = json.loads(source.read_text(encoding="utf-8"))
    if not isinstance(value, dict) or set(value) != {"schema_version", "runtimes"} or value["schema_version"] != 2:
        raise ValueError("The macOS runtime identity root is invalid")
    runtimes = value["runtimes"]
    if not isinstance(runtimes, list) or not 1 <= len(runtimes) <= 32:
        raise ValueError("The macOS runtime identity count is invalid")
    ids = set()
    signing = set()
    for entry in runtimes:
        if not isinstance(entry, dict) or set(entry) != FIELDS:
            raise ValueError("A macOS runtime identity has missing or unknown fields")
        if any(not isinstance(entry[key], str) for key in FIELDS - NUMERIC):
            raise ValueError("Every macOS runtime identity value must be text")
        bounds = {"guest_memory_mb": (256, 8192), "presentation_memory_mb": (256, 8192),
                  "guest_cpu_count": (1, 4), "scratch_mb": (16, 512),
                  "kernel_bytes": (64, 128 * 1024**2), "initrd_bytes": (64, 256 * 1024**2)}
        for key, (minimum, maximum) in bounds.items():
            if type(entry[key]) is not int or not minimum <= entry[key] <= maximum:
                raise ValueError("A macOS guest resource declaration exceeds its bound")
        if (entry["scratch_mb"] > entry["guest_memory_mb"] // 2
                or entry["initrd_bytes"] > entry["guest_memory_mb"] * 1024**2 // 2):
            raise ValueError("Guest writable RAM and image must fit their declared RAM budget")
        for key in ("kernel_sha256", "initrd_sha256"):
            if not re.fullmatch(r"[0-9a-f]{64}", entry[key]):
                raise ValueError("Invalid sealed guest image hash")
        if not RUNTIME.fullmatch(entry["id"]) or entry["id"] in ids:
            raise ValueError("A macOS runtime ID is invalid or duplicated")
        ids.add(entry["id"])
        if not TEAM.fullmatch(entry["team_identifier"]):
            raise ValueError("A macOS runtime team identity is invalid")
        for key in ("kernel", "presentation", "initrd"):
            name = entry[key]
            if not BASENAME.fullmatch(name) or name.startswith(".") or name.endswith("."):
                raise ValueError("A macOS runtime filename is invalid")
        if len({entry["kernel"], entry["presentation"], entry["initrd"]}) != 3:
            raise ValueError("macOS runtime files must be distinct")
        for key in ("presentation_identifier",):
            identity = entry[key]
            if not IDENTIFIER.fullmatch(identity) or identity in signing:
                raise ValueError("A macOS code-signing identity is invalid or duplicated")
            signing.add(identity)
    value = {"schema_version": 2, "runtimes": sorted(runtimes, key=lambda item: item["id"])}
    raw = (json.dumps(value, ensure_ascii=True, sort_keys=True, separators=(",", ":")) + "\n").encode()
    if len(raw) > 1024 * 1024:
        raise ValueError("The macOS runtime catalog exceeds its bound")
    output = args.output.resolve()
    if output.exists():
        raise ValueError("Choose a new macOS runtime catalog output")
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=output.parent, prefix=".omniplay-macos-catalog-", delete=False) as target:
            temporary = Path(target.name)
            os.fchmod(target.fileno(), 0o600)
            target.write(raw)
            target.flush()
            os.fsync(target.fileno())
        os.link(temporary, output)
    finally:
        if temporary:
            temporary.unlink(missing_ok=True)
    print("Authored the closed macOS runtime catalog. No code, helper or test was executed.")


if __name__ == "__main__":
    main()
