# Bounded native writable files and Windows core routes

Source only. No compiler, file-store operation, core/worker, sandbox/OS API,
application or test has executed. No core binary or installed catalog entry has
been produced or approved. Item 2 and the full original objective remain open.

## Memory store and ABI

`engines/native/src/writable_files_v1.h` defines an explicit versioned C callback
table for `save/` and `scratch/`. The frontend owns all payloads; a core receives
monotonic integer handles and bounded operations, not host file paths or OS file
handles. The table includes stat, mkdir, open/close, offset read/write, resize,
flush, remove, same-root rename, revision-bound directory batches and usage.
Exceptions stay inside frontend wrappers and become finite result codes.

`MemoryFiles` allocates value-initialized 4-KiB pages. Live allocated payload is
bounded to 272 MiB for save and 64 MiB for scratch, with 256 MiB per logical file,
4096 files, 8224 total nodes including roots/ancestors and 256 open handles.
Transfers are at most 8 MiB. Save allocation includes the existing 256-MiB save
envelope plus at most 16 MiB page-rounding overhead for 4096 small files; captures
still admit only 8192 archive entries and the existing 256-MiB total envelope.
Metadata and page-pointer arrays have finite independent node/file bounds.
The store allocates pages on demand; it does not preallocate its full allowance.

Every mutating/read operation holds one store mutex. Growth allocates all new
pages before touching old bytes; quota/allocation failure leaves content and
counters unchanged. Gaps and truncated tails are zeroed. Unlinked or replaced
files held by an open handle keep their payload/node charges until final close.
Name lookup uses Windows invariant Unicode case equivalence, strict UTF-8,
finite depth/length and reserved-device/stream/traversal rejection. Other
targets retain ASCII-only admission pending their native policy. Rename stages
the full namespace/path changes before commit, including subtree length checks.
Directory batches fail STALE after an intervening namespace/content revision.

This is bounded process memory. The OS may page it; it is not a no-disk-I/O or
whole-machine memory claim. `flush` acknowledges memory visibility only. Durable
save history still requires the broker's committed save protocol.

## Save capture and replacement

`SavedFiles` can capture the store directly into the existing OPSF0002 envelope,
then repeat the common strict archive admission and optional exact MD file-shape
checks. Scratch is excluded. In-game portability and OPNS0002 snapshot payloads
retain the existing formats and compatibility metadata; this storage transport
adds no unadvertised conversion relation.

Restore fully decodes the archive before staging a separate bounded save tree.
It preserves scratch and unrelated save roots. The owner must have no remaining
guest handles, and the store revision must still match the staging start. All
allocations, duplicate/path/type and aggregate quota checks precede ownership
and namespace transfer. Failure keeps the prior tree. Peak restore can contain
old and new save pages plus bounded input/output/rollback envelopes; it is not
a single-budget peak-memory claim. The newly authored Windows MD package limit
is 1536 MiB to leave room for that path, still subject to real qualification.
The existing guest shutdown/restart and prior file/core/disc recovery sequence
remains responsible for a rejected guest restore.

## Worker, core and OS permission agreement

The worker advertises `writable_files_protocol=1`. A requesting package must
select an exact admitted Windows software MD canonical-file or native3 PS1
combined-memory/file route, its checked-I/O contract and guest pause barrier.
Before retro_init, the worker
resolves `omniplay_install_writable_files_v1`, installs its owned table and checks
success. Prepare reports the actual installed mode; the broker requires the
matching acknowledgement. Missing/rejected ABI is not a disk-write fallback.
The table remains alive through guest restarts, unload/deinit and library
shutdown; uninstall occurs only after guest producers retire.

If the core refuses uninstall, the owned worker now calls immediate process
exit before unloading the DLL, destroying its memory table or acknowledging a
successful stop. It retains the callback context until actual actor process
retirement rather than trusting unconfirmed producer/handle shutdown. The
broker must still observe the real process/job exit before cleanup authority;
no final save or successful stop follows from this failure path. Actual OS and
blocked/unretired-producer evidence remains in item 16.

For protocol 1 the Windows broker creates ordinary work save/scratch placeholders
but grants the actor only readonly access. It does not apply the former writable
DACL/low-integrity grants. The separately authored default AppContainer
filesystem/private-registry sealing and actual token checks still precede
resume. Broker-owned selected ROM/firmware copies remain unchanged.

The ares MD native recipe defines `OMNIPLAY_NATIVE_MEMORY_FILES`, copies the exact
ABI header and omits the legacy OS publisher translation unit. SRAM/EEPROM
restore/read and temporary checked publication use only table callbacks. Missing
installation cannot fall through to fopen/write. The new core receipt binds
protocol 1, adapter/header hashes and the library hash; the packager requires
those values before emitting protocol 1 in its catalog. The shared Web branch
retains its existing browser filesystem path and does not advertise this native
mode. No active/development installed core catalog has been populated.

The native3 PS1 route adds exact C card callbacks for FrontIO reads/synchronous
saves, the asynchronous publisher and checked all-card barrier, with no OS card
fallback. It joins older jobs before inline index swaps, rejects malformed or
failed reads without changing guest NV, and blocks cleanup writes after read
failure. Exact core/version/source/format/ABI and adapter receipts are required.
The common worker's combined SRAM/RTC/file capture and guest restart/recovery
remain. See `PS1_MEMORY_CARDS.md`; its C bridge cases also remain unexecuted.

## Remaining work and final evidence

The Windows broker now withholds the former unconditional scratch write grant
from protocol-zero packages that declare neither filesystem saves nor a scratch
system directory. Placeholders remain readable; no failed launch can retry with
a broader grant. A single package-policy function selects the actual grant and
the changed route's opaque-state identity (`os_scratch_policy=1`). In-game
progress identity is unchanged, and no implicit opaque-state conversion is
added. Existing explicitly declared legacy disk routes still receive their
earlier grants and remain unfinished bounded-storage work; protocol-one routes
continue to use the bounded store. Cores relying on undeclared scratch writes
must be adapted and qualified, not silently granted extra access. This is source
policy, not actual-token or whole-filesystem quota evidence.

Antigravity batch 009 directly compiled the production MemoryFiles source with
its four existing contract groups on Windows MSVC, then executed successfully.
The production audio ring's four contract groups also compiled/executed. Source
hashes remained unchanged; actual core/ABI/store admission, allocation-fault,
device and OS-denial cases remain required. These fixtures do not exercise the
new broker scratch-grant decision or prove all cores' writer closure.

Other native cores still use their previous OS writable routes and require
separate adapters and exact packaging/ABI admission. Trusted helper/driver
caches, physical whole-profile accounting, broader Windows access qualification,
ambiguous legacy recovery and every original product/platform/release scope
remain due. These MD/PS1 routes do not close Windows containment generally.

The standalone source cases under `qa/native-memory/` cover actual store
operations, sparse/zeroing behavior, open-unlinked debt, aliases, moved handles,
revision-bound lists, replacement failure preservation, empty directories,
unchanged archive identity, integer/transfer bounds and handle/file caps.
The direct MSVC batch above ran the existing groups; full acceptance remains
in item 16. Run both allocation-fault and real worker/core/OS matrix cases;
the standalone store cases do not prove ABI, save recovery or access isolation.
