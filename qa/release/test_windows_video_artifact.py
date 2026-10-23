"""Authored, execution deferred: inert PE-layout fixtures, never executable code."""
import hashlib
from pathlib import Path
import struct
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'tools'))
from windows_video_artifact import admit_video_artifact


def fixture(machine=0x8664):
    data = bytearray(1024)
    data[:2] = b'MZ'
    struct.pack_into('<I', data, 0x3c, 128)
    data[128:132] = b'PE\0\0'
    struct.pack_into('<HH', data, 132, machine, 1)
    struct.pack_into('<HH', data, 148, 240, 2)
    struct.pack_into('<H', data, 152, 0x20b)
    struct.pack_into('<HH', data, 220, 3, 0x4160)
    struct.pack_into('<II', data, 408, 512, 512)
    return data


class VideoArtifactCases(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='vlikeon-video-pe-')
        self.addCleanup(temporary.cleanup)
        self.path = Path(temporary.name) / 'worker.exe'

    def admit(self, data, platform='windows-x64'):
        self.path.write_bytes(data)
        return admit_video_artifact(self.path, platform)

    def test_layout_receipt_hashes_exact_inert_bytes(self):
        data = fixture()
        result = self.admit(data)
        self.assertEqual(result['bytes'], len(data))
        self.assertEqual(result['sha256'], hashlib.sha256(data).hexdigest())
        self.assertTrue(result['executable'])
        self.assertIn('not_runtime_or_signature', result['verification'])

    def test_arm64_layout_requires_matching_platform(self):
        self.admit(fixture(0xaa64), 'windows-arm64')
        with self.assertRaises(ValueError):
            self.admit(fixture(0xaa64), 'windows-x64')

    def test_each_missing_mitigation_is_rejected(self):
        for bit in (0x20, 0x40, 0x100, 0x4000):
            with self.subTest(bit=bit):
                data = fixture()
                struct.pack_into('<H', data, 222, 0x4160 & ~bit)
                with self.assertRaises(ValueError):
                    self.admit(data)

    def test_dll_and_nonexecutable_are_rejected(self):
        for flags in (0, 0x2002):
            data = fixture()
            struct.pack_into('<H', data, 150, flags)
            with self.assertRaises(ValueError):
                self.admit(data)

    def test_section_cannot_overlap_headers_or_exceed_file(self):
        for offset, size in ((100, 20), (512, 513), (2000, 1)):
            data = fixture()
            struct.pack_into('<II', data, 408, size, offset)
            with self.assertRaises(ValueError):
                self.admit(data)

    def test_truncated_header_and_bad_offset_are_rejected(self):
        for data in (fixture()[:250], bytearray(1024)):
            with self.assertRaises(ValueError):
                self.admit(data)
        data = fixture()
        struct.pack_into('<I', data, 0x3c, 1000)
        with self.assertRaises(ValueError):
            self.admit(data)

    def test_nonwindows_release_platform_is_rejected(self):
        with self.assertRaises(ValueError):
            self.admit(fixture(), 'linux-x64')


if __name__ == '__main__':
    unittest.main()
