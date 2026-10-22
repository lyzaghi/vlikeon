# Mobile account action worker source

The shared sync settings UI now prefers native account-action protocol 1 when
declared by `sync_status`. Create/register/recover, certificate renewal, device
listing/revocation, account deletion and disconnect execute on one named native
worker with its own database connection. The start/status/cancel/collect/discard
commands only coordinate that worker; they do not await its HTTP send while
holding the local host's control mutex. The existing single reconciliation worker
and account worker share one foreground operation lease. Slow account HTTPS
operations therefore no longer occupy the current mobile UI's serialized native
request queue. SQLite, credential access and bounded result encoding can still
delay control work and need qualification.

This source path extends the typed contract with a strict tagged account-action
enum. Audience strings have a 512-byte admission bound and recovery/registration
fields a 256-byte bound; the shared sync client then validates its exact HTTPS
audience, root key and signed-operation contracts. Debug output for the nested
action exposes only its kind. Start contains secret input and Collect can return
the one-time root code, so both commands are excluded from request/result caches.
Status includes kind/state/times, result availability/expiry, stop-request state
and a bounded existing error shape, never action inputs or recovery-code bytes.

Each action uses a host-generated UUID and the local host's current owner UUID.
Status, collection, cancellation and dismissal must match that owner and current
action. Collection also requires the trusted native foreground gate. No renderer
supplies a filesystem path, key-store name, profile or native callback table.
`sync_status` exposes the current non-secret descriptor so a lost Start reply can
be resolved instead of immediately creating a second operation. Desktop retains
its existing direct account protocol; it does not declare this mobile protocol.

There is at most one action worker and one retained action result per profile.
An action cannot be replaced, nor a new account/run operation started, while its
worker is live or an uncollected result remains. The worker releases the database,
credential-owner reference and foreground lease after its operation finishes.
The same worker then waits on its result condition variable; it does not spawn an
extra expiry thread. Collection, dismissal and detach wake it. A worker-live flag
retains single-worker admission until that tail exits. Failures with no result
exit immediately; a failed thread spawn drops the owned secret action/lease.

A successful encoded result is held in a zeroizing Rust byte owner, bounded to
256 KiB, for at most 120 seconds of monotonic elapsed time from completion.
Polling does not extend that lifetime. The worker's independent timed wait clears
an idle result, and status/collection/admission also check the same deadline
before returning it. Collection consumes it once; dismissal explicitly clears it.
The wait rechecks its predicate/deadline after notifications or spurious wakeups.
Process/OS suspension can delay physical memory clearing until execution resumes;
this is not a hard real-time or whole-process zeroization promise.

Consumed action recovery/registration strings are wiped by a native owned-action
drop guard, including validation/admission/spawn failures after that guard is
created. Creation serializes the one-time recovery reply, clears that temporary
string and stores only the bounded result owner. Native mobile response encoding
now also clears owned strings in secret response Values, and clears the temporary
untyped request-ID inspection tree. These measures do not wipe immutable Java/JS
strings, arbitrary serializer/crypto/OS copies or every malformed-input allocation.
The general secret-copy/error-path audit and real platform evidence remain due.

Detach cancels the prior action, discards its pending result and marks any future
completion abandoned. A running worker keeps its operation lease until it really
finishes, so a cancelled or detached request cannot silently admit a replacement
network worker. Normal close follows the same behavior and retains closing native
profile entries while work is active. Already-sent HTTP/root actions and committed
local transitions can still complete after a cancellation request. A successful
completion records its result even when stop was requested, making that known
outcome collectible by the surviving owner instead of pretending it rolled back.

The UI polls with bounded one-at-a-time requests, stops automatic polling while
hidden, cancels pending form actions on dialog close/hide and allows an existing
result to be collected or dismissed explicitly. A recovered creation result shows
the recovery dialog before refreshing other settings, then clears its local
temporary field. A one-shot Collect reply lost after consumption cannot return
that root code again. Creation has not registered any remote account at that
point, so the pending local account can be disconnected/recreated if the code was
lost or expired. The UI explains that recovery step rather than registering a
root the user has never retained. Later root-operation lost replies still need
server outcome reconciliation; cancellation is not an atomic undo boundary.

Existing direct account commands remain for compatible clients and can still
block their serialized request queue while awaiting HTTP. They do not become the
new UI path when protocol 1 is declared. Transport interruption, OS-managed mobile
background sync, broader scheduling/quota/recovery, store/privacy and all remaining
product/release requirements remain due. No account action, HTTP/service/key/file
operation, worker/timer/lifecycle, UI/browser/device, compiler or test has run.

The final consolidated pass must cover every action/field bound, cache/debug
redaction, lost Start/Collect replies, wrong owner/profile/UUID, simultaneous
account/reconciliation requests, slow HTTP alongside save/control traffic,
foreground changes and detach/close during work/result wait, cancellation with
both committed and rejected outcomes, independent idle expiry and no extension
on polls, spurious wakes/poison/fault paths, exactly-once collection/dismissal,
thread-spawn/tail admission, recovery-code non-registration/loss workflow,
form/visibility cleanup, legacy clients and actual Android/iOS devices.

Wait contract: [Rust condition variable timeout semantics](https://doc.rust-lang.org/std/sync/struct.Condvar.html#method.wait_timeout).
Related source scope: `MOBILE_SYNC.md` and `MOBILE_CREDENTIALS.md`.
