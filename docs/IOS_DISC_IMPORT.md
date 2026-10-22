# iOS coordinated disc folder source

The iOS product now has an authored `.folder` document picker, selected-folder
reader and C ABI integration with the common Rust disc importer. No Swift/Rust
compiler, Apple SDK, File Provider, document API, browser, application, simulator,
device or test has been run. The core catalog remains a development catalog;
library import is separate from having a compatible admitted mobile runtime.

Only the trusted main-frame Game Hub handler can request the native picker. The
selected file URL comes from UIKit, and NativeCore binds the resulting callbacks
to its native profile. No renderer-supplied URL, absolute path, descriptor,
callback or provider ID is used. Before allocating a context, native preparation
retires elapsed Rust capabilities so abandoned selections need not permanently
occupy a context slot. No security-scoped bookmark or persisted folder grant is
created. The renderer receives the ordinary owner-bound, five-minute, one-use
grant and its bounded supported entry names/title.

`SelectedDiscFolder` starts access to the picker URL and retains that access
until its final owner and all queued/running I/O finish. Four contexts share a
process admission budget. Weak UI references permit navigation cancellation
without retaining completed providers. Rust consumes a retained Swift context
through every callback-admission success/failure path; callbacks are constructed
inside the actual FFI invocation, after JSON serialization has succeeded. Native
release marks stop before relinquishing that retained owner. Running work keeps
the scope and admission budget alive even after Rust returns an import failure.

Preparation and native `open` inspect immediately available metadata through
NSFileCoordinator. They do not read file contents under that metadata-only option.
Only supported top-level CUE/M3U/CHD/ISO/PBP names are offered, at most 64. A serial
metadata admission lock and per-context I/O queue bound coordination work. The
tree caches at most 256 directories, inspects at most 4096 entries and counts at
most two MiB of name metadata. Native provider/kernel directory allocations are
outside those application cache limits. A placeholder or incompletely enumerated
provider can require refresh/reselection; absent entries are not invented.

Paths remain normalized relative UTF-8, at most 2048 bytes/eight components, with
bounded names and no dot/parent, absolute, separator/colon/control or ambiguous
trailing dot/space component. Swift dictionary equality also treats canonically
equivalent duplicate names as ambiguous. The selected root is opened with
directory/no-follow flags and retained by descriptor. Descendants are traversed
with `openat`, directory/no-follow flags and regular-file checks. Static symlinks,
special entries and read-time Finder aliases are refused. The root path must
still identify its original device/inode; an unexpected coordinator relocation
requires a new picker. The accessor URL is checked and used, with named/opened
inode/type checks binding the coordinated entry to the scoped descriptor read.
This does not prove immunity to every provider, concurrent namespace mutation,
hardlink, kernel or malicious-storage behavior; final filesystem review remains.

Opening a reference creates a lazy unique token, with at most 129 live tokens.
It deliberately reports unknown length because immediately available metadata
may describe a cloud placeholder. At first read, a dedicated producer enters a
normal coordinated read, allowing the provider to materialize content and flush
presented changes. It holds the accessor through the complete file read and
descriptor cleanup. One producer is admitted per context; other planned tokens
need no open data handle or worker. Namespace metadata remains separate from the
actual data stream, and a changed/moved root fails closed.

The producer and Rust consumer exchange a single bounded 64 KiB slot. Partial
consumer reads retain its remaining bytes; the producer waits for consumption
before publishing another block. No queued operation stores or uses a Rust
pointer: only the synchronous `read` callback copies into its borrowed output.
EOF is published after the accessor, file handle and coordinator have completed.
The producer checks regular/nonempty file size, actual bytes and before/after
device/inode/type/size/mtime/ctime; Rust independently enforces per-file and set
limits, text decoding, CUE/M3U structure, exact hashes and immutable manifest
admission. A failed or partial copy does not publish a successful game entry.
Matching metadata is an integrity check, not proof of a coherent malicious
provider snapshot. Game Hub reports actual copied MiB for an unknown total.

The selection starts with the request's remaining deadline, receives five minutes
to begin after preparation, and starts a thirty-minute import window on first
reference open. Metadata waits and consumer read waits have thirty-second
cooperative limits. Provider timers and import Stop/detach wake the stream slot,
mark state independently of the control queue and cancel the current coordinator.
Apple specifies that coordinator cancellation can be called on any thread and
returns immediately; an accessor already running still must finish. Kernel/file
reads or close can stall, and iOS can suspend the app. Such work retains its scope
and one of the four context slots until it actually retires. No hard I/O
preemption, immediate OS cleanup, background execution or final-save guarantee is
claimed. A canceled consumer cannot later receive asynchronous native writes.

The native hello now advertises the authored folder capability on the iOS 17+
product. Bridge requests gain independent deadline replies and pending-ID
admission. Provisional document navigation, page failure and WebKit termination
cancel selected folders/pickers, reject old pending replies, clean private export
staging and detach the old Rust owner. Generation checks prevent old document
completions from presenting new pickers or issuing new file authorizations.
Existing single-file import also checks its deadline between bounded reads,
uses normal content coordination and applies a separate 128 MiB firmware bound.
A timed-out external operation may still have effects; a deadline reply is not
an undo acknowledgement.

The privacy manifest adds the user-selected-file metadata reason `3B52.1` beside
the existing app-container reason. Local monotonic timers retain `35F9.1`; raw boot
time and source metadata are not sent by this adapter. These declarations require
the final app/dependency/API and store audit; they are not a compliance result.

The consolidated final pass must compile the full Apple target and exercise local,
iCloud and third-party providers; fresh/revoked scopes; Unicode/normalization and
duplicate names; folder relocation/replacement and symlink/alias/special entries;
directory/cache/token/context limits; metadata-only placeholder handling and
hydration; complete CUE/M3U fixtures/encodings and real image hashes; partial
slot reads, EOF and producer retirement; file mutation, size/byte bounds and
slow/stalled reads/close; cancellation before/inside an accessor; native admission
failure, expired grants and lost replies; navigation/process termination/picker
deadlines; actor/thread/buffer lifetime and resource cleanup; real device memory,
energy, App Store/privacy behavior and unchanged single-file/export flows. Native
mobile runtime, Web multi-file playback, quotas/recovery, full background sync,
streaming, release signing and the remaining complete product scope remain due.

Primary contracts: [directory picker and security scope](https://developer.apple.com/documentation/uikit/providing-access-to-directories),
[coordinated accessor lifetime and supplied URL](https://developer.apple.com/documentation/foundation/nsfilecoordinator/coordinate(readingitemat:options:error:byaccessor:)),
[metadata-only reads](https://developer.apple.com/documentation/foundation/nsfilecoordinator/readingoptions/immediatelyavailablemetadataonly),
[coordinator cancellation](https://developer.apple.com/documentation/foundation/nsfilecoordinator/cancel()),
[file metadata/timer privacy reasons](https://developer.apple.com/documentation/bundleresources/app-privacy-configuration/nsprivacyaccessedapitypes/nsprivacyaccessedapitype).
