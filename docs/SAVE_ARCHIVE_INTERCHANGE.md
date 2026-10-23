# Item 51 — native / Web save archive interchange

Status: item 51 accepted against final frozen Antigravity batch `051g`, with
25 passing archive-exchange groups and four native backup/recovery regression
tests. Evidence: `../qa/evidence/sequential/item-051/acceptance.json`.

The existing group-save backup format is `OPBK0001`, distinct from item 50's
`OPRC0001` raw recovery packages. This qualification exchanges actual archives
between Rust `save_backup` / `SaveStore` and Web `WebSaveBackup` / `WebSaves`
backed by real Chrome IndexedDB. It does not substitute a second parser or mock
storage implementation for either product implementation.

## Required outcomes

| Requirement | Evidence to obtain |
| --- | --- |
| Native export restored in Web, and Web export restored in native | Actual binary archives, hashes, per-entry outcomes and verified payload reads |
| Capture, imported, authenticated-remote and legacy origins | Seed records and imported branch/reference inspection |
| Unicode, nullable fields and emulator-state bytes | Full semantic reference equality and payload SHA-256 |
| Remote canonical/local projection differences and missing parents | Canonical metadata retained descriptively, distinct local game/generation values |
| Preserve existing heads; never promote imported archives to remote authority | Head identities unchanged, imported origin on each new local branch |
| Retry the same archive | Same derived save IDs; no new records on healthy replay |
| Export restored branches again | Complete prior metadata/origin/head references survive each direction |
| Explicit purge then reimport | New derived IDs with old suppression markers preserved |
| Missing/active target games and damaged restored generations | Per-entry error outcomes; successful entries retained; damaged content not overwritten |
| 64 saves and 16 prior references | Valid boundary accepted, overflow rejected without silent truncation |
| Total 128 MiB and manifest bounds | Valid total boundary accepted, overflow refused before publication |
| Malformed, duplicate, truncated or corrupted archives | No history change on structural/integrity rejection |

The native QA endpoint in `platform/core/examples/backup_interop.rs` is a test
example, not a product command. The browser orchestrator is
`qa/browser/backup-interop.cjs`. Only synthetic game content and fresh profiles
are used; actual native and Web serializers, hash checks, branch creation,
origin handling, suppression and storage transactions execute.

## Findings so far

Initial batch `051a` exposed an inconsistent synthetic remote identity in the
QA seed; Codex fixed the fixture. Batch `051b` then passed eight exchange/retry/
head/origin/reimport/partial/damage groups and three rejection groups before
finding a product validation discrepancy: native accepted `missing_parent:true`
on legacy origins without canonical metadata. The same native validator also
needed to reject that flag when canonical metadata has no parent. Codex added
both checks, matching the existing Web origin validator.

Batch `051c` exercised the 64-save boundary successfully but exposed an invalid
nested `prior` field in the 16-reference fixture. Codex corrected that fixture
to use the specified metadata/origin/current-only reference schema. Batch
`051d` passed 23 groups, including actual 128 MiB total-boundary and overflow
exchanges plus malformed UTF-8/manifest-length rejection. `051e` added further
boundaries and found that the QA padded manifest used a character count rather
than a UTF-8 byte count. After that fixture correction, `051f` exposed a product
error-code discrepancy: Web classified an oversized retained origin reference
as `invalid_request`, whereas native returned `resource_limit`. Codex added an
explicit reference-size check before Web publication so both return the latter.

Final `051g` reran the complete 25-group matrix and all four native regression
tests. A real 4 MiB manifest is accepted; a 4 MiB + 1 byte manifest is refused.
A structurally valid archive with more than 256 KiB of retained origin history
returns a per-entry resource error without dropping references or adding a
generation. The 16-reference case preserves all references and refuses a later
export that would require a seventeenth. Both native and Web active-game
exclusion and missing-game partial outcomes are covered.

## Acceptance review

Codex reviewed actual terminal results, the native/Web fixture implementation,
per-entry native responses, binary archive identities, and 215 current source
inputs compared with the frozen manifest. The native endpoint is compiled from
the same frozen source and calls product APIs. Browser execution uses Chrome
155.0.8059.85 with real IndexedDB, WebSaves and WebSaveBackup. The tested source
remained unchanged throughout the final batch. The native regression suite
also covers ordinary archive roundtrip/replay, corrupt-before-import refusal,
active-session refusal and exact orphan-copy repair.

The evidence directory retains failures and the final pass, source manifest,
runner, request, executed logs, artifact hashes/sizes and bounded synthetic
archive samples. Large boundary archives stay in the local batch with hashes
and exact byte lengths recorded in acceptance; they are reproduced by the
checked-in fixture instead of committing hundreds of MiB of generated data.

## Qualification limits

This item establishes archive interchange for the tested format and the two
actual storage implementations. It does not establish full-profile/database
backup (item 46), disk-full/crash/schema-upgrade behavior (item 52), browser
product packaging or all target devices (items 65–76 and 106). Archived remote
origin records remain descriptive references: an imported local file cannot
create authenticated remote authority.

Raw batch results are retained under
`E:\V10\work\vlikeon-items-50-109-01a16780\runs`, using this chat's test-only
Antigravity session `7e50d28b-3a5f-4cc8-b9b1-621393db4733`. Codex owns diagnosis,
fixes and the completion decision.
