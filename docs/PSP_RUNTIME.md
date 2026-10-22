# PSP native source integration

Current native product versions use `sw3`/`gl3` and an explicit FFmpeg 9.0.2
override. The original parent gitlink lock and previous source projects remain
retained. Current native source paths have the `-v3-committed` suffix and use
committed archives plus complete adapted-source/producer inventories. An actual
product build uses `--build-prepared` and requires `--ffmpeg-receipt` for the exact
compiled modern static decoder and the generated committed atlas receipt. See
`PSP_SOURCE_PROVENANCE.md` for the producer and preparation/build boundaries, and
`PSP_DECODER_MAINTENANCE.md` for API/ownership/budget changes and pending evidence.

The shared Web player has a separate UI-owned pthread/typed-callback contract
in `WEB_THREADS.md`, and `WEB_PSP.md` describes its pinned software/IR-interpreter
producer, source multimedia recipe, read-only system assets and joined file
barrier. No PSP Web executable has been built or approved. Actual multimedia,
file/thread/core behavior, GPU and platform/device qualification remain open
alongside the native work below.

The selected PPSSPP parent is `a5275067b62433375bbde12921e96329a795dff0`. `security/ppsspp-source.lock.json` records thirteen selected gitlinks resolved directly from that parent, including libretro-common, bundled FFmpeg, shader build dependencies and libchdr. `tools/fetch_ppsspp_sources.py` materializes those immutable sources without configuring or executing them. Optional test repositories, Discord, UPnP, mobile GPU drivers, OpenXR and platform frontend modules are excluded from this selected dependency set.

`tools/build_ppsspp.py --prepare-only` authors the adapted corresponding-source tree. The explicit later build targets a Windows x86-64 libretro library with bundled FFmpeg built from source, software GPU, one-times native resolution and the MIPS interpreter. CPU/vertex JIT capability is disabled, network options are off, and the existing worker retains its network-denied AppContainer and executable-heap mitigation. Upstream GPU/JIT implementation files are still compiled build inputs; they are unselected by this adapter. This baseline does not fulfill the product's later GPU/JIT, Android/iOS or performance requirements.

The adapted loader completes upstream boot initialization before reporting Ready. It does not execute a guest frame in preparation. Missing system assets request shutdown, and snapshot loads return their actual result synchronously instead of acknowledging a deferred load. The worker now accepts libretro's null-data shutdown notification and rejects failed initialization before loading content. Actual boot/failure semantics remain unverified.

Approved runtime assets live read-only beside the worker under `assets/PPSSPP`. The package includes exact sizes/hashes for compatibility data, the PPGe atlas/metadata and selected upstream replacement font assets. Asset hashes are part of save compatibility. Internal writable data uses private session storage. No manufacturer BIOS is required by upstream PPSSPP's HLE design. The font/atlas provenance and all compiled third-party license obligations remain explicit distribution review items; source inclusion alone does not establish clearance.

The broker creates separate writable `save` and `scratch` subtrees, grants only the game container write/delete access and applies a low mandatory integrity label. Temporary files use `scratch`; ROM/firmware and installed runtime assets remain readonly. Data grants allow traversal on directories without granting file execution. Actual DACL/integrity inheritance remains part of the final containment pass.

PSP saves are files, not SRAM. The worker's `SavedFiles` interface captures `save/PSP/SAVEDATA` after a guest pause barrier and an exported upstream asynchronous-I/O barrier. `OPSF0002` preserves empty directories as well as zero-byte and ordinary files, with a 256 MiB envelope, 4,096-file/8,192-entry bounds, eight-level path limit and strict UTF-8 names. The decoder also reads the old file-only `OPSF0001` shape; the authoritative compatibility contract and UI declare only that concrete one-way reader relation. Existing immutable save records keep their original identity. A subsequent save creates a normal v2 child. See `PSP_SAVE_FORMAT.md` for the wire format and failure boundaries.

Traversal, device names, alternate streams, case aliases, file/directory collisions and links/reparse points are rejected. Windows conversion uses strict UTF-8 decoding, invariant Unicode case mapping and explicit UTF-8 filesystem paths. Exclusive file creation also rejects aliases at the actual filesystem boundary. This is an account-independent save blob handled by the existing atomic history/export/sync path. Other desktop filesystems still need their own filename-equivalence policy; non-ASCII behavior is not declared portable to those unimplemented adapters.

Snapshot format `OPNS0002` contains both exact core state and the persistent-file archive. Restoration first validates the complete archive, captures the current state, shuts down guest/VFS/file handles, replaces the save directory through private staging/rename and reinitializes the core. A failed state load restores both prior files and prior guest state; rollback failure terminates the contained runtime. Ordinary in-game restore restarts the guest with the selected savedata. No portable cross-core or cross-platform snapshot claim is made.

The new directory archive is bounded at capture/restore; a running guest's disk-write quota and profile cleanup remain separate required containment work. Hardware capture/presentation, full-speed performance, device input behavior, filesystem power-loss behavior, async I/O, rejected-state recovery and all legal/content verification still require the final consolidated pass. No PSP core/worker has been compiled, executed, approved or benchmarked.

The Windows worker manifest declares per-monitor DPI awareness and long-path awareness. Archive filenames still have explicit byte/depth/component bounds, and actual system long-path behavior and Unicode collision cases remain unverified. Completed Windows staging writes flush their file handles before directory replacement. Oversized save captures return a recoverable resource-limit response without destroying a healthy session; malformed/incomplete IPC remains terminal.

Provenance inspection found that the pinned PPGe atlas script names KozGoPro and Arial Unicode font files from a Windows directory. The product therefore requires an atlas rebuilt from audited open font sources instead of treating that source reference as redistribution permission. Official PPSSPP documentation identifies its supplied PGF files as free replacements, but exact asset notices/source identities still require a separate inventory. These decisions are recorded in `security/retro-provenance-review.json`; they have not been cleared by a product build. [Official font documentation](https://www.ppsspp.org/docs/getting-started/using-original-fonts/).

That open-font rebuild now has immutable, materialized Noto/FreeType source inputs, retained OFL/FTL notices, a generated local atlas script and an explicit source/build recipe. The product core build requires the matching generated-atlas receipt rather than silently copying the upstream atlas. Source preparation has completed; the generator and output verification have not run. See `PSP_OPEN_FONT_ATLAS.md`.

A separate OpenGL/IR-interpreter profile and shared threaded lifecycle integration are now authored; see `PSP_OPENGL_RUNTIME.md`. The software profile remains available as its own package identity. Neither profile has been compiled or executed.
