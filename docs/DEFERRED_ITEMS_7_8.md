# Items 7 and 8 — deferred by owner

The owner's latest direction is to record these gaps for later and proceed to
items 11 and 12. Deferral is not completion. Preserve all existing authored source.

## Item 7

- Full-profile/database backup, corruption diagnosis and recovery.
- Reconstruction of multi-file disc layouts and missing import metadata.
- Broader physical storage quotas and retained-reference/accounting limits.
- Manual recovery of data without trustworthy metadata; never guess save
  compatibility, lineage or authenticated provenance from raw bytes.
- Native/PWA archive interchange, failure/retry/purge behavior, filesystem
  durability, mobile providers and real device qualification.
- Group backup currently covers at most 64 saves / 128 MiB, and the UI exports
  the current history page. This is not one-click backup of the entire profile.

Source details: `ITEM7_SAVE_LIBRARY_STORAGE.md` and
`BULK_BACKUP_AND_ORPHAN_RECOVERY.md`.

## Item 8

- Linux/macOS owned capture, application audio and native input adapters.
- Signed virtual-gamepad integration and remaining device arbitration.
- Independent native encode service and hardware/codec qualification.
- Safe launcher-child ownership; the Windows binding covers direct owned games.
- Durable/mobile credential lifecycle and remaining relay/TURN deployment work.
- Fresh Chromium patch application, broker/helper/browser builds and actual
  capture/audio/input/reconnect/revocation/fault/device acceptance.

Source details: `ITEM8_REMOTE_PC_BRIDGE.md`, `OWNED_NATIVE_CAPTURE.md` and
`qa/streaming/`. No new build/test batch is released by this note.
