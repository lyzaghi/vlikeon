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
GAME_HUB_PATCH_PATH = REPO_ROOT / "browser" / "patches" / "0001-omniplay-game-hub.patch"
OWNED_CAPTURE_PATCH_PATH = REPO_ROOT / "browser" / "patches" / "0003-owned-game-capture.patch"
SNAPSHOT_ROOT = REPO_ROOT / "browser" / "upstream-snapshots"

SECURITY_INVARIANTS: tuple[tuple[str, str, str], ...] = (
    (
        "component_loader_off_the_record_guard",
        "chrome/browser/extensions/component_loader.cc",
        "if (!skip_session_components && !profile_->IsOffTheRecord()) {",
    ),
    (
        "component_extension_allowlist",
        "chrome/browser/extensions/component_extensions_allowlist/allowlist.cc",
        "case IDR_OMNIPLAY_MANIFEST:",
    ),
    (
        "isolated_linux_user_data_dir",
        "chrome/common/chrome_paths_linux.cc",
        'std::string data_dir_basename = "omniplay";',
    ),
    (
        "isolated_mac_product_dir",
        "chrome/common/chrome_paths_mac.mm",
        'product_dir_name = "OmniPlay";',
    ),
    (
        "isolated_windows_install_dir",
        "chrome/install_static/chromium_install_modes.h",
        'inline constexpr wchar_t kProductPathName[] = L"OmniPlay";',
    ),
    (
        "isolated_windows_native_messaging_registry",
        "chrome/browser/extensions/api/messaging/launch_context_win.cc",
        'L"SOFTWARE\\\\OmniPlay\\\\NativeMessagingHosts"',
    ),
    (
        "strict_window_audio_policy_enforcement",
        "chrome/browser/extensions/api/desktop_capture/desktop_capture_api.cc",
        "const bool strict_window_audio = params->options &&",
    ),
    (
        "owned_game_pid_and_creation_time_verification",
        "chrome/browser/extensions/api/desktop_capture/desktop_capture_base.cc",
        "WaitForSingleObject(process.get(), 0) != WAIT_TIMEOUT",
    ),
    (
        "owned_game_capture_schema_contract",
        "chrome/common/extensions/api/desktop_capture.json",
        '"omniplayOwnedCaptureProtocol": {',
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
        entries = review_manifest.get("integration_sources") or review_manifest.get("files") or []
        for entry in entries:
            if not isinstance(entry, dict):
                continue
            rel = str(entry["path"])
            cand_rel = entry.get("candidate_Copy") or f"source/candidate/{rel}"
            baseline_rel = entry.get("baseline_copy") or f"source/baseline/{rel}"
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

        sources: dict[str, str] = {}
        for rel in copied_files:
            target_file = scratch_tree / rel
            if target_file.is_file():
                sources[rel] = target_file.read_text(encoding="utf-8")

        stage_results: list[dict[str, object]] = []

        try:
            # Stage 1: Verify pinned desktop capture audio anchors against candidate tree
            touched_audio = desktop_capture_audio_patch.author(scratch_tree)
            stage_results.append(
                {
                    "stage": "desktop_capture_audio_patch.author",
                    "status": "APPLIED",
                    "touched_files": sorted(touched_audio.keys()),
                }
            )

            # Stage 2: Apply 0001-omniplay-game-hub.patch with zero fuzz
            patch_0001_text = GAME_HUB_PATCH_PATH.read_text(encoding="utf-8")
            touched_0001 = source_patch.apply_exact(sources, patch_0001_text)
            sources.update(touched_0001)
            for rel, updated in touched_0001.items():
                (scratch_tree / rel).write_text(updated, encoding="utf-8", newline="\n")
            stage_results.append(
                {
                    "stage": "0001-omniplay-game-hub.patch",
                    "status": "APPLIED",
                    "touched_files": sorted(touched_0001.keys()),
                }
            )

            # Stage 3: Apply 0003-owned-game-capture.patch on top of Stage 2 with zero fuzz
            patch_0003_text = OWNED_CAPTURE_PATCH_PATH.read_text(encoding="utf-8")
            touched_0003 = source_patch.apply_exact(sources, patch_0003_text)
            sources.update(touched_0003)
            for rel, updated in touched_0003.items():
                (scratch_tree / rel).write_text(updated, encoding="utf-8", newline="\n")
            stage_results.append(
                {
                    "stage": "0003-owned-game-capture.patch",
                    "status": "APPLIED",
                    "touched_files": sorted(touched_0003.keys()),
                }
            )
        except (ValueError, SystemExit) as exc:
            raise RuntimeError(f"Chromium upgrade patch rehearsal failed: {exc}") from exc

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
        "baseline_release_tag": lock_data.get("version") or lock_data.get("release_tag"),
        "baseline_revision": lock_data.get("revision"),
        "candidate_source": str(source_root.resolve()),
        "series_count": len(series_data.get("patches", series_data.get("series", []))),
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
