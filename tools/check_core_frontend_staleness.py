"""Detect native core/frontend packages built against stale input/presentation sources (Item 36).

Compares every built package's recorded compiled_first_party_sources (written by
build_native_runtime.py / build_linux_native_runtime.py) with the current engines/
sources, and checks the frontend protocol constants the broker requires. It only
reads; it prints the ordered rebuild commands the operator must run later.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
BROKER_REQUIRES = {"presentation_protocol": 2, "input_profile_protocol": 4}


def current_hashes(prefixes):
    result = {}
    for base in prefixes:
        for path in sorted((ROOT / base).rglob("*")):
            if path.is_file() and not path.is_symlink():
                result[path.relative_to(ROOT).as_posix()] = hashlib.sha256(path.read_bytes()).hexdigest()
    return result


def source_protocols():
    wire = (ROOT / "engines/presentation/src/wire.hpp").read_text(encoding="utf-8")
    session = (ROOT / "platform/native-runtime/src/linux_session.rs").read_text(encoding="utf-8")
    found = {}
    match = re.search(r"kProtocol\s*=\s*(\d+)", wire)
    found["wire_kProtocol"] = int(match.group(1)) if match else None
    for key in BROKER_REQUIRES:
        match = re.search(rf'"{key}"\]\.as_u64\(\)!=Some\((\d+)\)', session)
        found[key] = int(match.group(1)) if match else None
    return found


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    protocols = source_protocols()
    drift = {k: (protocols[k], v) for k, v in BROKER_REQUIRES.items() if protocols.get(k) != v}
    packages = []
    for receipt in sorted((ROOT / "out").glob("*native-workers/*/corresponding-source.json")):
        value = json.loads(receipt.read_text(encoding="utf-8"))
        recorded = value.get("compiled_first_party_sources", {})
        bases = sorted({"/".join(p.split("/")[:2]) for p in recorded if p.startswith("engines/")}) or ["engines/native", "engines/presentation"]
        now = current_hashes(bases)
        stale = sorted(p for p, h in recorded.items() if p.startswith("engines/") and now.get(p) != h)
        added = sorted(p for p in now if p not in recorded)
        packages.append({"package": receipt.parent.name, "platform": value.get("platform", "windows-x86_64"),
                         "presentation_protocol": value.get("presentation_protocol"),
                         "stale_files": stale, "new_files": added, "needs_rebuild": bool(stale or added or value.get("presentation_protocol") != 2)})
    ordered = [p["package"] for p in packages if p["needs_rebuild"]]
    report = {"schema_version": 1, "source_protocols": protocols, "protocol_drift": drift, "packages": packages,
              "rebuild_order": ordered, "commands": [f"python tools/{'build_linux_native_runtime.py' if p['platform']=='linux-x86_64' else 'build_native_runtime.py'} <core> --core-receipt <receipt> ..." for p in packages if p["needs_rebuild"]],
              "status": "SOURCE_READY_UNVERIFIED", "note": "No package was rebuilt by this tool."}
    text = json.dumps(report, indent=2) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(text, encoding="utf-8")
    else:
        print(text)


if __name__ == "__main__":
    main()
