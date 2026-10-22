# WASM disc switching and disc-aware snapshots

The shared WASM frontend now has authored libretro disc callbacks, paused disc
selection and disc-aware snapshot/rollback. No C/JS compiler, interface callback,
disc/game, state decoder, browser/mobile application or test has run. No advanced
WASM core binary, image, manufacturer BIOS or compatibility result is supplied.
This closes a source-level frontend gap; it does not complete qualified PS1/PSP
playback or the original platform/browser/streaming/sync/storage/release scope.

Only a core catalog explicitly declaring `disc_control`, disc-state protocol 1
and media filesystem protocol 1 can enable the path. Resource assembly requires
that combination and a bounded snapshot base version; M3U declarations also
require it. Runtime ranking omits a playlist candidate missing its disc snapshot
protocol. Existing GB/GBC/NES/GBA recipes do not advertise disc controls merely
because their shared C frontend has the implementation. The development catalog
continues to contain no built/admitted emulator.

The worker handles the base and extended libretro disc-control environment
interfaces plus interface-version discovery. It checks wasm32 table alignment,
extent and the seven required callback pointers. A small linked C frontend
copies only the five used callbacks with their C boolean/unsigned signatures;
JavaScript does not guess table-call signatures or retain the supplied table
pointer. The C prefix and pointer size have compile-time assertions. Optional
extended labels/paths/initial-image callbacks are not invoked. Replacement/add
callbacks are never exposed: user selection stays within the core's existing
admitted image list and read-only media namespace. Trusted compiled core callback
validity still requires actual ABI/build qualification.

After loading, an enabled core must report one to eight images, a valid selected
index and boolean eject state. A normalized M3U list must agree with its admitted
entry count. Invalid/missing callbacks or counts fail preparation rather than
claiming disc support. This bounded contract does not expose an empty-image
sentinel, arbitrary newly selected image or native optical device.

Disc changes require a loaded paused game. The controller reads the previous
state, opens the tray when needed, selects an existing index, optionally closes
the tray and verifies the resulting index/count/eject state. If an operation
fails, it attempts to restore the previous tray/index state. Failed restoration
marks a fatal runtime error; the UI cannot resume as if it succeeded. Merely
returning false from a core callback is not treated as successful rollback.

The player shows numbered discs and current/open-tray state only after an actual
enabled core reports support. It pauses, waits for the current save and commits
available SRAM/RTC before changing the disc. It blocks overlapping save/load,
export, resume, preparation and disc actions. Previous snapshots remain in
history. A successful change refreshes the selected disc and may resume only if
the session was running, the document still has focus/is visible and no newer
pause/lifecycle notification occurred. Failure leaves it paused and refreshes
observable state when possible. User progress without exposed SRAM/RTC still
needs its core-specific persisted-data implementation and qualification.

Disc-enabled emulator snapshots use `OPDS0001`: an eight-byte magic, little-endian
image count and selected index, one eject byte, three zero reserved bytes, a
little-endian raw-state length and the raw core state. The fixed header is 24
bytes and the whole payload must fit the existing 256 MiB save bound. Snapshot
creation checks that disc state did not change during serialization. No firmware,
host path, image payload or extra file reference is stored in this envelope.

Loading validates envelope size/magic/reserved fields, exact raw length, bounded
count/index and agreement with the currently admitted image list before applying
it. The frontend captures the previous raw core/disc state, selects the target
disc/tray, unserializes and verifies disc state again. If any stage fails, it
attempts to restore both previous disc and raw core state. A failed core/disc
rollback becomes fatal; no successful load or resume is reported. Slice views
avoid unnecessary whole-payload decoder copies, but target/backup/core snapshot
memory can still be large and requires real device qualification.

Save metadata appends `.opds0001` to that catalog's snapshot base format only for
disc-state protocol 1. Old unwrapped states therefore are not silently passed to
this decoder. The actual game bundle hash, core/build/configuration/firmware and
existing save compatibility still apply. Non-disc emulator-state bytes retain
their old raw format; the SRAM/RTC envelope remains unchanged. Loading any saved
progress now requires the worker to be paused. UI disc status refreshes after a
successful state restoration, including restoration to another disc/open tray.

Core unload/deinit occurs before callback ownership is cleared. The existing
bounded worker command queue keeps mutation requests in order. Runtime stop and
native media cancellation retain their documented cooperative I/O limits; a
worker/native/core call can still stall until its deadline or termination.
General core/controller/file/device behavior, power-loss/crash recovery, memory
pressure and actual multi-disc save continuation have no execution evidence yet.

The final single verification pass must compile the wasm32 callback shim against
every admitted core and exercise base/extended registration/deregistration,
unaligned/out-of-bounds/null/missing callbacks, counts and playlist disagreement;
same-index and cross-disc selection, open trays, every failing transition and
rollback; envelope magic/reserved/length/count/index bounds, large states and
raw-format compatibility; unserialize rejection/throw/disc changes and fatal
rollback; multiple-disc continuation, SRAM/memory-card persistence and snapshots
to another disc; UI overlap, lifecycle interruption and no unintended resume;
native/OPFS media reads during core callbacks, cancellation/unload and real
device memory/latency. Every other original full-product requirement and release
evidence remains due.

Primary interface: [libretro disk-control callback ABI and lifecycle](https://github.com/libretro/libretro-common/blob/master/include/libretro.h).
The exact pinned PS1 header is retained in `third_party/sources/beetle-psx`.
