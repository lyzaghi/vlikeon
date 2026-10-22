# Native frontend presentation source integration

This extends the separate host/client foundation with an optional software-core
frontend path. No CMake/SDK/compiler, native core, client/helper/thread/pipe/SDL/
window/audio/input/controller/OS operation or test ran. The Windows pair launcher,
optional signed catalog declaration and focus recovery are now authored in
`NATIVE_PRESENTATION_LAUNCH.md`; no catalog or platform availability is enabled by
this source. Windows direct mode remains the default and macOS/Linux native
isolation remains unavailable.

## Separate media path

The native worker accepts an additional exact bootstrap form:

`--stdio-runtime-v1 --presentation-client-v2 <session-uuid> <input-endpoint> <output-endpoint>`

The approved launcher must create, authorize and explicitly inherit the
private pipe pair and session. Endpoint numbers are finite canonical unsigned
decimal values, with no path or shell syntax. The client validates and takes
ownership of the actual endpoints. It requires Ready within five seconds before
the core is loaded. These are source rules, not proof of launcher ownership or
handle behavior. Standard control stdio remains a separate framed protocol.

An admitted client makes Engine initialize SDL without video/audio/gamepad
subsystems. The worker still uses reviewed SDL base-path/object/timer services
and loads its checked core beside its own executable. It creates no window,
renderer, output device or controller in that branch. Software libretro frames
retain the bounded RGB1555/RGB565/XRGB8888 conversion into checked ARGB words,
then the client produces owned RGBA packets. Actual packet dimensions must also
fit the host's admitted maximum. The guest's logical display width is sent
separately, so non-square-pixel games retain their declared aspect ratio.

Stereo callbacks use a separately published integer sample rate, drop data while
the guest is paused/not started, and enqueue unscaled signed samples through the
bounded client. The host applies the owner-limited user volume once. Existing
direct-mode ring/SDL callback behavior stays in its existing branch. A host source
rate update does not grant a guest access to an ambient audio server.

The pure snapshot mapper handles four ports, logical button remapping and masks,
triggers, radial deadzone sticks, analog keyboard overrides, nonanalog directional
fallback, normalized pointer coordinates and touch/down/offscreen state. It calls
no SDL device or keyboard getter. The direct profile now uses the same pure stick
normalizer for already validated settings. Stale/unfocused snapshots return
released controls. Per-port host/device haptics admission is checked before
finite rumble requests, in addition to the game's opt-in.

Polling refreshes the actual core-loop progress hook, consumes focus/epoch changes
and pauses the guest on release or expired input. The own-window Escape edge can
pause/resume a loaded guest; a newly received epoch does not synthesize that edge.
Unavailable presentation requests a close and guest pause so the existing broker
control channel can still request a final save. Actual guest pause/barrier/unload
precedes client Stop at shutdown. The worker keeps inbox/core process-lifetime
state alive beneath a blocked detached control reader on both success and error;
it does not unwind that memory while the reader may still reference it.

Submitted frames are not counted as presented. Polling consumes the helper's
actual SDL-present counter. That sampled counter and actor-loop intervals do not
yet provide a qualified display/audio benchmark clock, so brokered mode reports
measurement protocol 0 and rejects measurement start. Direct mode retains its
existing measurement protocol. No save/opaque state or account wire identity
changes are introduced by the media path.

## Presentation protocol 2

This unbuilt source advances the private media protocol to 2, including OMPRES02
magic, header version 2, Ready protocol 2 and the v2 helper/client bootstrap names.
Earlier protocol-1 source snapshots retain their exact inventories; there is no
fallback that silently treats an older descriptor as the new format.

Video has a 32-byte descriptor: raw width/height, exact RGBA stride, format 1,
logical display width, zero reserved u32 and bounded timestamp. Pixel bytes follow
at offset 32. Logical width is at most 40,960 and does not enlarge the raw pixel
allocation. The host sets its letterbox logical width and maps pointer/touch
coordinates against that same width.

State byte 1 is a finite fullscreen change: 0 leaves the current mode, 1 requests
windowed and 2 requests fullscreen for this own window. A failed fullscreen change
keeps the window and reports a finite reason; it does not grant global display
configuration rights. The helper also handles an actual own-window F11 key event.
Input gamepad byte 1 is a boolean device/owner rumble admission field, and remaining
reserved bytes stay zero. Disconnected records remain entirely released.

## Source, activation and remaining work

The source producer now freezes the frontend, presentation targets, selected SDL,
selected nlohmann/json and the pinned reviewed libretro interface/license. It
retains actual checkout-byte hashes separately from Git blob identities. The
earlier frontend-only candidate is out/presentation/host-v3-frontend-fenced-source: 1,627 files /
33,582,636 bytes, unbuilt/unapproved and product-disabled. The first
host-v3-frontend-source candidate and earlier snapshots remain preserved.
Native worker corresponding-source metadata now names its
presentation dependency; its normal build recipe defaults to direct mode, with
the optional Windows pair described in `NATIVE_PRESENTATION_LAUNCH.md`.
A future native core build still needs its actual compiled core identity
and artifact/SDK receipts; placeholders are not executable admission evidence.

The Windows pair/process launch, exact inheritance/session/hello agreement,
catalog helper admission, termination guards and coordinated focus resume are now
authored in `NATIVE_PRESENTATION_LAUNCH.md`. Their actual package/SDK/OS behavior
still needs the single final qualification. Start waits briefly for fresh actual
own-window focus and keeps an idle session recoverable if that grant is refused;
the source does not invent an activation grant or count a queued State as physical
playback evidence. POSIX pair ownership/activation remains required.

This is not OS containment merely because the cooperating frontend avoids GUI
calls. A contained core could attempt its own syscalls. Real POSIX filesystem,
process, IPC/network/device/display isolation still must prevent that authority,
and hardware-core/GPU paths plus qualified timing/measurements remain required.
Every original browser/runtime/platform/streaming/sync/security/licensing/store/
release requirement remains active.

The one final pass must cover real frontend/helper builds and bootstrap, exact
packet/input/aspect/endian behavior, queue and epoch/focus/pause races, callbacks
and guest barriers around save/restart/unload, final-save handling after helper
loss, actual user activation/fullscreen, physical haptics, audio clocks, sampled
counter semantics, blocked I/O and process death/cleanup, SDK/source/dependency/
license provenance and real primary-platform devices. No such pass was run.
