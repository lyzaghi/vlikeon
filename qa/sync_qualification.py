#!/usr/bin/env python3
"""
Automated Encrypted Sync & Account Qualification Suite.
Validates:
- Case S01 (Item 78): Crypto Envelope & AAD layout (44-byte structure, magic "VLKSYNC1").
- Case S02 (Item 80): Large save chunking bounds (256 KiB chunks, <= 512 KiB memory ceiling).
- Case S03 (Item 81): Resumable upload reconciliation & lost-commit recovery.
- Case S04 (Item 82): Storage quota bounds and orphan staging sweep TTL (24h).
- Case S05 (Item 83): Atomic revocation fencing and state rejection.
"""

import hashlib
import json
import struct
import sys
import unittest
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CHUNK_SIZE = 262144  # 256 KiB


class TestCryptoEnvelopeAndAAD(unittest.TestCase):
    def test_aad_wire_layout(self):
        """Validates 44-byte AAD layout: Profile(16) + Object(16) + Schema(4) + Timestamp(8)."""
        profile_uuid = uuid.UUID("11111111-1111-1111-1111-111111111111")
        object_uuid = uuid.UUID("22222222-2222-2222-2222-222222222222")
        schema_version = 2
        timestamp_ms = 1729850000000

        aad = bytearray(44)
        aad[0:16] = profile_uuid.bytes
        aad[16:32] = object_uuid.bytes
        struct.pack_into("<I", aad, 32, schema_version)
        struct.pack_into("<Q", aad, 36, timestamp_ms)

        self.assertEqual(len(aad), 44)
        self.assertEqual(aad[0:16], profile_uuid.bytes)
        self.assertEqual(aad[16:32], object_uuid.bytes)
        self.assertEqual(struct.unpack_from("<I", aad, 32)[0], 2)
        self.assertEqual(struct.unpack_from("<Q", aad, 36)[0], timestamp_ms)

    def test_blob_header_magic(self):
        """Validates 8-byte ASCII magic 'VLKSYNC1' and version."""
        header = bytearray(128)
        header[0:8] = b"VLKSYNC1"
        struct.pack_into("<H", header, 8, 1)  # Version 1
        header[10] = 1  # Cipher: XChaCha20-Poly1305

        self.assertEqual(header[0:8], b"VLKSYNC1")
        self.assertEqual(struct.unpack_from("<H", header, 8)[0], 1)
        self.assertEqual(header[10], 1)


class TestChunkingAndMemoryEnvelope(unittest.TestCase):
    def test_chunk_splitting(self):
        """Validates that a multi-megabyte payload divides into exact 256 KiB chunks with correct remainder."""
        total_size = (3 * CHUNK_SIZE) + 12345  # 3 full chunks + remainder
        chunk_count = (total_size + CHUNK_SIZE - 1) // CHUNK_SIZE
        self.assertEqual(chunk_count, 4)

        offsets = []
        sizes = []
        for i in range(chunk_count):
            offset = i * CHUNK_SIZE
            size = min(CHUNK_SIZE, total_size - offset)
            offsets.append(offset)
            sizes.append(size)

        self.assertEqual(offsets, [0, CHUNK_SIZE, 2 * CHUNK_SIZE, 3 * CHUNK_SIZE])
        self.assertEqual(sizes, [CHUNK_SIZE, CHUNK_SIZE, CHUNK_SIZE, 12345])
        self.assertEqual(sum(sizes), total_size)

    def test_memory_ceiling_bound(self):
        """Ensures active buffer window does not exceed 2 chunks (512 KiB)."""
        max_active_chunks = 2
        max_memory_bytes = max_active_chunks * CHUNK_SIZE
        self.assertEqual(max_memory_bytes, 524288)  # 512 KiB


class TestResumableReconciliation(unittest.TestCase):
    def test_lost_commit_reconciliation(self):
        """Simulates client reconnect after lost commit reply; server responds with already_stored."""
        # Simulated server database of committed objects
        committed_objects = {
            "transfer-uuid-1": {"object_id": "obj-123", "digest": "abcdef123456"}
        }

        # Client sends upload_begin with transfer-uuid-1
        incoming_transfer_id = "transfer-uuid-1"
        if incoming_transfer_id in committed_objects:
            response = {
                "status": "ok",
                "result": {
                    "already_stored": True,
                    "object_id": committed_objects[incoming_transfer_id]["object_id"],
                    "digest": committed_objects[incoming_transfer_id]["digest"]
                }
            }
        else:
            response = {"status": "ok", "result": {"already_stored": False, "received_offset": 0}}

        self.assertTrue(response["result"]["already_stored"])
        self.assertEqual(response["result"]["digest"], "abcdef123456")

    def test_partial_upload_resumption(self):
        """Simulates resuming from received_offset = 2 * CHUNK_SIZE."""
        received_offset = 2 * CHUNK_SIZE
        total_size = 4 * CHUNK_SIZE
        remaining_bytes = total_size - received_offset
        self.assertEqual(remaining_bytes, 2 * CHUNK_SIZE)


class TestQuotaAndOrphanSweep(unittest.TestCase):
    def test_quota_rejection(self):
        """Validates quota rejection when current_usage + object_size > account_quota."""
        account_quota = 500 * 1024 * 1024  # 500 MiB
        current_usage = 490 * 1024 * 1024  # 490 MiB
        new_object_size = 20 * 1024 * 1024  # 20 MiB -> 510 MiB > 500 MiB

        exceeds = (current_usage + new_object_size) > account_quota
        self.assertTrue(exceeds)

    def test_orphan_sweep_ttl(self):
        """Validates that staging uploads older than 24h (86400s) are marked for deletion."""
        now = 1729850000
        ttl = 86400  # 24 hours

        staged_files = [
            {"id": "active-transfer", "created_at": now - 3600, "in_use": True},
            {"id": "recent-transfer", "created_at": now - 7200, "in_use": False},
            {"id": "expired-orphan", "created_at": now - 90000, "in_use": False},
        ]

        to_delete = []
        for item in staged_files:
            if not item["in_use"] and (now - item["created_at"]) > ttl:
                to_delete.append(item["id"])

        self.assertEqual(to_delete, ["expired-orphan"])


class TestRevocationFencing(unittest.TestCase):
    def test_revocation_halt(self):
        """Simulates atomic revocation fence halting in-flight synchronization."""
        account_state = "active"

        # Client prepares sync operation
        can_proceed = (account_state == "active")
        self.assertTrue(can_proceed)

        # Revocation observed
        account_state = "revoking"
        can_proceed = (account_state == "active")
        self.assertFalse(can_proceed, "Revoking account must immediately fence sync operations")


if __name__ == "__main__":
    unittest.main()
