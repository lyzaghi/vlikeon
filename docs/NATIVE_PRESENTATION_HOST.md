# Separate native presentation host foundation

The current native actor owns SDL window/audio/input inside its Windows
AppContainer. Native isolation remains unavailable on macOS/Linux in the current
product registry. This new source starts the separate presentation path needed to
keep native cores away from ambient X11/Wayland/Pulse/WindowServer authority.
It does not enable those platforms, loosen the existing sandbox, or claim an
isolated working product. No compiler, configuration, helper, SDL/window/audio/
controller/pipe/OS API, browser/device or test was run.

## Boundary and wire

`engines/presentation` authors a standalone trusted SDL presentation helper plus a
reusable binary wire library. The helper never loads a core or ROM and accepts no
filename, library path, executable command, JSON or native handle on its wire.
It displays bounded raw pixels and queues bounded signed stereo PCM; it returns
snapshots of input for its own game window. A future broker must own the helper's
approved executable/clean environment, session, private pipe endpoints, lifetime
and fixed maximum dimensions/user volume/haptics policy. The producer-side
client library is now authored in `NATIVE_PRESENTATION_CLIENT.md`; core frontend
and private-channel launch integrations remain due.

The fixed little-endian 56-byte header is:

| Offset | Field |
| --- | --- |
| 0 | Eight-byte OMPRES02 magic |
| 8 | u16 protocol 2 |
| 10 | u16 closed message kind |
| 12 | u32 zero flags |
| 16 | Sixteen-byte nonzero session UUID |
| 32 | u64 strictly increasing per-direction sequence, 1 through 2^63−1 |
| 40 | u64 host-issued focus/lifecycle epoch in the same range |
| 48 | u32 bounded body length |
| 52 | u32 zero reserved field |

Header/session/direction/type/sequence/epoch/body limits are checked before body
allocation. The helper's owner-selected dimensions reduce the video allocation
bound further. A malformed/truncated/replayed/wrong-direction frame closes the
channel rather than attempting resynchronization or logging payload contents.

| Actor to host | Shape |
| --- | --- |
| Video | 32-byte descriptor: width/height, exact width×4 pitch, RGBA8 format 1, logical display width, zero reserved u32, bounded timestamp; tightly packed bytes, dimensions 1–4096 |
| Audio | 16-byte descriptor: 8–192 kHz, 1–8192 frames, two channels, signed 16-bit LE format 1, zero reserved; exact frames×4 bytes |
| State | Eight bytes: running boolean, fullscreen change 0–2, volume 0–100, zero u32 |
| Rumble | Eight bytes: port 0–3, zero byte, two u16 strengths and duration at most 250 ms |
| Heartbeat / Stop | Empty body |

| Host to actor | Shape |
| --- | --- |
| Ready | 24 bytes: protocol, maximum dimensions, maximum audio rate and frames, haptics admission |
| Input | 160-byte full snapshot: 512 physical scancode bits, four fixed gamepad button/axis records, focus/inside/down and normalized pointer/wheel fields |
| Metrics | 32 bytes: presented/replaced-video/dropped-audio counts and finite reason/running fields |

Gamepad records have explicit connected/rumble-admission/reserved/button/axis fields; disconnected
records are zero. Unfocused input snapshots are entirely zero. No device name,
serial, host path or other application's keyboard event is emitted. Relative
pointer delta fields are presently zero; installed-game remote control remains
its separate protocol and is not replaced by this local core presentation wire.

## Host behavior and bounds

One SDL main thread owns the literal OmniPlay window, renderer, audio stream,
controller handles, touch/mouse mapping and haptics. Incoming video has one latest
slot; old queued frames are replaced and counted. Audio has at most 32 packets and
128 KiB of PCM, also capped at 250 ms for the current source rate. Oversized audio
chunks are dropped, rate changes retire earlier queued audio, and oldest data is
discarded when necessary. There are at most eight
pending nonmedia commands. A current consumer batch and one bounded reader frame
can coexist with these queues; this is not a claim that process RSS is one frame.
The SDL playback queue is also checked and cleared when its source-format data
exceeds roughly 250 ms. Actual output-device/driver buffering is unqualified.

Pixel uploads use a streaming RGBA texture and letterbox scaling. The loop takes a
new video frame and publishes a latest input snapshot at intervals of at least
8,333 microseconds. This is a source pacing limit, not proof of 120 Hz presentation,
low latency or GPU support. Metrics report only actual helper-side operations and
queue replacement counts; they are not yet connected to product telemetry.

Focus changes advance the host epoch, clear queued audio, stop haptics and publish
a released input snapshot. A running actor with no admitted packet for 250 ms is
fenced similarly. Earlier-epoch media/control cannot reactivate playback; a future
epoch is a protocol failure. New-epoch State must still be admitted while the own
window is actually focused. Volume is clamped to the owner-provided ceiling and
haptics require the owner's explicit admission and current focus/running state.
The helper opens at most four gamepads and samples at most 32 enumeration IDs.

Independent pipe threads prevent a blocked outgoing pipe from holding the SDL
main loop. Input and metrics each have one latest outgoing slot; the Ready message
precedes snapshots. Thread/channel errors close the bus. Stop, close or EOF closes
the window/audio/controllers before later queued media or commands are applied.
Private protocol descriptors have close-on-exec/noninherit flags; ordinary process
stdio is redirected to null before SDL initialization. Blocking pipe threads refer to process-lifetime
state: success/failure closes SDL resources and exits without unwinding that state
beneath a blocked reader/writer. The future broker must still enforce process-tree
termination, deadlines, session identity and descriptor ownership; script or
thread intent alone cannot prove an OS operation has stopped.

## Source producer and remaining integration

`tools/prepare_presentation.py` can freeze this helper and selected SDL 3.4.18
source from revision 829a65d769d935c4852f8159e964312c0957260a. It reads Git metadata/
tracked index paths, copies ordinary bounded source and inventories actual checkout
bytes. Git blob IDs are recorded separately: Windows-converted checkout bytes are
not claimed to equal original Git blob bytes. It invokes no CMake/SDK/compiler or
helper and emits no executable/admission evidence. The CMake project requires the
explicit frozen SDL source and authors a wire library plus host target, with SDL
tests/examples disabled. Actual source coverage and SDK/driver/transitive compiled
inputs, notices and license completeness remain required final evidence.

The earlier protocol-1 source candidate is out/presentation/host-v1-private-handles-source; the first
host-v1-source and host-v1-bounded-source snapshots remain preserved. The current
candidate inventories 1,542 files / 31,059,149 bytes. It remains unapproved,
unbuilt and product-disabled. The current Windows worker/catalog and unsupported
macOS/Linux isolation decisions are unchanged. The optional software frontend is
now authored in `NATIVE_PRESENTATION_FRONTEND.md`; broker-owned private duplex
launch, helper/package/SDK admission, real POSIX containment and process-tree
cleanup, hardware GPU presentation paths and the entire original platform scope
remain required.
Granting an emulator ambient desktop sockets is not treated as completing it.

The current unbuilt source now advances to media protocol 2 and connects the
optional native software frontend as described in
`NATIVE_PRESENTATION_FRONTEND.md`. Earlier protocol-1 candidates above are
historical. Windows pair launch, optional catalog selection, process guards and
focus recovery are now authored in `NATIVE_PRESENTATION_LAUNCH.md`; no binary or
platform admission follows from that source. POSIX containment and activation,
actual package/SDK admission and hardware presentation remain unfinished.

Lower-level GNU/Linux pair and owner-focus source is now authored in
`LINUX_PRESENTATION_PAIR.md`. Its gated CLI establishes trusted owner credentials
before SDL, accepts a separate fixed owner channel and requests only its own
window on the main thread. Core media still uses independent protocol 2 and
fresh observed focus. Linux registry/adapter enablement, complete package/work/
delegation lifecycle and real compositor/driver qualification remain unfinished.

The single final verification must cover the actual cross-platform build/ABI,
direction/header/length/sequence/epoch and malformed body behavior, full-size
allocation/queue limits, producer/consumer cancellation and blocked pipes, input
focus/letterbox/touch/controller/haptics races, source rate/format/audio queue
changes, SDL/driver failures, watchdog/pause/resume behavior, process death/cleanup,
real device latency/memory/performance and compiled dependency/source provenance.
No such evidence exists. All original browser/runtime/mobile/streaming/sync/
security/licensing/store/release requirements remain active.
