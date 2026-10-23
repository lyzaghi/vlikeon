# Item 55 — virtual gamepad ownership source

Status: open; source authoring only. The owner deferred build/test execution
and moved this fork from item 54 to item 55. No compiler, test, driver install,
device operation, Linux build or Antigravity batch was executed for this work.
The existing product still advertises `gamepad:false` and the broker still
grants keyboard/mouse only. These additions are not completed integration.

## Private host ownership and full-state delivery

`platform/stream-input/src/gamepad.rs` adds a provider-independent owner for
one gamepad authorization group with up to four stable slots. The private
owner binds grant/game/pair/session/capture UUIDs, an epoch, and the captured
process creation identity and exact window. It has no serde entry point and
must not be populated from a renderer-supplied PID/window or peer observation.

Before device creation, native host consent must explicitly cover the peer,
game, player slots and the system-visible nature of virtual controllers.
An existing generic `input_allowed` certificate is insufficient. A trusted
host must verify current pairing, document/capture lifetime, process/window
identity, focus and local override status before authorization, on every
frame and on an independent watchdog tick. This module does not itself
observe OS focus, read pairing storage or create that watchdog.

The authorization lifetime is monotonic and at most 30 minutes. Each accepted
authorization consumes a strictly newer epoch on this group, including a
partially failed device-creation attempt. A group is scoped to one host session;
new-session construction does not replace broker/service replay protection.
Neutral states are written to each created device before active state. If any
creation/update fails, all created devices are neutralized and removed.
The input-gap clock starts at successful activation; the authorization deadline
still includes all device setup/readiness time.

Frames use explicit slot numbers, not connected-list positions. Duplicates,
unknown slots, invalid trigger/axis values or oversized snapshots retire the
group before accepting another frame. Every snapshot omits no held state:
an authorized slot absent from a snapshot gets a neutral report. All frame
validation precedes OS writes, but multi-device OS publication is not atomic.
A provider failure during a partial update retires the complete group.

Frame and reliable release-fence sequences share a positive increasing range
up to 10,000,000. A late frame at/below a successful release fence is rejected.
Process/capture/document/authorization loss, focus loss, local override,
deadline expiry or a gap over 250 ms retires devices. A stopped group cannot
silently resume old input. No keyboard shortcut fallback is supplied.

Neutralization failure does not establish release. If provider removal also
fails, actual device handles and slot reservations remain in `CleanupRequired`
for retry; the group refuses a new authorization. An acknowledged removal can
retire a device even if an earlier neutral write failed. Drop attempts teardown,
but cannot report a failed OS cleanup. Process-loss behavior must be qualified
in the real provider/helper rather than inferred from Rust object destruction.

## Linux uinput backend

`gamepad_uinput.rs` implements the provider trait for Linux x86-64/ARM64 source.
It opens only `/dev/uinput`, with existing OS permission, `O_NOFOLLOW`,
`O_CLOEXEC` and nonblocking mode. It verifies a character device with major 10,
minor 223 and uinput interface version at least 5. No sudo/chmod/udev rule,
module loading, elevated process, arbitrary device path or installer is added.
The pinned `libc` version already in the workspace lock supplies UAPI layouts
and architecture-specific ioctl construction; its target dependency/lock entry
was authored without running Cargo resolution or a build.

Creation uses `UI_DEV_SETUP`, `UI_ABS_SETUP` and `UI_DEV_CREATE`. Four signed
stick axes, two unsigned analog triggers and two hat axes are configured;
the first twelve standard Web Gamepad buttons map to Linux gamepad buttons,
and D-pad bits 12–15 map to hats. Opposing D-pad directions cancel. Reports
include a final `SYN_REPORT`. A partial/nonblocking write is treated as failure,
not a successful state update. No force-feedback/haptics, keyboard, mouse,
home-button synthesis or borrowed manufacturer identity is claimed.

The process-local slot mutex prevents two instances in the same process from
reserving the same slot. It is **not** global user/device arbitration. Signed
service packaging, single-owner IPC and cross-process reservation are still
required. Slot numbers are logical authorization slots, not a promise that
Linux evdev/SDL or a game assigns the same controller order. Removal requires
`UI_DEV_DESTROY` acknowledgment; failed destroy retains the descriptor/lease.
Final descriptor closure is a cleanup backstop that needs process-crash tests.

The design follows the kernel's [uinput interface](https://www.kernel.org/doc/html/latest/input/uinput.html)
and [gamepad specification](https://www.kernel.org/doc/html/latest/input/gamepad.html).
These interfaces create system-visible devices. Foreground checks and stop
policy do not make a controller exclusive to one game: other processes may
observe it while it exists. That limitation must be part of native consent
and supported product behavior, not hidden behind an ownership label.

Linux build/runtime remains deferred under item 53. Authoring this backend
does not install, build, load, enable or test it on any machine.

## Windows WinUHid candidate

`engines/gamepad-windows/` contains an original generic HID descriptor, report
encoder and private C adapter. `gamepad_winuhid.rs` bridges it to the private
provider trait behind Cargo feature `winuhid`, off by default. The unchanged
product input helper does not construct this provider. This is authored source,
not a driver approval or completed Windows gamepad implementation.

The candidate uses [WinUHid](https://github.com/cgutman/WinUHid) at immutable
commit `d6cebbef5c7909168d1f881185be8f607d6aefd4`, whose public header, client,
precompiled-header source, driver ABI header and MIT notice have explicit SHA-256
pins. The CMake recipe accepts an explicit local checkout, verifies raw file
bytes and copies only those selected files into a private build staging folder.
It never fetches, installs or elevates. A checkout with CRLF conversion must not
be silently accepted as the pinned raw bytes. Compile only from a new, trusted
build directory and admitted immutable inputs; hashes do not stop concurrent
writers or approve the separate installed kernel driver.

The library is statically linked. There is no DLL search, arbitrary device path,
renderer-selected identity, custom driver IOCTL entry point or impersonated
controller hardware ID. Vendor/product IDs must come from an assigned identity
in a future admitted package; no default or borrowed IDs are supplied. A nonzero
ID or driver interface version 1 is insufficient proof of provenance. Upstream
MIT attribution is recorded in `NOTICE` and `security/dependencies.json`; a later
build copies the full upstream license, which release packaging must retain.
That does not close the final dependency/driver distribution review.

The 16-byte input report carries twelve generic buttons, four symmetric signed
stick axes, two unsigned analog triggers and a null-state eight-way hat. Standard
D-pad bits map to the hat; opposing directions cancel. Neutral sticks/triggers,
no buttons and hat value 8 form the release state. This is a generic HID gamepad,
not XInput emulation, automatic standard browser mapping or an established game
compatibility claim. Trigger independence and four-controller ordering still
need real-game qualification. No output reports, vibration or accessory support
is implemented.

Reservation returns an opaque process-local token before any driver allocation.
The manager owns that token before the first neutral write creates/starts a
device. A separate `neutralize` provider operation ensures retiring an unused
reservation cannot accidentally create a controller. Each created device gets
a fresh container GUID at reservation, available to the parent before device
creation, and a GUID instance string. Start must be followed by a
successful observation of that exact container in the present HID inventory
before initial write succeeds; a one-second readiness loop is not a guarantee
that underlying OS/driver calls return within that time.

Retire prohibits further writes, attempts neutral and closes the private client
connection. The pinned upstream driver passes the container GUID to VHF and its
file-close callback invokes synchronous `VhfDelete(..., TRUE)`; this source review
does not prove behavior of an independently installed driver. The adapter also
queries the present HID inventory through [SetupAPI device properties](https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupdigetdevicepropertyw)
and matches [the exact container GUID](https://learn.microsoft.com/en-us/windows-hardware/drivers/install/devpkey-device-containerid).
Only an observed-present device followed by a successful absence query frees its
slot. Property/enumeration failures, inventory overflow and a device still
present keep cleanup pending. A creation attempt that was never observed present
retains its reservation even after close: absence alone cannot rule out delayed
enumeration. A cleanup retry can observe a late exact-container arrival and
subsequent absence. If it never becomes observable, resolving that case needs
external helper retirement and reviewed recovery; there is no automatic
force-release or PnP removal operation.

Rust drop attempts the same cleanup. If it fails, the bounded C++ registry keeps
the closing record, up to four slots, and a later reservation retries confirmed
cleanup. No still-active slot is reclaimed. This registry is process-local, not
global arbitration; helper process loss and delayed PnP removal remain unverified.
Tokens are monotonic and never reused; retrying a confirmed retired token is
idempotent and cannot operate on a replacement device in the same logical slot.
Several pinned upstream calls wait indefinitely. The library must remain disabled
until an independently supervised helper, exact provider/driver admission,
cross-process slot arbitration and crash/blocked-call cleanup are integrated.
The helper must not report successful revoke merely because it exited.

Future linkage requires the MSVC target matching the Rust architecture, the
authored `vlikeon-winpad` static library and its explicit native library search
directory, plus Windows `setupapi`, `hid` and `ole32`. There is deliberately no
Cargo build script that runs CMake or locates arbitrary binaries. CMake configure,
compiler/linker and all device operations remain deferred; these source recipes
have not been executed or accepted.

## Private helper lifecycle and parent lease

`gamepad.rs` now separates prepare/reservation from activation. `authorize`
still composes both for an in-process caller, while the private Windows helper
uses the explicit two-step flow. Provider cleanup identities must be non-nil
and unique. The Windows adapter exposes its reserved GUID before any device
creation and a read-only exact-container presence query for a future external
supervisor. UUID/native GUID conversion is explicit, including little-endian
Windows GUID fields. Linux does not yet supply this external identity contract;
the new supervised path refuses to invent one for that backend.

`gamepad_host.rs` composes that lifecycle with OS observations and a private
parent lease. Construction admits an initial 300 ms lease after a validated
parent prepare request. Thereafter, the independent pipe reader must renew it
with a strictly increasing sequence and the exact grant/game/pair/session/
capture/epoch/process/window binding. Each renewal must affirm the current
document, capture and pairing lifetime. EOF/drop, false lifetime flags, replay,
wrong identity or a missed deadline permanently close authority. Remote input
packets do not renew this lease, and late heartbeats cannot revive it.

The actor polls target ownership, foreground and local activity independently
of remote frames. There is up to 30 seconds for initial foreground/quiet
readiness, five seconds to acknowledge prepared identities, a fixed overall
authorization deadline and the existing 250 ms active-input gap. Once stopped,
the host object cannot rearm. Failed detach remains `cleanup_required`, retried
at most once per 100 ms by this orchestration; this is a retry cadence, not a
promise that an underlying OS operation returns within 100 ms.

`gamepad_windows_host.rs` holds the actual process handle, verifies creation
time and observes the exact foreground window/PID, visibility, minimization,
local keyboard/mouse activity and override keys through OS calls. The parent
cannot attest to those observations over IPC. It reuses the existing local
activity hook implementation's counter-only privacy behavior and limitations;
raw input, silent hook removal and local physical-controller coexistence still
need stronger qualification/integration where required. This observer currently
assumes one gamepad helper/authorization per process. Simultaneous keyboard and
gamepad forwarding must compose local observation/injection attribution in the
same trusted helper design; separate existing v1 keyboard and v2 gamepad helpers
must not be enabled together as if that integration were finished.

`gamepad_protocol.rs` and `gamepad_worker.rs` author private stdio protocol 2.
The existing 16 KiB framing bounds apply. Requests bind the entire owner, use
increasing request IDs and deny unknown fields. Prepare carries both absolute
grant/capture expiry and bounded remaining lifetime; the earlier limit becomes
a monotonic deadline including local guard setup. The reader handles heartbeats
without queueing behind provider calls; a full two-command actor queue closes
authority instead of blocking the heartbeat reader. Heartbeats consume request
IDs but have no reply. Prepare sends `complete:false` while waiting for the
target, then one final `complete:true` receipt for the same request ID. Status
requests can complete while prepare is waiting, so a parent must correlate by
ID and completion rather than assume reply IDs arrive in increasing order.

The parent must retain the exact prepared slot/container list **before** sending
Activate; an empty, changed, reordered or partial acknowledgment is refused.
Returned identity sets are optional: `null` means unknown and must never be
treated as an empty/removed set. An active receipt follows actual initial
neutral writes and present-container observation. Cleanup retains actual
unresolved identities; helper exit code 3 means cleanup is still required.
Neither exit code 0 nor any process exit can replace the parent validating the
known identity set and actual cleanup receipts. Without completed external
process supervision and journal integration, no product path may yet launch
this candidate.

The helper source entry is `--stdio-gamepad-v2`; it returns unavailable (exit 2)
unless built for Windows x86-64/ARM64 MSVC with the default-off `winuhid` feature
and explicitly linked library. No current broker uses it. The existing
`--stdio-input-v1` path and keyboard/mouse capability remain unchanged. The
private wire's native-consent boolean and provider IDs must originate in an
admitted native broker/package; they are not a renderer permission API.
The reader/actor design does not preempt a blocked driver call. Independent
process supervision, global slot arbitration, parent-side transport integration
and signed admission are still required. The source-only parent journal and
orchestration below do not activate the current service path.

## Durable parent ownership and recovery

`platform/core/src/stream_gamepad_journal.rs` stores one bounded native ownership
record in the profile's existing SQLite preferences table. It uses no schema
migration or separate filesystem journal. The record binds profile/device/client,
full game/capture/pair/session/epoch/process/window owner, exact helper PID plus
creation time, helper SHA-256, provider, logical slots and container GUIDs. A
fresh reservation UUID and increasing revision prevent a stale snapshot from
mutating a replacement, even if other identity fields happen to repeat. The
canonical JSON record is bounded to 8 KiB and four unique devices; oversized,
noncanonical, malformed or foreign-profile/device records block reuse.

Writes require an autocommit SQLite connection in WAL mode with `synchronous`
FULL or EXTRA. Reservation, activation intent, active presence and cleanup debt
commit independently with an IMMEDIATE transaction and exact previous-value
compare-and-swap. A connection already inside another transaction is refused;
an uncommitted outer transaction cannot yield an activation ticket. Uncertain
commit results require reloading the authoritative record. No successful disk
fault/crash test or power-loss durability claim is made for this new source.

The complete prepared identity set commits before activation intent, and intent
commits **before** any Activate request. Therefore a broker crash or lost pipe
reply cannot turn an attempted device creation back into an unused reservation.
Only a validated active response from the exact admitted child records prior
positive presence. An authenticated cleanup response can acknowledge an exact
subset's removal; `null` retains unknown ownership. A changed/foreign device,
reintroduced removed device or inconsistent pending/terminal set is rejected.
Even a complete removal receipt remains journaled until exact helper exit is
confirmed; this prevents a still-running process from creating again after
slot release.

Reconciliation never deletes debt by age, grant expiry, empty generic inventory
or child exit code. An unattempted reservation can be removed after exact helper
retirement. An attempted device requires an authenticated removal receipt or
prior positive container presence followed by confirmed absence. A device never
observed present stays retained; an OS query failure stays unknown. A late
positive observation is stored for a later absence check. Compare-and-swap
protects mutations if another broker changes the record during OS queries.

`platform/broker/src/stream_gamepad.rs` authors parent orchestration around this
journal and a required supervised `ParentChannel`. It checks final prepared
identity/slot order, commits intent before activation, validates reply owner,
request ID, completion, bounded reason and identity set, and preserves cleanup
on exchange failure. IDs must be minted in one serialized transport writer with
heartbeat IDs; the old Prepare ID is not the current wire counter. Input and
release share an increasing sequence, validate slots/axes before sending and
refuse authority observations older than 100 ms or from a different owner. A
slow activation-intent commit triggers another freshness check before sending.
Shutdown cancels writes/attempts bounded child stop before a potentially blocking
cleanup journal write; storage failure cannot grant another group.

`platform/stream-input/src/gamepad_process.rs` duplicates the actual launch
process handle with query/synchronize rights and binds PID plus creation time.
It checks termination on that original kernel object, so PID recycling does not
substitute another process's exit status. The optional broker feature
`windows-gamepad` joins that witness with the Windows exact-container presence
query for live-parent reconciliation. It does not enable any service command,
change the current keyboard helper, install a driver or launch a process.

The Windows transport and creation-time job/watchdog source is described below.
The actual native authority source and cross-profile/OS reservation remain
unfinished. The default service does not construct this supervisor or invoke
its journal; the explicit-feature recovery lane below is separate. App capability stays
false. One profile journal is not a machine-global lease, an OS attestation or
permission to clear another profile's device. Startup recovery must obtain a
positive process retirement witness; access refusal or PID lookup failure
alone must not satisfy the observer trait. Signed helper/driver admission and
native consent remain mandatory before wiring any runtime path.

Fifteen journal/coordination regression cases (one Windows-only) are authored behind `cfg(test)` for the later
Antigravity batch: stale reservation/replacement, reopen after lost activation
reply, never-observed absence and later presence/absence, cleanup receipt before
child exit, synthetic SQL write failure, outer-transaction refusal and unknown/
foreign pending sets, existing-device-only recovery connection and refusing a
future schema without migration (some checks share one case). They have not been executed. Fake
process/container observations test journal decisions only; they cannot qualify
Windows PnP removal, driver cleanup, disk-full or real process crash behavior.

## Windows private transport source

`gamepad_child.rs` authors a normal-user launch with an unnamed private Job
Object assigned through `PROC_THREAD_ATTRIBUTE_JOB_LIST` in `CreateProcessW`.
The helper starts suspended; the parent verifies membership, records the real
process witness, starts I/O/watchdog threads, then resumes it. The job has one
active process, a 128-MiB process memory limit and kill-on-close. It is not an
AppContainer or permission to execute an untrusted program. The caller must
admit the exact installed executable and supply its immutable verified file
guard before launch. Only private stdin/stdout pipes and NUL stderr are in the
explicit inherited-handle list; neither job/process handles nor unrelated broker
handles are inherited. The environment contains only SystemRoot/WINDIR and a
System32 PATH. No elevation, shell, driver installation or arbitrary argument
forwarding is supplied.

`gamepad_client.rs` authors concrete parent transport with separate reader,
writer and watchdog threads. One process-local permit bounds it to one transport
instance. Commands/replies use bounded channels of two entries and the existing
16-KiB length framing. The writer alone assigns all request IDs, including
automatic/explicit heartbeats, so heartbeats cannot overtake an allocated but
unsent command ID. It prioritizes an automatic heartbeat every 50 ms even under
continuous input commands, and sends them only after the worker's initial
waiting receipt and while native parent authority remains live.

That authority belongs to a separate `AuthorityLease` held by the broker's
independent native observer. Initial setup has a 300 ms lease. Refresh requires
the exact owner, increasing lease sequence, current document/capture/pairing and
a monotonic observation no older than 100 ms. Its expiry is observation time
plus 200 ms. Missing/false/replayed/stale renewal or dropping that lease closes
transport permanently; automatic heartbeats cannot extend it themselves. The
native observer is still not connected to the current service, so no timer or
peer frame may be substituted for actual current native authority.

The reader validates owner/protocol, reply shape, ID, pending operation and
deadline. Prepare allows one initial waiting receipt followed by one final
receipt; an unexpected reply or duplicate completion closes transport. The
watchdog separately checks authority, overall deadline, pending reply/write
deadlines, I/O-thread liveness and original child exit at an 8 ms polling target.
The initial Prepare receipt has five seconds; final preparation has 35 seconds;
other calls have caller-bounded timeouts up to five seconds; each pipe write
has a 200 ms watchdog deadline. Successful activation/input/release also sets
a separate 350 ms parent input deadline; status/heartbeat cannot renew it. This
is an independent stop fallback if the helper's 250 ms input-gap cleanup blocks
in the provider. A terminal cleanup/revoked receipt closes transport after
delivery, preserving that receipt for journal reconciliation.
The reader checks the old input deadline before extending it, and the caller
rechecks live authority after receiving a nonterminal reply. A late active/input
reply cannot revive an expired gap while waiting for the watchdog's next tick.
These are authored scheduling/timeout policies,
not measured guarantees about OS scheduling or driver cancellation.

Stop closes further writes, requests `CancelSynchronousIo`, terminates only the
owned job/child and waits at most one second per termination attempt. It joins
only threads already known to have finished. Cancellation is a request, not a
completion receipt; unresolved I/O retains shared ownership of the process-local
permit and executable file guard until the last thread/lease retires. That
prevents repeated launches from accumulating stuck transport threads or losing
the admitted executable guard. If the last owner still cannot confirm process
exit, one process-local tombstone retains the job and file guard and blocks new
transport launches until process restart/recovery. No force-release API is
provided; this conservative stop needs real OS fault qualification and startup
recovery integration. No helper exit, job termination or I/O cancellation
deletes the durable device journal. Device cleanup still requires exact process/
container evidence through the parent journal/reconciliation path.

`WindowsParentChannel` now adapts this client to the broker orchestration and
exposes a duplicate original process witness for recovery. The default app still
does not construct/launch it, provide native consent, refresh its authority or
advertise gamepad support. The WinUHid worker still requires explicit native
linkage and its default-off feature. Platform builds, process/job containment,
heartbeat timing, I/O faults, stalled driver calls and crash/PnP cleanup all
remain unexecuted and unaccepted. Local implementation follows the Microsoft
[process attribute contract](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute)
and [synchronous I/O cancellation contract](https://learn.microsoft.com/en-us/windows/win32/api/ioapiset/nf-ioapiset-cancelsynchronousio).

## Read-only Windows startup recovery source

`RecoveryProcessWitness` adds a separate path when the original launch handle
was lost with the broker. It accepts only PID/creation identity from the validated
native journal, not a renderer or peer request. It creates a fresh read-only
process snapshot using `TH32CS_SNAPPROCESS`, walks it to successful
`ERROR_NO_MORE_FILES`, requires the current broker itself to appear and bounds
the walk to 65,536 entries. An empty, truncated, failed or excessive inventory
is unknown and cannot establish retirement. No process names, paths, modules,
threads or command lines are retained.

A complete snapshot without the saved PID is positive disappearance evidence.
When that PID exists, recovery opens only query/synchronize rights and compares
its actual creation time. A positively observed different creation identity
means the saved instance has retired; it never uses the replacement's exit code
or terminates it. A matching identity retains the queried kernel handle and
waits for that exact object to signal. Open/query errors remain unknown, even
if a concurrent exit is likely; the next explicit recovery can retry from a
fresh snapshot. Nothing is inferred from wall-clock boot estimates, grant age,
`GetTickCount` subtraction or an assumed reboot. A possible timestamp collision
therefore retains ownership instead of clearing it.

The optional broker `recover_windows_gamepad` source loads the current bounded
journal, obtains this process witness and invokes the existing exact-container
reconciliation. It can clear only an unattempted reservation after proven helper
retirement, or attempted devices that have actual prior presence/removal evidence
and the required absence. Never-observed attempted devices remain unresolved.
Compare-and-swap still refuses a row changed during the OS queries. No input is
resumed, no driver/devnode/process is removed, and no privilege is requested.

Default builds do not invoke this function. Startup in an explicitly built
`windows-gamepad` Windows x64/ARM64 MSVC broker now schedules it on the dedicated
lane below, never the browser/UI thread. Cross-profile arbitration, OS query
qualification and real restart/PID-reuse/device testing remain due.
API source review follows Microsoft's [process snapshot contract](https://learn.microsoft.com/en-us/windows/win32/api/tlhelp32/nf-tlhelp32-createtoolhelp32snapshot),
[enumeration completion contract](https://learn.microsoft.com/en-us/windows/win32/api/tlhelp32/nf-tlhelp32-process32nextw)
and [process identifier lifetime](https://learn.microsoft.com/en-us/windows/win32/procthread/process-handles-and-identifiers).
No new build/test or OS recovery execution has occurred.

## Recovery lane and existing-database admission

`stream_gamepad_recovery.rs` starts one background recovery worker at broker
startup only for the default-off Windows feature/target combination. It accepts
the active broker's known profile directory, profile UUID and device UUID, with
no renderer-supplied path. A process-local permit bounds simultaneous recovery
to one thread. A still-running or blocked thread retains that permit; startup
does not manufacture a replacement on timeout or close. Internal state separates
running, complete, retained, failed and acknowledged cancellation. No renderer
recovery/force-clear API or controller permission is introduced.

The worker calls `open_recovery`, which opens an **existing** SQLite file read/
write without `SQLITE_OPEN_CREATE`, schema migration, directory creation or the
normal `Database::open` save/native-work maintenance. It rejects linked/reparse
root/database entries, mismatching device UUID bytes, wrong schema version and
non-WAL mode. The connection uses FULL synchronization, foreign-key/trusted-schema
settings and a 100 ms SQLite busy timeout. These path checks operate under the
existing trusted profile-root boundary; they are not a new race-proof OS file
admission service. Failure leaves the durable ownership row in place.

Broker close raises a cooperative cancellation flag and joins only a thread
already observed finished. The recovery observer checks cancellation before
process-retirement and container queries; unknown/cancelled queries cannot
establish device absence. A completed atomic journal transition is not rolled
back by a later cancellation. Windows API calls can still block, so neither
cancel nor drop is proof that the worker stopped; the single-thread permit stays
owned until its actual return. Error/status does not authorize slot reassignment.

This connects only the optional startup recovery path. Actual input activation,
native permission observation, machine/user-wide slot arbitration, driver/helper
admission and supported recovery UX remain unfinished. The ordinary product
still advertises `gamepad:false`; all new build/test execution remains deferred.

## Shared per-user arbitration source

`stream_gamepad_arbitration.rs` adds a separate fixed-name SQLite store under the
existing native application data root, shared by cooperating profile brokers.
The broker derives that root only from its validated `profiles/<profile UUID>`
directory; a caller cannot pass a different store to `GamepadSupervisor::prepared`.
Directory ACL/ancestor admission remains the install service's responsibility.
The API creates no directory, refuses linked/reparse directory/database entries,
checks an application ID and schema version before changing a foreign database,
and stores at most one canonical 8-KiB owner record. A single group reserves up
to four slots, matching current host authorization. It does not implement
multiple simultaneous groups, other-user arbitration or kernel driver access
restrictions against unrelated software.

An IMMEDIATE transaction serializes the singleton reservation across processes;
FULL WAL state survives closing a connection or process. The complete profile
prepared snapshot commits first, then the shared reservation. Profile activation
intent commits next and must be mirrored to shared state before Activate is
sent. Active presence and retained cleanup transitions are also mirrored.
Reservation UUID, profile/device/client, full owner, exact helper/artifact and
container list must match; revisions cannot go backwards. Expiry, Drop and
an absent in-memory broker do not delete the shared row.

These two SQLite connections do not form a distributed atomic transaction.
Any uncertain commit stops activation and retains ownership. `reconcile_retained`
now commits a complete profile retirement proof without immediately deleting it.
Only that current cleanup proof, with all devices acknowledged removed and exact
helper retirement, can release the matching shared row; profile deletion comes
afterwards. This ordering also handles a crash between the two cleanup commits.
A stale old group cannot delete a replacement group's row. A missing/corrupt
profile cannot force-clear shared debt; startup reports retained ownership rather
than treating that as successful cleanup.

The optional startup recovery lane now uses that shared store as well. Recovery
of a row belonging to another profile still needs the installed profile resolver/
recovery UX; this lane does not follow arbitrary paths or clear foreign owners.
Admission after deletion/restoration/loss of the coordination store cannot assume
it is a new installation. The explicit debt-free initialization source below is
separate from ordinary open; debt-bearing repair, install/ACL admission and real
concurrent/crash/disk-fault qualification remain required before product activation.

Four additional regression cases are authored for later execution: two profile
connections competing for one persistent row, stale mirror refusal, correct
two-store release and replacement protection, and refusal after profile proof
was deleted too early. Combined with the earlier eight cases, twelve are
authored at that checkpoint; none has run. Current product capability remains false.

## Missing-store refusal and explicit initialization

Ordinary `Arbitration::open` now opens an existing store without SQLite CREATE
and rejects missing, empty or incomplete initialization. Startup/activation
does not silently replace a lost coordinator with an empty reservation table.
`initialize_new` is a separate native install/bootstrap API; current service
startup does not invoke it. It requires the existing private application root
and `profiles` directory, walks at most 4,096 canonical UUID profile directories,
and reads each existing profile database without creating or migrating it.
Unreadable/missing databases, unexpected entries, wrong schema/device identity
and **any** gamepad ownership row, even corrupt data, refuse initialization.

Only a debt-free inventory can reach `create_new` for the coordinator. An
existing file, including an incomplete earlier initialization, is never
overwritten or deleted. Failure after creation leaves that file for reviewed
native recovery. SQLite then initializes only that newly created ordinary file.
Cooperating current brokers cannot activate while the store is missing or its
header is incomplete. Mixed older brokers without these ownership rules are
outside this admission contract and must not remain active during install or
repair; exact binary/version admission is still required.

On Windows, each live store retains ordinary-file and directory handles allowing
read/write sharing but excluding FILE_SHARE_DELETE, plus OPEN_REPARSE_POINT
and directory BACKUP_SEMANTICS. SQLite can still share/lock the database, while
ordinary delete/rename/replacement requests are refused until those guards
close. The connection drops before its guards. This follows the Microsoft
[file sharing contract](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew),
but real NTFS/file-filter/SQLite interaction has not been exercised. Unix source
uses no-follow descriptors; those descriptors do not establish equivalent
delete/rename exclusion, so no Linux arbitration qualification is claimed.
All ancestor/ACL admission remains under the trusted install-root boundary.

Three additional regression cases are authored: missing-store open refuses
implicit recreation and bootstrap refuses overwriting; even corrupt profile
debt blocks initialization; and a Windows live-store rename must be refused.
There are now fifteen authored cases (fourteen on other platforms), all deferred.
No coordination file, profile inventory, driver or recovery operation was run
while authoring this checkpoint. Native bootstrap/repair UX and lost-store
reconstruction with actual retained devices remain unfinished.

## Revocation acknowledgment in the product source

`NativeInputForwarder` now stops forwarding synchronously but reports local
`revoking` until the matching native reply arrives. Reply validation binds grant,
game, pair, session, keyboard/gamepad capability flags and original expiry.
`cleanup_required`, a release failure, missing reply or changed native identity
retains a separate retiring capability and blocks another arm. Retry checks the
native status first; it does not silently create a new grant. A pending revoke
shares one promise, and stale frame/status/late authorization responses cannot
restore an already detached forwarding grant. An initial terminal arm result
cannot enable the remote channel. Document close still attempts teardown, but
cannot claim a device-removal receipt from page destruction.

The host panel distinguishes waiting for revocation, pending cleanup and unknown
cleanup, with an accessible status message and Check revocation action. New
authorization stays disabled until confirmation. The component worker no longer
forgets a grant just because a revoke RPC succeeded; it removes only matching
confirmed `revoked` results without `release_failed`, retaining status/retry
authority for unresolved cleanup. This does not delegate device permission to
the remote peer or change the advertised `gamepad:false` policy.

The shared status contract adds `cleanup_required`. In the existing keyboard/
mouse helper, `stop_confirmed` preserves the actual private Stop observation;
process exit/kill is not substituted for a released-state reply. The v1 broker
now retains `cleanup_required` after release failure or helper disconnection and
blocks new capture/input grants while that debt is present. Releasing capture
stops the input worker through the same acknowledgment path rather than simply
discarding its record. A capture/document shutdown is not a clean-device claim.
Legacy keyboard teardown still has unqualified process/I/O behavior and no new
durable held-key recovery across a complete broker loss; do not infer the v2
gamepad journal's guarantees for the old v1 keyboard process.

Seven asynchronous forwarder cases are authored in
`qa/browser/native-input-revocation.test.mjs` for a later frozen Antigravity run:
acknowledgment/deduplication, lost-reply retry, pending cleanup, foreign identity,
late frame completion, approval after close and terminal initial approval. They
use the actual forwarder source with fake native/RTC endpoints; they prove no
OS/device/browser behavior until executed and reviewed. Existing generated
component/browser resources must be assembled later from the authoritative
streaming source; that build and all tests remain deferred.

## Receiving browser sampling behavior

`engines/streaming/input.js::sampleStandardGamepads` now preserves browser
indices 0–3 as stable slots, including neutral gaps. Disconnecting slot 0 no
longer compacts a controller in slot 1 into slot 0. Only connected devices
with `mapping==='standard'` and the expected standard shape are sampled;
unmapped devices stay neutral until explicit calibration/protocol support.
Four sticks come from standard axes 0–3; analog triggers come from standard
button values 6/7, not arbitrary axes 4/5. Values are finite/clamped, without
retaining device IDs or names. Hotplug clears input through the existing
reliable release fence before subsequent snapshots.

The browser still polls gamepads only when the current session's remote host
policy advertises them. That policy remains false; this checkpoint does not
unilaterally grant the client control or bypass native host consent. Wire
`RemotePad` stays unchanged (six numeric values and sixteen digital bits).
Its future native adapter must map only authorized positional slots into
the explicit slot tuples accepted by the private group.

## Remaining implementation and later verification

1. Integrate provider admission, exact signed artifacts and host permission UI
   with broker/helper IPC and the authenticated capture/pair/session. Current
   `InputWorker` v1, status shape, native forwarder and host policy do not expose
   this provider yet. The private v2 actor/reader, parent orchestration and durable
   cleanup ledger and Windows supervised transport are authored, but the native
   authority source, service integration and combined keyboard/gamepad flow are
   not wired.
   Version the product path coherently before advertising support.
2. Finish the Windows candidate's supervised helper, assigned device identity,
   signed-driver admission, installed package and unresolved-creation recovery;
   implement the macOS provider/entitlement path. Explicit installation consent
   is required where needed. No existing Windows input helper gains gamepads
   from this module. Final provider license/provenance review remains open.
3. Qualify/wire the shared per-user arbitration store, installed common-root/ACL
   admission and recovery after coordination-store loss; complete foreign-profile
   recovery and local/remote controller coexistence. Preserve independent watchdog
   observations and supported system-visible device semantics. Actual cleanup
   receipts remain required before reassignment.
4. Add device readiness/admission, actual game-controller ordering and analog
   compatibility; remaining haptics/feedback and supported accessories are open.
5. When the owner resumes build/test, Antigravity executes frozen batches only:
   compile platform targets; verify actual neutral/press/release/trigger/hat
   output, four-controller hotplug, sparse indices, unauthorized peer/slot,
   replay/release fences, partial OS writes, stale focus/target, expiry, provider
   loss, failed removal, owner/helper crash and no retained device after cleanup.
   Codex reviews results and decides fixes/acceptance. No current pass is claimed.
