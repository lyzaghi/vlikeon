from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
import re
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT / "tools") not in sys.path:
    sys.path.insert(0, str(REPO_ROOT / "tools"))

import desktop_capture_audio_patch  # noqa: E402
import generate_browser_patch  # noqa: E402
from prepare_chromium_upgrade import MAX_FILE, REPOSITORY, blob, git_bytes, version
from prepare_desktop_browser import ordinary, relative  # noqa: E402
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


def required_integration_paths() -> list[str]:
    series_data = json.loads(SERIES_PATH.read_text(encoding="utf-8"))
    paths: set[str] = set()
    for item in series_data.get("modified_files", []):
        relative(str(item))
        paths.add(str(item))
    for snap in SNAPSHOT_ROOT.rglob("*"):
        ordinary(snap, SNAPSHOT_ROOT, False)
        if snap.is_file():
            rel = snap.relative_to(SNAPSHOT_ROOT).as_posix()
            relative(rel)
            paths.add(rel)
    if not paths:
        raise RuntimeError("Chromium integration snapshot inventory is empty")
    return sorted(paths)


def validate_safe_relative_path(raw_path: object, context: str) -> str:
    try:
        rel = relative(raw_path).as_posix()
    except ValueError as exc:
        raise RuntimeError(f"Invalid or escaping path in {context}: {raw_path!r} ({exc})") from exc
    return rel


def validate_safe_file_within(file_path: Path, root_dir: Path, context: str) -> Path:
    try:
        return ordinary(file_path, root_dir, file=True)
    except ValueError as exc:
        raise RuntimeError(f"Unsafe or missing file in {context}: {file_path} ({exc})") from exc


def candidate_git_inputs(
    manifest: dict[str, object], review_dir: Path, checkout: Path, revision: str, lock: dict[str, object],
) -> tuple[str, dict[str, tuple[bytes, str]]]:
    if not re.fullmatch(r"[0-9a-f]{40}", revision) or revision == lock["revision"]:
        raise RuntimeError("Select an explicit candidate revision different from the baseline")
    if manifest.get("candidate_revision") != revision or manifest.get("baseline_revision") != lock["revision"]:
        raise RuntimeError("Upgrade review candidate or baseline revision does not match the selected revisions")
    git = shutil.which("git")
    if git is None:
        raise RuntimeError("Git is required to verify an upgrade review")
    env = {**os.environ, "GIT_NO_LAZY_FETCH": "1", "GIT_NO_REPLACE_OBJECTS": "1", "GIT_OPTIONAL_LOCKS": "0"}
    try:
        remote = git_bytes(git, checkout, env, "remote", "get-url", "origin").decode("utf-8").strip()
        commit = git_bytes(git, checkout, env, "rev-parse", "--verify", revision + "^{commit}").decode("ascii").strip()
        if remote != REPOSITORY or commit != revision:
            raise ValueError("Use local objects for the selected official Chromium commit")
        raw_version, version_blob = blob(git, checkout, env, revision, "chrome/VERSION")
        candidate_version = version(raw_version)
        baseline_version = tuple(int(part) for part in str(lock["version"]).split("."))
        tag = ".".join(map(str, candidate_version))
        if candidate_version <= baseline_version or manifest.get("candidate_version") != tag:
            raise ValueError("Candidate VERSION must match the manifest and be newer than the baseline")
        version_file = validate_safe_file_within(review_dir / "candidate-VERSION", review_dir, "candidate VERSION")
        if manifest.get("candidate_version_blob") != version_blob or version_file.read_bytes() != raw_version:
            raise ValueError("Candidate VERSION does not match the selected Chromium Git blob")
        inputs = {name: blob(git, checkout, env, revision, name) for name in required_integration_paths()}
    except (ValueError, OSError, subprocess.SubprocessError) as exc:
        raise RuntimeError(f"Cannot establish Chromium candidate Git provenance: {exc}") from exc
    return tag, inputs


def copy_candidate_inputs(
    candidate_source: Path,
    scratch_tree: Path,
    review_manifest: dict[str, object] | None,
    git_inputs: dict[str, tuple[bytes, str]] | None = None,
) -> list[str]:
    required_paths = required_integration_paths()
    copied: list[str] = []

    if review_manifest is not None:
        if git_inputs is None:
            raise RuntimeError("An upgrade review requires trusted candidate Git objects")
        raw_sources = review_manifest.get("integration_sources")
        raw_inventory = review_manifest.get("source_inventory")
        if not isinstance(raw_sources, list) or not isinstance(raw_inventory, list):
            raise RuntimeError("Upgrade review requires integration_sources and source_inventory lists")

        inventory_by_path: dict[str, dict[str, object]] = {}
        for item in raw_inventory:
            if not isinstance(item, dict):
                raise RuntimeError("Malformed entry in upgrade-review.json source_inventory")
            rel = validate_safe_relative_path(item.get("path"), "source_inventory")
            if rel in inventory_by_path:
                raise RuntimeError(f"Duplicate source_inventory entry: {rel}")
            inventory_by_path[rel] = item

        source_by_path: dict[str, dict[str, object]] = {}
        for item in raw_sources:
            if not isinstance(item, dict):
                raise RuntimeError("Malformed entry in upgrade-review.json integration_sources")
            rel = validate_safe_relative_path(item.get("path"), "integration_sources")
            if rel in source_by_path:
                raise RuntimeError(f"Duplicate integration_sources entry: {rel}")
            source_by_path[rel] = item
        if set(source_by_path) != set(required_paths):
            raise RuntimeError("Upgrade review must list exactly the required candidate integration paths")

        for rel in required_paths:
            candidate_rel = f"source/candidate/{rel}"
            entry = source_by_path[rel]
            if "candidate_copy" in entry:
                declared = validate_safe_relative_path(entry["candidate_copy"], "candidate_copy")
                if declared != candidate_rel:
                    raise RuntimeError(f"Candidate file must use its exact integration path: {candidate_rel}")
            item = inventory_by_path.get(candidate_rel, {})
            expected_size = item.get("bytes")
            expected_sha = item.get("sha256")
            if type(expected_size) is not int or not 0 <= expected_size <= MAX_FILE:
                raise RuntimeError(f"Missing or invalid candidate byte length for {candidate_rel}")
            if not isinstance(expected_sha, str) or not re.fullmatch(r"[0-9a-f]{64}", expected_sha):
                raise RuntimeError(f"Missing or invalid SHA-256 for {candidate_rel}")

            candidate_file = validate_safe_file_within(candidate_source / candidate_rel, candidate_source, "candidate input")
            with candidate_file.open("rb") as stream:
                raw = stream.read(expected_size + 1)
            if len(raw) != expected_size:
                raise RuntimeError(f"Candidate byte length mismatch for {candidate_rel}")
            if sha256_bytes(raw) != expected_sha:
                raise RuntimeError(f"Candidate SHA-256 mismatch for {candidate_rel}")
            upstream, identity = git_inputs[rel]
            if entry.get("candidate_blob") != identity or raw != upstream:
                raise RuntimeError(f"Candidate file does not match the selected Chromium Git blob: {candidate_rel}")
            dest = scratch_tree / rel
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(raw)
            copied.append(rel)
        return copied

    # Direct candidate_root mode (no fallback to baseline snapshots)
    if not candidate_source.is_dir():
        raise RuntimeError(f"Candidate root directory does not exist: {candidate_source}")

    manifest_digests: dict[str, str] = {}
    optional_manifest = candidate_source / "candidate-manifest.json"
    if optional_manifest.is_file():
        manifest_data = json.loads(optional_manifest.read_text(encoding="utf-8"))
        files_map = manifest_data.get("files")
        if not isinstance(files_map, dict) or not files_map:
            raise RuntimeError("candidate-manifest.json must contain a non-empty 'files' mapping")
        for raw_rel, expected_sha in files_map.items():
            rel = validate_safe_relative_path(raw_rel, "candidate-manifest.json")
            if not isinstance(expected_sha, str) or len(expected_sha) != 64:
                raise RuntimeError(f"Invalid SHA-256 digest in candidate-manifest.json for {rel}")
            manifest_digests[rel] = expected_sha

    for dirpath, dirnames, filenames in os.walk(candidate_source, followlinks=False):
        curr_dir = Path(dirpath)
        for d in dirnames:
            sub = curr_dir / d
            if sub.is_symlink() or getattr(sub, "is_junction", lambda: False)():
                raise RuntimeError(f"Candidate tree cannot contain symlink or junction directory: {sub}")
        for f in filenames:
            fp = curr_dir / f
            if fp.is_symlink():
                raise RuntimeError(f"Candidate tree cannot contain symlink file: {fp}")

    missing_required: list[str] = []
    for rel in required_paths:
        validate_safe_relative_path(rel, "required_integration_paths")
        candidate_file = candidate_source / rel
        if not candidate_file.is_file():
            missing_required.append(rel)
            continue
        validate_safe_file_within(candidate_file, candidate_source, f"candidate file {rel}")

    if missing_required:
        raise RuntimeError(
            f"Candidate root is missing {len(missing_required)} required file(s): {', '.join(missing_required[:10])}"
        )

    for rel, expected_sha in sorted(manifest_digests.items()):
        target_file = validate_safe_file_within(
            candidate_source / rel,
            candidate_source,
            f"manifest file {rel}",
        )
        actual_sha = sha256_file(target_file)
        if actual_sha.lower() != expected_sha.lower():
            raise RuntimeError(
                f"Candidate SHA-256 mismatch for {rel}: expected {expected_sha}, got {actual_sha}"
            )

    for file_path in sorted(candidate_source.rglob("*")):
        if not file_path.is_file():
            continue
        if file_path.name == "candidate-manifest.json":
            continue
        validate_safe_file_within(file_path, candidate_source, "candidate tree scan")
        rel = validate_safe_relative_path(
            file_path.relative_to(candidate_source).as_posix(),
            "candidate tree file",
        )
        dest = scratch_tree / rel
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(file_path, dest)
        copied.append(rel)

    return copied


def rehearse_upgrade_on_tree(
    candidate_source: Path, review_manifest: dict[str, object] | None = None,
    git_inputs: dict[str, tuple[bytes, str]] | None = None,
) -> dict[str, object]:
    with tempfile.TemporaryDirectory(prefix="omniplay-upgrade-rehearsal-") as tmp:
        scratch_tree = Path(tmp) / "src"
        scratch_tree.mkdir(parents=True, exist_ok=True)
        copied_files = copy_candidate_inputs(candidate_source, scratch_tree, review_manifest, git_inputs)

        sources: dict[str, str] = {}
        for rel in copied_files:
            target_file = scratch_tree / rel
            if target_file.is_file():
                sources[rel] = target_file.read_text(encoding="utf-8")

        stage_results: list[dict[str, object]] = []

        try:
            # Stage 1: Verify pinned desktop capture audio anchors against candidate tree (does not write to tree)
            touched_audio = desktop_capture_audio_patch.author(scratch_tree)
            stage_results.append(
                {
                    "stage": "desktop_capture_audio_patch.author",
                    "status": "ANCHORS_VERIFIED",
                    "mutates_tree": False,
                    "verified_files": sorted(touched_audio.keys()),
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
                    "mutates_tree": True,
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
                    "mutates_tree": True,
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
    candidate_checkout: Path | None = None,
    expected_candidate_revision: str | None = None,
) -> dict[str, object]:
    lock_data = json.loads(LOCK_PATH.read_text(encoding="utf-8"))
    series_data = json.loads(SERIES_PATH.read_text(encoding="utf-8"))

    review_manifest = None
    git_inputs = None
    candidate_version = None
    source_root = (upgrade_review_dir or candidate_root or SNAPSHOT_ROOT).absolute()
    try:
        ordinary(source_root, Path(source_root.anchor), file=False)
    except ValueError as exc:
        raise RuntimeError(f"Unsafe Chromium source root: {exc}") from exc
    if upgrade_review_dir is not None:
        if candidate_root is not None or candidate_checkout is None or expected_candidate_revision is None:
            raise RuntimeError("An upgrade review requires --candidate-checkout and --expected-candidate-revision")
        manifest_path = validate_safe_file_within(source_root / "upgrade-review.json", source_root, "review manifest")
        review_manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        if not isinstance(review_manifest, dict) or review_manifest.get("schema_version") != 1:
            raise RuntimeError("Invalid Chromium upgrade review manifest schema")
        candidate_version, git_inputs = candidate_git_inputs(
            review_manifest, source_root, candidate_checkout, expected_candidate_revision, lock_data,
        )
    elif candidate_checkout is not None or expected_candidate_revision is not None:
        raise RuntimeError("Candidate Git options require --upgrade-review-dir")

    rehearsal = rehearse_upgrade_on_tree(source_root, review_manifest, git_inputs)
    receipt: dict[str, object] = {
        "schema_version": 1,
        "item_id": 7,
        "generated_at_utc": utc_now(),
        "baseline_release_tag": lock_data.get("version") or lock_data.get("release_tag"),
        "baseline_revision": lock_data.get("revision"),
        "candidate_source": str(source_root),
        "qualification_scope": "upstream_candidate_patch_rehearsal" if git_inputs else "source_fixture_patch_rehearsal",
        "candidate_version": candidate_version,
        "candidate_revision": expected_candidate_revision,
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
    parser.add_argument("--candidate-checkout", type=Path, help="Local Chromium Git objects for the independently selected candidate.")
    parser.add_argument("--expected-candidate-revision", help="Full Chromium commit ID selected independently of the review bundle.")
    args = parser.parse_args(argv)

    receipt = verify_upgrade(
        candidate_root=args.candidate_root,
        upgrade_review_dir=args.upgrade_review_dir,
        output_receipt=args.output_receipt,
        candidate_checkout=args.candidate_checkout,
        expected_candidate_revision=args.expected_candidate_revision,
    )
    print(json.dumps(receipt, indent=2, sort_keys=True))
    return 0 if receipt["verified"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
