# Item 6 — retained implementation and verification gaps

On 2026-10-23 the owner explicitly requested noting remaining gaps and moving
this chat to item 13, then item 14. This transition does not complete item 6 or
change assignments in other chats. Current authored features are documented in
`GRAPHICS_AUDIO_INPUT_ITEM6.md` and `ADVANCED_INPUT_CUSTOMIZATION.md`.

## Missing implementation

1. Native SDL raw-joystick calibration/provider editor. The implemented raw
   min/center/max and button-source calibration applies to Web Gamepad sessions;
   native axis tuning requires an SDL-supported mapping.
2. SDL-native touch overlays and platform-native mobile input integration beyond
   the shared Game Hub Web touch surface.
3. Analog pressure-button configuration and per-core accessory/controller policies
   beyond the current buttons and two sticks.
4. Additional GPU backends and complete platform/core integration required by the
   full product plan. Existing WebGL/software/declared native routes are not a
   promise of universal backend/core/device support.
5. Actual rebuilt and admitted native/core packages. Native input-profile protocol
   4 and hardware frontend display protocol 1 require matching artifacts.

## Deferred evidence

6. Compile/build, persistence and malformed/legacy-profile acceptance; raw capture
   cancellation, asymmetric calibration, thresholds and four-controller hotplug.
7. Web/native axis behavior and actual controller compatibility; no calibration
   accuracy, latency or device compatibility result has been measured.
8. Touch editor/runtime geometry, simultaneous contacts, resize/rotation, handedness,
   accessibility and real Android/iOS/browser input.
9. Video/audio/context/device/focus failure handling, lifecycle races and final
   cross-platform acceptance listed in `GRAPHICS_AUDIO_INPUT_ITEM6.md`.

Items 1–5 are implementation/artifact obligations; 6–9 are pending verification.
Neither category is silently moved to completed status. Practical tests remain
deferred, with Antigravity assigned execution only when released by the owner;
Codex and the owner retain review and fix decisions.
