from __future__ import annotations

from pathlib import Path
import shutil
import sys
import tempfile
import unittest


REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "tools"))

import verify_chromium_upgrade  # noqa: E402


class UpgradeRehearsalTest(unittest.TestCase):
    def test_rehearsal_succeeds_and_verifies_security_invariants(self) -> None:
        receipt = verify_chromium_upgrade.verify_upgrade(
            candidate_root=REPO_ROOT / "browser" / "upstream-snapshots",
        )
        self.assertTrue(receipt["verified"])
        rehearsal = receipt["rehearsal"]
        self.assertTrue(rehearsal["all_security_invariants_passed"])
        self.assertGreaterEqual(len(rehearsal["security_invariants"]), 9)
        for item in rehearsal["security_invariants"]:
            self.assertTrue(item["passed"], f"Invariant failed: {item}")

    def test_rehearsal_rejects_drifted_upstream_anchor(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            candidate_dir = Path(tmp) / "candidate"
            shutil.copytree(REPO_ROOT / "browser" / "upstream-snapshots", candidate_dir)
            target = candidate_dir / "chrome" / "common" / "webui_url_constants.h"
            drifted = target.read_text(encoding="utf-8").replace(
                'inline constexpr char kChromeUIVersionURL[] = "chrome://version/";',
                'inline constexpr char kChromeUIVersionURL[] = "chrome://version-drifted/";',
            )
            target.write_text(drifted, encoding="utf-8")
            with self.assertRaises(RuntimeError):
                verify_chromium_upgrade.verify_upgrade(candidate_root=candidate_dir)


if __name__ == "__main__":
    unittest.main()
