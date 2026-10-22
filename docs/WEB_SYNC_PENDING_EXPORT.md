# Explicit export of retained browser saved progress

The browser sync source now authors an explicit pending-backup export, including
when its original local save row or origin ledger is missing. It never recreates
that row, promotes a local encrypted wrapper to authenticated remote provenance
or silently overwrites existing progress. This is source only: no browser,
IndexedDB, cryptographic module, credential/key, file picker/writer, network,
service, compiler or test ran. Product crypto/service approval stays disabled.

## Explicit export flow

The sync settings panel offers Export pending saved progress only when its
connected browser backend advertises pending_export_protocol 1. The
user first chooses an ordinary export destination. The backend accepts just
that destination grant, refuses concurrent sync/account activity, then owns the
same exclusive foreground profile lease as normal sync. It loads the exact
profile/account/device/credential generation, requires the existing admitted
crypto artifact and unlocks only that generation. Its distinct local-export
purpose grants no upload, network, reconciliation or origin-write authority.
Cloud opt-in and an enabled service policy are not required for this user-
selected local copy. It makes no HTTP call and does not enable synchronization.

The retained upload snapshot is read without requiring a surviving local save
source. Journal admission still checks its account/device/header/schema/bounds
and hashes actual retained ciphertext bytes. The credential adapter authenticates
and opens it; the result must match the bound object UUID, exact payload length
and original canonical-or-legacy metadata fingerprint. The plaintext Blob is
hashed again in the bounded worker against its save payload SHA-256. Valid save
metadata stays within the existing 256 MiB payload/64 KiB metadata policy.
Temporary encrypted and opened binary ArrayBuffers are wiped in finally paths.
Blob copies and actual filesystem/browser memory remain subject to real final
memory qualification, not a claim of whole-process zeroization.

The exported document uses the existing OPSV0001 envelope, original validated
metadata and checked payload. It contains no sync account credential, recovery
key, transfer ID or service token. The ordinary user-driven import flow can
later preserve/remap local game identity and conflicts under its existing rules.
Export itself writes no save, canonical origin, receipt, transfer advancement or
payload revision and does not discard/rebind the retained upload. A generation
explicitly marked removed is refused rather than exported around its purge.

## Fences and delivery

The exact upload owner/header/fingerprint/revision and local credential context are
rechecked inside readonly transactions before opening, after opening/hash and
before delivery. WebFiles accepts an optional internal guard so native browser
file-handle delivery rechecks that context after writer creation and after
writing, before close. Cancellation or guard failure aborts the uncommitted
writer. Browser-download fallback checks before creating its ordinary visible
download link and reports only download_started semantics; it never claims a
disk fsync acknowledgement. An export the user already completed remains their
local copy; revoking sync cannot retract a previously delivered external file.

The browser hello response exposes `sync_settings_protocol: 1` separately from
network synchronization admission. Game Hub keeps the settings panel reachable
when `encrypted_sync` is false, so a locally admitted pending export is not
hidden by a disabled or missing service policy. This is a control-surface
protocol declaration, not approval of a cryptographic artifact or network
service. Current status still authorizes each action independently. Native
platforms without the new field keep their existing visibility behavior.

The panel omits unavailable account/service actions, keeps admitted local export
and disconnect accessible, and uses the distinct `consent_enabled` status to
keep withdrawal available even when network readiness has become false. An
account whose service is absent from the current allowed audience inventory
cannot offer an enable/run/register/renew/device/delete action through this UI.
The backend continues to enforce its own admission and audience checks. A
foreground panel that observes another request's active export polls its status
until completion, stops polling while hidden and resumes after visibility
returns. These source changes provide no runtime capability evidence.

The frontend uses the existing numeric 120-second bridge deadline, revokes the
one-shot destination grant after the request and distinguishes a started download
from confirmed file-handle delivery in Vietnamese/English. A no-pending-copy or
integrity/credential error keeps the retained encrypted data and existing progress.
The user can still stop/withdraw synchronization. The export has its own abort
controller/task; shutdown aborts and joins it before credential teardown ends.
Profile blur/pagehide or parent request cancellation cancels the foreground
lease. Account mutations cannot pass assertIdle while export owns preparation.

The readonly snapshot path audits existing journal shape and account/device
binding without constructing a writable synchronization journal or advancing its
revision. The strict local-export context has a different purpose and no cloud
consent epoch, so ordinary sync admission cannot accept it. Credential/profile
changes, foreground cancellation and the explicit purge fence still stop export.
Disabling cloud sync requests stop of any current export; the user can start a
new local export afterward without enabling cloud sync. It cannot unlock
staging belonging to another account/device or reconstruct a disconnected
credential. Offline account-key reattachment, fuller orphan recovery/import
UX, physical quota/orphan/ancestor/backup accounting and all original remaining
platform/runtime/browser/streaming/security/store/release work remain active.

After all original authoring, the one final verification must cover missing
local row/origin, canonical/legacy fingerprints, corrupt cipher/plain bytes,
explicit purge and concurrent opt-in/account/stage changes, local-only versus
network purpose isolation, disabled/missing service policy, native writer abort/
close, download fallback, cancellation/deadline/shutdown joins, export/import
roundtrip/conflicts and actual memory/storage bounds. Source inventories provide
no runtime or cryptographic success evidence.

The source now also exposes an explicit pending-export cancellation command and
active/stop-requested status. The localized panel shows an in-progress or
stopping message, disables conflicting actions while leaving withdrawal/stop
available, retains a local stop flag across destination selection and refreshes
after the request finishes. It opens the destination directly from the click
flow rather than inserting a storage preflight before the browser picker. The
backend abort request does not claim the writer has already finished or retract
a previously delivered file; shutdown still joins the actual export task.
