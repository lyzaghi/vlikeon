# Full-Profile Backup, Corrupt Database Recovery, Disc Reconstruction & Physical Quota

This document records the architecture, envelope contracts, IPC protocols, UI surfaces, and fail-closed safety invariants implemented for Items **46–49** across [`omniplay-core`](file:///e:/V10/OmniPlay/platform/core/src/lib.rs), [`omniplay-stream-input`](file:///e:/V10/OmniPlay/platform/stream-input/src/windows.rs), [`omniplay-contracts`](file:///e:/V10/OmniPlay/platform/contracts/src/ipc.rs), [`omniplay-broker`](file:///e:/V10/OmniPlay/platform/broker/src/service.rs), [`omniplay-local-service`](file:///e:/V10/OmniPlay/platform/local-service/src/host.rs), Game Hub Settings UI ([`library-recovery.js`](file:///e:/V10/OmniPlay/apps/game-hub/library-recovery.js), [`unused-content.js`](file:///e:/V10/OmniPlay/apps/game-hub/unused-content.js), [`app.js`](file:///e:/V10/OmniPlay/apps/game-hub/app.js)), Android SAF export delivery ([`NativeDocuments.java`](file:///e:/V10/OmniPlay/apps/android/java/org/chromium/chrome/browser/omniplay/NativeDocuments.java)), and iOS export delivery ([`HubBridge.swift`](file:///e:/V10/OmniPlay/apps/ios/OmniPlay/HubBridge.swift)).

Accepted checkpoint evidence for manual data recovery (**Item 50**, [`MANUAL_DATA_RECOVERY.md`](file:///e:/V10/OmniPlay/docs/MANUAL_DATA_RECOVERY.md)) and save-only `OPBK0001` archives ([`save_backup.rs`](file:///e:/V10/OmniPlay/platform/core/src/save_backup.rs)) remains untouched and backward-compatible.

---

## 1. Streamed Full-Profile Backup & Restore (`OPPF0001` — Item 46)

Implemented in [`profile_backup.rs`](file:///e:/V10/OmniPlay/platform/core/src/profile_backup.rs).

### Envelope Format
- **Magic:** `OPPF0001` (`[u8; 8]`), written with invalid magic `OPPF0000` during streaming and atomically flipped to `OPPF0001` + `sync_all()` only after every payload and the trailer digest are flushed.
- **Header (`[u32 LE len][JSON]`):**
  - Captures `settings`, `physical_quota_bytes` (informational snapshot of the source profile's configured quota), up to `10,000` games (`manifest`, multi-disc `media`, `favorite`, `playtime_ms`, `last_played_ms`, and per-game `session_json`, `input_json`, `touch_json`, `core_options_json`), `firmware_assets` + `firmware_selections`, up to `20,000` save generations (`metadata`, `origin`, `prior_saves`), `save_heads`, `locally_removed_saves` suppression markers, and the deduplicated `blobs` manifest (`save`, `firmware`, and optional `asset` ROM/disc blobs when `include_rom_assets = true`).
- **Streamed Payload Records:**
  - Each unique `(kind, sha256)` blob is streamed in 64 KiB chunks (`[u8 kind][32-byte sha256][u64 LE bytes][raw bytes]`), verifying both byte length and SHA-256 against the declared manifest while incrementally hashing the entire archive under domain `b"Vlikeon full profile backup v1\0"`.
  - Before finalizing `OPPF0001`, [`export`](file:///e:/V10/OmniPlay/platform/core/src/profile_backup.rs) re-evaluates the live SQLite mutation fingerprint so concurrent writes cannot produce a torn snapshot.
- **Pre-Validated, Lease-Held, Atomic Restore:**
  - [`inspect`](file:///e:/V10/OmniPlay/platform/core/src/profile_backup.rs) and [`restore`](file:///e:/V10/OmniPlay/platform/core/src/profile_backup.rs) validate header invariants, every streamed blob digest, and the 32-byte trailer signature **before** mutating any local file or SQLite row.
  - Refuses restore when any game session is active (`ErrorCode::Busy`).
  - Preserves the destination profile's configured local `physical_quota_bytes` (never auto-raising local quota from archive metadata) and enforces [`enforce_physical_quota`](file:///e:/V10/OmniPlay/platform/core/src/storage_accounting.rs) (`ErrorCode::ResourceLimit`) over missing bytes and file counts prior to staging.
  - Holds [`AssetLease::acquire`](file:///e:/V10/OmniPlay/platform/core/src/asset_storage.rs) across the entire restore lifecycle—from initial staging through SQLite metadata publication—and persists `preferences["profile_restore_staging_v1"]` as an active [`storage_accounting`](file:///e:/V10/OmniPlay/platform/core/src/storage_accounting.rs) retainer so concurrent or post-crash unreferenced-asset sweeps cannot delete newly staged `assets/<sha256>` blobs before `games` rows commit.
  - Stages missing `assets/<sha256>` and `firmware/` blobs via crash-safe `PendingFile`, unlinking newly staged blobs via `cleanup_failed_restore_staging` if staging fails.
  - Commits firmware metadata/selections, conservative `settings` merge, `locally_removed_saves`, `games` manifests + favorite/playtime + per-game `session`/`input`/`touch`/`core_options`, and removal of `profile_restore_staging_v1` inside **one** `TransactionBehavior::Immediate` SQLite transaction.
  - Reads save payloads via [`read_exact_archive_blob`](file:///e:/V10/OmniPlay/platform/core/src/profile_backup.rs) bounded strictly to each descriptor's exact `[payload_base + relative_offset .. + byte_length]` slice (verifying `byte_length <= MAX_SAVE_BYTES` and SHA-256 digest without overreading into subsequent blobs or the 32-byte archive trailer), importing save branches deterministically via `derived_profile_save_id` without overwriting diverging local progress.

---

## 2. Corrupted SQLite Diagnosis & Crash-Safe Quarantine Recovery (Item 47)

Implemented in [`database_recovery.rs`](file:///e:/V10/OmniPlay/platform/core/src/database_recovery.rs) and wired both at [`Database::open`](file:///e:/V10/OmniPlay/platform/core/src/database.rs) startup and pre-open in [`platform/broker/src/main.rs`](file:///e:/V10/OmniPlay/platform/broker/src/main.rs) so even an unopenable `omniplay.sqlite3` or an interrupted prior recovery can be reconciled, diagnosed, and recovered.

- **Read-Only Diagnosis (`DatabaseHealthCheck`):**
  - Reconciles any incomplete recovery journal first, then classifies `omniplay.sqlite3` into `Missing`, `HeaderCorrupt`, `Unopenable`, `NewerSchema`, `IntegrityFailed`, or `Healthy` using header magic (`"SQLite format 3\0"`), `PRAGMA user_version`, `PRAGMA quick_check(16)`, `PRAGMA foreign_key_check`, and core JSON/identity invariants.
- **Checksummed Multi-Phase Recovery Journal (`DatabaseRecoverCorrupt`):**
  - Acquires `database-recovery.lock` and refuses `Healthy` and `NewerSchema` (fail-closed against downgrading newer schemas).
  - Performs best-effort read-only salvage of `device_id`, `settings`, `physical_quota_bytes`, `locally_removed_saves`, disk-verified `firmware_assets` + `firmware_selections`, disk-verified `games` + per-game preferences, and disk-verified `saves` + `save_heads`.
  - Persists `salvaged-snapshot.json` inside `corrupt-db-quarantine/<quarantine_id>/` and advances checksummed `database-recovery-journal.json` (`journal_sha256` over domain `b"Vlikeon db recovery journal v1\0"`) through three explicit phases:
    1. `Quarantining`: moves `omniplay.sqlite3`, `-wal`, and `-shm` into `corrupt-db-quarantine/<quarantine_id>/` and writes `quarantine-receipt.json` (never deleting user files).
    2. `Rebuilding`: builds the replacement schema-15 database in isolated staging file `omniplay.recovering.sqlite3` ([`Database::open_staging`](file:///e:/V10/OmniPlay/platform/core/src/database.rs)), applies salvaged rows, runs [`reconstruct_unambiguous_all`](file:///e:/V10/OmniPlay/platform/core/src/library_reconstruct.rs) over orphaned `assets/<sha256>` blobs, reattaches salvaged save generations matching `game_content_hash`, and checkpoints/truncates WAL (`PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE`).
    3. `Publishing`: atomically renames `omniplay.recovering.sqlite3` to `omniplay.sqlite3` and removes `database-recovery-journal.json`.
  - [`reconcile_incomplete_recovery`](file:///e:/V10/OmniPlay/platform/core/src/database_recovery.rs) runs automatically on [`Database::open`](file:///e:/V10/OmniPlay/platform/core/src/database.rs) and [`recover`](file:///e:/V10/OmniPlay/platform/core/src/database_recovery.rs) to finish any interrupted `Quarantining` / `Rebuilding` / `Publishing` journal from `salvaged-snapshot.json` instead of silently creating an empty database.
  - On Windows, [`Database::recover_live`](file:///e:/V10/OmniPlay/platform/core/src/database.rs) swaps out the open SQLite connection handle before renaming, reconciles any incomplete journal on error, and only reopens `self.root` when `Database::diagnose` confirms `DatabaseHealthStatus::Healthy`.

---

## 3. Imported Game Metadata & Multi-File Disc Reconstruction (Item 48)

Implemented in [`library_reconstruct.rs`](file:///e:/V10/OmniPlay/platform/core/src/library_reconstruct.rs).

- **Content-Addressed Inspection Under `AssetLease`:**
  - Scans 64-hex-character `<sha256>` blobs in `assets/<sha256>` not referenced by `games`, verifying full SHA-256 digests and refusing symlinks/reparse points.
- **Multi-File `.cue` & Multi-Disc `.m3u` Pairing:**
  - Parses normalized `.cue` sheets (`disc.bin` / `track-NN.bin` / `.wav`) and `.m3u` playlists (`disc-01.<cue|chd|iso|pbp>` .. `disc-08.*`).
  - Cross-references candidate [`GameMedia::identity_hash()`](file:///e:/V10/OmniPlay/platform/contracts/src/library.rs) permutations against surviving `saves` and `locally_removed_saves` (`game_content_hash`) to bind exact track/disc ordering automatically when proven by saved progress.
  - When multiple raw track candidates remain ambiguous without save-hash proof, sets `needs_track_binding = true` and requires explicit `track_bindings` in `LibraryReconstructCommit` rather than guessing.
- **Single-File Container & Header Detection:**
  - Reconstructs `Chd` (`MComprHD` tag inspection for PS1 vs PSP), `Pbp` (`\0PBP`), `Cso` (`CISO`), ISO-9660 sector-16 `CD001` (`"PSP GAME"` vs `"PLAYSTATION"`), `N64Z64`/`N64V64`/`N64N64` internal 20-byte ROM title, `Nes` (`NES\x1a`), `Gba` (`0x96` fixed byte + 12-byte title), `Nds` (CRC16 logo check + title), `Gb`/`Gbc` (`0xCE 0xED 0x66 0x66` Nintendo logo + CGB flag), `MegaDrive` (`SEGA ` @ `0x100` + domestic/overseas title), and `Snes` LoROM/HiROM checksum complement scoring.

---

## 4. Physical Storage Quota & Data-Retaining Reference Accounting (Item 49)

Implemented in [`storage_accounting.rs`](file:///e:/V10/OmniPlay/platform/core/src/storage_accounting.rs) and [`platform/stream-input/src/windows.rs`](file:///e:/V10/OmniPlay/platform/stream-input/src/windows.rs).

- **OS-Level Physical Block Allocation Scanner:**
  - Measures true OS-reported physical file allocation across all 7 profile namespaces (`database`, `assets`, `repaired_saves`, `legacy_saves`, `firmware`, `native_work`, `corrupt_db_quarantine`) without synthetic directory cluster charges, failing closed on symlinks or Windows reparse points (`FILE_ATTRIBUTE_REPARSE_POINT`):
    - **Unix (Linux/macOS):** `MetadataExt::blocks() * 512` with `(dev, ino)` hard-link deduplication when `nlink > 1`.
    - **Windows:** [`windows_file_physical`](file:///e:/V10/OmniPlay/platform/stream-input/src/windows.rs) via `GetFileInformationByHandle` + `GetFileInformationByHandleEx(FileStandardInfo)` (`AllocationSize`, with `GetCompressedFileSizeW` fallback) and `(dwVolumeSerialNumber, file_index)` hard-link deduplication when `nNumberOfLinks > 1`.
- **Active Data-Retaining Reference Graph:**
  - Tracks every active retainer category (`library_games`, `active_sessions`, `profile_restore_staging`, `live_saves`, `save_heads`, `save_repair_files`, `save_legacy_payloads`, `save_payload_gc`, `firmware_assets`, `firmware_selections`, and `native_work_storage` including `unmaterialized_reserved_bytes`), and pins transient asset retainers inside [`asset_storage::snapshot`](file:///e:/V10/OmniPlay/platform/core/src/asset_storage.rs) so unreferenced-asset cleanup never sweeps blobs held by an active session or in-progress restore/recovery workflow.
- **Quota Enforcement (`StorageQuotaSet` / `StorageAccountingReport`):**
  - Default physical quota `64 GiB` (configurable `256 MiB..=512 GiB` in `preferences["storage_physical_quota_bytes_v1"]`), enforced with `ErrorCode::ResourceLimit` before ROM/disc imports, firmware imports, save payload commits, full-profile backup restores, and native work reservations.

---

## 5. Game Hub UI & Mobile Export Wiring

- **Game Hub Settings (`#view-settings`):**
  - [`ProfileStorageQuotaPanel`](file:///e:/V10/OmniPlay/apps/game-hub/unused-content.js) renders live physical vs. logical storage breakdown across namespaces, active retainer counts/bytes, and physical quota configuration (`storage_accounting_report` / `storage_quota_set`).
  - [`ProfileBackupAndRecoveryPanel`](file:///e:/V10/OmniPlay/apps/game-hub/library-recovery.js) drives `OPPF0001` export (with optional ROM/disc inclusion), archive inspection & restore (`profile_backup_export` / `profile_backup_inspect` / `profile_backup_restore`), SQLite health check & quarantine recovery (`database_health_check` / `database_recover_corrupt`), and orphaned `assets/<sha256>` metadata/multi-disc preview & commit with explicit track-binding selectors (`library_reconstruct_preview` / `library_reconstruct_commit`), mounted in [`app.js`](file:///e:/V10/OmniPlay/apps/game-hub/app.js).
- **Mobile Export Delivery:**
  - Android SAF export completion in [`NativeDocuments.java`](file:///e:/V10/OmniPlay/apps/android/java/org/chromium/chrome/browser/omniplay/NativeDocuments.java) (`finishExport` supporting `save_backup_export`, `profile_backup_export` up to `16 GiB`, and `manual_recovery_export`) and iOS export delivery in [`HubBridge.swift`](file:///e:/V10/OmniPlay/apps/ios/OmniPlay/HubBridge.swift).

## Round-2 review fixes (SOURCE_READY_UNVERIFIED — nothing below has been compiled or run)

- **Scoped restore capability.** `profile_backup_select_import` issues a single-use `FileAccess::ProfileBackupRead` grant (cannot be used by library/save/firmware import). `profile_backup_inspect` consumes it and stages the open file in `ProfileRestoreBroker` (owner-bound, TTL ≤ 5 min, max 4 pending) returning `restore_token`. `profile_backup_restore {restore_token, expected_archive_hash}` re-verifies length and the full-stream SHA-256 before any write; `profile_backup_cancel` and `permission_revoke` drop it. General `ImportRead` grants stay single-use and unchanged.
- **Staging retainers.** Per-archive rows `profile_restore_staging_v1:<archive_hash>:meta|chunk:NNNN` (≤512 hashes and ≤64 KiB per row, ≤64 chunks). Readers fail closed with `CorruptData` on oversized/malformed/incomplete rows, so asset cleanup and accounting never treat unreadable pins as absent. Interrupted restores of different archives keep separate pins.
- **Windows measurement quality.** The 4 KiB estimate fallback is removed. `StorageAccountingReport` carries `measurement_quality`, `volume_cluster_bytes` (from `GetDiskFreeSpaceW`), unmeasured/unverified counts and warnings; files without identity are never hard-link deduplicated; quota admission fails closed when allocation is unmeasured. The Game Hub panel shows these values.
- **Mobile profile import.** Dedicated `profile_backup_select_import` on Android/iOS with a symmetric 16 GiB cap (`MAX_PROFILE_ARCHIVE_BYTES`), streaming `OPPF0001` magic precheck, free-space/deadline/cancel checks. ROM (4 GiB) and firmware (128 MiB) limits are unchanged.
- **Journal interruption fixtures.** `#[cfg(test)]` fault points in `database_recovery.rs` (after journal write, between quarantine renames, mid-rebuild, before/after publish) with tests in `backup_recovery_tests.rs`, plus missing/tampered snapshot and journal cases. Not executed.

## Codex completion of review follow-up (2026-11-01 — SOURCE_READY_UNVERIFIED)

After `600c254`, the remaining authored coverage is added in `platform/core/src/backup_recovery_tests.rs`:

- `salvaged_save_heads_recover_after_every_injected_interruption` creates two parent/child save generations in slot 3 and explicitly selects the older generation as head. A foreign-key violation in an unrelated fixture session makes the profile fail diagnosis while the game, saves and head remain readable. The normal production `recover` path creates the snapshot and checksummed journal; no recovery API or snapshot serializer is replaced for the test.
- At all five fault points, the fixture requires the injected interruption itself, two complete original metadata records and the selected head in the persisted snapshot. After `Database::open`, it requires both exact payloads through `SaveStore`, original lineage in `SaveOrigin.imported_from`, new independent local save identities, correct kind/slot/generation, the selected older head, and per-save quarantine references. A second open must preserve those identities, origins, metadata and head without duplicates. The quarantined database and saved snapshot must retain their exact bytes across both opens.
- `corrupt_database_recovery_resumes_after_every_injected_interruption` retains the separate unreadable-database case. It now requires zero save/head rows, the original orphan payload at its original path with unchanged bytes, and unchanged game identity on the second open; the damaged database and WAL remain byte-for-byte in quarantine.

These fixtures have only been authored and reviewed against source signatures, migrations and recovery phases. `git diff --check` is a whitespace check, not compilation or test evidence. No compiler, test, assemble, benchmark or SSH command was run for this follow-up. The existing interruption mechanism returns an error with normal resource unwinding; hard process termination, power loss, real storage failures and device/filesystem qualification remain deferred. The restore-token cleanup and staging-count bounds from `600c254` also remain unverified by execution.

Deferred focused recovery commands (run from the repository root only when execution is resumed):

```powershell
cargo test -p omniplay-core salvaged_save_heads_recover_after_every_injected_interruption
cargo test -p omniplay-core corrupt_database_recovery_resumes_after_every_injected_interruption
cargo test -p omniplay-core interrupted_recovery_fails_closed_on_missing_or_tampered_snapshot_and_journal
```

The handoff ledger records a source checkpoint, not runtime acceptance. Items 46–49 keep `SOURCE_READY_UNVERIFIED`; accepted evidence for other items is preserved.
