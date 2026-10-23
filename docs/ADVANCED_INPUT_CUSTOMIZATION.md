# Advanced input customization — authored source

The 2026-10-23 item-6 extension adds per-game controller calibration, axis tuning
and a custom touch editor. Source review only: no build, syntax runner, browser,
device, compiler or Antigravity test was run. Save applies on the next launch.

## Controller calibration

Open a game's Controls settings, press a controller button and choose Refresh
controllers. Select the device and Create raw profile. Inspect live axis/button
numbers, choose each logical axis source and bind the sixteen game buttons to a
raw button or an axis direction. New profiles deliberately have unassigned buttons.
Capture released center with the sticks at rest, then capture their full range
for ten seconds, or edit minimum/center/maximum manually. Save the draft.

The Web Gamepad provider matches the exact device ID and axis/button counts.
Unconfigured nonstandard devices are ignored. Profiles support at most four
device identities per game, 32 raw axes and 64 raw buttons per device. Four logical
stick axes use distinct optional raw sources. Calibration values use thousandths
in [-1000,1000], with at least 50 units on either side of the center. Calibration
rescales each side independently to [-1,1]; a missing source emits neutral input.
Button thresholds are 1–100%; axis-direction bindings use the raw signed sample.
The existing sixteen-button permutation applies after raw mapping.

Range capture commits only valid ranges after completion. Stop, blur, hidden or
closed settings, disconnect, changed profile or Save cancels unfinished capture.
Live samples are displayed without retaining a history. Device IDs stay in local
per-game preferences, outside telemetry and save synchronization. The broker
removes these Web-only profiles before native prepare IPC; they never become
SDL GUIDs or a native joystick mapping database.

This implements raw calibration for Game Hub **Web sessions**. Native SDL still
requires a device supported by SDL's mapping provider. A native raw-joystick
calibration/provider editor is not supplied by this extension.

## Axis customization

Each of left X/Y and right X/Y can select a distinct source, invert direction,
set sensitivity from 50–200%, and set response exponent from 50–300%. Selecting
an occupied source swaps it with the previous source. Processing order is source
mapping, radial deadzone, then `sign(v) * abs(v)^(curve/100) * sensitivity/100`,
optional inversion, clamp and signed 32767 conversion. Curve 100% is linear.
Gain is per axis and may change the direction after the radial stage.

The same policy is authored in Web game input and native SDL direct/private
presentation input. Keyboard and virtual-touch stick overrides keep their own
existing input priority and do not receive this physical-stick tuning twice.
Native workers now advertise input-profile protocol **4**; Windows/Linux/macOS
brokers require it. Existing worker packages need rebuilding and admission.
The Web logical input wire remains protocol **3**, since tuning happens before
that wire and does not change its packet shape.

## Custom touch placement

Open Touch layout, enable Use custom layout, select or drag a control, and adjust
X/Y, size or visibility. Arrow keys move the focused control by one unit; Shift
moves ten. The editor covers sixteen buttons and two sticks, independently.
Reset custom positions resets this layout; disabling it restores the ordinary
group layout. Save persists the draft for the next Web session.

Positions are normalized to 0–1000. Frame height is 400–1200 thousandths of its
width. Each control has 50–200% individual size, multiplied by the existing global
size setting. Runtime and preview clamp controls inside the frame; buttons use
a 44 CSS-pixel base/minimum, sticks a 110-pixel base, bounded by the frame size.
Left-handed mode mirrors X positions. Hidden controls are faded in the editor
so they remain selectable, and actually hidden in gameplay. Runtime contacts
are released on geometry changes, reconfiguration, blur, hiding and disposal.

This is an actual Game Hub touch runtime layout, used by the shared Web frontend
and its resource copies. It does not create an SDL-native overlay or supply
missing native mobile/core runtime artifacts. Overlapping or intentionally hidden
controls are permitted and remain the user's layout choice.

## Compatibility and deferred qualification

Schema 1 adds optional `axis_tuning` (identity default), `raw_gamepads` (empty
default) and touch `custom_layout` (null default). Existing mappings remain valid.
Rust and Web enforce exact nested shapes and finite integer bounds; native C++
validates tuning again and accepts only empty Web calibration arrays. SQLite
profile loads/stores are bounded at 16 KiB for input and 8 KiB for touch. No
database migration or emulated save-state compatibility change is introduced.

Canonical modules are collected by the shared asset pipeline. Existing generated
iOS source copies were refreshed without running assembly. Source-freeze lists
include the new modules/contracts; no freezer was executed.

Deferred acceptance must check malformed/legacy profiles, persistence, capture
cancellation, offset/asymmetric axes, threshold boundaries, four-device hotplug,
native/Web tuning parity, worker-version rejection, touch editing and actual
multi-touch gameplay across narrow/rotated/mobile/desktop layouts. These are
pending checks, not successful device or compatibility results. Pressure buttons,
accessory selection and native raw-device providers remain separate work.
