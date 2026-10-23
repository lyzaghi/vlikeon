# Optional local gamepad rumble source

The shared keyboard/controller profile now includes optional `haptics_enabled`,
defaulting to false. The existing draft editor exposes a labeled checkbox and
saves it for the next launch. Rust, Web and C++ validators accept the preceding
four-field schema-1 profile with rumble disabled and reject a non-boolean optional
value. This preference stays local and outside save compatibility, diagnostics
and save synchronization. No keyboard/controller, browser, device effect,
compiler, runtime or test has been executed.

Native worker input-profile protocol is now 4, with a distinct haptics protocol 1.
The broker intersects the stored opt-in with the admitted package's haptics
capability and acknowledged worker protocol before preparation. Explicit native
and Web build recipes now declare the common frontend capability. Such a
declaration describes the conditional output path; it does not prove that a
particular core, title, controller, driver or operating system supports rumble.
There is still no built or admitted core/worker artifact. Old inventories do not
gain capabilities merely because the shared frontend source has changed.

Both frontends expose the libretro strong/weak rumble interface only when enabled.
A callback checks port, effect, strength, playback and available hardware. Motor
values remain latched until the core changes them or the session clears them;
the frontend does not require a core to repeat an unchanged rumble request every
frame. Values never become keyboard/mouse injection or phone-wide vibration.

Native callbacks update per-port state under a short mutex and never call SDL
device APIs. The frontend copies that state outside device I/O, limits renewal to
one call per port per 50 ms and requests effects lasting 150 ms. SDL's reported
rumble property gates availability. A failed device operation disables that
port's rumble until a new device is opened. Pause, focus loss, removal and normal
shutdown clear desired state and request zero where an effect was sent. Other
connected guest ports retain their original positions. The worker's event loop,
driver and OS still affect effect processing; a requested duration is not proof
of a hard physical stop deadline after a process or driver fault. The selected
SDL API explicitly requires device/event processing for timely completion.

Web output uses only an available standard gamepad's `dual-rumble` actuator with
both `playEffect` and `reset`. Unsupported devices remain unavailable. The worker
publishes at most one outstanding haptics message plus one coalesced latest
packet. The UI acknowledges sequence numbers and rejects packets older than
250 ms using the shared time-origin-based monotonic timestamp. A stalled UI
cannot accumulate an unbounded haptics event history. Disabled/disconnected
ports clear their desired worker state.

The page coalesces four pairs of bounded 16-bit motor strengths, requests up to 100 ms
effects at most once per 50 ms per port and permits only one pending play promise
and one reset per actuator state. Packet age is retained when renewing effects,
so receipt does not restart an old packet's freshness window. A 500 ms promise deadline bounds waiting in the
page; it cannot cancel an underlying browser/driver operation. Lack of fresh
worker output, pause, blur, hidden/pagehide, port removal and normal exit request
reset. Errors disable rumble for that active port state without failing game
playback. Browser lifecycle freezing, controller firmware and physical motor
timing still need qualification; no universal fail-safe or latency claim is made.

The Web controller registry preserves four guest slots by the browser's index
and bounded device identity. Removing one controller releases its port and its
actuator state while keeping other players in place. Raw/nonstandard device
calibration, virtual streaming gamepads and remote rumble, touch feedback,
trigger-specific effects, platform-native mobile adapters and broader input
customization remain required by the complete product scope.

The final consolidated verification must cover old/new/malformed profiles,
disabled defaults, runtime capability/handshake rejection, games with latched
rumble, both motors independently, unsupported devices/APIs, four-port disconnect
and replacement, focus/pause/resume/stop, stale/backpressured worker packets,
rejected and never-settling actuator promises, worker/thread/process failures,
SDL event processing, actual controller/driver matrices and user accessibility
preferences. These scenarios have not been executed.

Primary contracts: [libretro rumble interface](https://github.com/libretro/libretro-common/blob/master/include/libretro.h),
[Gamepad API and haptics](https://w3c.github.io/gamepad/),
[SDL rumble semantics](https://wiki.libsdl.org/SDL3/SDL_RumbleGamepad),
[SDL gamepad properties](https://wiki.libsdl.org/SDL3/SDL_GetGamepadProperties).
