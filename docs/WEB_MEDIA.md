# File-backed media for the shared WASM frontend

This is authored source. No browser, OPFS operation, Web Lock, emulator, core
build, compiler, game, manifest decoder, hash implementation or test has been
executed. The built-core catalog remains empty in development assemblies. This
transport does not supply qualified PS1/PSP cores, firmware, native mobile engines
or compatibility evidence. Disc switching and state envelopes now have separate
authored frontend source; see `WEB_DISC_STATE.md`. They have not been executed.

Native products now also have an authored verified positional-read route, selected
when their capability and actual isolated shared-memory support agree. It reuses
native library descriptors and a bounded four-MiB worker cache, avoiding an OPFS
copy for those sessions. See `NATIVE_MEDIA_RANGES.md` for its independent I/O lanes,
leases, integrity, cancellation, page ownership and remaining qualification. The
OPFS route below remains separate and keeps its documented platform limitations.

The shared player now validates a native-style media manifest before choosing
its runtime. Names, ordering, format/system/entry agreement, asset/disc counts,
individual sizes and the four-GiB total match the bounded Rust contract. Its
explicit property order reconstructs Rust's domain-separated serialized bundle
identity; ordinary single-file identities remain their direct content hash.
Assets are requested only by library game ID and manifest name, with expected
length/hash checked against each transfer descriptor. Legacy unnamed entries
keep their existing asset request. No host path or provider URI reaches a worker.

Small raw ROMs keep the bounded in-memory path. Multi-file, non-raw or larger
media uses a separate staging protocol. Catalog entries must explicitly list
their content formats and filesystem protocol 1 for that path; older catalogs
retain only the small raw-ROM fallback. The existing NES/GB/GBC/GBA recipes add
their actual raw format declaration and a retained C frontend constant shim.
They do not suddenly declare optical-disc compatibility. Core system information
must also accept full paths for multi-file content; an in-memory-only core cannot
receive a CUE sheet as if it were a complete ROM.

Preparation creates one origin-private staging area under the application's
dedicated `omniplay-runtime-media-v1` bucket. A Web Lock admits one staged session
per origin, including its active playback lifetime. A random UUID names its
flat directory; only generated payload names and an ownership record are used.
Before creating it, bounded recovery scans at most 32 prior entries and removes
only recognized flat generated files inside canonical UUID staging directories,
then their empty directories. Unknown names/subdirectories are retained; failed
recovery and too many retained areas stop new admission. It does not walk the
library, save store, browser profile or a user-selected filesystem directory.

When approximate browser quota/usage is available, staging requires its total
plus 64 MiB of headroom. OPFS, worker synchronous access and the origin lease
must actually be available. There is no full-disc ArrayBuffer fallback or claim
that private/ephemeral WebKit storage always exposes these APIs. Unsupported,
denied, full or locked storage fails without inventing a successful session.
Staging is a temporary additional disk copy, and its initial transfer time and
quota cost remain material limitations. A direct authenticated random-read/cache
path and further platform optimization can still improve this route.

The UI streams one native transfer and one acknowledged worker chunk at a time,
each at most 48 KiB. It validates IDs, exact metadata, offsets, encoded/decoded
chunk bounds, boolean finality and acknowledgements. The worker independently
checks asset order, offsets and total length while incrementally hashing bytes
as it writes. Each completed asset must match its declared SHA-256, flush and
report its exact stored size before the set is sealed. The streaming SHA-256
source is shared with standalone Web imports; no new hash implementation or
conformance result is asserted. The native transfer retains its own integrity
check. Storage read-back, malicious native storage and power-loss behavior still
require the final verification.

Up to 128 synchronous access handles remain exclusively held through playback.
Only sealed records are mounted into `/game`. The Emscripten FS backend exposes
the exact generated manifest names, read-only metadata and bounded positional
reads, without copying a complete disc into MEMFS or WASM memory. It checks
buffer views and safe integer ranges, limits one underlying read to one MiB,
rejects premature EOF/count errors and supports checked seeks beyond EOF.
Creation, writes, append, truncation, rename, deletion, links and metadata changes
are denied; mmap is explicitly unsupported rather than silently allocating a
whole disc. C-library errno/mode/open/seek constants come from the linked frontend
shim, avoiding guessed JS constants for an SDK ABI. Real SDK/libc/large-offset
and core-specific I/O behavior remain qualification requirements.

For full-path loading, retro_game_info receives a path, null data and size zero.
A single staged entry can use the existing capped in-memory path if its core
requires data; multi-file content cannot. The player now uses the actual admitted
content extension, including disc and N64 variants. The worker reports media
protocol 1 and serializes at most 32 control operations so asynchronous OPFS
admission/writes cannot overlap prepare, seal or stop. Existing bounded input and
haptics acknowledgement paths remain separate from those storage mutations.

Staging has a two-minute idle and thirty-minute total preparation window, checked
on protocol work. Individual UI requests also time out. Active sealed playback
does not expire merely because it stops staging chunks. Exit aborts preparation,
checks cancellation around transfers/core selection and joins runtime shutdown.
Late session registration is explicitly stopped. Concurrent stop callers share
one shutdown task. Native transfer abort and temporary staging cleanup follow
failure; normal stop unloads/deinitializes the core before unmounting/closing
handles and retiring its own generated files. Cleanup failure is reported as a
warning and left eligible for later recovery. Worker termination can bypass
application cleanup; browser handle/lock release and next-launch recovery need
actual evidence. No immediate kernel I/O preemption or zero leftover bytes is
claimed.

Recovery limits are per origin. Native loopback products currently use ephemeral
ports, so a later native server instance can own a different origin and cannot
reach an old origin's OPFS bucket through this JS recovery path. Separate native
origin registration/targeted storage retirement or an origin-stable trusted
storage strategy remains required before enabling staged cores in release.
Nonpersistent WebKit eviction must also be demonstrated, not assumed. The empty
development catalog does not establish that this release requirement is solved.

The frontend C source is included in retained build inputs and its hash is
recorded by the authored core recipes. It has not been compiled. All old/new core
artifacts, Emscripten toolchains, licenses, corresponding source, platform memory
and build reproducibility still need the final release qualification. No native
content-serving HTTP route, extra origin or network credential is introduced.

The consolidated final pass must compare Rust/JS identity fixtures; validate
legacy/single/bundle and format/extension handling; exercise 4-GiB boundaries,
multiple tracks, M3U and real core full-path/seek behavior; malformed descriptors,
chunk/ordering/hash/flush/size errors; quota denial and browser feature absence;
exclusive handle and Web Lock contention/release; stale/partial generated cache
recovery and unknown-file retention; cancellation at every await/transfer/core
and session boundary; serial command admission and late messages; read-only FS
operations, errno/mode/offset ABI and mmap refusal; shutdown/worker/process loss;
unchanged raw ROMs and save transfers; shared hash vectors; full device memory,
copy time, I/O latency and energy. Advanced core builds, firmware mounting,
multi-disc controls/state, browser/mobile product work, streaming, complete sync,
quotas/recovery and every other original requirement remain in scope.

Primary contracts: [File System Standard and exclusive synchronous handles](https://fs.spec.whatwg.org/),
[WebKit origin-private filesystem interface](https://webkit.org/blog/12257/the-file-system-access-api-with-origin-private-file-system/),
[Emscripten FS mount operations](https://emscripten.org/docs/api_reference/Filesystem-API.html),
[Emscripten filesystem node API](https://emscripten.org/docs/api_reference/advanced-apis.html).

Standalone source ingestion now also admits bounded selected-folder CUE/M3U sets into its immutable local content store, followed by this existing named-asset playback route. See `WEB_DISC_IMPORT.md` for capability, encoding/reference, copied-Blob, transaction and memory limits. No directory/picker/parser/import/browser/core/test has run, and advanced binary/device/ABI/release work remains due.
