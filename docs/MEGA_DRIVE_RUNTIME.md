# Mega Drive runtime source

The initial native Mega Drive adapter uses ares revision
`9408cb43d4948fc3ea6e152a307a34348df3fe04`. The adapter exports a real libretro
boundary over the upstream CPU, sound, video, cartridge, mapper and controller
implementations. It identifies itself as `OmniPlay ares MD` with version
`0.3.0+ares.9408cb43d494`. No binary, gameplay or performance result exists yet.

`tools/build_ares_md.py --prepare-only` authors the selected corresponding-source
project in `out/native-sources/ares-md`. It does not configure CMake, compile,
execute a core or run a check suite. The later explicit build recipe uses a fresh
output, a selected compiler and the Windows fiber backend. Other operating-system
containment backends and native mobile integration remain project requirements.
Preparation now materializes explicit paths from the exact committed tree and
the exact pinned libretro header, records their identities, and retires its
temporary input archive/tree after retaining selected corresponding source.
Working-tree additions are not compile inputs. Nall's header-only definitions
are explicitly selected by the native and Web recipes.

The selected source omits the ares resource aggregator, desktop UI, embedded
firmware and unrelated binary/media resources. Cartridge mapper/storage/region
analysis is adapted from the pinned upstream mia implementation without its file
loader or SVP firmware fallback. TMSS is disabled for this adapter. Games requiring
SVP use the selected 2 KiB user firmware from the private vault; ordinary games
do not require it. The native catalog marks this file optional for staging, while
the adapter rejects SVP content if the selected file cannot be loaded. No firmware
download, embedded manufacturer byte array or automatic firmware extraction is
added to the product.

The shared SH2 declarations referenced by ares's MD component use interpreter
types in this build. Dynamic-code compiler implementations and the Sljit backend
are omitted, and the generic recompiler reports unsupported. Shared allocator
storage has no executable flag. Windows contexts use the new stable-handle fiber
backend, including real re-derivation on reset/re-power. Pending entrypoint
callbacks are retired before replace/destroy. The same backend is authored into
the native bsnes recipe. Guest stack/count bounds and owner/fiber checks are
explicit; actual API behavior and containment remain unqualified. The adapter
loads only the Mega Drive system models;
it does not claim Sega CD, 32X or LaserActive support merely because ares also has
related source. Existing Chromium and native containment policies are retained.

Video delivery is synchronous with the libretro calling thread, bounded in size
and exported as XRGB8888. PSG/FM streams resample to 48 kHz and mix with bounded
pending samples. The frontend receives the actual upstream refresh-rate hint.
The normal PAL/NTSC region policy and the upstream list of games incompatible
with six-button controllers inform automatic selection. Three- and six-button
options also exist at the approved core-configuration boundary. The physical
Retropad mapping supplies A/B/C, X/Y/Z, Mode, Start and directional controls for
two normal controller ports. Complete remapping/virtual controls, remaining
special peripherals and the real compatibility matrix still require work.

The current native file profile captures upstream canonical SRAM and EEPROM
images through checked Pak writers and private `save/MD` files. Standard, linear
and banked board memory accessors are authored; the admitted file profile keeps
libretro SRAM/RTC regions empty and captures both declared files. The barrier
checks the actual core save root, Pak mode/size/write completion, full temporary
write/flush/close and a fixed-name Windows replacement result. Its single-owner
pause entry has no background writer to join: publication only runs between
core calls and optional threaded video is disabled. The worker requires actual
checked I/O and pause exports; a matching binary/source/policy receipt is required
by the package recipe. In-game saves use OPSF0002; OPNS0002 snapshots include
core state and files with restart/recovery. The 0.3 source now declares the
bounded canonical in-game file profile across selected native/WASM candidates;
opaque states remain specific to their runtime/platform. The actual loaded-game
descriptor, file-set preflight, metadata/worker admission and explicit 0.2
readers are documented in `MEGA_DRIVE_PORTABLE_SAVES.md`. This is source
behavior, not qualified interoperability or storage durability.

State decoding now bounds primitive reads/growth, compares the fixed version
bytes and requires synchronized chip state. It cannot load raw native coroutine
stack images. Chip-specific state invariants, allocations, termination and every
recovery stage still need final fault/security qualification. A minimum 64 KiB
system-RAM observation guards native preparation; it does not prove boot or
compatibility. Finite region and three-/six-button choices use the common local
per-game editor and participate in configuration compatibility.

The standalone WASM source recipe and its private-file/result bridge are
documented in `WEB_MEGA_DRIVE.md`. Both source projects are materialized without
building or executing a runtime. Compiled/device/release evidence remains due.

Upstream ISC and component notices accompany the selected source, with SDL and
JSON notices added by the common worker packager. Full compiled dependency,
binary-content, corresponding-source, containment, controller, save and timing
evidence remains pending. The runtime cannot enter the installed registry until
the later build, signed inventory and distribution policy admit it.

Primary sources: [ares repository](https://github.com/ares-emulator/ares),
[pinned license inventory](https://github.com/ares-emulator/ares/blob/9408cb43d4948fc3ea6e152a307a34348df3fe04/LICENSE),
[pinned Mega Drive source](https://github.com/ares-emulator/ares/tree/9408cb43d4948fc3ea6e152a307a34348df3fe04/ares/md).
