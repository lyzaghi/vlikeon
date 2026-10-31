# Full-Profile Backup, Corrupt Database Recovery, Disc Reconstruction & Physical Quota

This document records the architecture, envelope contracts, IPC protocols, and fail-closed safety invariants implemented for Items **46–49** across [`omniplay-core`](file:///e:/V10/OmniPlay/platform/core/src/lib.rs), [`omniplay-contracts`](file:///e:/V10/OmniPlay/platform/contracts/src/ipc.rs), [`omniplay-broker`](file:///e:/V10/OmniPlay/platform/broker/src/service.rs), [`omniplay-local-service`](file:///e:/V10/OmniPlay/platform/local-service/src/host.rs), and iOS export delivery ([`HubBridge.swift`](file:///e:/V10/OmniPlay/apps/ios/OmniPlay/HubBridge.swift)).

Accepted checkpoint evidence for manual data recovery (**Item 50**, [`MANUAL_DATA_RECOVERY.md`](file:///e:/V10/OmniPlay/docs/MANUAL_DATA_RECOVERY.md)) and save-only `OPBK0001` archives ([`save_backup.rs`](file:///e:/V10/OmniPlay/platform/core/src/save_backup.rs)) remains untouched and backward-compatible.

---

## 1. Streamed Full-Profile Backup & Restore (`OPPF0001` — Item 46)

Implemented in [`profile_backup.rs`](file:///e:/V10/OmniPlay/platform/core/src/profile_backup.rs).

### Envelope Format
- **Magic:** `OPPF0001` (`[u8; 8]`), written with invalid magic `OPPF0000` during streaming and atomically flipped to `OPPF0001` + `sync_all()` only after every payload and the trailer digest are flushed.
- **Header (`[u32 LE len][JSON]`):**
  - Captures `settings`, `physical_quota_bytes`, up to `10,000` games (`manifest`, multi-disc `media`, `favorite`, `playtime_ms`, `last_played_ms`, and per-game `session_json`, `input_json`, `touch_json`, `core_options_json`), `firmware_assets` + `firmware_selections`, up to `20,000` save generations (`metadata`, `origin`, `prior_saves`), `save_heads`, `locally_removed_saves` suppression markers, and the deduplicated `blobs` manifest (`save`, `firmware`, and optional `asset` ROM/disc blobs when `include_rom_assets = true`).
- **Streamed Payload Records:**
  - Each unique `(kind, sha256)` blob is streamed in 64 KiB chunks (`[u8 kind][32-byte sha256][u64 LE bytes][raw bytes]`), verifying both byte length and SHA-256 against the declared manifest while incrementally hashing the entire archive under domain `b"Vlikeon full profile backup v1\0"`.
  - Before finalizing `OPPF0001`, [`export`](file:///e:/V10/OmniPlay/platform/core/src/profile_backup.rs) re-evaluates the live SQLite mutation fingerprint so concurrent writes cannot produce a torn snapshot.
- **Pre-Validated Restore:**
  - [`inspect`](file:///e:/V10/OmniPlay/platform/core/src/profile_backup.rs) and [`restore`](file:///e:/V10/OmniPlay/platform/core/src/profile_backup.rs) validate header invariants, every streamed blob digest, and the 32-byte trailer signature **before** mutating any local file or SQLite row.
  - Refuses restore when any game session is active (`ErrorCode::Busy`) or when missing blobs exceed [`enforce_physical_quota`](file:///e:/V10/OmniPlay/platform/core/src/storage_accounting.rs) (`ErrorCode::DiskFull`).
  - Stages missing blobs through crash-safe `PendingFile`, merges settings/quotas/suppression markers/games/firmware, and imports save branches deterministically without overwriting diverging local progress.

---

## 2. Corrupted SQLite Diagnosis & Quarantine Recovery (Item 47)

Implemented in [`database_recovery.rs`](file:///e:/V10/OmniPlay/platform/core/src/database_recovery.rs) and wired pre-open in [`platform/broker/src/main.rs`](file:///e:/V10/OmniPlay/platform/broker/src/main.rs) so even an unopenable `omniplay.sqlite3` can be diagnosed and recovered over the native host channel.

- **Read-Only Diagnosis (`DatabaseHealthCheck`):**
  - Classifies `omniplay.sqlite3` into `Missing`, `HeaderCorrupt`, `Unopenable`, `NewerSchema`, `IntegrityFailed`, or `Healthy` using header magic (`"SQLite format 3\0"`), `PRAGMA user_version`, `PRAGMA quick_check(16)`, `PRAGMA foreign_key_check`, and core JSON/identity invariants.
- **Zero-Loss Quarantine & Salvage (`DatabaseRecoverCorrupt`):**
  - Acquires `database-recovery.lock`; refuses `Healthy` and `NewerSchema` (fail-closed against downgrading newer schemas).
  - Performs best-effort read-only salvage of `device_id`, `settings`, `physical_quota_bytes`, `locally_removed_saves`, disk-verified `firmware_assets` + `firmware_selections`, disk-verified `games` + per-game preferences, and disk-verified `saves` + `save_heads`.
  - Atomically moves `omniplay.sqlite3`, `-wal`, and `-shm` into `corrupt-db-quarantine/<timestamp>-<uuid>/` with `quarantine-receipt.json` recording SHA-256 digests (never deleting user files). On Windows, [`Database::recover_live`](file:///e:/V10/OmniPlay/platform/core/src/database.rs) swaps out the open SQLite connection handle before renaming.
  - Initializes a clean schema-15 database, restores salvaged rows, runs [`reconstruct_unambiguous_all`](file:///e:/V10/OmniPlay/platform/core/src/library_reconstruct.rs) over orphaned `assets/` blobs, and reattaches salvaged save generations whose `game_content_hash` matches reconstructed games.

---

## 3. Imported Game Metadata & Multi-File Disc Reconstruction (Item 48)

Implemented in [`library_reconstruct.rs`](file:///e:/V10/OmniPlay/platform/core/src/library_reconstruct.rs).

- **Content-Addressed Inspection Under `AssetLease`:**
  - Scans `<sha256>.bin` blobs in `assets/` not referenced by `games`, verifying full SHA-256 digests and refusing symlinks/reparse points.
- **Multi-File `.cue` & Multi-Disc `.m3u` Pairing:**
  - Parses normalized `.cue` sheets (`disc.bin` / `track-NN.bin` / `.wav`) and `.m3u` playlists (`disc-01.<cue|chd|iso|pbp>` .. `disc-08.*`).
  - Cross-references candidate [`GameMedia::identity_hash()`](file:///e:/V10/OmniPlay/platform/contracts/src/library.rs) permutations against surviving `saves` and `locally_removed_saves` (`game_content_hash`) to bind exact track/disc ordering automatically when proven by saved progress.
  - When multiple raw track candidates remain ambiguous without save-hash proof, sets `needs_track_binding = true` and requires explicit `track_bindings` in `LibraryReconstructCommit` rather than guessing.
- **Single-File Container & Header Detection:**
  - Reconstructs `Chd` (`MComprHD` tag inspection for PS1 vs PSP), `Pbp` (`\0PBP`), `Cso` (`CISO`), ISO-9660 sector-16 `CD001` (`"PSP GAME"` vs `"PLAYSTATION"`), `N64Z64`/`N64V64`/`N64N64` internal 20-byte ROM title, `Nes` (`NES\x1a`), `Gba` (`0x96` fixed byte + 12-byte title), `Nds` (CRC16 logo check + title), `Gb`/`Gbc` (`0xCE 0xED 0x66 0x66` Nintendo logo + CGB flag), `MegaDrive` (`SEGA ` @ `0x100` + domestic/overseas title), and `Snes` LoROM/HiROM checksum complement scoring.

---

## 4. Physical Storage Quota & Data-Retaining Reference Accounting (Item 49)

Implemented in [`storage_accounting.rs`](file:///e:/V10/OmniPlay/platform/core/src/storage_accounting.rs).

- **Physical Block Allocation Scanner:**
  - Measures true physical disk allocation across all 7 profile namespaces (`database`, `assets`, `repaired_saves`, `legacy_saves`, `firmware`, `native_work`, `corrupt_db_quarantine`) using `MetadataExt::blocks() * 512` with `(dev, ino)` hard-link deduplication on Unix and 4 KiB cluster-aligned physical sizing on Windows, failing closed on symlinks or Windows reparse points (`FILE_ATTRIBUTE_REPARSE_POINT`).
- **Active Data-Retaining Reference Graph:**
  - Tracks every active retainer category (`library_games`, `active_sessions`, `live_saves`, `save_heads`, `save_repair_files`, `save_legacy_payloads`, `save_payload_gc`, `firmware_assets`, `firmware_selections`, and `native_work_storage` including `unmaterialized_reserved_bytes`), and pins transient asset retainers inside [`asset_storage::snapshot`](file:///e:/V10/OmniPlay/platform/core/src/asset_storage.rs) so unreferenced-asset cleanup never sweeps blobs held by an active session or recovery workflow.
- **Quota Enforcement (`StorageQuotaSet` / `StorageAccountingReport`):**
  - Default physical quota `64 GiB` (configurable `256 MiB..=512 GiB` in `preferences["storage.physical_quota_bytes"]`), enforced before ROM/disc imports, firmware imports, save payload commits, full-profile backup restores, and native work reservations.
