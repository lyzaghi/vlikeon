from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import shutil
import sys
import tempfile


REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT / "tools") not in sys.path:
    sys.path.insert(0, str(REPO_ROOT / "tools"))

import desktop_capture_audio_patch  # noqa: E402
import generate_browser_patch  # noqa: E402
import source_patch  # noqa: E402


LOCK_PATH = REPO_ROOT / "browser" / "upstream.lock.json"
SERIES_PATH = REPO_ROOT / "browser" / "patches" / "series.json"
OWNED_CAPTURE_PATCH_PATH = REPO_ROOT / "browser" / "patches" / "0003-owned-game-capture.patch"
SNAPSHOT_ROOT = REPO_ROOT / "browser" / "upstream-snapshots"

SECURITY_INVARIANTS: tuple[tuple[str, str, str], ...] = (
    (
        "webui_game_hub_host",
        "chrome/common/webui_url_constants.h",
        'inline constexpr char kChromeUIGameHubHost[] = "game-hub";',
    ),
    (
        "component_extension_allowlist",
        "chrome/browser/extensions/component_loader.cc",
        "AddOmniPlayGameHubExtension",
    ),
    (
        "isolated_linux_user_data_dir",
        "chrome/common/chrome_paths_linux.cc",
        '"omniplay"',
    ),
    (
        "isolated_mac_product_dir",
        "chrome/common/chrome_paths_mac.mm",
        '"OmniPlay"',
    ),
    (
        "isolated_windows_install_dir",
        "chrome/installer/util/util_constants.cc",
        'L"OmniPlay"',
    ),
    (
        "private_api_off_the_record_rejection",
        "chrome/browser/extensions/api/omniplay_private/omniplay_private_api.cc",
        "browser_context()->IsOffTheRecord()",
    ),
    (
        "private_api_component_id_enforcement",
        "chrome/browser/extensions/api/omniplay_private/omniplay_private_api.cc",
        "extension()->id() != omniplay::kGameHubExtensionId",
    ),
    (
        "owned_game_pid_and_creation_time_verification",
        "chrome/browser/media/webrtc/omniplay_owned_game_registry.cc",
        "VerifyOwnedProcess",
    ),
    (
        "native_capture_picker_bypass_restricted_to_game_hub",
        "chrome/browser/extensions/api/desktop_capture/desktop_capture_base.cc",
        "extension()->id() == omniplay::kGameHubExtensionId",
    ),
)


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def copy_candidate_inputs(candidate_source: Path, scratch_tree: Path, review_manifest: dict[str, object] | None) -> list[str]:
    copied: list[str] = []
    if review_manifest is not None:
        for entry in review_manifest.get("files", []):
            if not isinstance(entry, dict):
                continue
            rel = str(entry["path"])
            cand_rel = entry.get("candidate_Copy")
            baseline_rel = entry.get("baseline_copy")
            source_file = None
            if isinstance(cand_rel, str):
                candidate_path = candidate_source / cand_rel
                if candidate_path.is_file():
                    source_file = candidate_path
            if source_file is None and isinstance(baseline_rel, str):
                baseline_path = candidate_source / baseline_rel
                if baseline_path.is_file():
                    source_file = baseline_path
            if source_file is not None:
                dest = scratch_tree / rel
                dest.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(source_file, dest)
                copied.append(rel)
        return copied

    for file_path in sorted(candidate_source.rglob("*")):
        if not file_path.is_file():
            continue
        rel = file_path.relative_to(candidate_source).as_posix()
        dest = scratch_tree / rel
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(file_path, dest)
        copied.append(rel)
    return copied


def rehearse_upgrade_on_tree(candidate_source: Path, review_manifest: dict[str, object] | None = None) -> dict[str, object]:
    with tempfile.TemporaryDirectory(prefix="omniplay-upgrade-rehearsal-") as tmp:
        scratch_tree = Path(tmp) / "src"
        scratch_tree.mkdir(parents=True, exist_ok=True)
        copied_files = copy_candidate_inputs(candidate_source, scratch_tree, review_manifest)

        stage_results: list[dict[str, object]] = []

        # Stage 1: Core anchor series (0001 + 0002 + overlays)
        generate_browser_patch.apply_series(scratch_tree)
        stage_results.append(
            {
                "stage": "generate_browser_patch.apply_series",
                "status": "APPLIED",
            }
        )

        # Stage 2: Owned-game capture patch (0003)
        patch_text = OWNED_CAPTURE_PATCH_PATH.read_text(encoding="utf-8")
        touched_0003 = source_patch.apply_exact(scratch_tree, patch_text)
        stage_results.append(
            {
                "stage": "0003-owned-game-capture.patch",
                "status": "APPLIED",
                "touched_files": touched_0003,
            }
        )

        # Stage 3: Desktop capture audio & WebUI plumbing patch
        touched_audio = desktop_capture_audio_patch.apply_to_tree(scratch_tree)
        stage_results.append(
            {
                "stage": "desktop_capture_audio_patch.apply_to_tree",
                "status": "APPLIED",
                "touched_files": touched_audio,
            }
        )

        # Stage 4: Verify all security invariants
        invariant_checks: list[dict[str, object]] = []
        all_invariants_ok = True
        for check_id, rel_path, needle in SECURITY_INVARIANTS:
            target_file = scratch_tree / rel_path
            exists = target_file.is_file()
            text = target_file.read_text(encoding="utf-8") if exists else ""
            passed = exists and (needle in text)
            if not passed:
                all_invariants_ok = False
            invariant_checks.append(
                {
                    "id": check_id,
                    "path": rel_path,
                    "expected_anchor": needle,
                    "passed": passed,
                    "patched_sha256": sha256_file(target_file) if exists else None,
                }
            )

        patched_digests: dict[str, str] = {}
        for file_path in sorted(scratch_tree.rglob("*")):
            if file_path.is_file():
                rel = file_path.relative_to(scratch_tree).as_posix()
                patched_digests[rel] = sha256_file(file_path)

        return {
            "copied_input_count": len(copied_files),
            "stages": stage_results,
            "all_security_invariants_passed": all_invariants_ok,
            "security_invariants": invariant_checks,
            "patched_file_count": len(patched_digests),
            "patched_digests": patched_digests,
        }


def verify_upgrade(
    *,
    candidate_root: Path | None = None,
    upgrade_review_dir: Path | None = None,
    output_receipt: Path | None = None,
) -> dict[str, object]:
    lock_data = json.loads(LOCK_PATH.read_text(encoding="utf-8"))
    series_data = json.loads(SERIES_PATH.read_text(encoding="utf-8"))

    review_manifest = None
    source_root = candidate_root or SNAPSHOT_ROOT
    if upgrade_review_dir is not None:
        manifest_path = upgrade_review_dir / "upgrade-review.json"
        review_manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        source_root = upgrade_review_dir

    rehearsal = rehearse_upgrade_on_tree(source_root.resolve(), review_manifest)
    receipt: dict[str, object] = {
        "schema_version": 1,
        "item_id": 7,
        "generated_at_utc": utc_now(),
        "baseline_release_tag": lock_data.get("release_tag"),
        "baseline_revision": lock_data.get("revision"),
        "candidate_source": str(source_root.resolve()),
        "series_count": len(series_data.get("series", [])),
        "rehearsal": rehearsal,
        "verified": bool(rehearsal["all_security_invariants_passed"]),
    }

    if output_receipt is not None:
        output_receipt.parent.mkdir(parents=True, exist_ok=True)
        output_receipt.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return receipt


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Rehearse and verify Vlikeon's Chromium patch series and security invariants against an upstream candidate."
    )
    parser.add_argument(
        "--candidate-root",
        type=Path,
        default=None,
        help="Path to candidate Chromium source or upstream snapshot directory (defaults to browser/upstream-snapshots).",
    )
    parser.add_argument(
        "--upgrade-review-dir",
        type=Path,
        default=None,
        help="Optional directory produced by tools/prepare_chromium_upgrade.py containing upgrade-review.json.",
    )
    parser.add_argument(
        "--output-receipt",
        type=Path,
        default=None,
        help="Optional path to write upgrade-verification-receipt.json.",
    )
    args = parser.parse_args(argv)

    receipt = verify_upgrade(
        candidate_root=args.candidate_root,
        upgrade_review_dir=args.upgrade_review_dir,
        output_receipt=args.output_receipt,
    )
    print(json.dumps(receipt, indent=2, sort_keys=True))
    return 0 if receipt["verified"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
