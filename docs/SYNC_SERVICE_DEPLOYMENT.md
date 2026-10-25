# Authenticated Sync Service Deployment Guide — Item 79

Status: Authored production configuration templates and deployment documentation; production deployment and key provisioning deferred per project owner policy.

## 1. Architecture Overview

The Vlikeon OmniPlay encrypted sync service provides multi-device save synchronization and account reconciliation over authenticated HTTPS:
- **Wire Endpoint:** `POST /v1/sync`
- **Protocol:** JSON-RPC over HTTPS with signed device certificates and bearer authentication.
- **Payload Security:** Client-side zero-knowledge encryption using XChaCha20-Poly1305. The server never observes plaintext saves, encryption keys, or unauthenticated metadata.
- **Resumable Chunking:** Large saves (> 256 KiB) are transmitted in fixed 256 KiB chunks, enabling resilient recovery across network drops without duplicating already-uploaded bytes.

## 2. Infrastructure Requirements

- **Server Host:** Linux (Ubuntu 22.04 LTS / Debian 12 / Rocky Linux 9).
- **Public Network:** Pinned DNS domain (e.g. `sync.vlikeon.example.org`), open inbound port 443 (HTTPS).
- **TLS Termination:** Nginx / Caddy reverse proxy or native Rust TLS with TLS 1.3 enforced.
- **Storage:** NVMe or high-IOPS SSD volume for the sync storage root (`/var/lib/vlikeon/sync-storage`).
- **File System:** POSIX-compliant filesystem supporting non-blocking `fcntl` file locking (`F_SETLK`).

## 3. Configuration & Security Hardening

The service configuration template is located at [`security/sync/sync-service.example.json`](file:///E:/V10/OmniPlay/security/sync/sync-service.example.json):
1. **TLS Enforcement:** Minimum TLS version 1.3 with modern AEAD cipher suites (`TLS_AES_256_GCM_SHA384`, `TLS_CHACHA20_POLY1305_SHA256`).
2. **Authentication:** Pinned JWT issuer, audience verification matching the exact endpoint URL, and 30-second clock skew tolerance.
3. **Storage Quota:** Account quota bounded at 500 MiB (524,288,000 bytes) and max individual object bound at 512 MiB.
4. **Staging Cleanup:** Abandoned staging uploads expire after 24 hours (86,400 seconds); background sweeper purges uncommitted staging files every hour.

## 4. Operational Maintenance

- **Service Supervision:** systemd service unit (`vlikeon-sync.service`) with `NoNewPrivileges=true`, `ProtectSystem=strict`, and `PrivateTmp=true`.
- **Health Checks:** `GET /healthz` returning 200 OK with storage volume free space.
- **Log Hygiene:** Request logs record only timestamps, client IP hashes, transfer UUIDs, and byte counts. Plaintext tokens, decryption keys, and account passphrases are strictly forbidden in logs.
