# Android selected-tree disc import

This is authored product source, with no Android/JNI/Rust compilation, provider
request, document import, application launch or test execution. Android 11+ now
has a connected native SAF disc-folder path; iOS's coordinated folder adapter
remains outstanding. No core compatibility, hard I/O deadline, snapshot of a
changing provider or store qualification follows from these sources.

`NativeDocuments` intercepts `disc_select_folder` only on the trusted Game Hub
channel and opens `ACTION_OPEN_DOCUMENT_TREE` with read access. It requires a
readable `content:` tree returned by that picker. The renderer never supplies a
URI, provider authority, document ID, filesystem root or callback. No persistable
grant or broad storage permission is requested. The application-context resolver
avoids retaining an Activity in a long-lived native provider. Ordinary single-file
imports and native export staging keep their existing separate flows.

Preparation runs on the document I/O executor and authorizes only bounded native
metadata through `om_mobile_authorize_media`: a title and 1–64 supported top-level
CUE/M3U/CHD/ISO/PBP names. Game Hub then chooses the entry, system and existing
explicit text encoding. Rust keeps the owner-bound five-minute one-use folder
grant and its common CUE/M3U normalization, copy/hash, actual byte accounting and
media manifest. The existing player still requires a compatible admitted runtime;
importing a PS1/PSP library entry does not create a playable mobile core.

The native hello adds `disc_import` and provider protocol 1 only on Android 11+,
where the selected nonblocking descriptor API is available. Older Android keeps
normal single-file import without advertising this folder path. Missing folder
pickers and provider failures return errors; there is no fabricated successful
selection. This does not change upstream Chromium's actual Android minimum.

`SelectedDiscTree` checks the root is the returned directory and resolves each
normalized path component using exact display names from direct-child queries.
Document IDs remain opaque and every query/open URI is built using that selected
tree grant. It never derives a native storage path from an ID. Absolute, empty,
dot/parent, separator/colon/control, malformed-surrogate, ambiguous trailing
dot/space and oversized names are refused. Duplicate usable names are ambiguous
and fail closed. Intermediate entries must be directories; repeated ancestor IDs
and virtual/transformed files are refused. Providers still declare their own
subtree semantics; the app cannot turn a compromised provider into trusted storage.

Per context, traversal has at most eight components, 256 cached directories,
4096 inspected rows and two MiB of counted ID/name metadata. A directory cursor
that reports `EXTRA_LOADING` is refused, so an incomplete listing is not silently
published. A provider's own Cursor/Binder allocation is outside these local
cache limits. Four provider contexts share a process-wide admission budget. Rust
allows sixteen pending folder grants, but Android's smaller context budget also
applies. Native storage preparation and ordinary host requests retire expired
capabilities before another context is allocated. Stale grants need not wait for
a renderer RPC to become eligible for release.

Up to 129 unique, increasing, nonzero stream tokens can be planned. A token retains
metadata without opening game data. The first read opens an AssetFileDescriptor;
its offset and declared subrange are honored and cross-checked against known
document length. Pipes are accepted only with zero offset. Reads use an owned
nonblocking descriptor, polling in at most 250 ms slices, retrying EINTR/EAGAIN
and rechecking cancellation/deadline around I/O. Known subranges end at their
declared boundary; short reads and known document-length disagreement fail.
Unknown lengths remain indeterminate while Game Hub shows actual copied MiB.
Rust independently bounds each file and the complete set to four GiB, checks
EOF/counts, and never publishes a partial game manifest as a successful import.

JNI retains the native provider and cached method IDs until Rust releases its
owned context. The sixteen-byte profile binding is checked again in Java. One
retained 64 KiB Java byte buffer is serialized for reads, with the actual borrowed
Rust capacity passed separately and checked before copying. Methods and native
entry points are retained by the ProGuard configuration. Thread attachment,
local frames, generic exception handling and GlobalRef release follow the
existing native credential bridge. ROM data, URIs and provider errors are not
logged or returned as control metadata. No native memory-safety result is claimed.

Preparation uses the remaining request deadline. A prepared selection has five
minutes to start; the first reference open starts a thirty-minute import window.
Each query/open/read operation also has a thirty-second cooperative deadline.
Timers mark cancellation independently of the serialized request queue. The
Rust import job holds a weak provider cancellation handle: explicit Stop, detach
and job destruction signal the provider without retaining it just for job status.
Native channel destruction also marks every selected tree stopped.

Cancellation synchronously changes bounded state and queues signal/descriptor
cleanup on two separate cleanup workers. Deadline workers perform no provider
I/O. A context's global budget is released only after both native release and
cleanup completion; a hung cleanup retains its permit and prevents unlimited
replacement contexts or queued cleanup. This remains cooperative: a provider,
kernel read/close or Binder transaction may ignore cancellation or stall despite
the watchdog. No hard preemption, background execution or process-kill guarantee
is made. Known/observed content limits and immutable admission still apply after
an operation returns. Source changes during import are not a coherent OS snapshot.

The final consolidated pass must compile JNI/Java/Rust at the pinned Chromium
revision and cover pre-30 capability hiding, real picker cancellation, root and
nested Unicode names, duplicates/aliases/cycles, loading/cloud providers, virtual
documents, permission revocation, asset slices, regular files and pipes, known/
unknown/empty/changing/oversized lengths, cached metadata/row/token admission,
stale selection expiry followed by a fresh picker, all CUE/M3U encodings, native
Stop during every I/O stage, tab destruction, late replies, hung providers and
cleanup, exact ownership on failed admission/spawn/EOF/error, manifest/hash
identity and actual device memory/latency. Full filesystem/quota/orphan recovery,
mobile core/runtime, iOS, background sync, streaming and release work remain.

Primary API contracts: [SAF subtree URI builders and loading cursors](https://developer.android.com/reference/android/provider/DocumentsContract),
[cancellable provider access](https://developer.android.com/reference/android/content/ContentResolver),
[asset offsets and declared subranges](https://developer.android.com/reference/android/content/res/AssetFileDescriptor),
[descriptor polling/nonblocking reads](https://developer.android.com/reference/android/system/Os).
