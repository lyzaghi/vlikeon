# Item 4 — macOS source completion

## Current revision: diskless guest, source activation authorized

The inherited host-core route described in the historical record below has now
been replaced in source by a Linux ARM64 diskless guest owned by the XPC process.
See `MACOS_DISKLESS_GUEST.md` for the actual current interfaces and limits.
The schema-2 catalog, fixed guest RAM/vCPUs, read-only input share, guest PID 1,
tmpfs overlay, embedded worker/core hash admission, owner VM lifecycle,
device-free guest transport and release assembly integration are authored.
Untrusted core descendants stay inside the guest; the native helper lifetime
reader is outside their address space. No native host-core fallback remains.

**Verified completion is not claimed.** After the earlier automatic review
rejection, the user explicitly authorized activation before qualification.
`kExecutionBoundaryComplete` is now true in source; all Rust capability and
signature/image/profile/resource checks remain. No binary was rebuilt or run.
No build/test/signing/notarization occurred. Actual
guest/core/dependency artifacts (the separately pending item 5), Mac boot,
crash/resource behavior and release qualification remain unverified. Unknown
legacy/no-provenance data is deliberately retained, never automatically deleted.

The user now requests completion of the remaining work followed by item 6.
Independent unblocked media fixes are tracked in `GRAPHICS_AUDIO_INPUT_ITEM6.md`;
the source activation is now implemented, and qualification remains deferred.

## Historical record before the diskless guest revision

The following source status and gap list describe the former host-child design.
The current revision above and `MACOS_DISKLESS_GUEST.md` supersede that design.

Current item: **4, in progress**. The owner's latest instruction is to finish
all source obligations in this item before moving to item 5. Do not move a
missing implementation to item 17 merely to advance the sequence. Practical
verification remains deferred by the owner.

No compiler, build, codesign, notarization, workflow, runtime, test or Antigravity
batch was run for the current worktree changes. Reading source, reviewing diffs
and reading primary implementation documentation are not target qualification.

## Authored composition

- A C ABI and Objective-C++ bridge provide bounded asynchronous XPC request/reply
  calls. Calls have deadlines; retained callbacks consume a process-wide limit
  until they actually retire. Replies are admitted with the public message-bound
  Security API, strict/all-architecture validity, the designated requirement,
  exact expected installed owner executable, hardened-runtime flag and closed
  owner entitlements before descriptor adoption. Rust contains unsafe FFI within
  the sandbox crate and returns owned Files rather than PID/signalling rights.
- The owner authenticates each message with `SecCodeCreateWithXPCMessage`, using
  default lookup flags and retaining strict/all-architecture validity against
  the sealed broker requirement. This replaces the former manual/private audit
  token lookup; it does not relax signature validation. Connection IDs saturate.
  Closed typed dictionaries have finite depth, item, string and control budgets.
- Each connection binds one immutable profile UUID. Concurrent profile ownership
  is exclusive. Staging lives below `NativeSessions/Profiles/<profile>/<session>`;
  four profile directories and four staged sessions per profile bound managed
  staging capacity. Receipt schema 3 also binds the profile. Legacy unscoped
  directories are not silently adopted or deleted.
- The owner has launch/status/stop/activation request implementations. Launch
  endpoints carry the existing bounded worker protocol, including checked binary
  save/load payloads; save bytes do not become unbounded XPC JSON. RuntimeAdapter
  composition supplies prepare, options, input profile, start/pause/resume,
  save/load, compatibility, disc control, observation, cancellation and metrics.
  SessionManager retains normal broker save history/final-save ownership.
- Registry admission reloads the authenticated catalog, binds all packages to
  one owner authority, compares the sealed owner catalog and installed mirrored
  actor/helper/core/assets against the outer release inventory, and rechecks
  package declarations before preparation. macOS has no development-root fallback.
  The broker has a dedicated macOS registry entry point.
- Four process-owner slots are reserved before pipes or spawn. Both direct
  children start suspended with reset signals and descriptor allowlists. Static
  bundle, live code, closed child entitlements and observed parent/start identity
  are checked before resume. Signal and WNOHANG wait ownership share a mutex.
  Uncertain children/slots remain retained; an uncertainty flag is not discarded.
- Children establish a private read-only lifetime pipe before loading a core or
  initializing SDL, lower hard non-root NPROC to zero and install finite file/
  address-space/file-size limits. Memory values come from the sealed runtime
  catalog, not renderer parameters. A private activation pipe raises only the
  owned helper window. The actor has a macOS device-free loader branch using
  fixed absolute core paths and no SDL implementation linkage.
- Lifetime EOF terminates a cooperating child. **It is not independent proof
  that a compromised core cannot interfere with that in-process reader.**
  RLIMIT_AS/FSIZE declarations do not establish whole-container physical-memory
  or aggregate storage accounting. No such kernel-quota claim is made.
- Staged input admission holds the exact private lease and directory identity,
  checks all actual hashes/sizes/inventory and flushes a pre-spawn launch fence.
  Only confirmed owned-child waits allow removal of that same fence. Persisted
  PIDs are never adopted as signalling/deletion authority after owner restart.
- Managed discard uses a durable exclusive `.retiring` rename before flat bounded
  unlinking. Inventory resumes a supported interrupted retirement, including an
  empty terminal directory after lease removal. A pending directory creates and
  flushes its lease before copying payloads. Nonempty lease-less legacy shapes,
  retained launch fences, links and unknown directories remain refused.
- CMake requires an explicit runtime tree and populates owner Resources before
  final signing. An assembly tool authors the broker app with its named nested
  XPC service. Release packaging requires both app seals, explicit executable
  paths and matching runtime mirrors. These source checks do not perform signing
  or replace later notarization/admission evidence.
- Broker-private MacWork journals now commit boot provenance atomically with
  copy debt before staging. The staged transaction precedes launch; orderly
  retirement requires owner removal and exact local metadata removal before
  atomic debt/journal release. A profile lease excludes concurrent creators.
- Reboot recovery uses that protected journal, not writable owner metadata or
  PIDs. Both sides read kernel boot identity. Current-boot staged work stays
  retained; supported prior-boot schema-2 fences retire by flushed rename before
  fence removal. Recovery passes inspect four entries with a rotating cursor.
  See `MACOS_BOOT_RECOVERY.md` for crash ordering and remaining limitations.

## Required source work before closing item 4

1. Implement the actual actor/helper resource boundary: inherited App Sandbox
   authority still includes a shared writable owner container. Private modes,
   finite staged input bytes, per-file FSIZE and an in-memory VFS are insufficient
   to prevent hostile code changing owner metadata or allocating other files.
   Physical/aggregate memory accounting must also be explicit and enforceable.
2. Complete independently enforced owner-loss retirement and recovery. The new
   lifetime reader improves orderly failure but is in the same address space as
   the core. An owner crash still leaves durable launch fences whose authority
   cannot be reconstructed by guessing PIDs or deleting unknown work. The new
   protected-journal reboot route handles supported previous-boot fences;
   independent same-boot termination and unsupported legacy repair remain open.
3. Complete the release-specific native core/dependency, install/upgrade/repair
   composition against that final boundary, including source rules for exact
   signed nested code and operator recovery. Assembly is not a signed release.
4. Integrate the final boundary into the already-authored adapter and remove the
   execution guard only when every required source obligation above is met.

`kExecutionBoundaryComplete` remains false. The owner reports `session_launch`
false and refuses launch requests; the macOS registry consequently stays
unavailable for execution. Authenticated control access permits bounded recovery
without satisfying or bypassing the separate execution guard. The resource and
lifetime gaps remain implementation gaps, not merely missing test evidence.
Do not mark item 4 source complete or move to item 5 in this state.

## Later verification, separately deferred

Target SDK/compiler/linker, exact Mach-O dependencies and signatures, sandbox/XPC,
notarization, resource/fault/owner-loss, production core/save/device and complete
acceptance checks remain required when the owner resumes verification. Existing
Windows/Linux evidence does not validate these changes.

Historical automatic review refused two earlier proposed relaxations. The
uncertain-child guard remains intact. The current public message-bound lookup
was authored through the normal tool with unchanged strict validity and added
identity/entitlement checks. No permissions or security settings were changed.
