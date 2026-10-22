# Owned Linux session composition source

`platform/native-runtime/src/linux_session.rs` now composes the previously
separate authenticated package, immutable images, staged game/firmware inputs,
durable work journal and caller-provisioned profile delegation. This internal
source has not run. NativeRegistry does not select it and Linux native execution
remains unavailable. No compiler, database, package/ELF/hash admission, asset copy,
process, descriptor, thread, kernel, SDL, core/helper, browser/device or test ran.

## Ownership and launch

The composition requires the admitted cache and delegation to name the same
profile as its private work owner. It derives the complete actor/root/scratch/
save/UI reservation from the approved package, creates a fresh resource pair
through the work journal's durable before/after callbacks, and creates the two
private cross-connected media pipes. It builds the actor's fixed bootstrap and
trusted UI specifications from those typed owners, approved dimensions/CPU/
volume/haptics ceilings and whitelisted desktop environment. It uses no shell,
renderer-supplied program path or implicit host delegation grant.

LinuxWork attaches the exact staged input receipt/ledger/inodes, commits both
launch intents and records each actual process identity before its private
start grant. LinuxSession retains actor/helper guards before creating the
control-channel worker; failure drops the owned guards. Active immutable images
retain the shared cache through the existing resource reaper even when the
temporary package preparation owner disappears. Selected input descriptors and
actor/UI lease clones remain owned through actual pidfd and resource-tree exit.

The shared WorkerChannel now consumes a private WorkerProcess trait implemented
by the existing contained Windows guard and LinuxChild. Its framing, bounded
request/reply queues, sequence matching, finite response allocation, caller
timeout/cancellation, error redaction and nonterminal focus/storage behavior are
unchanged. It always signals the passed live owned process guard; an identity
read from disk cannot implement that guard or authorize a numeric-PID kill.
This is channel source reuse, not enabling a new product platform.

Before accepting a core session, the source requires the worker's hello to match
the approved runtime/library/version, software backend, media protocol 2,
brokered RGBA mode, input protocol 3 and no measurement collector. Checked file,
combined-save and portable-MD contracts must match the declared package. Missing
haptics support removes that capability; it never grants a measurement result.

## Core preparation, lifecycle and saves

prepare_core binds the original launched configuration and typed staged game
identity, validates the input profile and resolves the signed option policy
against the profile's saved core-option selection. It clamps input haptics to
the owner-selected launch ceiling and acknowledged capability. It constructs
the existing native worker's prepare request from the approved package and
staged primary entry; no unchecked filesystem path is introduced.

The reply must have bounded RAM/RTC/state sizes, the declared persistent-file/
combined-memory policy, a valid disc inventory and matching portable-MD profile.
Combined saves require a rollback snapshot. Playlist disc counts must agree.
The worker must be ready and its own window must still be open. Missing actual
save/disc support removes the corresponding optional capabilities. Any failed
worker preparation stops the owned pair and retains uncertain work/debt.

The internal session has bounded start, pause, resume, save, load, status and
stop operations. Start/resume first request the trusted helper's own window;
the existing core client still requires fresh actual focused input. A queued
owner request does not prove focus. Save/load enforce the existing 256 MiB
payload policy. Compatibility includes the actual core artifact, selected signed
options, selected user firmware hashes, system assets and persistent-file/
lifecycle policy, using the same effective configuration fields as Windows.
Status accepts only ready/running/paused and explicit close/playtime values.

Graceful stop has a finite worker-control deadline and then stops both owned
process guards. If exact actor/tree exit remains unknown, it abandons the blocked
channel worker rather than waiting indefinitely for a child holding a pipe.
The underlying process reaper keeps its resource owners. The composition does
not delete the work directory or release its copy debt based only on process
death: confirmed leaf/charge retirement and complete work recovery are still
needed. The normal SessionManager final-save/adopted RuntimeAdapter flow has
not yet been connected to this internal session.

## Remaining product work and final evidence

An actual Linux RuntimeFactory/RuntimeAdapter must compose Store's authenticated
release with profile cache, private work/staging, legitimately provisioned host
delegation and this session, then expose the full final-save/metrics/disc/close
flow. Host provisioning/migration, existing-tree crash recovery and bounded
confirmed reclamation, compositor activation-token/user-gesture handoff,
whole-profile physical accounting, compiled SDK/loader/core/driver/license/
device qualification and every original remaining product/release requirement
remain due. No installed catalog or platform capability was enabled.

The one final verification after all original source authoring must cover actual
GNU spawn/FD/gate/work ordering, package/cache/staging/profile association,
hello/prepare protocol and reply shape, option resolution, save compatibility,
focus/cancel/timeout/nonterminal error behavior, graceful/failing pair teardown,
blocked pipes, resource/lease/charge retention and full device/core behavior.
No runtime, ABI, security or performance result follows from this source copy.

`LINUX_RUNTIME_ADAPTER.md` now authors the explicit trusted factory and shared
RuntimeAdapter composition around this session, including normal save/adoption/
metrics/disc/close interfaces. The default broker still does not select it.
Actual delegation provisioning/startup, crash recovery/confirmed reclamation,
compositor activation and full platform qualification remain required.
