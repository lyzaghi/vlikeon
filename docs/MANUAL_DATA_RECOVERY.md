# Item 50 — manual recovery of bytes with missing metadata

Status: item 50 implemented and accepted against final isolated Antigravity
batch `050d`. Evidence: `../qa/evidence/sequential/item-050/acceptance.json`.

Settings exposes **Manual data recovery** when the host advertises
`manual_recovery_protocol:1`. Use **Inspect retained data**, select **Inspect
this copy**, review its namespace, filename, byte length, SHA-256 and first
32 bytes in hexadecimal, then **Export recovery package**. A changed source
requires a fresh inspection. **Extract a recovery package** verifies a selected
package before copying its payload to a separately selected output file.

This provides preservation and manual analysis when game/save metadata cannot
be trusted. The resulting raw bytes can be inspected with the appropriate
format tools, or explicitly imported through the normal game import workflow
after identifying the original system. Save payloads require trustworthy
compatibility metadata before the normal save import or repair path can accept
them. The recovery package itself grants no compatibility or provenance.
There is no automatic publication, save-head change or executable launch.

## Inventory and ownership

Native inventories ordinary files under the profile's `assets`,
`repaired-saves-v1`, and `saves` namespaces. Legacy saves may have one directory
level with an unknown game name. Unknown leaf names and zero-length files are
included; deeper directories, links, reparse points and special files are not
traversed. A refused root remains retained. Firmware, credentials, browser data
and unrelated directories are outside this inventory. Managed files may appear
alongside orphaned data; listing is not an ownership or deletion decision.

The opaque candidate ID hashes the private relative filesystem identity and
never accepts a path from the UI. Inventory/inspection/export acquire the
existing asset and save-payload leases. Inspection opens a regular file with
the existing no-follow policy and Windows write/delete sharing restrictions.
Leases coordinate product operations; they do not claim to contain an
adversarial owner replacing ancestors of a profile path.

Web/PWA uses a read-only IndexedDB transaction to capture asset `blob` and save
`payload` Blobs independently of their metadata. String keys need not be valid
content hashes or save UUIDs. The captured Blob remains immutable if its record
is later removed or replaced. Unsupported records/keys remain retained.
Checksums and package construction run outside storage transactions.

Both implementations cap inventory at 20,000 entries and 15 seconds, return
64 candidates per page, and allow inspecting/exporting payloads up to 4 GiB.
Oversized files remain visible with inspection disabled. Native streamed file
operations have a cooperative 90-second budget within the 120-second request.
The Web request deadline aborts Blob hashing. Limits refuse work instead of
truncating user data. There is no claim of a full-profile or database backup.

## Recovery package

`OPRC0001` is eight magic bytes, a little-endian u32 manifest length, UTF-8 JSON,
the exact payload, then a 32-byte SHA-256 trailer. The trailer hashes
`Vlikeon unverified recovery v1\0` followed by all preceding package bytes.
The manifest is at most 4 KiB and has exactly these fields:

| Field | Meaning |
| --- | --- |
| `schema_version` | `1` |
| `trust` | Always `unverified` |
| `source_kind` | `assets`, `legacy_saves`, `repair_files`, `web_assets`, `web_saves` |
| `display_name` | Descriptive leaf name/key, at most 240 UTF-8 bytes, no controls |
| `payload_bytes` | Actual length, 0 through 4 GiB |
| `payload_hash` | Lowercase SHA-256 of actual payload bytes |
| `prefix_hex` | Actual first min(32, length) bytes, lowercase hex |

Neither the label nor any manifest field is used as an output path. Output is
authorized by the existing one-use file grant. Native destinations are fresh,
empty selected files; existing output contents are refused. The component
worker checks that both import and export grants belong to the requesting tab.
UI cancellation, failure and page closure revoke grants in `finally` blocks.

Native export writes an invalid magic first, streams and rehashes the payload,
synchronizes the package, then publishes its magic and synchronizes again.
Extraction validates the complete package before its first payload write,
then rehashes the copy pass and empties a failed partial output when possible.
An I/O failure is reported; originals are never moved, overwritten or deleted.
Web extraction verifies the immutable package before handing its payload Blob
to the existing file writer/download path. Browser download initiation is
reported as initiation, not proof of durable file storage. Packages contain
raw user data and are not encrypted.

## Acceptance tracking

Assigned test-only Antigravity session:
`7e50d28b-3a5f-4cc8-b9b1-621393db4733`, owner
`01a16780-b30a-7152-a974-27d0ea351f79-items50`.
Final frozen batch: `050d`, with nine passing native fixtures, 13 passing
JavaScript cases, Windows checks of broker/local-service/mobile consumers,
repeatable generation of 444 resources (837 resolved or explicitly gated
imports), and Chrome 155.0.8059.85 UI/real-IndexedDB integration. Browser checks
cover inspection, export, extraction, changed sources, cancelled export,
grant retirement, no automatic publication and a 360-pixel layout. The file
picker bridge in that browser fixture is controlled; OS pickers and a full
Chromium product window are not claimed by that fixture.

Batch `050a` retained two QA path failures. Codex corrected the harness and
added writer-lease/junction checks in `050b`, then corrected responsive layout
and checked extraction in `050c`. Final `050d` reran all five groups after LF
normalization and resource regeneration; it supersedes earlier passes without
counting repeated tests as additional coverage. Source remained unchanged
during every recorded batch. Codex reviewed actual logs, source identity,
package rules, grant wiring and rendered desktop/mobile screenshots before
accepting this item. Original runs remain under
`E:\V10\work\vlikeon-items-50-109-01a16780\runs`.

Item 51 retains native/PWA save-archive interchange. Item 52 retains physical
disk-full, crash, transaction-abort and schema-upgrade qualification. This
manual tool does not supersede the existing save repair, group backup,
multi-file import reconstruction or whole-profile/database recovery work.
Linux/macOS/mobile device qualification, real OS file providers, hostile
ancestor replacement, power-loss durability and very large/slow device
qualification remain in their respective platform and consolidated gates.
