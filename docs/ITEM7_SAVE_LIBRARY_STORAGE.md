# Item 7 — saves, library and storage

The owner's current assignment is to finish item 7, then item 8. Source authoring
comes first; no build, test, workflow or Antigravity batch is executed in this
phase. Existing macOS work is preserved separately. This document records source
changes, not verified completion or permission to defer missing implementation.

## Implemented source in this pass

- Native library deduplication, media enrichment, import metadata and audit now
  share an immediate transaction. Two import owners cannot both publish from an
  earlier absent-hash lookup. Library removal checks persisted active sessions
  under that same write serialization, independently of the page's checks.
- Native current-save promotion checks active persisted sessions and rereads the
  selected metadata under the publication transaction after payload verification.
  A stale history result does not overwrite changed metadata or current heads.
- Web promotion captures the immutable Blob revision with the metadata it hashes,
  then rechecks that revision, metadata and live game leases in one strict
  transaction before changing the head. Native and Web both require stopping
  the game before selecting a different current generation.
- Native Settings has a separate user-reviewed unused-content collector. The
  existing Web collector keeps its own strict IndexedDB transaction semantics.

## Native unused-content contract

`storage_unused_protocol:1` advertises `storage_unused_preview` and
`storage_unused_remove`. The cursor is an optional canonical SHA-256; removal
requires the exact reviewed revision. Each page contains at most 64 asset files.
A complete bounded scan of games and saved-progress owners is required even
for a small page. Invalid owners, hash-shaped links/special files, active
sessions, an incomplete inventory or resource/time limits refuse collection.

Both raw and disc-bundle imports retain `library-assets.lock` from before any
copy through manifest publication. Collection holds that same nonblocking lease
and an immediate SQLite transaction through its reference scan and unlinks.
The PC host also rejects collection while a locally observed installed game
is running. A preview holds no long-lived lock while the user considers it;
removal rescans all references and compares its revision before touching files.

Only ordinary hash-named files inside the private profile's `assets` directory
can be candidates. The revision binds profile identity, cursor, candidate sizes
and timestamps, reference metadata and pagination. It is a change fence, not a
secret or proof against a malicious writer already controlling the profile.
Original selected files, firmware, saves, cloud objects, links, unknown names
and `.pending-*` imports are never collection targets. No startup, background
timer, game removal or Settings opening automatically deletes content.

Native multi-file unlink is not an atomic database operation: partial failures
return actual removed counts and retained counts, and the next preview observes
remaining files. Settings shows retained failures separately and reports an
unconfirmed durability observation instead of implying a fully durable removal.
Unknown retained entries are visible in the preview. Size is removed
file-reference bytes, not measured filesystem
space recovered. Directory sync and the audit/transaction outcome determine the
separate durability observation; Windows has no directory-flush claim here.
No compensating deletion or guessed accounting repair runs after an error.

The collector caps directory entries and game/save rows at 20,000, cumulative
game metadata at 32 MiB and each game record at 256 KiB. A cooperative 15-second
scan deadline is checked between operations; it does not preempt blocked kernel
I/O. Unsupported/corrupt historical content remains retained for explicit
recovery. Older app processes that do not use the asset lease require upgrade
coexistence qualification; source changes alone do not establish it.

## Completion gates still open

The subsequent owner request adds authored native/PWA multi-save archives,
retained raw-content reimport and native exact-match orphan payload repair.
See `BULK_BACKUP_AND_ORPHAN_RECOVERY.md` for the implemented limits and preserved
origin references. These source paths are not executed completion evidence.

Finish and review multi-file/import metadata reconstruction, full-profile
backup/database corruption treatment and remaining quota/accounting contracts against the
original product scope. Preserve uncertain originals rather than automatically
deleting unknown entries. Complete build/resource/source receipts for every
target, including the shared Web/mobile copies. These remain item-7 obligations,
not tasks moved to item 17 in order to claim completion.

Ten authored core fixtures (one Unix-only) cover retained live content,
explicit unused removal, reimport invalidation, import-lease exclusion,
unknown/pending preservation, persisted session exclusion, save promotion and
link uncertainty, 64-file pagination, changed candidates and corrupt ownership.
They have not been compiled or run. Final acceptance also
requires real fault injection around publication, removal, directory durability,
leases, full disk, partial unlink, browser transaction abort and profile upgrade.
Four additional authored fixtures in `backup_recovery_tests.rs` cover the new
archive/recovery paths and are likewise not compiled or executed.
