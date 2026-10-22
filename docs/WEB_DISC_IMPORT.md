# Standalone selected-folder disc import

The standalone Web/PWA backend now has authored folder capability, CUE/M3U
planning, referenced-asset snapshot and atomic library publication source.
It connects the shared Import a disc set panel and existing format-aware
WASM media/firmware/disc/files routes. No file picker, browser/File/Blob/worker,
decoder/hash, lock, IndexedDB transaction, app/core, compiler or test has run.
No advanced core binary, manufacturer firmware or game asset is supplied.

## Selected browser capability

Where the browser exposes directory selection, the existing trusted file-input
flow requests `webkitdirectory` and consumes its FileList/relative paths. A
standalone hello advertises disc import only when that surface exists. There
is no arbitrary host root, file URL, directory path text box or persistent OS
directory permission. The browser supplies the selected File objects. This is
a browser capability boundary, not proof of native inode identity, symlink
denial or a common File Provider implementation on every platform.

Selection checks at most 4096 files, 256 represented subdirectories, 2 MiB of
metadata and eight relative components. It requires one selected root, valid
Unicode/exact names, bounded paths/components and no case/normalization aliases
or file/directory shadows. Top-level CUE/M3U/M3U8/CHD/ISO/PBP entries are bounded
at 64. It does not enumerate a filesystem beyond the returned list, reconstruct
empty directories or open unrelated selected file contents. Browser-side picker
enumeration happens before application admission and has no claimed application
memory/latency bound.

The offered entry list and suggested title accompany an opaque, five-minute,
single-use page capability. At most four pending folder capabilities and 32
total file/export/folder capabilities are retained per page. One expiry timer
retires elapsed selections even without another request; browser suspension can
delay that timer, and access still repeats monotonic expiry admission. Closing
the selecting page releases its capabilities. Consuming the capability transfers
its File references to the admitted import job, not to another page.

## Descriptor policy and names

`disc-plan.js` follows the existing native import grammar and generated-name
policy; equality still needs cross-language fixtures in the final pass. Metadata
is at most 128 KiB, normalized descriptor output at most 512 KiB, and cue lines
at most 4096 UTF-8 bytes. Encoding admission includes UTF-8/BOM, explicit
Windows-1252, Shift JIS, GBK and UTF-16 little/big-endian with strict decoder,
unit and null handling. The source uses Rust-compatible whitespace/ASCII case
rules rather than assuming JS trimming is equivalent.

FILE directives admit BINARY/WAVE referenced files. Track numbers/order/sector
modes, required index information, CD timestamps, gaps and flags have bounded
parsing. Metadata-only directives remain text; unknown auxiliary-file directives
fail instead of silently opening more files. Playlists contain at most eight
local CUE/CHD/ISO/PBP entries. Nested playlists, CCD/TOC/MDS and archive extraction
remain separate required implementations where the product scope needs them.

References normalize separators and dot/parent components while refusing root
escape, absolute/scheme paths, controls, unsafe Unicode, ambiguous endings and
oversized/deep names. A normalized reference must match an actual selected File
exactly; there is no guessed case-insensitive fallback or HTTP fetch. CUE paths
resolve relative to their cue; offered playlist entry points are top-level and
their references resolve under the selected root, matching the native path.

Only referenced data is planned. Track names become generated `track-NNNN`
names; descriptor/disc/entry names use the existing safe naming shape. Repeated
file references share their generated identity when their types agree. The
normalized manifest uses sorted assets, actual lengths/hashes and the existing
domain-separated media identity. Total planned/copied content is at most four
GiB and 128 assets. Source filenames/folder metadata are not stored in it.

## Immutable snapshot and strict publication

File API snapshot-state tracking for OS files is not itself a universal strong
source-file snapshot guarantee. The importer therefore reads/copies each asset
in a dedicated worker and hashes those actual bytes. Each at-most-256-KiB read
becomes a new browser-owned Blob part; the resulting immutable Blob has no
source File/path reference. This also replaces the standalone single-file
import's earlier hash-then-publish-provider-File path. Ordinary hashing users,
including the firmware vault, retain their hash-only operation.

There is one import job per page and an exclusive nonqueued origin Web Lock
through preparation/publication; another page receives a busy result. The lock
is required, never stolen or approximated with a timeout-only lease. One asset
snapshot worker runs at a time, with checked monotonic progress and response
shape/length/hash. The whole preparation has a thirty-minute cancellation
deadline. Raw PSP entry extensions now also match the shared media contract.

Although the working read buffer is bounded, copied Blob data can retain up to
four GiB in browser-managed backing storage, with additional cost while IDB
serializes/commits it. This is not constant RSS, a kernel quota, an assured disk
spill or qualified low-memory/large-game performance. Actual browser memory,
temporary storage, interruption and pressure behavior remain required evidence
and may require another storage adapter. A source-level bound cannot establish
that a phone can import the maximum-sized image.

All completed copied Blobs, metadata and generated media identity enter one
strict transaction with library quota/count admission. Unique content hashes
are counted once. Reimport preserves the existing game identity/history and
can replace bad stored payload bytes with newly verified identical-hash bytes,
while rejecting inconsistent accounting/system/length records. Content blobs,
the library row and accounting publish together, so a failed transaction does
not deliberately expose a partial new library entry or partial asset set.
Browser durability/crash/quota behavior still needs real evidence.

## Cancellation, UI and ownership

Planning/copying observes cancellation between reads and terminates its hashing
worker. The final strict publication has an explicit boundary: once committing,
the cancel API reports that a cancellation was not accepted and the panel waits
for the transaction result. It does not label an already-publishing game as
cancelled. Platform/page close still aborts pending work/transactions where the
browser permits; lost acknowledgements require refreshing the library. No hard
OS read/commit preemption or survival through suspension is claimed.

Shared import UI now guards polling/cancel responses with its current job
revision, shows the finishing phase, and retires the captured old panel rather
than a newer import panel. Pending folder grants are revoked when its selection
dialog closes without starting. File/descriptor/worker failures remain bounded
job errors; no host path or arbitrary exception text is added to diagnostics.

The public game/media/save identities, player transport and native import
commands remain the existing contracts. No IndexedDB schema migration is needed
for disc import; the current standalone schema remains version 4 from firmware.
This source adds browser ingestion, not qualified core compatibility or a claim
that all original browser/mobile/runtime/storage/release work is complete.

## Deferred consolidated qualification

The single final pass must cover actual directory-picker activation/cancel and
platform/provider FileLists; missing/foreign roots, alias/path/Unicode/depth/
entry/metadata limits; all encodings, native/JS cue/playlist/identity fixtures,
directives/tracks/index/gaps/types, missing/changed files and malicious references;
snapshot bytes/hashes/worker cancellation, four-GiB boundaries, origin lock
contention/absence/close, dedup/repair/accounting, strict commit/quota/crash and
late cancellation; job/panel races, firmware/multi-disc/file-save continuation,
offline/cache/privacy and real device memory/latency/energy/retention. Every
other original product/release requirement remains active.

Primary contracts: [selected-directory FileList/relative paths](https://wicg.github.io/entries-api/),
[File and Blob byte/snapshot semantics](https://w3c.github.io/FileAPI/) and
[Web Lock ownership and nonqueued admission](https://w3c.github.io/web-locks/).
