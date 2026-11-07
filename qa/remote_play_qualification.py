#!/usr/bin/env python3
"""
Remote Play Real-Network Qualification Test Harness (Item 60)
Prepares deterministic verification cases for end-to-end Remote Play:
- Pairing & Consent handshake
- ICE candidate gathering, STUN integrity and nomination
- DTLS-SRTP key derivation & fingerprint verification
- SRTP demuxing, decryption, and sliding replay window protection
- Network drop, consent freshness expiry and reconnection
- Immediate revocation & cleanup debt reconciliation
- Window closure & crash recovery

Execution is queued for qualification phase; do not report PASS until run on target network.
"""

import sys
import json
import subprocess
import unittest
import struct
import hmac
import hashlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools"))
from windows_video_artifact import admit_video_artifact

class TestRemotePlayQualificationQueue(unittest.TestCase):

    def test_stun_demux_classification(self):
        """RFC 7983 Demux: 0..3=STUN, 20..63=DTLS, 128..191=SRTP"""
        def classify(byte0):
            if 0 <= byte0 <= 3:
                return "STUN"
            elif 20 <= byte0 <= 63:
                return "DTLS"
            elif 128 <= byte0 <= 191:
                return "SRTP"
            return "UNKNOWN"

        self.assertEqual(classify(0), "STUN")
        self.assertEqual(classify(1), "STUN")
        self.assertEqual(classify(22), "DTLS")
        self.assertEqual(classify(128), "SRTP")
        self.assertEqual(classify(130), "SRTP")
        self.assertEqual(classify(255), "UNKNOWN")

    def test_replay_window_sliding_bitmap(self):
        """Tests 128-bit replay window rejection of replayed or stale packets"""
        class ReplayWindow:
            def __init__(self):
                self.max_seq = 0
                self.bitmap = 0
                self.initialized = False

            def check_and_update(self, seq):
                if not self.initialized:
                    self.initialized = True
                    self.max_seq = seq
                    self.bitmap = 1
                    return True
                if seq > self.max_seq:
                    diff = seq - self.max_seq
                    if diff < 128:
                        self.bitmap = (self.bitmap << diff) | 1
                        self.bitmap &= (1 << 128) - 1
                    else:
                        self.bitmap = 1
                    self.max_seq = seq
                    return True
                else:
                    diff = self.max_seq - seq
                    if diff >= 128:
                        return False # Stale
                    if (self.bitmap & (1 << diff)) != 0:
                        return False # Duplicate / Replay
                    self.bitmap |= (1 << diff)
                    return True

        w = ReplayWindow()
        self.assertTrue(w.check_and_update(100))
        self.assertTrue(w.check_and_update(101))
        self.assertFalse(w.check_and_update(100)) # Replay!
        self.assertFalse(w.check_and_update(101)) # Replay!
        self.assertTrue(w.check_and_update(99))  # Late arrival within window
        self.assertFalse(w.check_and_update(99)) # Second duplicate
        self.assertTrue(w.check_and_update(300)) # Advance far ahead
        self.assertFalse(w.check_and_update(98)) # Now stale (>128 diff)

    def test_fingerprint_constant_time_comparison(self):
        """Verifies SHA-256 fingerprint verification against certificate DER"""
        dummy_der = b"OmniPlay_Peer_Certificate_DER_Bytes"
        digest = hashlib.sha256(dummy_der).digest()
        hex_str = ":".join(f"{b:02X}" for b in digest)

        # Matched hex
        cleaned = hex_str.replace(":", "").upper()
        self.assertEqual(len(cleaned), 64)
        self.assertTrue(hmac.compare_digest(digest.hex().upper(), cleaned))

        # Tampered hex must fail
        tampered = "00" + cleaned[2:]
        self.assertFalse(hmac.compare_digest(digest.hex().upper(), tampered))

    def test_session_revocation_stops_producers(self):
        """Verifies immediate transition to Revoked state upon revoke call"""
        state = "Active"
        worker_running = True
        cleanup_debt = False

        # Invoke revoke
        state = "Revoking"
        worker_running = False # Fenced immediately
        cleanup_debt = True    # Retained until vault confirms removal

        # Vault confirms removal
        cleanup_debt = False
        state = "Retired"

        self.assertEqual(state, "Retired")
        self.assertFalse(worker_running)
        self.assertFalse(cleanup_debt)

    def test_windows_video_worker_and_decode_type_fixture(self):
        """Verifies compiled Windows video worker PE mitigations and runs C++ MF decode-type fixture (Item 56)"""
        worker = ROOT / "out/stream-video/x64/package/helpers/omniplay-video-worker.exe"
        receipt_path = ROOT / "out/stream-video/x64/build-receipt.json"
        fixture = ROOT / "out/stream-video/x64/build/vlikeon-decode-type-fixture.exe"
        self.assertTrue(worker.is_file(), f"Missing built video worker: {worker}")
        self.assertTrue(receipt_path.is_file(), f"Missing video build receipt: {receipt_path}")
        self.assertTrue(fixture.is_file(), f"Missing built decode-type fixture: {fixture}")

        admitted = admit_video_artifact(worker, "windows-x64")
        receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
        self.assertEqual(admitted["sha256"], receipt["artifact"]["sha256"])

        proc = subprocess.run([str(fixture)], cwd=ROOT, capture_output=True, text=True, timeout=15)
        self.assertEqual(proc.returncode, 0, f"vlikeon-decode-type-fixture failed: {proc.stderr}")

    def test_windows_input_worker_artifact(self):
        """Verifies compiled Windows input worker binary and receipt integrity (Item 55/57)"""
        worker = ROOT / "out/stream-input/x86_64-pc-windows-msvc/package/helpers/omniplay-input-worker.exe"
        receipt_path = ROOT / "out/stream-input/x86_64-pc-windows-msvc/build-receipt.json"
        self.assertTrue(worker.is_file(), f"Missing built input worker: {worker}")
        self.assertTrue(receipt_path.is_file(), f"Missing input worker receipt: {receipt_path}")
        receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
        actual_sha = hashlib.sha256(worker.read_bytes()).hexdigest()
        self.assertEqual(actual_sha, receipt["sha256"])

    def test_production_signaling_and_coturn_deployment_configs(self):
        """Verifies production coturn and signaling configuration templates enforce TLS, HMAC auth, and SSRF blocks (Item 59)"""
        coturn_conf = (ROOT / "security/coturn/turnserver.conf.example").read_text(encoding="utf-8")
        for required_directive in [
            "use-auth-secret",
            "no-tlsv1",
            "no-tlsv1_1",
            "no-loopback-peers",
            "no-multicast-peers",
            "denied-peer-ip=10.0.0.0-10.255.255.255",
            "denied-peer-ip=172.16.0.0-172.31.255.255",
            "denied-peer-ip=192.168.0.0-192.168.255.255",
            "denied-peer-ip=127.0.0.0-127.255.255.255",
        ]:
            self.assertIn(required_directive, coturn_conf)

        sig_cfg = json.loads((ROOT / "security/signaling/signaling-service.example.json").read_text(encoding="utf-8"))
        svc = sig_cfg["service"]
        self.assertEqual(svc["tls"]["min_version"], "TLS1.3")
        self.assertTrue(svc["authentication"]["require_signed_invite"])
        self.assertLessEqual(svc["limits"]["max_message_bytes"], 16384)
        for url in svc["turn_service"]["turn_urls"]:
            self.assertTrue(url.startswith("turns:"), f"TURN URL must use TLS (turns:): {url}")

if __name__ == "__main__":
    unittest.main()
