"""Apply authored integration files to the exact upstream checkout without tests."""
from pathlib import Path
import argparse
import hashlib
import json
import shutil
import subprocess
from game_hub_assets import prepare

ROOT=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument("--android",action="store_true",help="Stage the independently authored Android source module and patch")
arguments=parser.parse_args()
CHECKOUT=ROOT/"browser/chromium"
STAGED=ROOT/("out/browser-android-integration" if arguments.android else "out/browser-integration")
SNAPSHOTS=ROOT/("browser/android-snapshots" if arguments.android else "browser/upstream-snapshots")
LOCK=json.loads((ROOT/"browser/upstream.lock.json").read_text(encoding="utf-8"))
receipt_path=ROOT/("browser/android-integration-receipt.json" if arguments.android else "browser/integration-receipt.json")
receipt=json.loads(receipt_path.read_text(encoding="utf-8")) if receipt_path.is_file() else {}
published={}
revision=subprocess.check_output(["git","-C",str(CHECKOUT),"rev-parse","HEAD"],text=True).strip()
if revision!=LOCK["revision"]:
    raise SystemExit("Refusing to edit a different Chromium revision")
series_path=ROOT/("browser/patches/android-series.json" if arguments.android else "browser/patches/series.json")
series=json.loads(series_path.read_text(encoding="utf-8"))
for relative in series.get("modified_files",[
    "chrome/browser/extensions/component_loader.cc",
    "chrome/browser/extensions/component_extensions_allowlist/allowlist.cc",
    "chrome/browser/resources/component_extension_resources.grd",
    "chrome/app/theme/chromium/BRANDING",
]):
    original=(SNAPSHOTS/relative).read_bytes()
    source=STAGED/relative
    destination=CHECKOUT/relative
    if not destination.exists():
        # The exact original blob was obtained and identity-checked by the
        # source downloader. Sparse checkout need not hydrate unrelated trees.
        destination.parent.mkdir(parents=True,exist_ok=True)
        destination.write_bytes(original)
    # Read-only source identity guards prevent overwriting a user's local edits.
    normalized=destination.read_bytes().replace(b"\r\n",b"\n")
    if normalized not in [original.replace(b"\r\n",b"\n"),source.read_bytes().replace(b"\r\n",b"\n")] and hashlib.sha256(normalized).hexdigest()!=receipt.get(relative):
        raise SystemExit(f"The target contains unrelated local edits: {relative}")
    shutil.copy2(source,destination)
    published[relative]=hashlib.sha256(source.read_bytes().replace(b"\r\n",b"\n")).hexdigest()
relative_resources="chrome/android/omniplay" if arguments.android else "chrome/browser/resources/omniplay_game_hub"
resources=STAGED/relative_resources
# Retire only obsolete generated resources after the confined-tree guard.
# A removed privileged module must not survive in a previously staged checkout.
expected={path.relative_to(resources).as_posix() for path in resources.rglob('*') if path.is_file()}
prepare(CHECKOUT/relative_resources,expected)
shutil.copytree(resources,CHECKOUT/relative_resources,dirs_exist_ok=True)
LOCK["checkout_state"]="sparse_integration_authored"
(ROOT/"browser/upstream.lock.json").write_text(json.dumps(LOCK,indent=2)+"\n",encoding="utf-8")
receipt_path.write_text(json.dumps(published,indent=2)+"\n",encoding="utf-8")
print("Applied authored OmniPlay integration source to pinned Chromium checkout.")
