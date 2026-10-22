# Native mobile background synchronization source

Android and iOS now have authored OS scheduler integrations for optional encrypted
save reconciliation. This is source implementation, with no scheduler, Keychain,
Keystore, request, process, database, compiler, simulator, device or test run.
It does not qualify mobile synchronization or any release requirement.

## Consent and admission

The shared localized sync panel offers a separate background-sync checkbox only
when native status declares background protocol 1. Missing consent stays false.
The setting uses the profile's existing SQLite preferences table under
`mobile_background_sync_v1`; no schema migration or external scheduler credential
is introduced. Enabling requires foreground Game Hub access, a registered account
bound to this profile, and the master cloud-sync preference. Disabling persists
first and requests cancellation of the background lease. Disconnect clears the
consent, including when subsequent credential removal needs to be retried.

`sync_background_configure` changes this consent; it cannot start background work.
Native-only `om_mobile_background_policy/start/status` entry points accept no
renderer path, key, account, URL or callback. Start rechecks persisted consent,
master enablement, registration/profile binding and native credential availability.
The worker opens its own SQLite connection and rechecks the persisted enablement.
The existing content mapping, immutable identities, inbox, receipts, conflict
branches, cursor commit order and locally removed-ID suppression remain in use.
Only save reconciliation runs; no ROM/BIOS upload or root/account operation is
scheduled. Production authenticated HTTPS hosting remains outstanding.

## Lease, deadlines and cancellation

Account actions, foreground sync and scheduled sync share the same single
operation lease per open profile. A background start requires the Game Hub's
trusted lifecycle gate to be inactive. Native app visibility checks add a separate
application boundary. A foreground transition cancels a background lease; a
background transition cancels a foreground lease. Neither transition converts or
resumes an old operation. Foreground account/result collection rules stay intact.
Uncollected account results also prevent a new background run.

Each OS opportunity includes native preparation time in its 25-second wall-clock
budget. Rust accepts only a remaining budget from 1 to 25 seconds and independently
arms a condition-variable watchdog. The current request timeout is reduced to
the remaining monotonic deadline, including response-body reads under the pinned
reqwest contract; later requests recalculate it and refuse less than 250 ms.
Foreground requests retain their existing 100-second request/10-second connect
timeouts. Cancellation does not imply an atomic transport, cryptographic, database
or filesystem interruption. An already-sent upload can finish, and its durable
receipt is retained before moving to another operation. Existing immutable-object
conflict reconciliation covers a lost upload acknowledgement.

Every reconciliation phase checks lifecycle cancellation and the native
noninteractive lock gate, including before/after encryption, decrypting staged
objects, publication, and conflict recovery. Temporary plaintext save payloads in
these reconciliation paths use zeroizing Rust owners. Other OS, Swift, Java,
crypto-library and allocator copies are outside that claim. A lock notification
also requests cancellation of the native opportunity. There is no plaintext key
fallback or background root/recovery-key retention.

The returned UUID is a native background ticket. Status/cancel use that exact ID
and a cloned observer outside the LocalHost request mutex. An old ticket cannot
stop a newer foreground/background job. OS completion/expiration cancels the
ticket but leaves the worker, credential context and profile lease retained until
the worker actually exits. Close requests cancellation before waiting on the host
mutex. Closing-entry collection, four-profile admission and duplicate-open
exclusion continue to count retained workers. An old slow worker does not cause a
replacement run. Registry locks, OS APIs, DNS/runtime scheduling and process death
still prevent a hard real-time termination guarantee.

## Android

`BackgroundSyncService` uses Android's `JobScheduler`, without an additional
WorkManager dependency. One persisted periodic job requests a 15-minute interval
and 5-minute flex with network, battery-not-low and storage-not-low constraints.
These are scheduling requests, not guaranteed run times. The pinned manifest
already declares Internet, network-state and boot-completion permissions. The
authored service declaration requires `BIND_JOB_SERVICE`, is not direct-boot aware,
and is not a renderer message endpoint. The job carries no account, URI or secret.

Before replacing/cancelling its app-wide job ID, the service checks the stored
component, preserving an unrelated Chromium job on a future ID collision. Native
profile open, sync status and account/settings responses reconcile scheduling.
Job rejection or background restriction is reported separately from persisted
consent and cannot make local play/profile opening fail. A stored job rechecks
current eligibility at native start; stale jobs never authorize a connection.

Pinned `ChromeActivity` start/stop/destroy hooks track native-visible activities
with weak owners, independently of the Game Hub tab. The native task has sticky
cancellation before profile loading, while waiting in the I/O queue, and across a
late start reply. A dynamic system screen-off receiver cancels the current task;
the native credential check also queries user-unlocked/keyguard state before each
phase. A main-loop 25-second expiry and 500-ms status polls own exactly-once
`jobFinished`; `onStopJob` and destruction clear timers, receiver and ticket
without trying to finish an OS-stopped job. Periodic scheduling supplies later
opportunities rather than immediate failure retry loops. Other Chromium activity
types and real lifecycle/OS scheduling behavior remain final-pass qualification.

## iOS

The app registers `org.omniplay.browser.save-refresh` during launch using
`BGTaskScheduler`, declares it in `BGTaskSchedulerPermittedIdentifiers`, adds the
`fetch` background mode, and links `BackgroundTasks`. App/scene background events
and native policy responses reconcile a `BGAppRefreshTaskRequest` with an earliest
begin date 15 minutes later. iOS controls actual delivery and available runtime;
registration/submission/restriction status does not promise a run.

App scenes and OS tasks share one idempotently opened native primary profile.
A scheduler launch never creates a replacement profile when its identity is
missing/corrupt. Work is refused while the application is active or protected data
is unavailable. Native protected-data notifications maintain a fail-closed lock
gate without calling UIKit from credential worker threads. Keychain items retain
`WhenUnlockedThisDeviceOnly`, non-sync and noninteractive properties. Foreground
entry, protected-data loss, the OS expiration handler and a 25-second local timer
stop the opportunity. Ticket attachment preserves earlier cancellation; one main
actor owner completes the OS task once and schedules no parallel native worker.

## Native credential ABI revision

The callback table is now ABI 2, with a profile-bound, noninteractive `check`
callback returning permitted/locked/failure without returning a secret. The new
symbol is `om_mobile_open_with_credentials_v2`; old smaller tables cannot bind to
that symbol. Rust, C header, JNI/R8 and Swift declarations move together. The
renderer control protocol does not expose or register a credential provider.

## Required consolidated verification

No checks have been executed. The final pass must cover source/build/link/ABI
consistency; scheduler manifest/identifier/permission admission; consent defaults
and account changes; rejection/restriction/ID collisions; OS reboot/force-stop and
real scheduling; late open/start replies; expiry before/during credential/network/
crypto/SQLite/file work; multiple activities/scenes; locking/unlocking; app resume;
stale tickets; uncollected account results; bounded retained workers/watchdogs;
lost HTTP acknowledgements, partial transfer retry and oversized saves;
inbox/receipts/CAS conflicts; backup/uninstall/reinstall; resource/energy use; store
and privacy declarations; production TLS; security/device evidence. Resumable
large-object transport is now authored in `RESUMABLE_SYNC.md`; it is not yet built
or qualified. Strict asynchronous abort, segmented-crypto/resource design and
device/OS transfer evidence remain required. This milestone does not remove any
other original platform, engine, browser, streaming or release requirement.

Primary contracts: [Android JobService](https://developer.android.com/reference/android/app/job/JobService),
[Android job constraints/persistence](https://developer.android.com/reference/android/app/job/JobInfo.Builder),
[Apple background-task guidance](https://developer.apple.com/documentation/backgroundtasks/refreshing-and-maintaining-your-app-using-background-tasks),
[Apple task expiration](https://developer.apple.com/documentation/backgroundtasks/bgtask/expirationhandler),
[pinned reqwest request timeout](https://docs.rs/reqwest/0.12.28/reqwest/blocking/struct.RequestBuilder.html#method.timeout).
