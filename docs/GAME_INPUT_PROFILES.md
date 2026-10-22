# Per-game input profile source

Game details now expose next-session keyboard and standard-gamepad customization
for emulator games. This is authored source; no keyboard/gamepad, native worker,
browser, settings transaction, compiler or test has run. The feature does not
complete raw-device calibration, touch-layout customization or the full
input/platform epic.

The schema-1 profile has sixteen optional physical keyboard bindings for virtual
game buttons, four optional left-stick keyboard directions, a sixteen-button
source permutation, optional boolean `haptics_enabled` (default false) and optional
integer `analog_deadzone_percent` (0–50, default 15). Existing four/five-field
profiles retain their mappings and receive the missing defaults.
Missing stored profiles use explicit defaults. Rust, shared
Web validation and the contained C++ worker enforce exact shapes, supported key
codes, unique assigned keys and bounded/permuted gamepad sources. Unassigned
keyboard controls remain null. The allowed set includes letters, digits, arrows,
Space/Enter/Tab/Backspace and Shift; Escape, F11, Ctrl/Alt/Windows/Meta and arbitrary
OS shortcuts do not become configurable injected actions.

The Rust profile decoder now requires an object map explicitly; ordinary Serde
struct decoding can also accept positional arrays. Its map-based field decoder
preserves unknown/duplicate-field checks and optional defaults. These are source
guards, not executed malformed-input evidence.

The editor captures keys only while its selected binding button has focus in the
open detail dialog. Escape cancels; loss of focus/close/new editor cancels its
temporary listener. Assigning a used key clears its earlier assignment, and
choosing a used gamepad source swaps the two targets. Defaults change a draft and
require Save. The editor does not record typed text or an event history. Ordinary
UI navigation and system shortcuts keep their separate owners.

Preferences stay local under a game UUID, using SQLite profile preferences or
strict IndexedDB configuration transactions. Save/update audits contain the game
UUID, not key events. The control profile is not part of game-save synchronization
or diagnostic output. Saving a profile does not alter an active session. Physical
controls are frontend policy and are deliberately excluded from emulated-state
compatibility hashes; changing a keyboard binding does not make an otherwise
compatible snapshot unreadable.

The WASM player loads and validates its profile before starting a game. Physical
key down/up resolves through that fixed session profile; existing blur/visibility
pause clears held frontend state. Touch controls keep their virtual-button meaning.
Standard Gamepad API sources use physical positions, then the saved permutation.
The native frontend uses the equivalent SDL standard positions, trigger axes,
D-pad/digital-stick fallback and the same logical permutation. Shared N64/PSP
left-stick keyboard directions remain separate from D-pad bindings.

Default B/Y/A/X now consistently mean the libretro virtual bottom/left/right/top
positions across native SDL and standard Web gamepads. This corrects the prior
Web/native face-position mismatch; printed Xbox/Nintendo/PlayStation letters are
not assumed interchangeable. The editor includes physical-position hints.
Nonstandard Web gamepads are not silently treated as the standard layout; their
raw calibration/provider path is still required. The Web transport now carries
four signed stick axes per guest port and native input uses the same radial
deadzone contract. This does not supply missing advanced WASM core artifacts or
negotiate emulated controller/accessory types. See `ANALOG_INPUT.md`.

The native preparation path reads the profile from the broker-owned library,
transmits it in the bounded prepare request and requires input-profile handshake
version 3. The contained worker validates again before the core runs. Older
workers that cannot acknowledge the profile are rejected rather than silently
ignoring saved controls. Worker/core packages therefore need rebuilt artifacts and
their normal signed inventories before this path can execute. No such binary
has been built or admitted.

The rumble checkbox is saved in the same draft and applies on the next launch.
Only an admitted runtime declaring haptics and an available supported actuator can
receive game-requested rumble. The default does not request a device effect.
`GAMEPAD_HAPTICS.md` describes state lifetime, bounded transport/effects and the
remaining platform/device verification. Standard Web controllers now retain
their guest ports when a different controller disconnects; newly observed devices
take an empty port rather than shifting the remaining players.

The final consolidated pass must cover all allowed/null/duplicate/reserved keys,
cross-language malformed shapes/numbers, draft/default/save behavior, focus and
listener cleanup, four controllers, trigger/face-position semantics, source swaps,
disconnect/reconnect, held-key pause/blur, N64/PSP analog separation, unchanged
snapshot compatibility, old-worker rejection and actual platform/device layouts.
Additional touch/axis-remapping/provider controls and the complete remaining
product scope remain required; no compatibility or latency result is claimed.

A separate six-field local touch profile now has its own strict commands and
draft editor, without extending the native keyboard/gamepad packet. Full virtual
buttons/sticks and the Web NDS pointer adapter are described in `TOUCH_CONTROLS.md`
and `WEB_CANVAS_POINTER.md`; arbitrary layout placement, native overlays and
platform evidence remain required.

Contracts: [Gamepad standard mapping](https://w3c.github.io/gamepad/),
[SDL gamepad positions](https://wiki.libsdl.org/SDL3/SDL_GamepadButton),
[SDL scancodes](https://wiki.libsdl.org/SDL3/SDL_Scancode).
