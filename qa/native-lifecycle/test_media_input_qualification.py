"""Regression checks for preserving independent suite results after a failure."""
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location(
    "media_input_qualification", Path(__file__).with_name("media_input_qualification.py")
)
qualification = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(qualification)


class EvidenceFailures(unittest.TestCase):
    def verify_failure_is_recorded(self, *, raises):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "evidence"
            success = {"checks": {"fixture": True}, "passed": True}
            failure = {"checks": {"fixture": False}, "passed": False}
            with patch.object(qualification, "verify_item42_video_contracts", **(
                {"side_effect": OSError("fixture source unreadable")} if raises else {"return_value": failure}
            )), patch.object(qualification, "verify_item43_audio_contracts", return_value=success.copy()) as audio, \
                 patch.object(qualification, "verify_item44_gamepad_contracts", return_value=success.copy()) as gamepad, \
                 patch.object(qualification, "verify_item45_touch_and_mobile_contracts", return_value=success.copy()) as touch, \
                 patch.object(sys, "argv", ["qualification", "--suite", "all", "--output", str(output)]):
                self.assertEqual(qualification.main(), 1)
            evidence = json.loads((output / "evidence.json").read_text(encoding="utf-8"))
            self.assertFalse(evidence["passed"])
            self.assertEqual(evidence["failed_items"], ["item_42_video"])
            self.assertEqual(len(evidence["items"]), 4)
            for callback in (audio, gamepad, touch):
                callback.assert_called_once_with()
            for item in ("item_43_audio", "item_44_gamepad", "item_45_touch"):
                self.assertTrue(evidence["items"][item]["passed"])
            if raises:
                self.assertEqual(evidence["items"]["item_42_video"]["error_type"], "OSError")
            else:
                self.assertEqual(evidence["items"]["item_42_video"]["failed_checks"], ["fixture"])
            self.assertFalse(evidence["live_worker"]["executed"])

    def test_failed_check_preserves_later_suite_results(self):
        self.verify_failure_is_recorded(raises=False)

    def test_unreadable_source_preserves_later_suite_results(self):
        self.verify_failure_is_recorded(raises=True)


if __name__ == "__main__":
    unittest.main()
