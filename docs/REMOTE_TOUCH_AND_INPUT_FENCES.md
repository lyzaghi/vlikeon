# Remote touch controls and ordered release fences

Later item 55 source is tracked in `VIRTUAL_GAMEPAD_OWNERSHIP.md`. It adds
provider-side slot/authority rules and a Linux uinput provider without enabling
the current host's `gamepad:false` policy. Standard browser sampling now retains
neutral slot gaps and reads analog trigger button values. Existing touch still
uses keyboard/mouse permission; none of this claims completed virtual-device
integration or executed device evidence. The owner deferred new build/test.

This is authored source for the original remote keyboard/mouse/touch requirement.
No browser, input event, game, native helper, device, compiler or test was run.
Streaming remains disabled with no configured relay origin. Source assemblies
contain no emulator binaries and do not qualify this path for release.

## Local controls and host authority

The receiving Game Hub now offers an explicit touch toggle after a current host
keyboard/mouse policy arrives. It starts hidden for every session. Its movement
pad maps eight directions to four configured physical keys, the look pad emits
relative mouse motion, and eight held action buttons map to finite allowed keys
or primary/secondary/middle mouse buttons. Defaults are WASD, primary/secondary
mouse, Space, E, R, left Shift, Escape and Enter. These controls use the existing
signed keyboard/mouse pairing permission and separate native host authorization
for the selected running library game. They do not create another grant, expand
the target, install a driver or provide a virtual gamepad.

Four movement mappings must be distinct. Actions may share a mapping, with their
held state combined before publication. Sensitivity is an integer 25–300%, size
80–140%, opacity 40–100%; vertical look inversion and side swapping are booleans.
Unknown/missing fields and other actions are rejected. No arbitrary labels,
scripts, shortcuts containing privileged modifiers or shell actions are accepted.

Preferences are optional `control_profile` data on the existing receiving pair
row in `omniplay-streaming-v1` IndexedDB version 1. Load checks the pair and its
host-signed grant. Save validates the finite schema before a read/write
transaction, verifies pairing beforehand, and re-reads/merges the current row in
the same transaction. A preference save cannot restore a prior signaling
sequence, receiver cursor, lease, key or revocation. Revocation removes this local
preference with the pair encryption key. This addition needs no database upgrade.
It does not synchronize preferences, store held input, persist touch activation
or turn an unavailable/corrupt preference into a host authorization.

## Gesture, focus and lifecycle ownership

At most ten captured local pointers are retained, with one pointer per movement
or look pad. Each action pointer has its own identity: releasing one finger
does not release a second finger holding the same action. Pointer up, cancel and
lost capture remove that pointer. Move has a centered digital dead zone; look
uses finite bounded relative deltas and retains subpixel motion in the shared
input aggregator. Physical keyboard/mouse and touch state are combined by union,
within the existing 32-key and mouse limits. Native gamepad input stays disabled;
the Gamepad API is not polled when the current session does not admit it.

Pads and buttons have keyboard focus and accessible names. Arrow keys operate
the focused movement/look pad; Space/Enter hold a focused action button.
Action state is exposed through `aria-pressed`. Hit areas are at least 44 CSS
pixels and visible focus follows product contrast settings. Shift+Tab from the
video releases input and follows the local focus order, preserving a keyboard
route to Stop and settings even though plain Tab can be sent to the game.

The only active input focus region is the video or this session's control
region. Window blur, hidden document, page hide, focus leaving that region,
lost pointer lock, resize, policy disable, save/apply, stop and close clear
held state and pending relative motion. Opening settings also releases input.
Saving deactivates touch; applying preferences never resumes input by itself.
Loading/saving results arriving after close cannot reattach or activate controls.
Listeners, held pointer captures and keyboard-look timers retire on close.

The expanded view contains video, touch controls and an independent Stop button.
Element fullscreen is requested where supported; an in-document expanded view
is the fallback, preserving the DOM controls. A video-only platform fullscreen
does not silently hide touch controls. Exit and Stop remain visible, with safe
area padding and responsive control layout.

## Data channel v2

Both channels now require the following labels/protocols; unknown versions or
duplicates close the session. Signaling, pairing certificates and the private
native helper remain at their existing version 1 protocols. Both media endpoints
must be updated; there is no silent v1 data-channel downgrade.

| Channel | Label | Protocol | Delivery |
| --- | --- | --- | --- |
| Control | `omniplay-control-v2` | `omniplay.control.v2` | Ordered, reliable |
| Input | `omniplay-input-v2` | `omniplay.input.v2` | Unordered, zero retransmissions |

All messages retain the exact session identity and existing 8 KiB bound. A host
policy carries `epoch`, `enabled`, and `gamepad` (currently false). Epoch is a
positive safe integer at most 1,000,000, advances on every host arm/disarm, and
never wraps. A duplicate epoch cannot change capabilities. Client frames carry
that epoch plus a monotonically increasing client sequence at most 10,000,000;
neither rearm nor focus changes reset the sequence within a session.

A client clear sends a reliable `release_input` with the same epoch and the next
sequence from the same sequence space. The host ignores another epoch and any
sequence no greater than the last accepted frame/fence. It advances that sequence
before clearing native held state. Consequently a delayed unordered frame from
before a release cannot revive its keys/buttons. A later full input state can
resume only under the current epoch. A native reauthorization advances the epoch,
so packets from a prior grant cannot become fresh input after rearm.

The host starts its input-gap observation only after a valid frame in the current
epoch. After a gap over 250 ms (observed on a 100 ms timer), it disarms with a
fresh epoch and revokes the native grant; the host must approve again. A reliable
explicit release instead clears held state and the gap marker, allowing the
existing grant to remain idle. The native helper independently releases held
keys/buttons after its own 250 ms gap; it retains its separate process/window,
local activity, F12, grant-lifetime and owner-pipe checks.

The native forwarder serializes frames and releases through one bounded pump.
A release clears the pending frame, coalesces other releases, and runs after any
in-flight call and before the next pending frame. It is not dropped by a cooldown
while keys could have changed. Old-grant results/errors cannot overwrite or
revoke a newly admitted grant; pending work for a fresh grant resumes after an
old call retires. These are authored ordering rules, not a proof of OS delivery.
Revocation also invalidates a pending arm generation: a native consent result
arriving after local revoke/close is retired instead of rearming the session.

## Required final evidence

The one consolidated final pass must cover actual multi-touch/capture/cancel,
keyboard focus and assistive access, portrait/landscape/safe-area/fullscreen,
late profile responses, corrupt/revoked/concurrently updated pair rows, subpixel
motion and bounds, both data-channel versions, reordered/lost/duplicated frames,
release fences across channels, rearm epochs, channel close/backpressure,
foreground/native call races, stalled helpers, held-key cleanup and real devices
on every supported receiving platform. No pass or measured behavior is asserted.

Actual platform virtual gamepad providers/drivers and haptics, native target-bound
capture and hardware codec/platform qualification, TURN provisioning and the
entire remaining browser/runtime/sync/security/licensing/store/release scope
remain required. Keyboard/mouse touch mapping does not complete those items.
