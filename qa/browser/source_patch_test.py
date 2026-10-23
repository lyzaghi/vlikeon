"""Strict source patch boundaries, including refusal instead of fuzzy migration."""
from pathlib import Path
import sys
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parents[2]/'tools'))
from source_patch import apply_exact

PATCH='--- a/source.cc\n+++ b/source.cc\n@@ -1,3 +1,4 @@\n one\n-two\n+new\n+added\n three\n'
class SourcePatchCases(unittest.TestCase):
    def test_exact_change(self):self.assertEqual(apply_exact({'source.cc':'one\ntwo\nthree\n'},PATCH),{'source.cc':'one\nnew\nadded\nthree\n'})
    def test_context_must_match(self):
        with self.assertRaises(ValueError):apply_exact({'source.cc':'changed\ntwo\nthree\n'},PATCH)
    def test_no_fuzzy_position(self):
        with self.assertRaises(ValueError):apply_exact({'source.cc':'extra\none\ntwo\nthree\n'},PATCH)
    def test_missing_or_foreign_target(self):
        with self.assertRaises(ValueError):apply_exact({'other.cc':'one\ntwo\nthree\n'},PATCH)
    def test_declared_counts_checked(self):
        with self.assertRaises(ValueError):apply_exact({'source.cc':'one\ntwo\nthree\n'},PATCH.replace('+1,4','+1,3'))
    def test_repeated_file_refused(self):
        with self.assertRaises(ValueError):apply_exact({'source.cc':'one\ntwo\nthree\n'},PATCH+PATCH)
if __name__=='__main__':unittest.main(verbosity=2)
