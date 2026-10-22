# Disc import and media identity

Disc handling is authored source and has not been run. A single-file game retains an immutable content hash and now records its actual format, including CHD, ISO, PBP, CSO and N64 byte order. Signature probes can identify several binary formats; an explicit file-extension hint handles formats without a unique header. The system/format compatibility check does not assert that a selected game will work in an untested core.

The CUE/M3U path begins with the native folder picker. The resulting expiring, single-use capability offers only supported entry points within that folder; the renderer cannot supply an arbitrary filesystem root. Entry discovery is bounded, referenced paths cannot be absolute or escape the selected folder, external URL references are rejected, and referenced files must be regular files. Static symlinks and junctions are rejected and canonical paths are checked before files are opened. A malicious concurrent directory-replacement race is not yet covered by handle-relative traversal; final filesystem-race review and platform hardening remain required.

The importer decodes UTF-8/BOM by default with explicit Windows-1252, Shift JIS, GBK or UTF-16 choices for existing libraries. It checks bounded cue directives, CD timestamps, track order and required index information. It rejects unsupported auxiliary-file directives instead of implicitly widening the folder grant. It normalizes references to generated ASCII filenames and keeps only referenced tracks, not an entire folder. M3U playlists contain at most eight local disc entries. The total content bound is four GiB, with at most 128 referenced assets and bounded metadata. Nested playlists, CCD/TOC/MDS descriptors and compressed archive extraction remain outside the current importer and need dedicated implementations where required.

Normalized descriptors and copied tracks are independently hashed, flushed and published as immutable blobs. A sorted manifest binds their generated names, formats, sizes and hashes into a domain-separated bundle identity used by saves. Re-import does not mutate an existing content identity. Interrupted or cancelled imports do not publish partial library entries; already committed unreferenced blobs remain available for explicit storage reclamation. A background import job exposes bounded progress and cancellation. Stalled filesystem reads may defer cancellation until the current read returns. Directory grants, import jobs and file transfers remain separate permissions.

The native worker stages every listed asset under the exact generated filename. The shared WASM frontend now has an authored manifest-identity, bounded chunk staging and read-only OPFS filesystem route for named assets, gated by actual catalog format/protocol declarations and core full-path support. It does not pass a cue file as if it were a complete ROM, supply an advanced binary or prove mobile playback. See `WEB_MEDIA.md` for temporary disk-copy/quota limits and remaining firmware/disc-control/state/device work. The native PS1 catalog admits only its explicitly listed content formats. The source recipe uses software rendering, no JIT and no direct optical-drive access, plus an immutable source revision and corresponding-source record. It does not imply verified PS1 compatibility or BIOS provenance.

Disc switching stays within one active runtime through the declared disc-control capability. The UI saves accessible in-game memory before switching. The OPNS0001 native state envelope contains core state and selected disc/eject state; incompatible state loads attempt to restore the previous state and disc. Save RAM and native snapshot metadata continue to use explicit version/configuration/platform compatibility. The later native PS1 source candidate now connects complete SRAM/files and disc snapshots to its checked card barrier, enabling an explicit default-off second-card choice; see `NATIVE_PS1_SAVES.md`. No binary/card/restore behavior has run.

The shared WASM frontend now also has explicit disc callback negotiation, paused
selection/rollback and its separate bounded OPDS0001 snapshot envelope, gated by
actual catalog/worker support. It binds the existing media list and save identity;
it does not supply an advanced binary or qualified multi-disc playback. See
`WEB_DISC_STATE.md` for callback, UI, compatibility and final verification limits.

Required final evidence includes real permitted single/multi-track images, non-ASCII descriptor names, each supported text encoding, multi-disc continuation, restoration to another disc, bad references, malicious descriptors, partial copies, concurrent imports, checksum failures, firmware region mismatch, audio timing and sandbox file/network denial. None of this verification has started.

Primary upstream references: [Beetle PSX source](https://github.com/libretro/beetle-psx-libretro), [Libretro PS1 core documentation](https://docs.libretro.com/library/beetle_psx/). Exact source headers and license files are retained in the pinned local dependency checkout.
# Native provider reader foundation

The common importer now also accepts an explicitly supplied scoped native reader,
with optional lengths, observed byte admission and the same CUE/M3U parser and
manifest identity. A separate profile-bound mobile callback ABI is authored;
Android and iOS now also have authored native picker/scoped-reader adapters.
See `NATIVE_DISC_PROVIDERS.md`. No provider, parser, import, compiler or test has run.


The standalone Web/PWA backend now also authors a selected-browser-FileList capability, equivalent bounded descriptor/reference planning intent, owned-byte hash snapshots and strict whole-bundle publication. Its cross-language normalization/identity equality and actual file/provider/browser behavior remain unverified. It does not inherit native inode/symlink guarantees from a FileList. See `WEB_DISC_IMPORT.md`; no parser/import/core/test has run and the complete original scope remains active.
