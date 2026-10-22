# NDS runtime source

A separate pinned single-Worker software Web candidate is now authored in
`tools/build_melonds_web.py`, with committed dependency materialization,
file-backed cartridge ownership, selected immutable system files and a
post-load system-memory floor. See `WEB_NDS.md`. This does not qualify a binary,
make the native/Web progress formats interchangeable or complete all NDS modes.

The initial NDS adapter uses melonDS DS 1.4.0 at immutable upstream revision
`f394adbacb5722ee97c1b37c8064da9a25818310`, with its ten selected source dependencies
resolved to full commits in `security/melonds-source.lock.json`. The maintained
melonDS DS frontend replaces the obsolete original melonDS libretro port for
this integration. Its core dependency is separately pinned, including its
original FreeBIOS replacement and source notices.

`tools/fetch_melonds_sources.py` retrieves only these immutable public source
inputs. `tools/build_melonds.py --prepare-only` authors an adapted source tree and
complete local dependency trees without CMake configuration, compilation or
tests. This prepared source exists in `out/native-sources/melonds-ds`. The actual
build recipe copies pristine inputs to a separate fresh output, disables runtime
dependency downloads and uses explicit local FetchContent overrides. No compiler
or linker has been invoked for the core or worker.

The initial Windows package declares DS mode, direct boot, native-size software
video, no JIT, no OpenGL/compute renderer, no network backend, no tracing and no
automatic microphone capture. Upstream's otherwise unconditional libslirp fetch,
build and unused attribution block are removed from the adapted offline source.
DSi NAND, host SD-card mirroring, real microphone capture, GBA slot content and
other expanded modes are outside this adapter's declared capability. They do not
become supported because the upstream core also implements them.

Users supply the selected ARM7 BIOS, ARM9 BIOS and firmware through the private
firmware vault. These are staged into the per-session work directory and included
in save compatibility configuration. The adaptation rejects a native system-file
read failure instead of silently switching to generated firmware or FreeBIOS.
No manufacturer firmware is added to the
product bundle. Upstream FreeBIOS is separately identified as an original
BSD-2-Clause replacement; its binary/source notice, other embedded dependencies
and Open Sans OFL notices must accompany distribution. Source notices have been
inspected; complete artifact-level license/content verification remains pending.

The isolated software worker now implements the libretro pointer device for a
mouse or one active touch. It converts window coordinates through SDL's actual
logical presentation transform, includes letterboxing and aspect ratio, and
releases touch on pause/focus loss/cancellation. The core receives normalized
coordinates for its full dual-screen layout and handles bottom-screen mapping.
Physical gamepad buttons use standard Retropad positions. The worker also handles
joypad bitmask polling and core-option v2 defaults without discarding its signed
configuration overrides. This is stylus input; it does not claim a complete
native mobile virtual gamepad overlay.

A signed runtime may require a minimum system-RAM observation after content load.
The NDS declaration requires 4 MiB. melonDS DS's error-message screen reports zero
system RAM, so the broker cannot label that error display a ready game session.
The guard does not prove that every game boots or plays correctly. Immutable
in-game saves and exact-version native snapshots use the existing common save
protocol; restoring, autosaving, pointer accuracy, sound timing and controller
behavior require the final consolidated real-device/compatibility verification.

The dependency source lock, source preparation and build recipe are authoritative
authoring outputs. No binary is admitted into the installed runtime catalog until
its build, package inventory, distribution approval and release policy permit it.
No FPS, latency, compatibility or device result has been fabricated.

Primary source references: [melonDS DS](https://github.com/JesseTG/melonds-ds),
[libretro NDS migration guidance](https://github.com/libretro/docs/blob/master/docs/library/melonds.md),
[original replacement BIOS source](https://github.com/JesseTG/melonDS/tree/16f127dbb587a73371827ce79689c5d237c4b59b/freebios).
