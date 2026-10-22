# Actor-side presentation client source foundation

This extends `NATIVE_PRESENTATION_HOST.md` with the producer-side library. The
optional native software frontend is now connected in source as described in
`NATIVE_PRESENTATION_FRONTEND.md`; broker process launch is still required. Current
native availability, catalog and Windows direct presentation stay unchanged;
macOS/Linux isolation remains unavailable. No configuration, compiler, client/
helper, thread/pipe/handle/signal/window/audio/controller/OS API or test ran.

## Owned transport and admission

`Client` takes a nonzero binary session identity and two inherited endpoints from
the future broker-controlled launcher. It does not accept paths, environment
lookups or endpoint commands on the presentation wire. Windows admission checks
pipe handle type, duplicates noninheritable handles and binds binary private CRT
streams. POSIX admission accepts only bounded descriptors above stdio, checks FIFO
type and read/write direction, and duplicates with close-on-exec. Successful
admission consumes the original endpoints; failed admission leaves them with
their caller. The caller still must prove which launcher/session owns them.

The wire object now owns and closes its private FILE streams when no I/O user
remains. POSIX pipe setup ignores SIGPIPE so a broken pipe can report a channel
failure instead of immediately terminating a cooperating client. This is process
signal state, not protection against arbitrary contained native code changing
its own handlers. No native code or OS behavior was executed to qualify this.

The receive worker requires Ready first, validates the existing closed protocol
and captures the host's maximum dimensions and haptics admission. Its bounded
wait helper can wait at most five seconds. Repeated Ready is a protocol error.
Earlier-epoch snapshots are ignored. A new host epoch releases old input and
retires unsent earlier-epoch controls/audio/video. It does not automatically
resume an actor through a focus/lifecycle fence.

## Bounded producers and snapshots

Video accepts a checked owner-bounded ARGB word image, converts into explicit RGBA
wire bytes and keeps one latest pending frame. It checks the epoch again after
copying, so an expensive frame copy cannot queue under a superseded input grant.
No guest pointer, SDL object, native handle or backing-file descriptor crosses
this interface. Pixel copying still consumes bounded memory and CPU and does not
establish a zero-copy or high-refresh rendering result.

Audio accepts finite signed stereo samples and an 8–192 kHz source rate, encodes
samples explicitly as little-endian and splits into roughly 10 ms packets. Its
queue has at most 32 packets and 128 KiB of PCM, additionally limited to 250 ms
at the current rate. Old audio is retired on source-rate or epoch changes; old
queued data is dropped to stay within the bound. A blocked pipe never makes a
core callback write directly to the OS. Snapshot input is copied as a fixed
160-byte value, with unknown/stale/unfocused input returned released.

Playback commands enforce the received focus snapshot and finite volume field.
Pause retires queued media/controls, so earlier rumble or resume work cannot remain
behind a new pause in the pending control queue. Resume coalesces an older pending
State. Rumble requires current focus, a fresh snapshot, host admission, port bound
and at most 250 ms. These are send admission decisions, not an acknowledgement
that a physical device rumbled or an audio buffer has stopped.

Independent receive/transmit workers hold shared process-local state with no core
callback or SDL pointer. There is one latest video and at most eight pending
controls. Control messages precede queued media. The transmitter only sends normal
work while the future core loop has reported actual progress within 150 ms; a
blocked core's pipe thread cannot generate keepalives forever. Idle heartbeats
are spaced by 100 ms. Host input older than 250 ms becomes released. The future
core lifecycle must actually call this progress hook, consume epoch/focus changes,
pause the guest, and join its producers before teardown. That wiring remains due.

Closing requests a prioritized asynchronous Stop and rejects new producer work.
It does not claim a blocked write has ended, that the receive thread has joined,
or that the helper process has stopped. Shared state/transport can outlive the
front object while an OS call unwinds, without retaining a core-library callback.
Actual actor/helper process guards and termination/reaping must still be owned
by the broker. A future frontend must not use wire unavailability as proof of
process termination or discard the process handles on a timeout.

## Build source and remaining integration

The CMake source now exposes a separate client library with an explicit Threads
dependency. The client/wire target can be authored with the SDL host disabled;
building the host separately requires the pinned SDL source. The source producer
freezes both libraries and the helper and lists both build targets for the final
phase. It still runs no CMake, compiler, helper or test.

The earlier protocol-1 candidate is out/presentation/host-v2-client-source: 1,544 files /
31,072,719 bytes. All earlier source snapshots retain their own inventories.
It is unbuilt, unapproved and not enabled
in the product. The current protocol-2 optional software frontend and its newer
candidate are described in `NATIVE_PRESENTATION_FRONTEND.md`. Private endpoint
launch and inheritance, coordinated activation/focus resume and process-tree
ownership are now authored for Windows in `NATIVE_PRESENTATION_LAUNCH.md`.
Their actual OS behavior, package/toolchain/source-license admission, POSIX containment and
advanced hardware presentation remain due along with the full original product
scope. None is satisfied merely by the existence of this library.

The one final verification must cover both sides' real wire/ABI, inherited handle
failure/ownership/cleanup, wrong-role/session/sequence/epoch/caps, blocked readers
and writers, producer/epoch races around deep copies, audio rate/count/endian and
queue admission, input expiry, core progress versus heartbeat, pause/resume/
rumble ordering, actual teardown/EOF and parent process death, primary platforms
and memory/latency/resource behavior. No such pass was run.
