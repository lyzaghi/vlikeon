"""Read-only release gate for the product core matrix (Items 22-34).

Derives the matrix from the actual recipes (build_web_core / build_native_runtime),
the pinned source tree, the provenance review and the release readiness record.
It executes nothing and never marks a core accepted: it reports exact blockers and
the qualification queue each core still owes.
"""
import argparse
import ast
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PERMISSIVE_OR_COPYLEFT_OK = {"MPL-2.0", "GPL-2.0-only", "GPL-2.0-or-later", "GPL-3.0-only", "GPL-3.0-or-later", "ISC AND MIT", "ISC", "MIT"}
# Ledger item -> (system, candidate recipe keys)
ITEMS = {22: ("gb", ["sameboy"]), 23: ("gba", ["mgba"]), 24: ("nes", ["fceumm"]), 25: ("snes", ["bsnes"]),
         26: ("nds", ["melonds-ds"]), 27: ("mega_drive", ["ares-md"]), 28: ("n64", ["mupen64plus-gl", "mupen64plus-next"]),
         29: ("ps1", ["beetle-psx"]), 30: ("psp", ["ppsspp", "ppsspp-gl"])}
N64_SAFETY = {31: "snapshot admission and device-resource safety (docs/N64_SNAPSHOT_FORMAT.md, N64_DEVICE_MEMORY.md)",
              32: "RAM/RTC/mempak/disk writers and writer revocation (docs/N64_MEMORY_IO.md, N64_RTC_BATTERY.md)",
              33: "memory bounds, GPU resources and graphics command errors (docs/N64_OPENGL_RUNTIME.md)",
              34: "AI FIFO, interrupt schedule, resampler and audio lifecycle (docs/N64_RUNTIME.md)"}
CASES = ["boots_representative_content", "save_ram_roundtrip", "save_state_roundtrip", "pause_resume_input_fence",
         "wrong_core_state_rejected", "resource_limits_enforced", "audio_video_pacing", "cancel_and_crash_cleanup"]


def literal_cores(path):
    tree = ast.parse(path.read_text(encoding="utf-8"))
    for node in tree.body:
        if isinstance(node, ast.Assign) and any(getattr(t, "id", None) == "CORES" for t in node.targets):
            return ast.literal_eval(node.value)
    return {}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    web = literal_cores(ROOT / "tools/build_web_core.py")
    native = literal_cores(ROOT / "tools/build_native_runtime.py")
    sources = {p.name for p in (ROOT / "third_party/sources").iterdir() if p.is_dir()}
    web_scripts = {p.stem for p in (ROOT / "tools").glob("build_*_web.py")}
    review = json.loads((ROOT / "security/retro-provenance-review.json").read_text(encoding="utf-8"))
    restricted = {entry["product_candidate"] for entry in review["entries"] if entry.get("distribution_decision") == "research_only_not_release_eligible"}
    rows = []
    for item, (system, keys) in ITEMS.items():
        candidates = []
        for key in keys:
            spec = native.get(key) or web.get(key) or {}
            license_expression = spec.get("license", "")
            blockers = []
            if not spec:
                blockers.append("no build recipe declares this core")
            if spec.get("research_only"):
                blockers.append("research_only: not release eligible (see security/retro-provenance-review.json)")
            if license_expression and license_expression not in PERMISSIVE_OR_COPYLEFT_OK:
                blockers.append(f"license expression requires review: {license_expression}")
            source = spec.get("source_name", key.replace("-gl", ""))
            if source not in sources and key not in {"mupen64plus-gl"}:
                blockers.append(f"pinned source tree missing: third_party/sources/{source}")
            candidates.append({"core": key, "native_recipe": key in native, "web_recipe": key in web or any(key.split('-')[0] in s for s in web_scripts),
                               "license": license_expression, "revision": spec.get("revision"), "blockers": blockers,
                               "restricted_by_review": spec.get("id") in restricted})
        eligible = [c for c in candidates if not c["blockers"] and not c["restricted_by_review"]]
        rows.append({"item": item, "system": system, "candidates": candidates,
                     "product_candidate": eligible[0]["core"] if eligible else None,
                     "owed_cases": CASES, "status": "QUEUED" if eligible else "BLOCKED"})
    rows += [{"item": item, "scope": text, "owed_cases": ["fault_injection", "bounded_resource_denial", "writer_revocation_on_stop", "legacy_save_compatibility"],
              "status": "QUEUED"} for item, text in N64_SAFETY.items()]
    report = {"schema_version": 1, "suite": "core-release-gate", "executes_nothing": True, "rows": rows,
              "status": "SOURCE_READY_UNVERIFIED"}
    text = json.dumps(report, indent=2) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(text, encoding="utf-8")
    else:
        print(text)


if __name__ == "__main__":
    main()
