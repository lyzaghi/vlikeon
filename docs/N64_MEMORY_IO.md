# N64 owned scratch and save I/O source

Item 2 remains open. The bridges and a separate pinned configuration/shader/texture
integration candidate are authored source, not an enabled N64 runtime. Active
N64 package identities and OS write grants have not changed. No compiler,
configuration, core, browser, OS runtime operation or test has run for this work.

## Owned scratch adapters

`engines/native/adapters/n64-memory-io.h/.cpp` provides the C stdio subset used
by the pinned configuration/database code. It accepts only bounded scratch/
relative UTF-8 names, normalizes Windows separators and delegates canonical
path/alias/device/stream admission to the actual MemoryFiles table. There is no
ordinary FILE, OS path or disk fallback. Directory creation also admits the
scratch root. Every callback must be nonthrowing, and the owner must keep the
table/context alive through all callers.

The scratch adapter admits at most 64 live streams and 64 MiB per logical file, within
the store's shared 64-MiB scratch page budget. Formatting admits at most 1 MiB
per call; reads/writes chunk through the existing 8-MiB transfer bound. Modes
cover read, replace, read/update and append. Seeks are bounded; holes remain
zero through the underlying store. Append writes choose the end even after a
seek. Multiple distinct streams can operate concurrently; the caller serializes
close against every use of the same stream, including compound formatted calls.

Each writer owns an exclusively created private temporary. Update/append copy
the old image and pay quota for both images. Flush acknowledges memory only;
healthy close flushes, confirms handle close and atomically renames the complete
image. Any short write, formatting/write/flush/close/rename fault or explicit
abort preserves the old target. Rewind cannot clear a writer fault. Cleanup can
remove only its own temporary, never an exclusive-name collision. Failed
temporary removal retains its bounded node/page debt. An uncertain callback
close latches a terminal lifetime failure: no new open/mkdir or callback-table
replacement/uninstall is accepted until actor retirement. This is not evidence
that an unresolved callback handle has ended.

`n64-memory-streams.hpp` provides input/output streams over that same C adapter
for the selected GLideN64 cache call sites. It supports formatted and binary
I/O, positioning and one-byte lookahead with corrected tellg/relative seeking.
Output writes a new complete image and publishes only on a healthy close or
destructor. Caller stream failure or exception unwinding aborts publication. Update/append/ate output
modes are explicitly rejected; selected cache call sites must be adapted with
their real transaction semantics before use. Paths must already be UTF-8;
there is no locale-dependent wide-path or ordinary fstream fallback.

`n64-memory-gzip.h/.cpp` uses zlib inflate/deflate with scratch stdio, not the
OS-file gz implementation. It preserves gzip framing, CRC and size checks,
rejects truncation/trailing bytes/multiple members, and admits one decoded
stream of at most 256 MiB while retaining bounded codec buffers. Compressed
bytes still pay the shared 64-MiB scratch allowance. Read seek is forward-only,
matching the selected cache skip operation. Close validates an unread footer
and bounded remaining decoded stream. A negative read is distinct from EOF;
callers must check every operation and stage parsed records before acceptance.
Gzip writer close propagates codec and atomic-publication errors. These byte
bounds are not an OS I/O deadline or a complete texture-allocation bound.

## Persistent file storage and Transfer Pak RAM

`om_n64_save_fopen` is a distinct save-only entry point. Ordinary N64 scratch
open still refuses save names, and save open refuses scratch/host names. Both
share the same 64-stream lifetime, uncertain-close latch and private-image
publication. A persistent file admits at most 256 MiB under the actual shared
272-MiB save-page allowance; old readers and private update copies are charged
concurrently. This is not an additional uncharged cache or durable disk store.

`n64-save-files.h/.cpp` provides exact-size staged reads, owned whole-image
loads, size queries, complete writes and copy/update publication. Caller bytes,
pointer and size commit only after complete read AND acknowledged close. Only
literal ENOENT permits an update to create a missing image; denied/corrupt/read
or allocation failure cannot retry using truncating write mode. Zero-byte
loads have an owned malloc allocation. Sizes/offsets are bounded before write.

The prepared actual `main/util.c` routes every utility write into this save
API with no OS write fallback. Readonly ROM/asset/import paths keep ordinary
read-only FILE access, now with a 256-MiB file limit, staged buffers and checked
close before publishing caller outputs. Memory save reads never fall back to
host files on a missing name. Utility reads of fixed-size RAM require exact
length instead of silently accepting a longer incompatible save.

Actual `file_storage.c` initializes bounded default RAM only on literal missing
save data, never after bad size/read/close or allocation. Rejected storage
exposes no guest data and remains write-blocked. First-access state clears
only after complete publication. Any failed later chunk retains a dirty flag;
the next write retries the full guest RAM so an earlier failed dirty range is
not lost. Child range/offset admission precedes parent pointer arithmetic.

The actual Transfer Pak RAM callback uses a malloc-owned filename derived from
the GB ROM content's existing MD5 identity and controller port under
`save/N64/tpak`. MD5 is a compatibility/content identifier, not an authenticity
claim. The former empty-string default and its invalid free are removed. An
explicit/adjacent old RAM filename is only a readonly import when no retained
memory save exists. Exact import and its memory publication must succeed
before exposing RAM; an already retained memory save wins. Import absence can
initialize default RAM, while other import failures reject RAM admission.

This remains a separately prepared, unbuilt candidate. Actual DD qualification,
the original primary SRAM/mempak/
EEPROM/flash and snapshot contracts, and an acknowledged quiescent producer/
flush/capture protocol still require completion. The storage backend has a
void guest save callback: an internal dirty flag alone is not a frontend save
acknowledgement. Legacy host save/state import/export compatibility remains in
scope. No package/broker admission or actor OS write grant has changed.

## Owned 64DD storage, format and geometry source

The actual selected disk.c now uses `n64-dd-format.c.inc` and
`n64-dd-geometry.c.inc`. MAME/SDK full images retain their exact sizes and
repeated-system/ID checks. Backup system offsets use the selected physical
block number (including blocks 8/9/10/11), with development sector length
handled separately. Header, defect, type, LBA, ROM/RAM range and total-image
admission precedes D64 expansion. Expansion returns the actual new length to
both main.c and rom.c; a failed check/allocation preserves every output and
input ownership. Empty/partial RAM is zero-filled only after valid layout
admission, and the normalized header describes the complete retained RAM.

The two rows of TrackZoneTable are indexed explicitly for both physical heads,
instead of reaching the second row through an out-of-bounds first-row access.
LBA arithmetic, defect/track geometry and inverse lookups are bounded. Table
generation stages every entry and rejects invalid/duplicate physical codes
before replacing the retained table. Real sector requests carry their actual
byte count, with checked head/track/block/sector/type/range and controller
buffer admission. RAM-only persistence clips a valid overlapping write to the
RAM interval without overflowing start+size. These are authored algorithm
changes and still require independent real-media/geometry qualification.

`n64-dd-storage.h/.c` stages the full owned descriptor before publication. It
derives the memory destination from original source content under
`save/N64/dd`, using the pinned MD5 content-identity convention (not a security
signature). A retained memory save wins. Only literal absence permits reading
the original adjacent `.disk_save`/`.ram` as a readonly legacy import. Bad
read/close/format/size/allocation is an admission error, never a fallback to
the base image that could later overwrite retained progress. A legacy import
must pass format admission and checked memory publication. The original image
and old legacy path are never write destinations.

The pinned actual core keeps its full-image save default. The same loader
supports explicit RAM-only and readonly integrations; physical MAME uses a
full image for its original layout. At most two old/new descriptors may be
owned, and two mutable descriptors cannot publish to the same destination.
`om_n64_dd_flush_all` reports checked full-image results. Failed close retains
the descriptor, RAM and ownership for retry. Deinit/table uninstall refuses a
remaining owned DD descriptor, preserving the common actor-exit boundary on
unconfirmed cleanup. Unstarted rejection has a separate release path, so a
bad/missing IPL does not manufacture a default save during cleanup.

The actual IPL loader honors an explicit media loader or supplied IPL path,
otherwise joins the bounded system directory. It checks complete readonly
load, signature/alignment and DD_ROM_MAX_SIZE before copying any bytes to the
core ROM buffer. A supplied unreadable/bad disk/save/IPL reports startup
failure; it does not silently continue as a cartridge-only session. The core
uses the admitted selected system block's region rather than the first word
of an unrelated backup block. No firmware or disk is bundled by this work.

The source-only DD suite uses actual prepared format/geometry/loader, MD5,
file-storage utilities, persistent bridge and store. Its synthetic fixtures
cover truncated/malformed images, expansion OOM, literal backup-block offsets,
independent LBA/head goldens, staged table retention, bounded coordinates,
RAM/full saves, legacy precedence, rejected retained saves, close failure and
retry, unstarted release and descriptor limits. Only parser calloc is a fault
fixture service. Read-source/legacy files are memory fixtures in that suite;
real OS readonly import, IPL/controller, signed package/producer capture and
full game/device/fault qualification remain required. No case has run.

Primary source reference for the retained format/geometry vocabulary:
https://github.com/mupen64plus/mupen64plus-core/blob/master/src/device/dd/disk.c
The adaptation is applied only to the separately inventoried exact N64 pin;
the upstream reference is not a changed dependency selection or runtime proof.

## Checked native owner, preparation and save acknowledgement

`n64-core-owner.h/.cpp` binds one actual OS thread and exact root/guest fiber
tokens. Its bounded state machine separates initialization, loaded content,
guest execution, paused boundaries, snapshots, flush and guest exit. It refuses
foreign-thread/token access, replacement/reentrancy, early readiness, premature
buffer release and owner retirement while the guest or snapshot is in flight.
Every mutex is released before a fiber switch or I/O callback. A rejected
snapshot ends its borrow and permits a later explicit retry; it does not claim
the guest state was rolled back. The frontend retains its existing checked
before-state/restore procedure.

The actual native core now exports finish-prepare, pause and checked flush.
Preparation switches to the real guest and parks after device setup, boot HLE
and valid interpreter PC/cache initialization, before the first guest opcode.
Both admitted pure/cached interpreter routes have this checkpoint. Frame yield
is deferred from the middle of new_vi to the safe end of gen_interrupt after
its event/job processing. Snapshot jobs execute on the parked guest at those
same safe checkpoints without running another guest frame, retaining the
existing fixed-size M64P snapshot wire. The selected native profile requires
synchronous workqueue callbacks; compiling an asynchronous M64P_PARALLEL
snapshot profile is explicitly rejected. Runtime options cannot start the
threaded renderer behind this single-owner package.

Checked flush republishes every active Transfer Pak RAM and owned DD image
while the guest stays paused. Primary SRAM/mempak/EEPROM/flash keep the original
raw libretro memory contract for the combined frontend envelope. Only all
successful peripheral publications can acknowledge capture. Individual memory
file commits may precede another file's failure; the frontend must not capture
or publish that intermediate collection as a successful save. A retry rewrites
complete live images. Live readonly scratch cache snapshots may remain during
capture, while any pending writer or unresolved close prevents acknowledgement.
Uninstall still requires every stream, including readonly snapshots, retired.

Generic writable storage close now reports failure and retains RAM/filename
ownership. Initial-read/import rejection uses an explicit unstarted discard,
so cleanup cannot publish guessed default RAM. Cart swaps refuse to overwrite
an unretired old RAM owner. Bad selected GB reads/types/RAM setup reach the
native owner's admission fault instead of silently reporting readiness without
the requested cart. The DD descriptor has stable storage beyond guest stack
unwind, permitting root-side retirement retry after guest exit. Peripheral
retirement, actual guest/root retirement and table detach remain distinct.

Snapshot mutex lifetime is now explicitly initialized and checked during
CoreStartup, cleared before destroy in CoreShutdown, and protected against
duplicate retirement. Wrong-magic rejection releases the mutex before the next
request; the saved work item is zero-initialized so allocation failure cannot
free an indeterminate filename. A refused mutex retirement or malformed guest
yield exits the owned actor before borrowed buffers/DLL/callback context can
be released. These are authored process policies, not observed OS behavior.

The common native worker now obtains a real table-uninstall acknowledgement
after void unload/deinit and before a restore replaces the memory-file tree.
It reattaches the stable frontend table for the next content initialization.
Refused/uncertain detach stops restore before any file swap; refused reattach
conservatively retains callback ownership for shutdown. This also applies to
the already authored MD/PS1 memory routes and needs actual regression evidence.

Source qualification now includes actual owner plus prepared peripheral helper/
storage/store cases, and a separate actual Windows fiber/owner/store exercise.
The first uses opaque tokens for protocol admission; it is not an OS fiber or
whole-core test. The Windows exercise uses synthetic guest steps, not an N64
CPU/ROM/GL implementation. Actual full core initial readiness, frame/snapshot
semantics, bad-state retry, restore rollback, producer/thread/handle shutdown,
GPU/audio/input, primary-memory compatibility, all process faults and exact
package/OS/device/dependency qualification still remain. No case has run.

## Pinned configuration and shader integration

`tools/prepare_n64_memory_core.py` materializes exact committed GLideN64/HLE
source, excludes Angrylion/CXD4/parallel directories, retains the native
software/interpreter/Windows baseline adaptations and selects the separate
unbuilt library identity `3.0.0+omniplay.gl2.12edd2c74a51`. It has no build or
distribution mode. Its receipt binds all adapter/header/recipe/source bytes and
keeps `n64_core_memory_io_integration_complete=false`, `product_enabled=false`.

`tools/n64_source_adaptations.py` now routes generated mupen64plus.ini, the
custom config writer and ROM-database reader through scratch stdio. It checks
write/close and read/close failures; failed ROM-database admission prevents
CoreStartup from publishing readiness, frees its memory/list and refuses game
load. All 256 CRC/MD5 index slots are reset across reloads, allocations are
checked and the source bounds entries to 65536. Other readonly asset/font paths
retain their system-directory route with bounded joining. The pinned custom
ConfigInit stub is not called by this libretro CoreStartup path and is retained;
no claim that this dormant API persisted settings is introduced.

Optional custom settings first read/close at most 4 MiB of complete NUL-free
text, then parse a copied Config and commit with a statically required
nonthrowing move. Failure leaves prior configuration. Builtin custom settings
parse a fresh copy instead of destructively tokenizing the static header;
repeated loads retain all builtin lines. No optional settings stream leaks into
the guest lifetime, and readonly media/firmware probes are not intercepted.

The candidate exports table installation required before init and refuses replacement until deinit.
Startup failure cannot enter game load/run. Deinit keeps the table through
CoreShutdown, deletes the suspended game coroutine and retires the Windows root
fiber before acknowledging the inactive phase. The Windows adapter checks root
conversion and owns one guest/root pair with explicit compiler TLS (the pinned
libco header may otherwise expand thread_local to nothing). It refuses a
foreign preexisting root and confirms ConvertFiberToThread only after the owned
guest is deleted and execution is back on its owned root. Failed retirement
retains the live phase and table authority until actor retirement. Full guest,
cache-destructor, OS-thread and restart behavior still needs real qualification.

Shader keys and binary storage now use memory streams under the bounded
scratch/Mupen64plus/shaders tree. Key formats 4/5 and binary format 0x3b retain
their layouts, with at most 65536 records, 4096-byte driver strings, 16-MiB
individual binaries and 64-MiB aggregate images. `n64-shader-cache.hpp` stages
complete parsed keys/records before GL; it rejects partial/trailing bytes,
duplicates, unknown versions, incorrect full driver/options identities and
negative/oversized binary lengths before their allocations. Input close must
also succeed. Invalid binary caches retain the existing key-based recompilation
path; failed program links release their temporary GL objects. Binary capture
checks the driver's advertised and actual lengths. Writers stream bounded
records into their private image and patch the count before atomic publication,
without a second unbounded all-shader serialization vector. Combiner fallback
uses at most the two physical stage slots even for malformed cycle-mode keys.

## Texture cache integration

The same separate gl2 source candidate now adapts the actual GLideNHQ
TxMemoryCache and TxFileStorage. Mutable cache paths use a fixed private scratch
root; readonly texture-pack/data/font paths keep their existing authority.
Wide cache names convert deterministically to UTF-8 without process locale
changes, and the table still rejects traversal, aliases and device/stream names.
No texture-memory/file-cache path opens an ordinary OS file.

The shared `n64-texture-cache.hpp` retains modern version 0x08000000 and the
preceding legacy config-first gzip/file layouts. Complete scalar reads precede
allocation. At most 65536 records and 256 MiB of stored/expanded texture bytes
are admitted per decoded cache. Dimensions are at most 16384 and their complete
byte product must fit; actual renderer/pool limits still apply. Format,
texture-upload format and pixel type must agree. Nested zlib records validate
their exact expanded byte count, full compressed input consumption and CRC,
using a fixed validation buffer. Short/extra payload, duplicate keys, invalid
lengths, outer gzip faults and incomplete close reject the whole candidate.

Memory-cache load stages a complete candidate, then swaps payload/LRU ownership
only after successful gzip close. Failed allocation or malformed input retains
the former cache. Add stages payload, list and map allocations before eviction;
same-checksum/different-N64-format entries retain distinct LRU ownership.
Nonzero requested cache limits use LRU within a 256-MiB hard payload ceiling;
the former zero/unlimited hires mode now has that finite ceiling and refuses an
over-budget add instead of silently dropping earlier hires textures. This is
an explicit new resource limit, not proof of equal performance or whole-process
memory use. Save retains legacy layout when loaded as legacy and publishes its
gzip image only after all checked records/footer/close complete.

File-backed caches retain a readonly memory-file snapshot plus a bounded set
of pending records. Index admission checks exact file length/count, positive
bounded packed offsets, N64-format agreement, duplicate keys and contiguous
nonoverlapping records before accepting the map. Each payload is fully checked,
including nested decompression and aggregate expanded bytes. The serialized
image has the shared 64-MiB scratch-file limit; old/new/retained-open images all
continue paying the underlying scratch page quota. New pending records pay
their own finite payload bound and do not truncate the active snapshot.

Save streams every old/pending record into a new private image, writes its
complete index, patches the index offset and closes/renames before switching
runtime snapshots. Write/flush/close/rename failure keeps prior committed bytes
and the current logical cache. If publication succeeds but reopening the new
snapshot fails, save reports failure and retains the former snapshot/pending
records for retry; it cannot claim the already committed file was rolled back.
Clear discards runtime snapshot/pending state without truncating a retained
committed cache. File read outputs change only after a complete checked record.
Cache methods remain serialized by the core owner; they do not introduce a
concurrent cache API.

The stdio bridge now retains the opened stream's own length for SEEK_END and
serializes name publication with open/stat admission. An old reader keeps its
own node/length when a name is replaced. The owner must not mutate those
readonly snapshot nodes directly through a second ABI route. This does not
make arbitrary out-of-band mutation safe.

## Bounded texture pool and filter lifecycle

The source now replaces the actual pinned TxMemBuf implementation with
`n64-texture-pool.hpp/.cpp` and `n64-texture-pool-core.cpp.inc`. Source inspection
confirmed that TxFilter already clamps dimensions to 4096 and
TxUtil::getNumberofProcessors already returns one; those capabilities remain.
The new pool admits strictly positive checked dimensions up to 4096, two fixed
shared buffers of at most 64 MiB each and one pair of lazily allocated workspace
buffers of at most 64 MiB each. Live pool payload is at most 256 MiB. Workspace
growth retains its old pair until both new allocations succeed, with a 384-MiB
old/new transient bound. These are pool payload bounds, not total process/GPU/
decoder/cache/restore/driver or physical-memory qualification.

Initialization stages both main arrays before publishing any pointer. Re-init
cannot invalidate an active pool. Workspace admission checks lane/slot/count,
requires the actual owner thread and an explicit processing/workspace scope,
and refuses growth after a pointer was borrowed in that workspace scope.
Different later scopes can grow the pair. Failed first/second allocation keeps
both old pointer images, counts and byte charges. Main pointers never move
while initialized. Processing ownership is recursively bounded to 16 scopes;
another thread cannot join/release that owner. A retirement request refuses new
root work and retains bytes until the actual scopes end. Shutdown cannot claim
success while a processing/workspace borrower remains.

TxFilter now initializes every field, rejects invalid constructor inputs,
records which instance owns the pool and cleans partial construction safely.
The public init result requires the actual initialized filter rather than a
successful new expression. The configured single lane calls the real filter;
the formerly dormant no-op multi-lane branch is not promoted into a feature.
Filtering checks source dimensions and output scale against actual pool shape.
Deposterize allocation failure is a returned failure instead of silent omission;
the new standalone-deposterize route actually writes its checked output. Its
workspace scope remains alive through the following scale/smooth operation.
Texture sizeof arithmetic uses checked 64-bit products. Actual cache admission
also checks current pool dimensions, preventing cached palette/upload metadata
from demanding more output than the real shared arrays.

Texture filter, hires lookup, dump and reload paths use processing scopes.
The native ABI still requires a serialized core owner; these guards do not make
arbitrary simultaneous direct export calls a supported API. Normal export
shutdown requests retirement before deleting the filter and releasing its
cache/pool ownership. A busy shutdown retains that object. Table installation
or uninstall requires the actual pool to be retired as well as the core phase
to be inactive. TxMemBuf's singleton destructor does not access a potentially
already-destroyed pool; explicit filter shutdown owns the retirement boundary.

The common worker now ends its owned actor process immediately with failure if
the core refuses writable-table uninstall. It does not unload the DLL, run its
static destructors, destroy the callback context or acknowledge successful stop
before that actual process exit. The broker must still observe OS process/job
retirement before cleanup; an immediate-exit request is not its own retirement
proof. This path establishes no successful final save and is unexecuted source.

## Scratch PNG dumps and diagnostics

The selected core's compiled texture dump now writes through the bounded
scratch table rather than FILE/_wfopen. It retains Rice checksum/format/size/
palette naming and ordinary/strong-CRC subdirectories. The caller's source,
dimensions, row stride, optional conversion and actual pool capacity are
checked first. Directory/name admission uses the same bounded UTF-8/table
policy. The writer's private image is aborted on exceptions or encoder failure;
success requires the final stream close/rename. Readonly PNG/BMP loading still
uses its original readonly source path and has not been globally intercepted.

`n64-memory-png.h/.c` is a real C RGB/RGBA8 encoder with explicit source extent,
stride and optional vertical flip. Input is at most 256 MiB, dimensions at most
16384 and the selected texture wrapper additionally admits only its existing
4096 shape. Every libpng/zlib codec allocation, including per-allocation header
overhead, is charged to an aggregate budget of at most 8 MiB; the small fixed
control state is separate. Callers can request a lower codec budget. All output
still pays the memory-file store's shared 64-MiB scratch page limit, including
retained old/open images and private replacement pages.

Write/flush/codec allocation errors stop PNG production and poison the private
writer. The C translation unit owns setjmp/longjmp and codec destruction, using
heap-owned mutable control/pointer state so error cleanup does not read changed
automatic values or bypass C++ destructors. Every tracked allocation is retired
before control returns. Encoder flush acknowledges memory only and cannot
publish the filename; the outer owner checks close/publication separately.

Texture diagnostics retain no persistent stream. Each call formats a bounded
wide message, converts bounded UTF-8 and appends transactionally under
scratch/Mupen64plus/debug, with a 1-MiB log ceiling. Full logs retain their old
image instead of truncating. The native GLideN64 log endpoint uses finite
format/framing buffers and passes message content to the frontend callback with
the literal "%s" format. A message/file name containing percent sequences
cannot become a second formatting instruction. Unknown severity or absent
callbacks are refused. Default disabled debug macros remain disabled.

The upstream separate screenshot.c translation unit is not selected by the
pinned libretro Makefile.common. This work does not enable it or claim that the
product screenshot feature is verified. Selected source/build closure and the
original frontend capture feature remain part of final qualification; other
native state/peripheral writes still need completion.

## Source provenance and qualification

`tools/prepare_n64_memory_io.py` freezes these actual adapters, MemoryFiles, QA
sources and committed `custom/dependencies/libzlib`, `custom/dependencies/libpng` and libco interface headers from N64 revision
12edd2c74a517ff86dfa8cfc71ad75e4c10486d5. It copies/hashes source only. The
result includes no compiled core or product catalog, and explicitly records
`n64_core_integration_complete=false` and `verification=not_run`.

The pinned vendor headers identify zlib 1.2.8 and libpng 1.6.32. This source selection
does not approve that dependency for distribution. The original dependency
security/update/license review remains mandatory before selecting and compiling
the final N64 closure; any changed closure needs fresh source/artifact identity.

The standalone QA project compiles the real adapters at C++11, the actual
MemoryFiles at C++20 and the selected zlib inflate/deflate source. It has source
cases for transaction visibility, old open-reader snapshots, sparse/append/
formatted operations, real page/stream limits, independent concurrent writers,
publication faults, unowned temporary collisions, C++ lookahead and failed
destructors, independent gzip bytes/decoder interoperability, corrupted framing,
decoded-byte limits and unresolved-close lifetime debt. Additional source cases
exercise complete optional text, current/legacy shader formats, every truncated
binary prefix, parser count/identity/length/aggregate bounds, failed-result
preservation and exception-unwind publication. A Windows-only fixture compiles
the real owned-fiber adapter as C and covers foreign-root rejection, guest/root
roundtrips, floating state, repeated retirement and concurrent TLS ownership.
None has executed.
Additional authored qualification compiles the actual adapted pinned
TxCache.cpp against the real memory adapters/store, actual bounded pool and
pinned codec. Only surrounding graphics enum values are fixture services;
they cannot prove real GL/driver or full filter/game behavior. It
exercises raw/compressed memory/file modes, same-checksum format variants,
serialization/reload, exact LRU, failed add/read/write/rename, post-publication
reopen failure and retry. Independent legacy/modern byte fixtures exercise
file-index and outer-gzip parsing, every truncated fixture prefix, malformed
offset/count/metadata/shape and result preservation. All are unexecuted.
The additional bounded-pool cases compile its actual C++11 code and real
TxMemBuf methods, with allocation faults supplied only by the pool's constructor
allocator template. Cases cover both init/growth allocation boundaries,
preserved old pointer contents, invalid dimensions/lane/slot/scope, foreign
thread refusal, nested limits, busy-retirement retention, later re-init and the
actual 256-MiB shared/workspace ceiling. No fake buffer-pool service remains in
the texture-cache cases. All pool, filter, worker-exit and OS evidence is still
reserved for item 16.
PNG qualification source compiles the actual C encoder, store/stdio and pinned
codec libraries, then decodes output through the separate libpng memory-read
API against independent RGB/RGBA/alpha/padded-row/flip pixel fixtures. Cases
cover pre-close name visibility, invalid dimensions/extent/stride/channels,
small codec budgets, repeated error cleanup, real scratch page exhaustion and
write/flush/rename/unknown-close handling. Actual prepared diagnostic cases
exercise bounded frontend messages/literal-format forwarding, severity/null
admission, UTF-8 scratch logs, full-log preservation and retired callback
contexts. No PNG, diagnostic, image/core/driver or resource case has executed.
See `qa/n64-memory/ACCEPTANCE.md` for the later full-product matrix.

## Remaining integration

1. Complete the remaining core write graph and actual DD qualification:
   native state-file/peripheral operations, their directory/codec writers and
   actual qualification of the newly authored pause/flush/snapshot admission.
   Preserve admitted features and readonly assets.
   Qualify the newly authored pool/filter paths and complete other image-decoder,
   checksum/resample/filter temporary allocations, combined memory peaks and
   whole-process resource policy. Bounded pool/cache payload alone does not
   bound every texture pipeline allocation, graphics object, helper or driver cache.
2. Qualify the new config/database/custom-settings, shader and Windows fiber
   paths against actual built core/worker behavior, including all error,
   allocation, thread-ownership, restore and destructor boundaries. Source
   guards and a standalone fixture cannot prove full-core quiescence.
3. Retain the table through every remaining guest producer/cache destructor.
   Bind new core/header/adapter/version/source/artifact identities into build,
   worker, package and broker admission before removing actor OS write grants.
   Keep N64 SRAM/mempak/EEPROM/flash portability and snapshot contracts intact.
4. Qualify actual native software/OpenGL/driver/OS behavior, cache exhaustion,
   game/region/media/save/state/device compatibility, containment and compiled
   dependency licenses in item 16. The research-only Angrylion distribution
   restriction and all original N64/product requirements remain unchanged.

MD and PS1 already have separately authored memory-file routes; this new N64
foundation does not assert that every core or trusted driver/helper write is
bounded, and does not close Windows item 2.

Windows API contracts used for the owned-fiber source:

- https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-convertthreadtofiberex
- https://learn.microsoft.com/en-us/windows/win32/api/fibersapi/nf-fibersapi-isthreadafiber
- https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-convertfibertothread

## Snapshot pointer and queue source admission

The selected gl2 recipe now installs `n64-snapshot-bounds.h` and the actual
`interrupt.c` queue replacement. The fixed native image is checked before the
loader locks or modifies guest state: known canonical M64P 1.0–1.9 versions,
bounded LE event records/sentinel, the real 16-node pool with a slot reserved
for normalized SPECIAL, and complete PIF command extents at restored offsets.
The variable Transfer Pak layout is walked for every present/absent combination
in both 1.2 and 1.3+ encodings. Legacy 1.0/1.1 PIF parsing follows the pinned
special-byte/known bogus-command rules. This is pointer/structure admission;
it does not yet establish semantic safety of all restored CPU/TLB, cartridge,
DMA, timer, framebuffer, flash and DD register values. That work remains open.

The actual queue loader validates the complete input before clearing its live
queue. Valid duplicate ordinary event types remain supported. Serialization
checks each node against its own fixed pool before dereferencing it, rejects
cycles/foreign links, stages the complete queue and zeroes serialized padding.
Neither a short output nor a corrupt queue publishes caller bytes. The actual
PIF setup receives available bytes, checks the entire Tx/Rx extent before any
post-setup callback, and disables truncated/unassigned channels. Processing
rechecks the retained extent and derived pointers. Snapshot emission compares
channel pointer identities instead of subtracting unrelated reset-buffer
pointers, and checks the completed image before publishing it. GB fingerprint
diagnostics are bounded to the actual 28 bytes.

Snapshot DD IDs now use the registered descriptor's admitted `offset_id` and
actual storage size with an unaligned-safe four-byte copy. This replaces the
stock fixed physical-block offset, which lies outside a small no-RAM D64.
Missing/short/overflowing or retired/foreign descriptors cannot supply an ID;
active invalid DD ownership rejects capture/load. Raw-word compatibility is
retained; the ID is a compatibility check, not authentication.

`snapshot_bounds_cases.cpp` and `event_queue_fixture.c` are authored only. The
full candidate freezes exact pinned pool/scheduler/CP0 accessor source bodies
for that focused fixture; it does not emulate the CPU or exercise the full
savestate loader. Separate actual DD cases cover small D64 and rejected ID
extents/ownership. Item 16 must still qualify the real parser, frame-boundary
restore/retry/rollback, callbacks, CPU/GL and all original product requirements.
No configure, compiler, test, core or browser execution has occurred.

## TLB snapshot and finite virtual-page walks

The selected source additionally preflights all 32 wire TLB entries and both
one-million-entry lookup tables before restore mutation. Hardware field widths
and Boolean/cache/region encodings must be bounded, and the six derived range
and physical values must match the pinned interpreter's 32-bit arithmetic.
Never-written all-zero power-on entries remain supported, padding is ignored,
all 4096 guest PageMask bit patterns are retained, and wrapped halves keep the
pinned behavior of being unmapped. No preferred-page-size list is substituted.
Lookup entries must be zero or page-ending values within the selected mapper's
possible range, also bounding cached-interpreter invalid-code aliases. Read and
write tables retain independent overlap history; they are not rebuilt/equated.

Actual `tlb_map`/`tlb_unmap` now iterate bounded page indices with an exclusive
limit up to 0x100000, rather than a byte counter that wraps after 0xfffff000.
The final virtual page, inclusive endpoints, KSEG/physical exclusions, overlap
and dirty/write-table behavior are preserved. Invalid entry indices are ignored
before dereference. The shared interpreter TLBWrite also rejects indices >=32
before accessing entries; the pinned WIRED>=32 RANDOM path can produce six-bit
indices, whereas TLBWI/TLBR already ignore nonexistent entries.

`tlb_snapshot_cases.cpp` and the actual prepared power-on/map/unmap fixture are
authored, not run. They cover every raw mask pattern, modular ranges, forged
fields/LUT entries, final slots/pages, exact mapping goldens, bounded maximum
spans and independent overlap history. This fixture is not a CPU instruction
execution test. Real pure/cached interpreter TLBWI/TLBWR/TLBR/TLBP, valid legacy
state roundtrips, malformed-state rejection before callbacks, valid retry and
frame behavior remain item-16 obligations. Other CPU, cartridge, DMA/timer,
DD/flash/graphics selectors and resource bounds still require semantic audit.
No N64 package admission, actor OS write grant or product enablement changed.

## Transfer Pak and Game Boy banked storage source

The fixed-image preflight now validates Transfer Pak enable/cart-enable flags,
bank 0–3 and reset state 0–3. Optional GB payloads validate ROM/RAM selector
widths using the cartridge type in the existing fingerprint, Boolean RAM enable,
MBC1 mode and RTC latch, including the unaligned 1.2 latch layout. MBC0/1/2/3/5
and camera limits retain their actual selected setter ranges. Raw RTC/time and
camera register bytes are not narrowed to preferred values; camera register 0
retains its three hardware bits. Other upstream cartridge implementations get
finite union bounds; this does not certify their implementation or compatibility.

Actual GB storage helpers now use subtraction-based complete extents and full
32-bit RAM offsets. The stock 16-bit write helper could alias high camera RAM
banks into lower memory. Nonrumble MBC5 now uses its four-bit RAM bank, while
rumble MBC5 keeps three bank bits and the separate rumble action. Absent or
out-of-range reads return deterministic 0xff instead of uninitialized bytes.
Public cartridge/Transfer Pak calls check owner access, a nonempty at-most-32
byte packet, valid dispatch extents and bounded selectors before indirect calls
or bank multiplication. Empty writes cannot evaluate `data[size-1]`.

Pocket Camera retains its admitted RAM extent and checks its complete 4096-byte
tile output before capture/fallback writes. A missing capture callback follows
the bounded failure image path. An undersized camera RAM header is rejected
before RAM open, and initialization errors use an explicit unstarted discard
callback in actual main.c. Normal dirty close/retry retains its separate checked
retirement path; failed initialization cannot publish a guessed default save.

`gb_transferpak_cases.cpp` is authored, not run. It selects the actual prepared
GB/RTC/sensor/Transfer Pak sources and actual file-storage/save bridge/store,
with synthetic ROM/clock/rumble/capture absence and a no-op diagnostic fixture.
High-bank persisted offsets, ROM bank 511, rumble separation, camera output
extent, failed startup, packet bounds and both snapshot selector layouts have
independent cases. The full snapshot loader, real camera/input/game behavior,
RTC elapsed-time arithmetic/halt/carry and the remaining upstream cartridge
stubs/compatibility still require source completion/qualification. Other N64
CPU/peripheral selectors, resources and all original item-16 gates remain open.

## MBC3 RTC and explicit snapshot 1.10 candidate

The current selected-source preparer now emits the separate unbuilt identity
`3.0.0+omniplay.gl3.12edd2c74a51`. Earlier gl2 snapshots remain preserved. The
actual RTC source bounds all public register/owner access and uses unsigned
elapsed-time arithmetic for any signed-64-bit timestamp pair, without an
elapsed-time loop or signed subtraction overflow. Power-on anchors the clock;
backward jumps reanchor without reverse ticks. Halted time is excluded on
resume, writes charge preceding running time, the nine-bit day high bit is
stored correctly, and carry stays set until the program clears it. Raw counter
bytes use finite eight-bit equality-carry progression; this policy still needs
hardware qualification outside documented valid register ranges.

Live timer registers, the full last latch-command byte and readable latch
validity are distinct. An exact zero-one command refreshes a capture; zero or
other commands and live writes leave a valid capture readable. The behavioral
reference is [Pan Docs MBC3](https://raw.githubusercontent.com/gbdev/pandocs/master/src/MBC3.md).
The candidate writes explicit M64P 1.10 flags, imports canonical 1.0–1.9 and
1.10, and stages every new flag before mutation/callbacks. Legacy Boolean latch
imports preserve their original interpretation. See `N64_SNAPSHOT_FORMAT.md`
for the fixed-size wire contract, separate identity and admission requirements.

Actual-RTC/synthetic-clock cases cover normal boundaries, all signed-time
distance, backward/halted intervals, day carry, frozen latch/command edges,
bounded raw-byte progression, invalid/foreign access, all optional-cart layouts
and staged flags with rejected late ports. They are authored only. The full
loader, cross-core/version admission, real clocks/games/devices, subsecond
timing and RTC battery persistence remain completion/qualification work, as do
the remaining cartridge implementations, other peripheral/resource/write
closure and original Windows/product requirements. No package or OS write
grant changed and no N64 candidate is enabled by this source preparation.

## RTC battery source and retirement

The current gl3 candidate additionally binds a content/port-specific RTC
sidecar before first power-on, including RTC-only cartridges. Four stable
owners retain exact staged records, battery fields/offline time and failed
publication debt. Cold start is allowed only on literal absence; malformed or
unreadable records reject without publishing default RAM. RAM retirement and
RTC receipt consumption are coordinated, and later clock/snapshot changes
invalidate older receipts. Aggregate capture and quiescence now include RTC.

See `N64_RTC_BATTERY.md` for the exact 32-byte format, bounded canonical names,
checksum limitations, power-on/volatile-latch behavior, partial multi-file
flush semantics, source cases and actual main/worker/clock/crash qualification
still required. This adds persistence source, not evidence of a successful
cross-session save. No build, test, core or browser execution has occurred.

## Flash/SI/DD memory and device selector source

Actual flash commands and DMA now admit complete buffer ranges; SI checks its
entire 64-byte block before PIF callbacks and again at deferred copy. DD C2
sector subtraction and geometry access are bounded, including host-sector
register diagnostics. Snapshot admission checks known device selectors and
uses actual disk context for zero-ID DD records. Active PIF packets require an
owned opcode byte. See `N64_DEVICE_MEMORY.md` for exact behavior, invalid-request
policy, focused source cases/stubs and remaining full-core/device qualification.
This does not complete N64 state/resource/write closure or Windows item 2.

## PI/framebuffer source admission

The candidate now checks full adjusted PI RDRAM requests before dispatch or
graphics callbacks, reads every intersecting dirty framebuffer page before
cartridge input and preserves pinned length/timing control flow. Actual
framebuffer descriptors, page indices, mappings and CPU wrapper words use
independent complete RAM admission. GLideN64's six-slot identity lookup and
stores no longer index a full array's sentinel. Exact source cases and the
remaining handler-receipt, direct SRAM/ROM/DD qualification, address-vector, pixel/GPU and
real coherence qualification are described in `N64_DEVICE_MEMORY.md`.
No source preparation establishes full-core safety or changes product admission.

## Direct cartridge DMA source

Actual SRAM methods now check storage and both complete transfer extents,
preserve byte order and publish the physical word cover for short swapped
writes so later incremental patches include every changed byte. Cartridge
ROM methods admit complete RAM output before copy/invalidation, use widened
source arithmetic and bounded EOF/tail fallback. DD DS/C2/IPL methods admit
complete logical/physical views, check MMIO words and use wide cycle arithmetic.
Zero ROM/DD copies no longer request global cached-code invalidation. Domain
dispatch guards owner access before selecting save type. Focused actual-source
cases are authored, not run. See `N64_DEVICE_MEMORY.md` for unchanged formats,
timing scope, explicit observer stubs and remaining image-loader, MS sequencer,
graphics, persistence and full-core/Windows qualification obligations.

## Renderer pending coverage and CPU color conversion

The selected RDRAM-to-color path now replaces its unbounded address vector
with exact fixed byte coverage, consumes only the processed image range and
retains other frames' writes. Full identity-cache reads check pending bytes.
Geometry, RAM and CPU staging extents are bounded before conversion; output
dimensions are admitted against the driver and a 64-MiB CPU staging limit.
See `N64_DEVICE_MEMORY.md` for actual converter fixtures, halfword/black-pixel
semantics, explicit lack of GPU success receipts and outstanding full renderer/
driver/context-loss/accounting qualification. No test or renderer was run.
