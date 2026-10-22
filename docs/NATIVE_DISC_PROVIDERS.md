# Scoped native disc reader foundation

The shared CUE/M3U disc importer now accepts either its existing selected local
directory or an explicit native `DiscProvider`. Both use the same reference
normalization, text decoding, CUE validation, playlist/disc/asset bounds, generated
local names, immutable payload hashing and final media-manifest admission. This
avoids making each mobile adapter invent a second CUE/M3U parser or copying every
file in a selected folder before the user chooses an entry point.

A native selection supplies 1–64 distinct top-level supported entry filenames
and a bounded title. The native provider remains responsible for the selected
document tree and actual OS authorization. The core validates filename/path
components again, refuses unsupported/ambiguous entries and admits the source
through the existing owner-bound five-minute, one-use directory grant. Only the
chosen entry's referenced assets are opened. Reference paths remain normalized,
local and at most eight components; no URL, arbitrary native path or callback
table is accepted through renderer IPC.

`ProvidedDiscFile` has an internal deduplication identity, an optional declared
length and an owned `Read + Send` reader. Local directory sources retain actual
opened files and known lengths. Providers can report unknown length and return a
lazy reader/token rather than opening every data handle during planning. CUE/M3U
text stays bounded to 128 KiB and uses the existing explicit encodings. A typed
provider I/O error can retain cancellation/permission/deadline classification;
ordinary filesystem error strings remain redacted.

Import tracks declared lengths where available and rejects short, long or empty
assets. Unknown lengths are not guessed. Per-asset and whole-set observed bytes
are checked against the existing 4 GiB bound before writing each 64 KiB chunk.
Cancellation is checked before and after a read; an invalid returned byte count
is rejected before slicing. The final media manifest uses the actually copied
byte count and hashes. Known-length desktop inputs keep their existing expected
total, generated names and content-identity contract. The shared progress panel
shows copied MiB and asset position when total length is unknown, then the final
observed total at completion, without inventing a percentage.

The mobile C ABI adds `om_mobile_authorize_media` with separate native-only
version-1 open/read/close/cancel/release callbacks. Rust consumes the context on success
or rejection, requires object-shaped bounded entry/title metadata, binds the
source once to the opened profile and exposes only the ordinary folder selection
grant to Game Hub. Each open receives the bound sixteen profile bytes and a
borrowed normalized relative UTF-8 path of at most 2048 bytes/eight components.
Callbacks cannot register themselves through a renderer message or reenter Rust.

A successful open returns a unique nonzero stream token plus length 1..4 GiB or
-1 for unknown. At most 129 live tokens are admitted, covering the 128-asset plan
and transient metadata reading. Lazy tokens may defer actual provider data I/O
until the first read. Native reads borrow an initialized buffer of at most 64 KiB;
count, EOF and declared/observed length are checked. EOF/error/drop closes a token
once. Readers retain their provider through Arc ownership after grant consumption,
and final context release follows the last reader/source owner. The import job
holds only a weak cancellation reference, so status retention does not prolong
the native provider's lifetime. Cancel/detach/drop mark the existing atomic flag
and signal native cancellation without waiting on provider I/O or reentering Rust.
Native open
failure owns its own partial cleanup; duplicate tokens are rejected as a protocol
failure rather than closing another reader's ambiguous token.

The unsafe native caller must supply valid callbacks/context, thread safety,
scope enforcement, borrowed-buffer safety, native cancellation and OS document
coordination. Runtime count checks cannot repair a callback that already wrote
past its borrowed memory. A generic `Read` can block; Rust cancellation cannot
preempt a hung native/provider read. Per-provider deadlines, descriptor/pipe abort,
security-scope/FileCoordinator ownership, native tree lookup/aliases and cleanup
are required in the platform adapters. The callback contract alone proves none
of those behaviors.

Android now has an authored SAF picker, selected-tree/lazy-stream implementation
and JNI adapter; its native hello augments `disc_import` only on Android 11+.
See `ANDROID_DISC_IMPORT.md` for its bounds and remaining provider/device evidence.
iOS now also has an authored security-scoped folder picker, retained coordinated
producer and bounded synchronous consumer, with native hello capability and
document-lifecycle integration. See `IOS_DISC_IMPORT.md` for its ownership and
cooperative I/O limits. Neither adapter has been compiled or qualified. No callback, provider,
file/URI, import/grant, parser, compiler, application or test has run. This source
does not complete coordinated mobile multi-file import, filesystem hardening,
orphan/live quotas, device/store/release qualification or the remaining product.

The final consolidated pass must cover unchanged local-directory identities and
all encoding/CUE/M3U fixtures, native selection bounds/owner/expiry/revocation,
relative traversal/aliases and selected-tree enforcement, known/unknown/empty/
changing lengths, duplicate tokens and stream limits, partial/oversized/error/
EOF reads, byte-count/total overflow, cancellation around blocking operations,
lazy-reader opening and exact close/release lifetime, failed admission/spawn and
native context cleanup, progress semantics, final manifest/hash admission and
actual Android/iOS document-provider behavior. ABI memory-safety tests require
trusted controlled native fixtures, not arbitrary callback pointers in production.

Primary contracts: [Rust reader bounds and blocking semantics](https://doc.rust-lang.org/std/io/trait.Read.html),
[Android selected document-tree access](https://developer.android.com/training/data-storage/shared/documents-files#grant-access-directory).
Platform implementations must additionally follow their native provider APIs.
