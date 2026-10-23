# Item 52 — storage fault and migration qualification

Status: open. Native batch `052a` passed 28 cases and Web batch `052e` passed
16 cases. Physical disk-full qualification remains pending the private VHDX.
Neither batch alone completes item 52.

The required matrix covers actual filesystem exhaustion, SQLite capacity
failure, interruption before and after save publication, aborted transactions,
restart/retry/cleanup and native/Web schema upgrades. Synthetic profiles and
fresh browser contexts are mandatory. A logical quota or simulated exception
does not prove an actual disk-full write. Process termination does not prove
power-loss durability or all physical storage devices.

## Executed fault matrix

Antigravity executed the frozen snapshots; Codex reviewed command exits,
assertions and unchanged inputs. Results are retained under
`qa/evidence/sequential/item-052/`.

| Requirement | Execution | Observed checks |
| --- | --- | --- |
| Native schema upgrades | `052a`, 16 cases | Actual migrations 0–15 to 15; existing save bytes, head, identity and settings preserved; reopen is idempotent |
| Interrupted native migrations | `052a`, four cases | SQLite `max_page_count` forces actual `SQLITE_FULL` in migrations from versions 10–13; old version/data remain; removal of the limit permits retry |
| Native transaction failure | `052a`, two cases | TEMP trigger abort and SQLite capacity failure; old save/head retained; staging cleaned after reopen; same save identity can retry |
| Native process crash | `052a`, six cases | Terminate the owned child at reservation, payload sync, metadata insertion, pointer publication, head update and committed return; reopen checks payload hashes, heads, foreign keys, integrity and staging debt |
| Web schema upgrades | `052e`, nine cases | Fresh schema 9 and upgrades 1–8 to 9; payload, heads, identities, settings, origins and revisions preserved |
| Web migration/lifecycle failure | `052e`, three cases | Aborted migration retains old schema/blob; blocked upgrade retries after old connection closes; version change closes stale handle and preserves newer schema |
| Web transaction cancellation | `052e`, one case | Abort after actual save insertion; metadata, blobs, origins, revisions, head and usage roll back; caller receives `cancelled` |
| Browser quota | `052e`, one case | Verified active 5-MiB CDP quota before opening IDB; 8-MiB save raises actual `QuotaExceededError`, maps to `resource_limit`, preserves old stores and retries after releasing override |
| Web renderer crash | `052e`, two cases | Crash actual save transaction after head write while held open, or after commit resolves; reopen preserves either all old stores or the complete new save/head/origin/revision/accounting |
| Physical NTFS full disk | Pending | Must observe filesystem `ENOSPC`, failed actual save publication, unchanged original data, cleanup and successful retry on the owned VHDX |

The native callbacks live only in the QA example; the SQLite `functions`
feature is enabled by a development dependency. No fault switches or SQL
callbacks are added to production APIs. QA uses synthetic game/save bytes.

Batch `052b` exposed a product defect: cancellation after a successful IDB
request could surface `TransactionInactiveError`, mapping to `storage_failure`.
`Store.transaction` now waits for rollback and checks the caller's abort signal
before classifying the exception. `052c` confirmed the fix but found a quota
fixture issue: lowering quota after seeding could reuse an earlier cached
space grant. The fixture now sets and verifies quota before opening IDB.
`052d` passed the earlier matrix; `052e` strengthens renderer crashes to use
actual save publication. Earlier failed runs are retained, not counted as
additional passing coverage. Chromium's bucket space cache is described in
its [implementation](https://chromium.googlesource.com/chromium/src/+/1c9524c5d3627044b0cf0a0b679978e82b702279/content/browser/indexed_db/instance/bucket_context.cc).

These are Windows native and headless Chrome `155.0.8059.85` observations.
They do not establish power-loss, filesystem-controller durability, other
OS/device/browser matrices, profile backup, corruption recovery or PWA
installation/update behavior; those remain in their numbered tasks.

## Private Windows disk-full fixture

`qa/windows/storage-fault-volume.ps1` prepares one new 256-MiB VHDX, mounted as
a folder beneath the explicit QA workspace
`E:\V10\work\vlikeon-items-50-109-01a16780\full-disk-052`. It first creates and
attaches that exact image, then resolves the associated disk and checks its
size, raw partition state and non-system/non-boot identity before initialization
and formatting. No host disk is selected for formatting. Existing workspaces
are refused. A receipt binds the image, mount folder, volume identity and size.
The helper refuses linked ancestors. Failure after attachment detaches the
owned image; the image and diagnostic evidence remain retained.

Administrative Windows confirmation is required for attachment; ordinary
Codex execution on this machine currently has a non-administrator token.
The owner authorized the UAC step. The first preparation launches exited before
creating the volume workspace: Windows PowerShell reported that script
execution is disabled. No image was created or attached. Read-only diagnosis
found no configured policy scopes, so the effective default blocks `.ps1`
execution. A separate request to allow `RemoteSigned` **only for the preparation
process** is pending; no execution policy has been changed.

Preparation command (UAC and a permitted script execution policy required):

```powershell
powershell.exe -NoProfile -File E:\V10\OmniPlay\qa\windows\storage-fault-volume.ps1 -Mode Prepare -Workspace E:\V10\work\vlikeon-items-50-109-01a16780\full-disk-052
```

`qa/windows/storage-full-disk.cjs` is prepared but has not run. It resolves the
receipt to the attached non-system image/partition, validates NTFS volume
identity and mount path, and checks capacity before every bounded filler
write. It stops at 320 MiB maximum, requires actual `ENOSPC`, releases only an
owned 1-MiB reserve for bookkeeping, then attempts an 8-MiB save that cannot
fit. It requires unchanged original data, releases only its filler, reopens,
checks integrity/cleanup and retries the same save identity. Logs remain on
the host volume. It must not reinterpret an unmounted folder as the test
volume. Antigravity executes the fixed test commands; Codex alone diagnoses
failures, changes product code and accepts this item.

Detach is explicit and preserves evidence:

```powershell
powershell.exe -NoProfile -File E:\V10\OmniPlay\qa\windows\storage-fault-volume.ps1 -Mode Detach -Workspace E:\V10\work\vlikeon-items-50-109-01a16780\full-disk-052
```
