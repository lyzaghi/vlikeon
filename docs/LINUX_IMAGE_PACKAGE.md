# Authenticated Linux image package and shared cache source

The lower-level Linux source now authors authenticated catalog selection,
fd-relative ordinary package files, sealed profile-shared image preparation and
a finite ELF DT_NEEDED closure. These functions have not been invoked. No ELF,
image hash/admission, memfd, process, loader, syscall, cache/profile lease,
compiler, core/helper or test has run. Linux NativeRegistry execution remains
unavailable and no actual package or SDK is approved by this source.

## Catalog and held package files

RuntimePackage gains an optional closed `linux` declaration, required for Linux
catalog entries. It declares a distinct static bootstrap, bounded root/scratch/
save/file/CPU policy and at most 192 explicitly mapped images with worker,
loader, core, dependency or asset roles. The software presentation helper and
protocol 2 are mandatory for this current actor path; performance measurement
is not advertised by this unqualified brokered collector. The existing GPU/JIT
and full platform requirements remain separate unfinished product work.

The worker target is `runtime/omniplay-native-worker`; the core is beside it
under its compile-time basename, the loader is
`lib64/ld-linux-x86-64.so.2`, and library providers are flat runtime basenames.
Assets preserve their explicit assets-relative paths beneath runtime/assets;
declared package assets cannot be silently omitted. Targets are distinct and
cannot cross a file used as a directory. Images fit the declared root minus
2 MiB, each is at most 256 MiB, and assets additionally have the existing
32 MiB/file, 128-file and 128 MiB aggregate ceilings. No ROM/firmware is bundled
by this mapping.

The future caller supplies Store's already authenticated outer release inventory.
PreparedLinuxPackage admits a non-executable inventoried catalog of at most
1 MiB from a held release directory, compares actual bytes/hash, parses its
closed schema and selects the approved non-revoked runtime. Every declared file
must agree with that outer inventory's hash and length. Bootstrap, worker and
UI helper must also have its explicit executable flag. There is no developer
root or renderer-supplied approval bypass in this preparation path. Authenticating
the outer release and binding it to the running broker remains the Store/
product-factory duty; a caller-created manifest is not a signature.

ReleaseDirectory opens every relative component against the previous held
directory with O_PATH/O_DIRECTORY/O_NOFOLLOW/CLOEXEC; package directories cannot
be group/other-writable or owned by an unrelated user. Leaf reads are
nonblocking, no-follow, current-user/root-owned, ordinary single-link files.
File contents become immutable only after their authenticated copy is sealed.
The separate bootstrap/helper executable admitters retain their own readonly
reviewed ELF descriptors through launch. A separately authorized host-account
writer is not claimed to be contained by path metadata or inode ownership.

The Unix unpacker now authors final 0500 executable and 0400 data modes on each
held successfully written file before its final sync. This matches the direct
executable admitter's non-writable requirement. It changes no existing installed
file and invokes no chmod operation during this source-authoring work. Package
writer/reader and private bootstrap path rules now accept an interior plus sign,
so a literal dependency such as libstdc++.so.6 need not be renamed inconsistently.

## Finite ELF dependency closure

Inspection reads the authenticated sealed descriptors; it never invokes ldd,
dlopen, a compiler or an ELF interpreter. The current finite policy accepts
ELF64 little-endian x86-64 ET_DYN, bounded program/load/dynamic/string tables,
non-overlapping aligned LOAD memory ranges, no writable/executable LOAD, an NX
stack and a covered GNU_RELRO range. Worker/loader need a file-backed executable
entry. The worker needs the exact private interpreter, PIE and immediate-binding
flags; the authored native CMake target supplies PIE/RELRO/NOW/NX-stack and no
build/install RPATH. An optional interpreter on a shared object, including the
SDK libc, must be that same literal; the loader cannot request another one.

The parser bounds and de-duplicates DT_NEEDED names, restricts them to ordinary
basenames, and refuses RPATH/RUNPATH/TEXTREL, text-relocation flags, audit/config/
auxiliary/filter loader extensions. Dependency and loader SONAMEs must equal
their mapped basenames; a core SONAME, when present, must also agree. It resolves
every dependency to one explicit immutable provider and checks reachability from
the worker, fixed core and loader; unlisted or unreachable dependency objects
fail admission. Assets cannot masquerade as ELF providers. See the primary
[ELF format](https://man7.org/linux/man-pages/man5/elf.5.html),
[GNU dynamic tag definitions](https://raw.githubusercontent.com/bminor/glibc/glibc-2.40/elf/elf.h)
and [loader search contract](https://man7.org/linux/man-pages/man8/ld.so.8.html).

This is a declared DT_NEEDED closure and finite shape check, not a complete ELF
linker/parser, symbol-version/ABI proof, complete dynamic-plugin discovery or
security audit of a binary. The isolated root supplies no host library fallback;
actual SDK/loader/core compatibility still must be qualified. The normal-user
trusted UI continues to use its separately qualified system/driver closure.

## Shared sealed cache and lifetime

ProfileImageCache is shared within the broker by held private profile-directory
device/inode and profile UUID. A second live owner must use the same policy;
there are at most 64 live profile cache owners. Cache keys are authenticated
SHA-256 plus exact length. Hits return the previously copied immutable bytes;
they do not trust a newly changed host pathname. Misses copy/hash in bounded
64 KiB blocks through the existing sealed-descriptor owner. Concurrent misses
for the same key join one preparation; waits check cancellation every 100 ms
and the queue admits at most 64 waiting callers.

The caller selects a page-aligned cache ceiling of 16 through 896 MiB. Resident
plus pending images use at most that capacity and 192 slots. Least-recently-used
entries can be evicted only when the cache is their sole image owner. Active
images retain both their sealed byte reservation and the shared cache owner,
including when the launch reaper outlives the temporary preparation adapter.
The cache holds unretained resident wrappers; returned wrappers hold the cache
owner, avoiding a resident-object/cache reference cycle. Failed or cancelled
preparations retire their pending slot and wake joiners without publishing bytes.

The same sealed budget reserves 128 MiB and the remaining slots for finite
bootstrap plans, with an absolute 1 GiB/256-descriptor maximum. This counts
owned sealed payloads and plan reservations; it is not a hard claim about all
allocator, kernel, tmpfs copy, driver or process RSS. The profile delegation's
pair limits account separately for the actor/root/scratch/save/UI tree. Complete
whole-profile charging/provisioning still belongs to the product integration.

## Remaining product integration and evidence

The product factory/adapter must connect the approved package, cache, private
game/firmware/save staging, work journal and legitimately provisioned delegation
to the actor/control/final-save flow. Existing-tree crash recovery and bounded
confirmed storage/resource reclamation, compositor/user-gesture activation,
actual compiled SDK/sysroot/static bootstrap/dynamic UI closure and license/
source/relink provenance remain required. No catalog is installed and the
metadata parser accepting a Linux declaration does not enable native execution.

Final verification remains deferred until all original source authoring is
complete. It must cover actual ELF/ABI/SONAME/symbol-version closure, malformed
headers/offsets/dynamic strings and extension tags, sealed identity, cache
dedup/cancellation/eviction/lifetime/resource races, Unix unpack permissions,
package/path/FD/gate/process/work recovery, actual driver/core/device behavior
and every original platform/product/release requirement. No such result exists.
