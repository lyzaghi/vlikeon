# Verified positional media for the WASM frontend

This is authored source only. No native file operation, hash worker, lease,
positional read, pipe, JNI/Swift call, shared-memory wait, browser, compiler, game
or test has run. It connects a native alternative to temporary OPFS copies; it
does not supply advanced core binaries, firmware, multi-disc controls/state or
qualified mobile/desktop playback. Every original release requirement remains.

Desktop and mobile native hello responses now expose optional range protocol 1.
The existing origin/profile channel checks still apply. A selected library game
can start one background verification job. The service derives all asset paths,
names, lengths and hashes from its validated library/media manifest; the caller
cannot submit a path, source URI, OS descriptor or arbitrary filename. At most
128 ordinary assets and the existing four-GiB set bound are admitted. Legacy
single assets receive a generated internal name. No new HTTP route is added.

Verification opens and retains the actual asset files, hashes every byte with a
64 KiB buffer on one worker and checks declared size, complete SHA-256 and file
metadata before/after. Named/opened metadata must agree. Windows opens restrict
sharing to readers and refuse reparse attributes; Unix metadata also records
device/inode and modification/change timestamps. Path-based opening still has
namespace-race limits; this is not a complete handle-relative filesystem proof.
Stamp/hash checks are not a guarantee against privileged storage or a malicious
same-user writer that defeats the observed metadata. Final platform hardening
and immutable-asset/quota qualification remain required.

The control thread exposes bounded verification status, progress, stop and
collect. A successfully collected set has a random owner-bound UUID, exact game
content identity, ordered asset descriptors, 64 KiB read limit and two-minute
lease. A completed uncollected result also expires after two minutes. Collection
can recover a lost reply for the same still-open job/set; it cannot resurrect a
closed or expired set. A native background worker retires elapsed sets even
without a renderer poll, closing descriptors outside the control mutex. Work
queue, active job/set and positional read admission remain bounded. Full hashing
has a thirty-minute cooperative deadline and cancellation checks around reads.
No native kernel read/metadata/close preemption is claimed.

The synchronous read capability is restricted to that owner/set and a numeric
asset index. Reads check safe bounded offsets/lengths, retained-handle metadata
before and after I/O, exact bytes and a capped request deadline. Each profile
admits one positional read at a time. A changed stamp cancels the entire set.
The recent 32 read request IDs retain only request fingerprints, never ROM
payloads; changed-content reuse is rejected across normal/read control paths.
Responses retain correlated set/index/offset/finality metadata and at most 64 KiB
encoded content. As with other bounded replay windows, ancient evicted IDs are
not a permanent anti-replay ledger.

Mobile Rust admits the read while briefly holding the local host lock and then
performs file I/O after releasing it. Android adds a single admitted read on a
separate bounded executor; iOS uses a separate serial queue with a nonblocking
permit. Stop, keep-alive and document detach can therefore mark cancellation while
the read is outstanding. A closing mobile registry entry retains ownership until
range I/O and deferred descriptor retirement actually finish, avoiding immediate
same-process profile reuse during that work. Native OS stalls may still hold it.

Desktop stdio now has one bounded media read lane and a shared framed output
writer. The main channel can dispatch later control work while an already
admitted read performs file I/O; output frames cannot interleave. The serial
control dispatcher can still delay initial admission behind a native dialog or
another blocking command. Process/pipe termination and hard I/O deadlines remain
qualification issues. No separate arbitrary-PID, filesystem or website API is
exposed. The component background service tracks each page's jobs and sets,
rejects another page's media capability, cancels on page disconnect and retires
late start/collect results instead of handing them to a replacement document.

The player prefers native ranges for file-backed content only when native hello,
actual cross-origin isolation and SharedArrayBuffer/Atomics availability agree.
It validates the complete verification/set/asset reply against the library
manifest, polls bounded progress and renews a collected set every thirty seconds
while its runtime owns it. Close, failure, cancellation and late session cleanup
retire the capability. A renewal failure stops the runtime; it cannot continue
indefinitely from cached blocks after its owner loses the lease.

The dedicated emulator worker receives a fixed 64 KiB shared byte region plus
four atomic header words. It emits one correlated read request, then blocks only
that worker with a bounded Atomics wait. The UI performs the asynchronous native
call and checks every response before copying bytes and atomically publishing
completion. No native thread borrows or writes a JS/WASM pointer. Header sequence,
state, count and capability/index/offset bounds are checked on both JS sides.
Late or canceled replies do not publish into a new request. Runtime stop marks
the channel closed and notifies the waiter before worker shutdown; asynchronous
native I/O can finish later but cannot write through an old borrowed pointer.

Reads are aligned into 64 KiB blocks with an LRU of at most 64 blocks (four MiB)
for the whole media set. The existing read-only Emscripten backend then serves
the exact named files, checked seeks and partial reads without a disc-sized
ArrayBuffer or browser-disk copy. Full-path libretro loading is still required
for bundles. Cached reads check channel closure too. Mmap remains unsupported.
Base64/JSON messaging, full initial hashing, control round trips and file stamps
have real performance costs; no latency/FPS/energy improvement has been measured.

The native route creates no OPFS content and therefore avoids per-port orphaned
staging for sessions that actually select it. Browsers without isolated shared
memory retain the separately authored OPFS route and its feature/quota/cleanup
limits, including recovery across changing native loopback origins. No universal
iOS/WebKit support or release qualification is inferred from exposed JS symbols.
An appropriate alternative for unsupported environments and actual device
evidence remain required. Empty development core catalogs remain explicit.

Pinned Chromium filesystem/JNI/profile snapshots were retrieved and checked
against upstream Git blob identities during design. No Chromium cleanup API or
new origin deletion implementation was executed or enabled. Those snapshots
remain source inputs; native ranges use the existing Rust library descriptors.

The single final verification pass must compile the full Rust/JNI/Swift/browser
targets; cover profile/page/set/index ownership and changed request IDs; every
manifest/hash/size and Unicode/legacy fixture; maximum asset counts/file handles;
zero/EOF/4-GiB/large-offset reads; mutation, symlink/reparse and namespace races;
concurrent admission, lost start/collect/read replies, expiry and renewal; stop
during hash/read/metadata/close; blocked I/O, mobile registry retirement and
desktop output framing; Atomics state/sequence races, partial/cache reads and
cache closure; UI responsiveness, core load/save/unload I/O and real device
latency/memory/energy; process loss and unchanged ordinary transfers. No narrow
source or assembly result proves those behaviors.

Primary contracts: [ECMAScript shared memory and Atomics waits](https://tc39.es/ecma262/2024/multipage/structured-data.html),
[HTML agent clusters and isolated shared memory](https://html.spec.whatwg.org/multipage/webappapis.html),
[Rust retained file operations](https://doc.rust-lang.org/std/fs/struct.File.html).
