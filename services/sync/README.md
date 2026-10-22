# OmniPlay encrypted save service source

This is an unbuilt, unlaunched private HTTP upstream. It binds only a loopback
address. Production TLS/ingress, credentials, operations, security/device evidence
and the final consolidated verification are still outstanding.

Rust 1.89+ is required. Account/device authorization and encrypted object transport
are defined in `platform/sync`; resumable protocol 1 and the separate service
schema-2 upgrade are described in [RESUMABLE_SYNC.md](../../docs/RESUMABLE_SYNC.md).
Core/profile schema is now 13 for the separate publication/recovery journal and save-origin ledger. Deploy matching server/client capability support;
new reconciliation does not silently downgrade after a failed extension request.

Configuration uses `OMNIPLAY_SYNC_AUDIENCE` (canonical HTTPS audience),
`OMNIPLAY_SYNC_REGISTRATION_TOKEN` (32–256 bytes), `OMNIPLAY_SYNC_DATA` (absolute
private storage root), and optional `OMNIPLAY_SYNC_LISTEN` (loopback only; default
127.0.0.1:8792). No real service credentials or release hosts are supplied.

Browser source additionally has an optional `OMNIPLAY_SYNC_WEB_ORIGINS` JSON
array of at most eight exact canonical HTTPS origins. The default empty array
allows no Origin-bearing browser requests. CORS is restricted to the fixed API
methods/headers, echoes one admitted origin and never shares ambient credentials;
root/device signatures remain mandatory. Registration tokens must be 32–256
visible ASCII bytes. The browser product still disables crypto/account transport
and full data sync. See [WEB_SYNC_ACCOUNTS.md](../../docs/WEB_SYNC_ACCOUNTS.md)
for the authored boundary and outstanding ingress/browser/service evidence.

Only one server may hold the storage-root kernel lease. Never unlink or replace
`sync-service-v1.lock` to force a second writer. An unfinished upload reserves
parts/assembly space, remains bound to its account/device and expires after 24
hours. Failed cleanup retains quota debt; unknown or linked files require storage
recovery rather than automatic recursive removal. Existing objects, upload rows,
parts and transactional database state must be considered together for backup,
recovery, deletion, disk monitoring and operational procedures. Those procedures
and their actual fault evidence remain required before deployment.
