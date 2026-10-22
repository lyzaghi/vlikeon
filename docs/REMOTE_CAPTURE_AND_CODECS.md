# Remote capture ownership and video preferences

This is authored source. No picker, permission prompt, getDisplayMedia,
getUserMedia, track, peer connection, encoder/decoder, browser/device or test has
been run. Streaming configuration remains disabled with no active relay origins.
The new source does not establish hardware acceleration, latency or security.

## Source selection

The Game Hub component extension declares `desktopCapture`. Its host can choose
the Chromium window picker, which offers windows and optionally authorized audio.
The alternate display picker uses getDisplayMedia and the browser's normal source
selection. Both are initiated directly inside the local capture action, after an
authenticated pending viewer request. Remote signaling never opens a picker.
Audio starts unchecked in the app. No microphone or camera constraint is used.

The extension window path consumes the browser's one-use source token locally
through desktop-source getUserMedia constraints. It does not persist, log,
transmit or route that token to another tab/origin. The existing browser chooser
and capture indicators remain responsible for consent and source authorization.
The four-argument options call follows the pinned Chromium 155 schema plus the
authored application-audio extension in `CHROMIUM_APPLICATION_AUDIO.md`; its
generated bindings and actual behavior still require final qualification. The public API documents
window selection, one-use tokens, cancellation and the user's audio choice.
[Chrome Desktop Capture API](https://developer.chrome.com/docs/extensions/reference/api/desktopCapture).

Standard window-audio and system-audio options are preferences to the browser.
The native window path now requests the product's strict application-audio
contract and requires its result protocol/classification before consuming an
audio grant. Its local snapshot says browser-reported application scope; display
picker audio remains user-selected unspecified. Neither a track nor a browser
classification independently proves isolation to a library game's process.
Owned-process/window binding,
independent Windows capture/audio paths, actual encoder selection and the full
native/mobile/codec scope remain required; this source does not replace them.

## Capture lifetime

`capture.js` owns each picker request, its deadline, cancellation and the actual
acquired tracks. A request is bound to the current pending remote session. A late
result after stop/replacement is retired before handoff. Track configuration also
races cancellation, so an unresolved applyConstraints call cannot retain the app's
pending capture action indefinitely. The Chromium picker has an explicit cancel
API; getDisplayMedia has no equivalent app cancellation API, so a late result is
stopped when it arrives. OS/browser prompt disappearance is not guaranteed by a
JavaScript cancellation flag.

Source constraints apply after the user chooses a source. They bound requested
width, height and frame rate to the selected 720p/1080p, 30/60 FPS profile and
admit reported settings without selecting a different source. Motion is the video
content hint. The handle retains the original track references independently of
later changes to a MediaStream container. Its snapshot contains sanitized source
state and reported measurements, without titles, device IDs or source tokens.
This follows the separation between user source selection and later track
constraints in the [Screen Capture specification](https://www.w3.org/TR/screen-capture/).
Reported settings do not independently verify every produced frame or resource
budget.

The local host preview is muted. A video-ended event ends the session; audio loss
is reported separately. Stop, page hide and the browser's freeze event retire
capture locally before signaling/storage cleanup. The Stop button remains usable
while a capture action is waiting. Native input begins revocation at the local
stopping transition. Network stop notification and receiver-lease release have
separate bounded app waits; those waits do not prove preemption of an underlying
OS/IndexedDB operation or successful remote acknowledgement. Full native mobile
suspend/termination behavior remains part of platform qualification.

## Codec preference and observations

The user can prefer locally advertised H.264, VP8, VP9, AV1 or HEVC. Unsupported
preferences are not invented. Auto keeps the browser's ordering. Explicit
preferences reorder native capability objects before offer/answer and preserve
remaining primary and RTX/RED/FEC entries. This allows normal negotiation fallback;
it neither edits SDP payload declarations nor forces a remote encoder or hardware
path. Selection uses the actual sender/receiver role and the standard
[setCodecPreferences contract](https://www.w3.org/TR/webrtc/#dom-rtcrtptransceiver-setcodecpreferences).
Proprietary codec distribution and actual build availability remain separate
release obligations.

One getStats report per sample supplies existing QoS feedback and a separate local
media observation. The UI shows the actual reported codec, frame dimensions/FPS,
byte-counter bitrate and interval encode/decode time when available. Counter resets,
missing values and unsuitable intervals remain unknown. A power-efficiency boolean
is retained only when directly reported; it is not a hardware claim. Implementation
fingerprint strings and candidate addresses are not included. These observations
are not added to the peer's existing quality-message schema or persisted as device
qualification evidence. Field meanings follow the
[WebRTC Statistics specification](https://www.w3.org/TR/webrtc-stats/).

The final consolidated pass must cover real extension permissions/bindings,
all picker/audio choices, cancellation before/during/after acquisition and track
configuration, source end/mute/resize, page/OS lifecycle, late results, independent
Stop, native input release, codec profile/intersection/fallback, reconnect,
stats resets/missing values, local preview audio, permissions/indicators,
containment and real device latency/RSS/energy. All original product requirements
remain open until their actual implementation and evidence are complete.
