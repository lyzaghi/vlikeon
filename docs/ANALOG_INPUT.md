# Shared analog input source

Current extension: `ADVANCED_INPUT_CUSTOMIZATION.md` documents Web raw-device
calibration and shared Web/SDL axis source, inversion, gain and response exponent.
These stages now wrap the radial baseline below. Source only; no device evidence.

Per-game input schema 1 now accepts optional integer `analog_deadzone_percent`
between 0 and 50, defaulting to 15. Rust, Web and contained C++ parsing reject
unknown fields, null, fractional, negative and out-of-range values. Four/five-field
stored profiles keep their bindings/haptics setting and receive the absent
deadzone default. The localized labeled slider changes a draft; Save applies on
the next launch. The physical control setting remains local and outside emulated
state compatibility hashes, diagnostics and save synchronization.

For each standard left/right stick the frontend clamps finite X/Y to [-1,1],
computes radius `r = hypot(x,y)` and deadzone `d = percent / 100`, and emits zero
when `r <= d`. Otherwise the scale is `(min(r,1)-d)/((1-d)*r)`. Multiplying X/Y by
that scale preserves direction, remaps the remaining travel and limits diagonal
magnitude to the unit circle. Conversion multiplies by 32767 and rounds halves
away from zero on both frontends. SDL's negative/positive raw endpoints use their
own 32768/32767 normalization. Missing/non-finite Web axes become zero. This
algorithm is authored product policy, not a device calibration or accuracy claim.

Native libretro input polling snapshots two processed sticks for each of four
stable guest ports. Analog queries read that snapshot; digital-only left-stick
D-pad fallback uses its processed +/-13107 threshold. N64/PSP keeps physical
D-pad separate and supports the saved four keyboard directions for left stick.
Opposing keyboard directions cancel on that axis; an unpressed keyboard axis
keeps the physical stick value. The native worker requires input-profile protocol
4, so an older binary cannot silently ignore the new policy.

The Web player now sends four fixed button arrays (16 zero/one values each), four
fixed stick arrays (left X/Y, right X/Y; signed integers -32767..32767), four
boolean actuator capabilities and a time-origin-translated observation timestamp.
The runtime allows only one input request in flight. Worker input protocol 3 is
required before ready; all shapes/numeric bounds are validated before state
replacement. Valid frames older than 250 ms or over 50 ms into the future become
neutral state. Playback ticks also neutralize buttons, axes and desired rumble
after the last observation exceeds 250 ms. Receipt does not renew an already-old
observation. This bounds retained input while the worker progresses; a hung core,
frozen user agent or OS/device fault still needs separate qualification.

The WASM input callback masks libretro device subclass bits, rejects invalid
port/index/ID, supplies JOYPAD buttons and the signed 16-bit bitmask, and exposes
left/right ANALOG X/Y. The environment reports those two device capabilities with
a zero high word and accepts the experimental bitmask query, including its null
data pointer. N64/PSP keyboard stick directions use the same saved mapping as
native and no longer become accidental D-pad presses. Pause/blur clears held
keyboard/touch frontend state and the worker's input tables; polling resumes with
current device observations. A core's actual controller port, accessory and
multitap policy remains a separate explicit emulation configuration.

No new advanced WASM runtime/core artifact has been supplied by this transport.
Web raw calibration, bounded axis remapping/inversion/response curves and custom
touch placement are now authored; see the extension above. Native raw SDL
provider editing, analog pressure buttons, per-core accessory
selection, platform-native mobile adapters, remote virtual controllers and the
complete remaining product scope still require implementation/qualification.
No stick/gamepad, worker, SDL API, browser, compiler or test has been executed.

The shared Web frontend now adds defaultable separate touch-layout preferences,
both virtual sticks and a geometry-bound NDS canvas pointer transport. Their
ownership/source priority and unverified scope are described in `TOUCH_CONTROLS.md`
and `WEB_CANVAS_POINTER.md`; neither supplies missing advanced core artifacts.

The final consolidated pass must cover all old/new/malformed profiles, slider
save/default behavior, zero/50% edges, both signed endpoints, diagonals, symmetry
and threshold boundaries, missing/non-finite axes, independent right/left sticks,
opposing keyboard directions, D-pad separation, all four ports, class/ID/bitmask
bounds, stale/future/blocked request handling, focus/pause/reconnect, unchanged
save compatibility and actual controller/core/device behavior.

Primary contracts: [standard Gamepad axes](https://w3c.github.io/gamepad/),
[SDL axis endpoints](https://wiki.libsdl.org/SDL3/SDL_GetGamepadAxis),
[libretro device/input ABI](https://github.com/libretro/libretro-common/blob/master/include/libretro.h),
[cross-context monotonic timestamps](https://www.w3.org/TR/hr-time-3/#examples).
