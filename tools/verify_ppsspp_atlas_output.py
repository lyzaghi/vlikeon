"""Inspect an actual generated PSP open-font atlas (Item 35). Read-only; runs no generator.

Admits the build receipt through ppsspp_atlas_receipt, then checks the real output
bytes: bounded sizes, non-degenerate content, retained OFL/FreeType notices and the
source-lock font pins. A pass here is necessary evidence, not glyph-layout approval.
"""
import argparse
import hashlib
import json
from pathlib import Path

from ppsspp_atlas_receipt import admit_generated

ROOT = Path(__file__).resolve().parents[1]
REQUIRED_NOTICE_WORDS = ("SIL OPEN FONT LICENSE", "FreeType")


def entropy_ok(raw):
    window = raw[: 4 * 1024 * 1024]
    return len(set(window)) >= 16 and window.count(0) < len(window) * 0.999


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--receipt", required=True, type=Path, help="atlas build receipt JSON from the generated project")
    parser.add_argument("--core-revision", required=True)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    lock = json.loads((ROOT / "security/ppsspp-atlas-source.lock.json").read_text(encoding="utf-8"))
    fonts = lock.get("font_sources", lock)
    receipt = admit_generated(args.receipt.resolve(strict=True), args.core_revision, receipt_fonts(args.receipt, fonts))
    source = Path(receipt["corresponding_source"])
    checks = {}
    for item in receipt["assets"]:
        raw = (source / item["name"]).read_bytes()
        checks[item["name"]] = {"bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest(), "non_degenerate": entropy_ok(raw)}
    notices = "\n".join(p.read_text(encoding="utf-8", errors="replace") for p in source.rglob("*")
                        if p.is_file() and ("LICENSE" in p.name.upper() or "OFL" in p.name.upper() or "NOTICE" in p.name.upper()))
    notice_ok = all(word.lower() in notices.lower() for word in REQUIRED_NOTICE_WORDS)
    ok = all(c["non_degenerate"] for c in checks.values()) and notice_ok
    report = {"schema_version": 1, "assets": checks, "notices_retained": notice_ok,
              "status": "OUTPUT_CHECKS_PASSED_LAYOUT_REVIEW_PENDING" if ok else "FAILED"}
    text = json.dumps(report, indent=2) + "\n"
    if args.output:
        args.output.write_text(text, encoding="utf-8")
    else:
        print(text)
    return 0 if ok else 1


def receipt_fonts(path, fallback):
    return json.loads(path.read_text(encoding="utf-8")).get("font_sources", fallback)


if __name__ == "__main__":
    raise SystemExit(main())
