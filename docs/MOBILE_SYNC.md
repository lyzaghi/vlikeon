# Optional mobile account and synchronization source

The native local service now connects create/register/recover, device certificate
renewal, device listing/revocation, remote account deletion, local disconnect and
explicit save synchronization to the shared encrypted sync client. A profile
must be opened with its trusted native credential provider. Mobile hello then
declares `encrypted_sync` and the `native_scheduled` policy; opens without a native
provider keep sync unavailable. The shared localized settings UI exposes account,
recovery, explicit foreground runs and separate background scheduling consent.
This is authored source, not a completed or qualified mobile sync product. Native
OS scheduling and deadline/lock gates are described in `MOBILE_BACKGROUND_SYNC.md`.

Creating an account still stores only delegated device/save-data secrets and
returns the root recovery code once, before a separate registration step requiring
that code. Root-bearing requests remain excluded from result caches and their
consumed typed secrets use zeroizing owners. The native Keychain/AndroidKeyStore
providers and their platform limits are in `MOBILE_CREDENTIALS.md`. No remote
identity/service operation or platform key API has been executed.

The native lifecycle gate starts inactive. A trusted native Game Hub visibility/
activity notification permits the next explicit account/run request; a renderer
command cannot activate it. A separate trusted OS start can admit background sync
only with persisted consent, registered/profile-bound identity and an idle lease.
One operation lease covers account changes and both sync modes. Concurrent work
returns Busy. Each lease gets a fresh cancellation flag. Foreground transitions
cancel background leases; background transitions cancel foreground leases. They
never clear an old flag, convert its mode or restart its worker. Credential access
checks the lease/lock gate, and root-operation HTTP requests recheck before send.
Shared reconciliation checks native access around crypto/list/download/publication
transitions. Disabling cloud sync persists first and requests cancellation.

One named native sync worker opens its own SQLite connection and uses existing
immutable save identities, encrypted staging, receipts, content matching, CAS
head/conflict preservation and intentionally removed-ID suppression. It does not
upload ROMs/BIOS or treat missing local content as permission to drop progress.
The profile's control connection remains available while a reconciliation job is
waiting on network I/O. The worker keeps one bounded current status snapshot with
the same counts/state shape as desktop, and status includes that snapshot so a
reloaded control page can recover the visible job. The UI pauses its polling when
hidden. The subsequent origin ledger introduced native schema 11; the current
profile schema is 13 for journaled payload publication. See `SAVE_ORIGINS.md`,
`SAVE_PAYLOAD_RECOVERY.md` and `SAVE_PUBLICATION_JOURNAL.md`. The reconciliation worker uses that profile database.

iOS uses view visibility and application active/resign/background notifications.
Android uses the trusted Game Hub tab's shown/hidden/channel events and the pinned
Chromium Activity resume/pause hooks; a background Activity cannot become active
merely because a channel handshake completes. A separate native lifecycle queue
delivers cancellation independently of serialized request/file I/O. Both native
queues coalesce current foreground state plus a retained must-cancel bit, so a
rapid leave/return preserves cancellation without an unbounded event backlog.
The C ABI notification obtains the lifecycle object independently of the host's
request mutex. A profile-open registry lock can still delay the notification;
it is not a hard real-time or process-fault guarantee.

The existing guest pause/save flush can complete later. Its delayed request does
not overwrite a newer foreground-resume notification or clear a new job's flag.
Channel detach still cancels its prior operation and revokes transient grants;
normal close marks the native entry closing, rejects new lookups and requests
cancellation. A closing profile entry is retained while its operation lease is
live, preventing same-process reopen over an unfinished worker. Subsequent native
close/open collection releases inactive entries and callback contexts outside
the global registry lock. At most four entries, including retained closing ones,
are admitted; no replacement worker is spawned merely because the old one is slow.

Cancellation is cooperative. Foreground blocking HTTPS operations have a 10-second
connect timeout and 100-second request timeout. Background requests are capped to
the remaining monotonic native opportunity deadline. An already-sent upload, signed
root action or acknowledged local transition can finish after a stop request;
this is not an atomic cancellation boundary. The current shared UI now uses a
separate bounded account worker declared by action protocol 1; its start/status/
collect commands do not await HTTP under the local host mutex. Older direct
commands can still occupy a compatible client's serialized I/O queue. See
`MOBILE_ACCOUNT_ACTIONS.md` for owner-bound one-shot results and independent expiry.
The lifecycle queue can signal cancellation but cannot forcibly interrupt
that send, a filesystem call, an OS credential operation or a hung driver. On
process death the in-memory job status disappears; committed saves, staging and
receipts remain the recovery inputs. Resumable binary transport is now authored
in `RESUMABLE_SYNC.md`; its runtime/device evidence, segmented-crypto design,
strict asynchronous transport abort, action reconciliation after lost replies, full resource/
quota recovery and lock/suspend/device qualification remain required.

The authored product behavior includes explicit foreground runs and optional
Android JobScheduler/iOS BGTaskScheduler opportunities. These sources have not
been built or executed and do not complete the synchronization requirement.
Platform scheduling/permissions qualification, store/privacy configuration,
production authenticated TLS hosting, recovery/backup/uninstall policies,
end-to-end cryptographic/fault/device evidence and all remaining browser/runtime/
streaming/release requirements still have to be completed. No sync service,
account/network/key/file/native lifecycle, app, browser, compiler, simulator,
device or test has been run. Cargo.lock was refreshed by offline fetch only.

The final consolidated pass must cover create/lost reply/register/recover, wrong/
expired/revoked identities, exact profile/provider binding, disabled defaults,
simultaneous account/run requests, foreground transitions before/during credential
and HTTP work, rapid leave/return and queued guest flush, view/channel/process
loss, status recovery, cancelled-worker retention/closing/reopen limits,
missing-content/inbox/CAS conflicts, local suppression, storage/network/TLS faults,
device lock/reboot and actual Android/iOS lifecycle/energy/store behavior.

Primary lifecycle contracts: [Android Activity callbacks](https://developer.android.com/guide/components/activities/activity-lifecycle),
[Apple resign-active notification](https://developer.apple.com/documentation/uikit/uiapplication/willresignactivenotification),
[Apple become-active notification](https://developer.apple.com/documentation/uikit/uiapplication/didbecomeactivenotification).
Chromium method anchors use the already pinned local Android snapshots.
