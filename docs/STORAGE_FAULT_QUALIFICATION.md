# Item 52 — storage fault and migration qualification

Status: 48 fault checks passed: native `052a` (28), Web `052e` (16) and physical
NTFS full disk `052g` (four). Closure awaits Windows UAC confirmation and
verified detach of the owned QA VHDX. Item 52 remains unchecked until cleanup.

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
| Physical NTFS full disk | `052g`, four cases | Actual filesystem `ENOSPC`, failed 8-MiB save publication, unchanged original data, staging cleanup on reopen and successful same-identity retry on the owned VHDX |

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

Physical batch `052f` stopped before fixture writes: `DiskImage | Get-Disk`
bound a null `Number` under the ordinary token. `052g` instead verifies the
image's exact `DevicePath` against the receipt's `PhysicalDrive` identity,
then explicitly queries that disk number and rechecks size, boot/system
flags, partition access path, NTFS label and volume identity. It does not
grant administrative rights to the test or remove identity checks.

The 256-MiB image contained a 251,588,608-byte NTFS volume. The filler wrote
234,881,024 bytes before a real `write` returned `ENOSPC` (errno `-4055`),
with 192,512 bytes reported available. Releasing only the owned 1-MiB reserve
left 1,241,088 bytes: the actual 8-MiB save failed with `storage_failure`.
Its old save/hash/head and database integrity remained valid. The failed
attempt left one journaled staging entry, which reopen cleaned after filler
release. Retrying the same save identity then produced valid 8-MiB progress
and advanced the head; both original and new payload hashes were checked.

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
execution. The owner subsequently authorized `RemoteSigned` **only for the
preparation process**. Preparation succeeded using that process argument,
without changing account or machine policy. The preparation receipt and
diagnostics are retained in the evidence directory.

After `052g`, the filler/reserve files were removed. Detach of the exact owned
image was requested through a separate `diskpart` UAC operation, with no
execution-policy change. At this checkpoint Windows `consent.exe` is still
awaiting confirmation; no detach is claimed. The retained image and synthetic
profile remain at the explicitly named QA path. No second prepare or detach
operation should be started while that request is unresolved.

Preparation command (UAC and a permitted script execution policy required):

```powershell
powershell.exe -NoProfile -File E:\V10\OmniPlay\qa\windows\storage-fault-volume.ps1 -Mode Prepare -Workspace E:\V10\work\vlikeon-items-50-109-01a16780\full-disk-052
```

`qa/windows/storage-full-disk.cjs` passed in `052g`. It resolves the
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
