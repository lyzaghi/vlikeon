# Web canvas stylus/pointer source

The shared Game Hub now has a local one-owner canvas pointer adapter for NDS
sessions when the admitted Web core declares touch controls. It accepts a trusted
primary mouse/pen/touch contact on the displayed game image, captures that pointer
and reports normalized signed X/Y plus pressed/inside state. A second canvas
contact does not steal the owner. Touch buttons/sticks have separate owners, so
a pen can coexist with other game controls. Mouse/pen hover can retain location;
lifting a touch contact removes its location. There is no OS injection, pressure,
tilt, physical-device identity or recorded event history.

Projection uses the known presented frame geometry, the canvas content box after
border/padding and centered `object-fit: contain`. It subtracts the letterbox,
then maps the displayed image to [-32767,32767] with symmetric rounding. Contacts
outside the image are unpressed/offscreen. Unknown geometry, invalid bounds and
a directly transformed canvas return neutral state. The current product CSS
uses an untransformed centered canvas; arbitrary transformed ancestors or a new
object-position/layout contract would need a corresponding projection adapter.
The pointer-enabled canvas alone disables touch panning and uses a crosshair.

The WASM worker records the dimensions of the actual last presented video callback.
Its bounded input reply carries that current geometry, including the OffscreenCanvas
path, through the existing one-request-in-flight gate. No new per-frame geometry
event queue is created. UI pointer requests include the geometry used to project
them. A mismatch or presentation-size change neutralizes the worker pointer until
a matching fresh observation arrives. This reduces stale-layout hits; it is not
proof of an exact guest-frame/input synchronization contract during a core's
internal layout changes.

Web worker input protocol is now 3. Strict pointer fields and geometry are
validated before state replacement. Only explicitly enabled port 0/index 0
RETRO_DEVICE_POINTER queries return coordinates, pressed/count (zero or one) and
the optional offscreen ID. The environment adds the pointer device bit only when
enabled. The common observation timestamp/deadline also neutralizes pointer state
after 250 ms without fresh input while the worker continues progressing.

Up/cancel/lost capture, absent primary-button state, pause, blur, hidden/pagehide,
scroll/resize/visual-viewport changes, runtime failure and normal disposal clear
ownership. A captured point moved outside the image is unpressed until it returns
or is released. Event listeners share an AbortController. A browser/worker/OS
freeze remains a platform fault requiring qualification; no hard timing or
physical-device guarantee is claimed.

The adapter does not provide an NDS WASM binary, firmware, core-specific screen
layout chooser or mobile native emulator. Current development catalogs remain
empty; approved advanced Web cores and their licensing/build/device evidence are
still required. No pointer event, projection, video/canvas, browser/device,
worker/core, compiler or test has been executed.

The final consolidated pass must cover both canvas presentation paths, frame-size
changes and mismatched geometry, borders/letterboxes/DPI/fullscreen/text scaling,
zero/maximum/signed/invalid coordinates, all pointer IDs/counts, hover/contact/
drag/leave/cancel/capture loss, competing fingers, simultaneous game controls,
scroll/rotation/safe-area/viewport changes, stale/future input, pause/failure/
reopen cleanup and actual core screen-layout interpretation on supported devices.

Primary contracts: [Pointer Events](https://www.w3.org/TR/pointerevents3/),
[CSS object fitting and positioning](https://www.w3.org/TR/css-images-3/#the-object-fit),
[libretro pointer ABI](https://github.com/libretro/libretro-common/blob/master/include/libretro.h).
