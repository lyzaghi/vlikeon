# Independent Windows owned-window capture candidate — item 56

Source only. No CMake configure/build, native dialog, capture session, D3D/driver,
window/frame/device operation or test was executed. No product capability is
enabled and no current worker/broker command calls this capture class.

`engines/encode-windows/src/capture.cpp` adds `WindowCapture`, included by the
static-library recipe. The factory accepts the trusted host's original game
process HANDLE, exact top-level HWND and finite 720p/1080p 30/60 profile/lifetime.
It duplicates only query/synchronize process access and never finds game authority
by PID scan. A caller must already have verified installed game/pair/document
ownership and an accessible independent Stop action before calling this API.

## Source ownership and consent

Before media starts, the candidate validates the original process object and
window's current PID, visibility, uncloaked/nonminimized top-level state and
monitor. It retains a native GraphicsCaptureItem and subscribes to its Closed
event before presenting a default-No native confirmation naming the window.
This preserves the original capture item across the dialog instead of selecting
by a potentially recycled HWND afterward. Native process/window/expiry checks
repeat after consent and setup. No frame pool/session starts before local Yes.
Title changes, item/handle lifetime races and real OS event ordering still need
qualification; no atomic race-free OS proof is claimed.

The normal Windows capture indicator is retained; no border-suppression API or
permission is requested. No camera, microphone, system audio or arbitrary display
capture is supplied. A foreground change alone does not revoke window viewing;
the separate input path still controls its own focus/override rules. Pair/viewer
fingerprint presentation, localization and product Stop UI remain host integration
requirements; the dialog is not a complete paired-session authorization workflow.

## Frame ownership and GPU processing

The free-threaded WGC frame pool uses two buffers. Its callback retains at most
two native frames, releasing oldest raw frames on overload. Callbacks hold shared
source state, never a raw WindowCapture pointer. An item Closed event, changed
content size, process loss, window/monitor loss or original deadline closes
delivery. A native frame Close failure is retained as terminal uncertainty.
Window resize/monitor migration currently requires a new generation; seamless
reconfiguration remains due.

The owner-thread consumer selects the newest raw frame, checks age and validates
the BGRA texture's dimensions/device/format/subresources. Source dimensions are
bounded by 8192 per axis and 16,777,216 pixels. Only an output reporting full-range
SDR P709/G22 is admitted; HDR/unknown output is rejected. Monitor/display state is
rechecked before and after readback. This is a conservative candidate admission,
not proof of per-window ICC/advanced-colour behavior. HDR/wide-gamut support and
real SDR colour validation remain unfinished.

A D3D11 video processor scales the chosen content with aspect preserved and black
letterboxing to the fixed output profile. Auto processing is disabled; full-range
RGB processing and opaque output alpha are requested. The immediate context is
multithread protected. One output texture/staging texture is retained; readback
copies only tightly packed rows into owned BGRA bytes. Output alpha, process,
display, frame Close and lifetime are checked before transferring bytes.

Output rate is bounded by the requested profile using a local monotonic clock;
stale raw frames are dropped rather than encoded. Source presentation timestamps
must increase and are rebased to capture-relative microseconds. Native frame
receipt-to-copy age must remain within 100 ms. Scheduling rate, video processor
quality, CPU/GPU cost and actual latency are unqualified. No encoder acceleration
or zero-copy claim follows from using a GPU scaler.
The returned frame includes the original callback's GetTickCount64 receipt value;
the future native adapter must preserve that age across its transport/conversion
instead of starting a fresh budget when it constructs a Rust mailbox frame.

## Teardown and unfinished integration

Factory/take/stop/destruction run on one dedicated COM MTA owner thread. The
normal native prompt, D3D Map/video processing and WinRT Close can block and need
external process supervision. close_delivery sets the shared fence before queue
cleanup; it does not preempt an in-progress OS call or recall already-returned
bytes. The future host must check owner/capture generation and authority before
encoding and publication, independently of returned frame data.

Explicit stop revokes callbacks and requests session/pool Close. Failure retains
references in the live object and never yields a successful Close receipt.
Destructor cleanup is best effort only. Actual original helper-process retirement
is separate from these API receipts. Startup failures also close/fence resources;
their OS cleanup still needs failure-injection qualification.

This candidate is not called by `omniplay-video-worker` or the Rust broker frame
bridge yet. The current video pipe accepts raw NV12, not window tokens or process
handles. An admitted private native capture adapter/transport must connect WGC
owned BGRA to the existing explicit colour converter/mailbox without exposing
process/window identity to a renderer. Installed admission, pair/document lease,
accessible Stop UI, application audio, receiver negotiation/transport, other
platforms and full original item-56 scope remain open.

Primary API references reviewed:

- [CreateForWindow](https://learn.microsoft.com/en-us/windows/win32/api/windows.graphics.capture.interop/nf-windows-graphics-capture-interop-igraphicscaptureiteminterop-createforwindow).
- [CreateFreeThreaded](https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.direct3d11captureframepool.createfreethreaded) and [FrameArrived](https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.direct3d11captureframepool.framearrived).
- [VideoProcessorBlt](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorblt), [alpha fill](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11videocontext-videoprocessorsetoutputalphafillmode) and [colour state](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/ns-d3d11-d3d11_video_processor_color_space).

Installed SDK declarations were inspected read-only. They are not compile,
capture/driver execution or device evidence. Build/test stays deferred by owner.
