from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "tools"))

import verify_chromium_upgrade  # noqa: E402


def git_blob_id(raw: bytes) -> str:
    return hashlib.sha1(f"blob {len(raw)}".encode("ascii") + b"\0" + raw).hexdigest()


def _build_valid_upgrade_review_dir(root_dir: Path, revision: str, raw_version: bytes) -> Path:
    review_dir = root_dir.resolve() / "upgrade-review"
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
                "candidate_blob": git_blob_id(raw),
            }
        )

    manifest = {
        "schema_version": 1,
        "source_only": True,
        "baseline_revision": json.loads(verify_chromium_upgrade.LOCK_PATH.read_text(encoding="utf-8"))["revision"],
        "candidate_revision": revision,
        "candidate_version": "999.0.0.1",
        "candidate_version_blob": git_blob_id(raw_version),
        "source_inventory": inventory,
        "integration_sources": integration_sources,
    }
    (review_dir / "candidate-VERSION").write_bytes(raw_version)
    (review_dir / "upgrade-review.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return review_dir


class UpgradeRehearsalTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        tmp = tempfile.TemporaryDirectory()
        cls.addClassCleanup(tmp.cleanup)
        cls.checkout = Path(tmp.name).resolve() / "upstream"
        shutil.copytree(verify_chromium_upgrade.SNAPSHOT_ROOT, cls.checkout)
        cls.raw_version = b"MAJOR=999\nMINOR=0\nBUILD=0\nPATCH=1\n"
        (cls.checkout / "chrome" / "VERSION").write_bytes(cls.raw_version)

        def git(*args):
            return subprocess.check_output(["git", "-C", str(cls.checkout), *args])

        git("init", "--object-format=sha1")
        git("config", "core.autocrlf", "false")
        git("remote", "add", "origin", verify_chromium_upgrade.REPOSITORY)
        git("add", ".")
        git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-m", "Candidate fixture")
        cls.revision = git("rev-parse", "HEAD").decode("ascii").strip()

    def build_review(self, root):
        return _build_valid_upgrade_review_dir(root, self.revision, self.raw_version)

    def verify_review(self, review_dir):
        return verify_chromium_upgrade.verify_upgrade(
            upgrade_review_dir=review_dir, candidate_checkout=self.checkout,
            expected_candidate_revision=self.revision,
        )

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
            review_dir = self.build_review(Path(tmp))
            receipt = self.verify_review(review_dir)
            self.assertTrue(receipt["verified"])
            self.assertEqual(receipt["qualification_scope"], "upstream_candidate_patch_rehearsal")
            self.assertEqual(receipt["candidate_revision"], self.revision)
            self.assertEqual(receipt["candidate_version"], "999.0.0.1")
            self.assertEqual(receipt["rehearsal"]["copied_input_count"], 36)

    def test_rehearsal_rejects_baseline_only_upgrade_review_without_candidate_files(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            review_dir = self.build_review(Path(tmp))
            # Remove all candidate files while keeping all 36 baseline files (exact Codex reproduction case)
            shutil.rmtree(review_dir / "source" / "candidate")
            with self.assertRaises(RuntimeError) as ctx:
                self.verify_review(review_dir)
            self.assertIn("candidate", str(ctx.exception).lower())

    def test_rehearsal_rejects_missing_candidate_file_in_candidate_root(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            candidate_dir = Path(tmp).resolve() / "candidate"
            shutil.copytree(REPO_ROOT / "browser" / "upstream-snapshots", candidate_dir)
            (candidate_dir / "chrome" / "common" / "chrome_paths_linux.cc").unlink()
            with self.assertRaises(RuntimeError) as ctx:
                verify_chromium_upgrade.verify_upgrade(candidate_root=candidate_dir)
            self.assertIn("missing", str(ctx.exception).lower())

    def test_rehearsal_rejects_wrong_sha256_in_upgrade_review_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            review_dir = self.build_review(Path(tmp))
            manifest_path = review_dir / "upgrade-review.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            for item in manifest["source_inventory"]:
                if str(item["path"]).startswith("source/candidate/"):
                    item["sha256"] = "0" * 64
                    break
            manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
            with self.assertRaises(RuntimeError) as ctx:
                self.verify_review(review_dir)
            self.assertIn("sha-256 mismatch", str(ctx.exception).lower())

    def test_rehearsal_rejects_escaping_or_invalid_candidate_path(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            review_dir = self.build_review(Path(tmp))
            manifest_path = review_dir / "upgrade-review.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["integration_sources"][0]["candidate_copy"] = "../outside/component_loader.cc"
            manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
            with self.assertRaises(RuntimeError) as ctx:
                self.verify_review(review_dir)
            self.assertIn("invalid or escaping path", str(ctx.exception).lower())

    def test_review_rejects_untrusted_identity_and_missing_length(self):
        with tempfile.TemporaryDirectory() as tmp:
            review = self.build_review(Path(tmp))
            path = review / "upgrade-review.json"
            manifest = json.loads(path.read_text(encoding="utf-8"))
            with self.assertRaisesRegex(RuntimeError, "requires --candidate-checkout"):
                verify_chromium_upgrade.verify_upgrade(upgrade_review_dir=review)
            manifest["candidate_version"] = "155.0.8059.85"
            path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(RuntimeError, "Candidate VERSION"):
                self.verify_review(review)
            manifest["candidate_version"] = "999.0.0.1"
            candidate = next(item for item in manifest["source_inventory"] if item["path"].startswith("source/candidate/"))
            del candidate["bytes"]
            path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(RuntimeError, "candidate byte length"):
                self.verify_review(review)

    def test_review_rejects_self_consistent_but_untrusted_candidate_file(self):
        with tempfile.TemporaryDirectory() as tmp:
            review = self.build_review(Path(tmp))
            manifest_path = review / "upgrade-review.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            source = manifest["integration_sources"][0]
            name = source["candidate_copy"]
            target = review / name
            raw = target.read_bytes() + b"\n// self-consistent counterfeit"
            target.write_bytes(raw)
            source["candidate_blob"] = git_blob_id(raw)
            item = next(item for item in manifest["source_inventory"] if item["path"] == name)
            item.update(bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest())
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(RuntimeError, "selected Chromium Git blob"):
                self.verify_review(review)

    def test_review_rejects_linked_source_ancestor(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            review = self.build_review(root)
            source = review / "source"
            outside = root / "outside-source"
            source.rename(outside)
            if os.name == "nt":
                result = subprocess.run(["cmd", "/d", "/c", "mklink", "/J", str(source), str(outside)], capture_output=True)
                if result.returncode != 0:
                    self.skipTest("Windows runner cannot create a test junction")
            else:
                source.symlink_to(outside, target_is_directory=True)
            try:
                with self.assertRaisesRegex(RuntimeError, "Unsafe"):
                    self.verify_review(review)
            finally:
                if os.name == "nt":
                    source.rmdir()
                else:
                    source.unlink()

    def test_rehearsal_rejects_drifted_upstream_anchor(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            candidate_dir = Path(tmp).resolve() / "candidate"
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
