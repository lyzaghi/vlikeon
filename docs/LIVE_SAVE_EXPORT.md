# Live progress export source

The shared players now expose separate **Export progress** and **Export snapshot**
actions when the selected runtime declares the matching capability. This is
authored source: no runtime, export, file picker/provider, compiler or test has run.

These actions capture paused live progress and write an ordinary OPSV0001 export
without committing a local save-history record. They therefore bypass the local
history count/byte admission limit. They keep the game paused, leave local heads
and sync/outbox state unchanged, retain the unsaved-progress warning and let the
player explicitly resume. The file can later be imported through the existing
content/compatibility checks with a fresh local save ID. Export is not a claim
that native snapshots are portable across incompatible platforms/cores/configs.

The destination is selected from the initial click before waiting for pause or
performing a potentially large capture. This preserves the Web file picker's user
activation requirement. Player flags gate conflicting save/load/resume/disc/
measurement/exit actions while exporting. Web pause requests share one in-flight
pause task. Background autosave waits. A failed ordinary save does not by itself
prevent a later explicit live export.

## Native capture and streaming Web capture

The desktop native command requires an actually paused owned session, checks its
declared save capability through `SessionManager`, captures from that adapter and
creates metadata from the actual game/core/config/device identities. Its new ID,
generation 1 and absent causal parent describe an independent export; no history
head is promoted. The broker consumes a native-selected single-use file capability
and writes directly to that open destination, without a profile transfer temp file.
This is still synchronous bounded-payload source; native file I/O and capture
cancellation/latency require the final platform qualification.

The WASM player captures its actual runtime buffer, preserves the active
compatibility fields and sends a bounded export-only stream through the platform
bridge. The host validates metadata, selected destination, exact offset/length,
canonical base64 and incremental SHA-256. There is one live export per host,
48 KiB chunks and a 256 MiB payload maximum. Its idle lease uses a monotonic clock
and is checked on transfer/host activity. It is not an independent watchdog that
can preempt stalled kernel/browser I/O.

The shared Rust export pool writes to the chosen open file and allocates no profile
save/temp blob for the streamed payload. Abort, expiry checks and host close drop
the uncommitted file guard, which attempts to truncate an incomplete newly selected
file. The desktop extension tracks export capabilities by originating page and
aborts abandoned/late responses where the native channel remains available.
Cleanup is best effort under blocked I/O, process termination or OS failure.

## Envelope completion and delivery

The OPSV0001 wire format and its importer remain unchanged. Native writers first
write an invalid zero magic, the validated metadata length/JSON and complete
payload. Only after byte-count/hash verification and a data flush do they publish
the magic and flush again. Ordinary history exports also use this helper. This is
not a filesystem transaction or power-loss proof; import still validates the full
envelope and payload hash. Failed exports may leave an empty/invalid artifact or
an outcome whose acknowledgement was lost. The player keeps live progress and
does not claim a successful export without a successful response.

Standalone Web uses a writable File System Access stream when supported; it
closes that stream before returning `file_handle` delivery. Its fallback retains
bounded Blob parts, builds a complete verified export and starts a browser download.
That result is explicitly `browser_download_started`, with a visible retry/dismiss
link inside the gameplay dialog. Ordinary history exports keep their download links in a persistent detail-dialog dock; history/diagnostic/measurement messages also distinguish a download start from completed delivery. The UI asks the player to check completion before
closing the game; a download start is not a durable-storage acknowledgement.
See the [File System Access contract](https://developer.chrome.com/docs/capabilities/web-apis/file-system-access).

Android/iOS route the new final commit through their existing native document
provider completion path, rather than acknowledging only a private staging file.
iOS now rejects a missing/expired native export selection. Provider delivery is
identified separately from a direct file handle. These adapters still use private
staging and their existing picker/provider deadlines; provider acceptance does not
prove remote cloud upload or power-loss durability. Staging, metadata access and
the selected destination still need physical space. This feature bypasses logical
history quota and does not promise recovery from every disk-full/provider failure.

The final consolidated pass must cover both save kinds, actual runtime barriers,
normal and full-history profiles, capture/ordinary-save overlap, pause/focus loss,
owner closure and late responses, malformed/replayed chunks and wrong destinations,
hash/length mismatch, partial writes, flush/close failure, stale capabilities,
mobile staging/provider deadlines, FSA cancellation, download blocking/retry and
round-trip compatibility. No such evidence exists yet. History paging now has source in `SAVE_HISTORY_PAGING.md`. Bulk backup/restore,
broader storage recovery and the full remaining product scope stay required.
