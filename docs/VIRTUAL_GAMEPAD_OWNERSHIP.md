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

## Receiving browser sampling

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
   this provider yet; version them coherently before advertising support.
2. Implement Windows and macOS providers and required driver/entitlement/package
   paths, with current API verification, explicit installation consent where
   needed and revocation. No existing Windows input helper gains gamepads from
   this module. Any provider choice needs license/provenance review.
3. Add independent helper watchdog observations, single-owner cross-process
   slot arbitration, local/remote controller coexistence and supported global
   device visibility semantics. Complete cleanup receipts before reassignment.
4. Add device readiness/admission, actual game-controller ordering and analog
   compatibility; remaining haptics/feedback and supported accessories are open.
5. When the owner resumes build/test, Antigravity executes frozen batches only:
   compile platform targets; verify actual neutral/press/release/trigger/hat
   output, four-controller hotplug, sparse indices, unauthorized peer/slot,
   replay/release fences, partial OS writes, stale focus/target, expiry, provider
   loss, failed removal, owner/helper crash and no retained device after cleanup.
   Codex reviews results and decides fixes/acceptance. No current pass is claimed.
