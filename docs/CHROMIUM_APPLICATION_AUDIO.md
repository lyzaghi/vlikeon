# Chromium application-audio source contract

This is a source change to the pinned Chromium 155 product fork. No generated
API binding, C++ compiler, browser, native picker, loopback/audio service, game,
device or test has run. Product streaming remains disabled with no relay origins.
No working audio, isolation, hardware or distribution evidence is established.

## Findings in the immutable source

The pinned extension schema declares `windowAudio`, but
`desktop_capture_api.cc` does not pass that field to its picker controller. The
base function only supplies the system/self/audio-suppression flags. The normal
getDisplayMedia handler separately passes its window-audio preference. This was
observed by reading the exact pinned source, rather than exercising a browser.
[Pinned extension entry point](https://chromium.googlesource.com/chromium/src/+/0e66b94549a8bd85f03fbdf189b752d6e7b63c32/chrome/browser/extensions/api/desktop_capture/desktop_capture_api.cc).

The pinned Views picker can classify selected window audio as application or
system. Its normal window preference falls back to the system mix when
application loopback is unsupported but system capture is available. That
behavior includes native chooser labeling/consent; a `windowAudio` preference
alone is not an application-only guarantee.
[Pinned native picker](https://chromium.googlesource.com/chromium/src/+/0e66b94549a8bd85f03fbdf189b752d6e7b63c32/chrome/browser/ui/views/desktop_capture/desktop_media_picker_views.cc).

## Authored native path

`tools/desktop_capture_audio_patch.py` applies guarded changes to six exact
original blobs. The extension entry point now maps its supplied window-audio
enum into the existing native picker parameter. Optional trailing base-function
arguments preserve the existing private WebRTC caller's defaults. The native
Params copy/assignment operations are defaulted in the pinned source and include
the new field automatically.

The optional product flag `omniplayStrictWindowAudio` requires window-only source
types, window audio and excluded system audio. The picker receives a separate
strict flag, defaulting false for existing callers. In strict mode, Views does
not offer the system-audio fallback. When application loopback is unavailable,
its offered window-audio type is none, and the existing category/checkbox logic
reflects that. The standard getDisplayMedia path retains its existing defaults.

The extension base also checks the selected source before RegisterStream. A
strict request cannot register a non-window source or audio classified other than
application. This protects against another picker/backend returning a broader
selection despite the request. A normal user cancellation stays an empty result.
The source must still pass upstream active-frame, origin, capture-policy and
chooser gates; their checks and capture indicators are retained.

The callback adds optional `omniplayCaptureProtocol: 1` and `audioScope` fields.
Scope is none when audio was not authorized; otherwise it is the selected native
classification application, system, tab or unspecified. This describes the
browser's selected DesktopMediaID, not an independent measurement of an OS audio
driver. The original one-use token and audio checkbox authorization stay intact.
The pinned renderer binding forwards the options/result dictionaries and does
not need a custom permission or callback bypass.

## App admission

The component's window capture requests the strict flag whenever audio is opted
in. Before getUserMedia, a positive audio grant must also carry result protocol 1
and application scope. An unsupported app build, unknown result or broader audio
grant fails before a stream is requested. It does not silently reopen a broader
picker or capture a system mix. Video-only selection remains separately usable;
the user can explicitly choose another source/audio mode through the normal UI.

If no audio was authorized, only video is consumed. A valid native application
audio result is reported locally as `browser_reported_application`. Display
picker audio remains `user_selected_unspecified`; no scope is inferred from a
track merely existing. Source tokens and scope metadata are not sent through the
pairing/signaling protocol. The UI distinguishes reported application audio from
generic user-authorized audio.

The authored path uses Chromium's existing capture/audio infrastructure. It does
not bind the chosen window to a library game/PID, qualify process-tree behavior,
implement an independent capture/encoding service or prove application-only bytes
on a device. Those original native and performance requirements remain open.

## Source and final evidence

The fetcher retains exact source snapshots by Git blob identity. The desktop
generator includes all six native/schema changes in its source patch and stages
them into the selected checkout with existing edit-preservation guards. Shared
resource producers include the corresponding app-side admission. Preparation is
not a compiler, generated-binding or media result.

The final consolidated pass must cover generated option/result types and the
four-argument renderer binding; public/private/default callers; all window-audio
preferences; supported/unsupported loopback; native chooser labels/checkboxes;
cancel and stale results; invalid source types and broader backend results;
registration-before-use ordering; no-audio and application-audio getUserMedia;
track end/device changes; process-tree/window ownership; actual audio content,
A/V sync, latency and resource use; enterprise/private-profile policy; all other
browser/platform/security/license/release requirements. No requirement is marked
complete from this source change.
