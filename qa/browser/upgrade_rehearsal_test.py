from __future__ import annotations

import hashlib
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest


REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "tools"))

import verify_chromium_upgrade  # noqa: E402


def _build_valid_upgrade_review_dir(root_dir: Path) -> Path:
    review_dir = root_dir / "upgrade-review"
    review_dir.mkdir(parents=True, exist_ok=True)
    snap_root = REPO_ROOT / "browser" / "upstream-snapshots"

    inventory: list[dict[str, object]] = []
    integration_sources: list[dict[str, object]] = []

    for snap_file in sorted(snap_root.rglob("*")):
        if not snap_file.is_file():
            continue
        rel = snap_file.relative_to(snap_root).as_posix()
        raw = snap_file.read_bytes()
        sha = hashlib.sha256(raw).hexdigest()

        for group in ("baseline", "candidate"):
            dest = review_dir / "source" / group / rel
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(raw)
            inventory.append(
                {
                    "path": f"source/{group}/{rel}",
                    "bytes": len(raw),
                    "sha256": sha,
                }
            )

        integration_sources.append(
            {
                "path": rel,
                "baseline_copy": f"source/baseline/{rel}",
                "candidate_copy": f"source/candidate/{rel}",
            }
        )

    manifest = {
        "schema_version": 1,
        "source_only": True,
        "source_inventory": inventory,
        "integration_sources": integration_sources,
    }
    (review_dir / "upgrade-review.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return review_dir


class UpgradeRehearsalTest(unittest.TestCase):
    def test_rehearsal_succeeds_and_verifies_security_invariants_and_stage_statuses(self) -> None:
        receipt = verify_chromium_upgrade.verify_upgrade(
            candidate_root=REPO_ROOT / "browser" / "upstream-snapshots",
        )
        self.assertTrue(receipt["verified"])
        rehearsal = receipt["rehearsal"]
        self.assertEqual(rehearsal["copied_input_count"], 36)
        self.assertTrue(rehearsal["all_security_invariants_passed"])
        self.assertGreaterEqual(len(rehearsal["security_invariants"]), 9)
        for item in rehearsal["security_invariants"]:
            self.assertTrue(item["passed"], f"Invariant failed: {item}")

        stages = {s["stage"]: s for s in rehearsal["stages"]}
        self.assertEqual(stages["desktop_capture_audio_patch.author"]["status"], "ANCHORS_VERIFIED")
        self.assertFalse(stages["desktop_capture_audio_patch.author"]["mutates_tree"])
        self.assertEqual(stages["0001-omniplay-game-hub.patch"]["status"], "APPLIED")
        self.assertTrue(stages["0001-omniplay-game-hub.patch"]["mutates_tree"])
        self.assertEqual(stages["0003-owned-game-capture.patch"]["status"], "APPLIED")
        self.assertTrue(stages["0003-owned-game-capture.patch"]["mutates_tree"])

    def test_rehearsal_succeeds_on_valid_upgrade_review_directory(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            review_dir = _build_valid_upgrade_review_dir(Path(tmp))
            receipt = verify_chromium_upgrade.verify_upgrade(upgrade_review_dir=review_dir)
            self.assertTrue(receipt["verified"])
            self.assertEqual(receipt["rehearsal"]["copied_input_count"], 36)

    def test_rehearsal_rejects_baseline_only_upgrade_review_without_candidate_files(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            review_dir = _build_valid_upgrade_review_dir(Path(tmp))
            # Remove all candidate files while keeping all 36 baseline files (exact Codex reproduction case)
            shutil.rmtree(review_dir / "source" / "candidate")
            with self.assertRaises(RuntimeError) as ctx:
                verify_chromium_upgrade.verify_upgrade(upgrade_review_dir=review_dir)
            self.assertIn("candidate", str(ctx.exception).lower())

    def test_rehearsal_rejects_missing_candidate_file_in_candidate_root(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            candidate_dir = Path(tmp) / "candidate"
            shutil.copytree(REPO_ROOT / "browser" / "upstream-snapshots", candidate_dir)
            (candidate_dir / "chrome" / "common" / "chrome_paths_linux.cc").unlink()
            with self.assertRaises(RuntimeError) as ctx:
                verify_chromium_upgrade.verify_upgrade(candidate_root=candidate_dir)
            self.assertIn("missing", str(ctx.exception).lower())

    def test_rehearsal_rejects_wrong_sha256_in_upgrade_review_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            review_dir = _build_valid_upgrade_review_dir(Path(tmp))
            manifest_path = review_dir / "upgrade-review.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            for item in manifest["source_inventory"]:
                if str(item["path"]).startswith("source/candidate/"):
                    item["sha256"] = "0" * 64
                    break
            manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
            with self.assertRaises(RuntimeError) as ctx:
                verify_chromium_upgrade.verify_upgrade(upgrade_review_dir=review_dir)
            self.assertIn("sha-256 mismatch", str(ctx.exception).lower())

    def test_rehearsal_rejects_escaping_or_invalid_candidate_path(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            review_dir = _build_valid_upgrade_review_dir(Path(tmp))
            manifest_path = review_dir / "upgrade-review.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["integration_sources"][0]["candidate_copy"] = "../outside/component_loader.cc"
            manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
            with self.assertRaises(RuntimeError) as ctx:
                verify_chromium_upgrade.verify_upgrade(upgrade_review_dir=review_dir)
            self.assertIn("invalid or escaping path", str(ctx.exception).lower())

    def test_rehearsal_rejects_drifted_upstream_anchor(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            candidate_dir = Path(tmp) / "candidate"
            shutil.copytree(REPO_ROOT / "browser" / "upstream-snapshots", candidate_dir)
            target = candidate_dir / "chrome" / "browser" / "extensions" / "component_loader.cc"
            original = target.read_text(encoding="utf-8")
            drifted = original.replace(
                "  if (should_disable_background_extensions) {\n    return;\n  }\n",
                "  if (should_disable_background_extensions) {\n    return; // drifted\n  }\n",
            )
            self.assertNotEqual(original, drifted)
            target.write_text(drifted, encoding="utf-8", newline="\n")
            with self.assertRaises(RuntimeError):
                verify_chromium_upgrade.verify_upgrade(candidate_root=candidate_dir)


if __name__ == "__main__":
    unittest.main()
