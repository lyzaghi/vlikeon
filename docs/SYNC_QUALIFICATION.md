# Encrypted Sync Platform Qualification Matrix & Acceptance — Item 83

Status: Automated qualification harness authored (`qa/sync_qualification.py`); live production server and network fault execution deferred per project owner policy.

## 1. Test Environment Requirements

To execute complete sync qualification with live networking and account stores:
- **Server Infrastructure:**
  - Authenticated sync service instance running over HTTPS with TLS 1.3 enabled.
  - POSIX storage volume with non-blocking file locking (`F_SETLK`).
- **Network Impairment Tools:**
  - Network emulator (`tc` / `netem` on Linux or Clumsy on Windows) to simulate:
    - Packet loss (5% to 25%).
    - Connection reset / dropped packets midway through multi-chunk upload.
    - High latency / jitter (100ms - 500ms).
- **Client Testbed:**
  - Desktop (Windows, macOS, Linux) and Mobile (Android, iOS) clients operating on the same test account.

## 2. Test Matrix Cases

| Case | Category | Description | Verification Gate |
|---|---|---|---|
| **S01** | Crypto Envelope | Verify XChaCha20-Poly1305 wire format and 44-byte AAD structure | Payload integrity check passes; tamper attempt returns error |
| **S02** | Chunked Memory Ceiling | Upload 10 MiB save file; verify buffer memory does not exceed 512 KiB | Memory stays strictly within the 2-chunk ceiling |
| **S03** | Resumable Upload | Interrupt 5 MiB upload after 5 chunks; reconnect and verify resumption from chunk 6 | Previous 5 chunks not re-uploaded; total bytes transferred = file size |
| **S04** | Lost Commit Reply | Drop server response for `upload_commit`; client re-invokes `upload_begin` | Server returns `already_stored: true`; client transitions to Finished |
| **S05** | Account Quota | Attempt upload that exceeds account 500 MiB quota | Server returns `quota_exceeded`; existing saves preserved intact |
| **S06** | Orphan Sweep | Create incomplete staging uploads; advance clock past 24-hour TTL | Abandoned staging chunks purged; active transfers untouched |
| **S07** | Revocation Fencing | Revoke device or account credentials during active sync | In-flight operations halted immediately; late server responses discarded |

## 3. Qualification Execution Commands

When live infrastructure and network impairment tools are authorized:
```bash
# 1. Run unit qualification suite
python3 qa/sync_qualification.py

# 2. Run network fault qualification against local mock server
# python3 -m qa.sync_fault_matrix --endpoint https://127.0.0.1:8443/v1/sync
```

## 4. Current Status

Items 77–82 are authored in source (`SOURCE_READY_UNVERIFIED`). Live network and server fault qualification (Item 83) is queued as `WAITING_PERMISSION_OR_RESOURCE` pending owner infrastructure authorization.
