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
FIELDS = {"id", "team_identifier", "actor", "actor_identifier", "presentation",
          "presentation_identifier", "core", "core_identifier"}


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
    if not isinstance(value, dict) or set(value) != {"schema_version", "runtimes"} or value["schema_version"] != 1:
        raise ValueError("The macOS runtime identity root is invalid")
    runtimes = value["runtimes"]
    if not isinstance(runtimes, list) or not 1 <= len(runtimes) <= 32:
        raise ValueError("The macOS runtime identity count is invalid")
    ids = set()
    signing = set()
    for entry in runtimes:
        if not isinstance(entry, dict) or set(entry) != FIELDS:
            raise ValueError("A macOS runtime identity has missing or unknown fields")
        if any(not isinstance(entry[key], str) for key in FIELDS):
            raise ValueError("Every macOS runtime identity value must be text")
        if not RUNTIME.fullmatch(entry["id"]) or entry["id"] in ids:
            raise ValueError("A macOS runtime ID is invalid or duplicated")
        ids.add(entry["id"])
        if not TEAM.fullmatch(entry["team_identifier"]):
            raise ValueError("A macOS runtime team identity is invalid")
        for key in ("actor", "presentation", "core"):
            name = entry[key]
            if not BASENAME.fullmatch(name) or name.startswith(".") or name.endswith("."):
                raise ValueError("A macOS runtime filename is invalid")
        if len({entry["actor"], entry["presentation"], entry["core"]}) != 3:
            raise ValueError("macOS runtime files must be distinct")
        for key in ("actor_identifier", "presentation_identifier", "core_identifier"):
            identity = entry[key]
            if not IDENTIFIER.fullmatch(identity) or identity in signing:
                raise ValueError("A macOS code-signing identity is invalid or duplicated")
            signing.add(identity)
    value = {"schema_version": 1, "runtimes": sorted(runtimes, key=lambda item: item["id"])}
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
