# Owned Windows capture, application audio and input binding

This is source authoring only. Chromium patch 0003, the broker/worker protocol
and UI integration have not been built, applied to a fresh checkout or executed.
Streaming policy stays disabled. No native dialog, capture, input, game or device
operation was run. This is not completion of all platform/native streaming work.

## End-to-end source path

The host first selects a running installed game and prepares capture for the
pending paired remote session. `stream_capture_prepare` verifies the signed pair,
configured audience and host identity, then retains bounded visible-window
choices from this installed session's Windows job. The Game Hub receives opaque
choice IDs and local titles. `stream_capture_select` consumes one explicit choice
after native job/window/process re-admission and retains its handle and creation
FILETIME. This includes job-owned descendants after their launcher exits. Capture
lifetime is bounded by the original certificate and thirty minutes. Native
PID/creation/HWND stay in the component worker; public capture replies contain
only opaque identity and game/pair/session/expiry bindings.

On the user's subsequent capture action the worker rechecks the live originating
document and invokes the native Chromium window chooser for that tab. New
private options carry the retained process identity. After local source consent,
the Windows browser callback checks the selected HWND's process and exact
creation FILETIME before registering a consumable media token. A screen/tab,
another process, exited target or reused PID is refused. Unsupported platforms
refuse this owned path. Generic view-only browser capture remains separate.

Window audio retains the existing application-only policy. Owned capture always
excludes system audio; an audio-sharing backend result with any other scope is
refused before token registration. If the local user deselects audio, video-only
capture can continue. The worker's native HWND binding stays private; public
replies contain only the media token and opaque identity. The renderer consumes
that one-use token through getUserMedia and feeds the resulting tracks into the
existing independent browser WebRTC capture/encode pipeline.

The capture source publishes its opaque binding, expires with the original
authorization and releases the native capability when retired. Picker cancel,
45-second timeout, document closure and late replies retire owned worker state.
No native process/window identity is sent to the remote peer, stored in pairing
records or included in user-facing diagnostics.

## Exact input target

`NativeInputForwarder` requires an active owned-capture snapshot for the selected
game and session before input authorization. The worker supplies its privately
retained HWND, overriding any renderer-supplied field. The broker requires its
matching capture UUID/game/pair/session/lifetime and rechecks the retained game
process identity. Native consent remains a separate local operation.

The trusted input helper receives that exact HWND in its private target record.
Its foreground/liveness check admits only that window of the original process,
rather than latching an arbitrary other window of the same game. Capture expiry
also bounds input expiry. Retiring capture closes its input helper. Existing
foreground loss, local override, watchdog, input-gap, release and owner-pipe
rules continue to apply. Virtual gamepads remain unavailable.

## Practical limits and unfinished scope

This uses Chromium's native window/video/application-audio capture and encoder
implementation, not a newly authored independent native encoder service.
The exact supported backend/OS versions, window lifetime/handle reuse, capture
mute/closure, application audio process trees, hardware encoding and real
input-to-photon behavior still need execution evidence. Selection checks and
later OS capture/input are not one atomic transaction; no race-free OS proof is
claimed. Existing SendInput focus/UIPI/local-hook limitations remain applicable.

Direct children and Windows job-owned descendants now have authored selection
bindings; neither has runtime qualification for this new flow. Linux/macOS owned
capture and input adapters, signed virtual gamepad
integration, independent native encoding paths and durable/mobile credential
lifecycle remain required work in item 8. No policy is enabled to hide those gaps.

Item 57 now connects Windows creation-time launcher job ownership, retained
window choices and two-phase broker/Game Hub selection; see
`WINDOWS_LAUNCHER_OWNERSHIP.md`. The component worker additionally requires the
Chromium picker's returned HWND to match the broker-selected window before
publishing the media token. Explicit native sharing consent is still required.
Input authorization now follows the capture's retained process handle and
rechecks its exact installed-job membership after local consent and helper
admission; it no longer substitutes the initial launcher at that stage. The
deadline is rechecked after those operations before worker lifetime is set.

API reasoning follows Microsoft's documented
[window process lookup](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindowthreadprocessid)
and [process creation time](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocesstimes).
Their contracts do not replace final product qualification.
