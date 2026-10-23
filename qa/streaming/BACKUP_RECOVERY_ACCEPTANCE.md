# Backup and orphan recovery — future execution scope

Current evidence note: sequential item 51 now executes the native/PWA archive
interchange subset and archive boundaries described below; see
`docs/SAVE_ARCHIVE_INTERCHANGE.md` and
`qa/evidence/sequential/item-051/acceptance.json`. Other fault, orphan-inventory,
provider and platform cases in this historical plan remain unqualified. The
source-authoring restriction below records the original phase, not the user's
later authorization to execute the numbered tests.

Authored only. This is not a released Antigravity batch or test result. Keep the
owner's source-first phase; dispatch no commands from this document.

1. Execute the four core fixtures only after freezing source/dependencies.
   Bind results to the actual source. Check prior references and exact heads.
2. Native-to-PWA and PWA-to-native archive interchange, including imported,
   remote-origin and legacy saves, Unicode metadata, nullable fields, divergent
   canonical/local generation values and missing parents. Imported provenance
   must remain imported; complete original references remain descriptive.
3. Re-export a restored branch. Retain earlier metadata/origin/head references.
   Exercise 16-reference, 256-KiB-reference, 4-MiB-manifest, 64-entry and total
   128-MiB boundaries; reject overflow without dropping origin history.
4. Damage magic, manifest length, UTF-8, JSON fields, checksum trailer, each
   payload and trailing bytes. No structural/hash failure may import any entry.
   Partial writes must not receive successful export acknowledgement.
5. Concurrent restores, abort before/during/after publication, uncertain commit,
   full disk/quota and mixed available/missing/active games. Preserve committed
   successes, report per-entry failures and retry without duplicate branches.
6. Purge a restored branch, explicitly select the same archive and restore again
   under a new UUID. Preserve old removal markers. A merely damaged existing
   branch must not be silently overwritten or treated as a successful retry.
7. Retained-content preview with active imports, unknown/pending names, wrong
   types, links/reparse roots, changed revisions and corrupt hash-named bytes.
   Recovery must copy/reuse content and preserve its original source.
8. Native orphan payload exact hash/length match, no match, active session,
   known managed file, suppressed UUID, source replacement and wrong metadata.
   Repair only a fresh journaled copy for existing metadata; preserve source.
9. Paginate 64 inspected leaves and 128-MiB hash work, including a candidate
   deferred by the byte budget. Resume without losing it or falsely completing
   an inventory. Observe cooperative versus blocked-kernel time behavior.
10. Real native file-picker, mobile document-provider and PWA download/file-writer
    completion, cancellation and lifecycle behavior. Browser download initiation
    is not an fsync receipt. Finish all platform/storage qualification separately.
