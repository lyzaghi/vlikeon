# Item 6 — graphics, audio and input

Current state: the source audit/fix pass for the declared native and web playback
routes is complete; target qualification has not run. The owner requested transition
to item 6 after the remaining item-4 work. Item 4 now has a diskless guest source
route; after the earlier automatic rejection, the user explicitly approved
source activation before qualification. That switch is now enabled in source,
without a build, install or launch. Verified completion has not been claimed.
Item 5 remains pending, not implicitly completed by this work order.

No new build, runtime, SDL/device call, browser operation or test was run.

The latest source extension implements Web raw-controller calibration, shared
Web/SDL axis remapping/inversion/gain/response curves, and individual touch
placement/size/visibility with a drag editor and actual runtime application.
See `ADVANCED_INPUT_CUSTOMIZATION.md` for bounds, UI, platform scope and pending
qualification. Native raw SDL provider editing is still separate work.

## Current source fixes

- Presentation queue admission retires prior queued video/audio on playback
  State messages. The consumer processes commands and media separately; without
  this retirement a pause and resume in the same batch could replay old samples.
- SDL source-format audio queue admission includes the incoming packet in its
  finite byte ceiling. The previous check admitted one additional packet after
  checking only existing bytes. This bounds that queue, not physical device or
  driver latency. Host/client packet queues remain independently bounded.
- An own window must have keyboard focus and be neither hidden nor minimized to
  publish active input. Losing that condition advances the lifecycle epoch,
  releases input/haptics and clears pending playback through the existing fence.
- The diskless guest wire explicitly permits its bidirectional virtio character
  port only when compiled with `OMNIPLAY_GUEST_ACTOR`. Native host actor builds
  keep their separate FIFO/access-mode checks.

## Completed source audit and fixes

| Area | Source outcome |
| --- | --- |
| Native geometry/video | Validate base/max dimensions and finite aspect for software and GL; check logical presentation/fullscreen results; preserve bounded pixel conversion and private framebuffer ownership. |
| Native graphics failure | Check robust GL reset status whenever acquiring the context, including before guest execution; renderer loss/reset retires the session instead of reusing invalid resources. No implicit GPU fallback. |
| Separate presentation | Retire queued pre-State media, cap the projected SDL audio queue, fence actual own-window focus-loss events even if focus returns in the same event batch, and fence an owned controller removal. |
| Native audio lifecycle | Preserve producer pause barrier, stream lock and quiescent ring reset; query the bound logical audio device periodically and before resume. SDL default-device migration remains supported; a failed query pauses/closes instead of pretending playback succeeded. |
| Web audio | Batch single-sample callbacks into at most 1024 stereo frames; batch callback input at most 16384 frames. At most four packets await consumption acknowledgements across Worker/UI/Worklet; no-ack drops stale audio without blocking the core. |
| Worklet recovery | Validate ArrayBuffer size/alignment/rate before viewing samples; clear interpolation phase and residual data after starvation; count trimmed frames separately from underruns. Worklet metrics have one outstanding acknowledgement. |
| Web video | Validate integer dimensions, pointer, pitch and complete last-row span before conversion; use explicit little-endian DataView reads, including unaligned rows. Bound fallback UI video to one acknowledged frame. Count its presentation only after successful UI drawing. |
| Display aspect/pointer | Apply the declared finite aspect to bounded software output and the WebGL destination, keeping the raw guest target separate. Pointer normalization uses that same displayed geometry; geometry changes clear held pointer state. |
| WebGL compatibility | Require `_om_web_hw_display_protocol() == 1`; export it from the native frontend recipe. Old core binaries refuse rather than silently resetting the canvas to raw geometry. Actual rebuilds remain pending. |
| Input | Preserve validated key/button permutations, four stable standard-controller ports, radial deadzones, touch contacts, pointer capture, epoch/freshness neutralization and finite opt-in rumble. Direct SDL and Game Hub suppress input outside their own active visible surface. |
| Runtime transitions | Coalesce start/resume and pause operations; check worker ownership after awaits so stop/failure cannot be overwritten by a late resume. Drop paused/preparation audio, clear before resume, handle AudioWorklet failure/context interruption and detach ports/listeners at stop. |
| Context loss | Worker 2D, UI fallback 2D and WebGL loss stop the session through existing failure ownership; no automatic restoration or save publication from an uncertain guest is introduced. |
| Reporting | Worker metrics and AV notifications have bounded acknowledgement credit. Report audio drops separately; include actual helper video replacements in native dropped-frame metrics. |
| Source distribution | Shared Game Hub asset collection reads the canonical source. Updated the existing generated iOS runtime source copies and player guard; source-freeze selectors include the new web/native contract files. No assembly or build tool was executed. |

The existing native ring, GL callback ownership, input-profile validation,
GamepadPorts/Haptics and CanvasPointer were inspected in addition to the changed
paths. Saved-state payload layout and remote-input authority are unchanged.
Local schema-1 controller/touch profiles now have defaultable customization
fields; native input-profile handshake is 4. Native brokered presentation still declines benchmark
protocol support rather than presenting actor-loop timings as device evidence.

## Explicit limits and deferred evidence

This closes the listed audit/fix tasks for the adapters currently declared in
source. It does **not** claim that every input/graphics feature in the full
product plan exists. Web raw calibration, axis remapping/curves and custom touch
placement are now authored as detailed in `ADVANCED_INPUT_CUSTOMIZATION.md`.
Native raw SDL provider editing, pressure/accessory configuration, native touch
overlays and additional GPU backends remain separate implementation work.
Per-core artifacts and platform-specific integration remain their own open work.

Source inspection and CRLF-aware `git diff --check` were performed. No JavaScript
execution/syntax runner, compiler, SDL call, browser/device, core, Worklet, VM,
materializer, build, workflow or Antigravity test was run. There is no measured
latency, compatibility list, hard driver-buffer ceiling or whole-GPU quota claim.

The deferred final pass must compile/rebuild the frontend and its admitted core
artifacts and exercise format/aspect/resize/fullscreen, renderer/context loss,
audio rate/starvation/device migration, repeated pause/save/load/stop races,
blocked media consumers, focus/visibility, controller hotplug/remaps, touch/pointer
geometry, haptics and actual Windows/Linux/macOS/Android/iOS/browser devices.
Record failures and fixes before claiming verified item-6 or product completion.
