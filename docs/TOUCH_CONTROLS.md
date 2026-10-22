# Per-game touch controls source

This document describes local emulator touch profiles. Remote playback has a
separate per-pair keyboard/mouse touch profile and explicit host authorization;
see `REMOTE_TOUCH_AND_INPUT_FENCES.md`. Neither profile implements an OS virtual
gamepad provider for remote play.

Game details now have a separate touch-layout draft with explicit Save/Defaults,
a localized preview and next-session application. The schema-1 profile has six
required fields: `schema_version`, `visibility` (`auto`, `always`, `hidden`),
`layout` (`auto`, `buttons`, `sticks`), `left_handed` (boolean), integer
`size_percent` (80–140) and integer `opacity_percent` (40–100). Defaults are
automatic visibility/layout, ordinary handedness and 100% size/opacity. Rust and
Web reject unknown/missing fields, null, fractional or out-of-range values. Rust
uses an object-only map visitor rather than Serde's positional struct-array
decoder, retaining duplicate-field rejection for raw typed JSON.

`game_touch_get/set` uses the authenticated profile and a known library UUID.
Native/mobile storage uses the existing SQLite preference transaction and a
game-UUID-only audit; Web uses its strict IndexedDB configuration transaction.
The profile stays local and outside diagnostic data, save compatibility and save
synchronization. It does not change a running session. No database migration is
needed for this separate existing preference namespace.

Automatic visibility uses touch/coarse-pointer device availability. Explicit
Always also makes the controls available with mouse input. Automatic layout adds
two sticks for N64/PSP and buttons for the other systems. All sixteen libretro
virtual buttons are reachable: an eight-direction draggable D-pad, four face
buttons, two shoulder/trigger pairs, Select/Start and both stick-click buttons.
Handedness swaps directional/button groups and the physical placement of the
labeled stick panels. Touch labels keep their virtual meaning independently of
the physical-gamepad source permutation. CSS wraps groups on a narrow viewport;
targets retain a 44 px minimum, focus indicators and normal keyboard navigation.
The user can adjust size and opacity without importing arbitrary CSS/assets.

Pointer ownership is explicit and bounded to sixteen contacts. A D-pad admits
one drag owner, each stick admits one owner, and independent fingers can hold the
same face/auxiliary button. Releasing one finger only removes its own source.
Sixteen logical button states are the union of live pointer/key/assistive sources,
not a history of events. Capture failure admits no held source. Up/cancel/lost
capture, missing primary-button state, blur, hidden/pagehide, resize, pause,
runtime failure and disposal clear the applicable contacts. Listeners share an
AbortController and owned pulse timers are cleared when the component is removed.

Focused button Space/Enter activation supports hold/release; focused stick panels
support arrow keys. A trusted high-level assistive click has one replaceable
120 ms pulse per virtual button and does not duplicate ordinary keyboard click
completion. There are at most 32 button-key sources and 16 assistive timers.
The component only consumes its own game controls; it never creates OS events.
Trusted-event checks are a frontend behavior guard, not protection against a
compromised trusted renderer.

Both virtual sticks use the shared per-game radial deadzone and signed conversion.
A live virtual contact overrides the corresponding port-0 physical/keyboard
stick, including its centered zero; release returns to current live inputs. For
digital-only game controls, the virtual left stick drives virtual D-pad meanings
and suppresses that port's physical-stick digital fallback while owned. Physical
D-pad/button presses remain independent. The worker's observation-age limit also
neutralizes retained touch input when the UI stops providing fresh frames.

This is the Game Hub Web frontend used by desktop, Web/PWA and mobile resource
assemblies. It does not add an SDL-native button overlay, native mobile emulator,
advanced WASM core artifact or arbitrary drag-and-drop layout editor. Those and
raw/remote controller paths remain part of the full scope. No touch/key/assistive
event, layout render, browser/mobile UI, storage transaction, worker, compiler or
test has been executed.

The final consolidated pass must cover every strict profile value, draft/save/
default persistence, device visibility, all sixteen buttons, diagonals/dragging,
same-button multiple fingers, simultaneous sticks/buttons, contact replacement,
capture failure/cancellation, keyboard and assistive activation, pause/fault/
reopen listener cleanup, source priority, narrow/large/rotated/safe-area layouts,
text scaling/high contrast/opacity/focus, physical input coexistence and actual
Android/iOS/browser devices. Claimed minimum target sizes and source lifetimes
still require real layout/lifecycle evidence.

Primary event contract: [Pointer Events capture, cancellation and accessibility](https://www.w3.org/TR/pointerevents3/).
Analog policy: `ANALOG_INPUT.md`. Native-window behavior: `NATIVE_RUNTIME.md`.
