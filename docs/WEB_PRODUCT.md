# Standalone Web/PWA source

The master plan's Web/PWA platform remains part of the complete product scope. This source adds a standalone local Game Hub path without native messaging or native executable access. It is an authored baseline, not a qualified Web release. No web server, browser, service worker, storage transaction, core or test has run.

The Web source now also has the shared-native Rust browser crypto ABI, a finite
foreground WASM worker, an origin-bound wrapped delegated-credential vault,
generation-fenced coordination, asynchronous account tickets and a scoped
HTTPS/CSP/CORS adapter, encrypted transfer journals, shared profile leases,
authenticated canonical save reconciliation, epoch-fenced consent, foreground
start/status/cancel jobs and explicit queued cloud repair. Both actual artifact/service policies remain disabled,
so `encrypted_sync` is false and the account UI is hidden. See
`WEB_SYNC_CRYPTO.md`, `WEB_SYNC_ACCOUNTS.md`, `WEB_SYNC_JOURNAL.md` and
`WEB_SYNC_RECONCILIATION.md`, `WEB_SYNC_RECOVERY.md` and
`WEB_SYNC_UPLOAD_RECOVERY.md`; no crypto, account,
network or storage operation was executed.

The shared player now has an additional bounded file-backed media transport:
manifest/bundle identity validation, named sequential transfers, OPFS staging,
one origin lease and a read-only Emscripten filesystem. Existing small raw ROMs
keep their memory path. Standalone firmware storage is now authored separately in `WEB_FIRMWARE_VAULT.md`;
selected-folder disc ingestion is now authored in `WEB_DISC_IMPORT.md`. Actual
folder/core/device qualification and admitted advanced cores remain required. Browser storage support/quota, full disc controls and
state, device evidence and the complete product scope remain due. See `WEB_MEDIA.md`.

## Product and storage boundary

`tools/assemble_web.py --without-cores` authors an explicitly developmental bundle in `out/web`. Release assembly requires the same explicitly approved built core manifests/artifacts and notices as the other products. A generated document marker selects `apps/web/platform/platform.js` through the shared bridge. The assembler relocates `platform/*.js` into `web/*.js`; shared compatibility, UI and runtime modules keep their existing root paths. Desktop extension and native mobile documents receive no standalone marker. Capability declarations expose the local firmware vault and hide unavailable native/sync/updater controls; folder import is exposed only with an actual browser directory-selection surface.

One browser origin owns one IndexedDB profile/device identity, library, content blobs, immutable saves, heads, preferences, measurements, firmware vault and lifecycle leases. The current schema is version 9: version 4 added firmware stores, version 5 save origins, version 6 payload revisions, version 7 encrypted sync staging/inbox/receipts, version 8 global save keyset ordering and version 9 explicit recovery requests and ciphertext purpose. Credential wrapping and explicit sync consent use the existing config namespace. There is no arbitrary profile selector or filesystem path parameter. Transactions request and require the exposed `strict` durability policy and await transaction completion before acknowledgement. This is the browser's storage contract, not an independent filesystem/power-loss guarantee. IndexedDB transaction durability is defined by the [Indexed Database API specification](https://w3c.github.io/IndexedDB/#dom-idbtransactionoptions-durability). Browser retention, user clearing of site data and real crash recovery still require platform qualification.

The storage panel reports separate logical content/save/firmware sizes, approximate browser quota/usage when available, and the result of an explicit persistent-storage request. It explains file backups and browser-managed retention. No account, telemetry or cloud upload is enabled. User-selected firmware remains local; it is not part of application caches or diagnostics. Dedicated HTTPS origin deployment is required; the included server recipe binds loopback for later local authoring/verification only.

## Library, data integrity and saves

File choices create bounded, single-use, five-minute grants in the selecting page. Imports read/copy and hash actual bytes in a dedicated worker using bounded 256 KiB buffers, retain immutable browser-owned Blob snapshots, expose progress/cancellation, reject incompatible detected formats and publish metadata/content/accounting together in a strict transaction. A nonqueued origin import lock prevents concurrent bulk writers. Reimport can repair payload bytes under the same verified identity while preserving existing game history; browser backing memory/quota and large-image performance remain unqualified. The new streaming SHA-256 source follows the [NIST algorithm specification](https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.180-4.pdf); no test-vector, WebCrypto comparison, performance or conformance claim has been made.

The library stores compact metadata separately from content blobs, bounds content at 4 GiB, and uses indexed stable title/UUID ordering and query-bound cursors. System filtering uses a compound index. Substring matching/counts still scan the selected metadata index; large-library performance and fuller search indexing remain required evidence/work. A profile has explicit logical content/save/count bounds in addition to browser quota. These limits do not promise that a browser grants that much storage. Removing a game is refused while progress or an active lease requires it; unreferenced content remains retained pending explicit storage maintenance.

Save capture validates metadata and payload integrity before opening the write transaction. That transaction assigns a local generation, validates the causal parent, adds immutable payload/metadata and moves a head only when its expected prior head still matches. Concurrent progress becomes a retained branch. Quota failure or transaction abort cannot publish a new head alone. Promotion verifies payload bytes and repeats compare-and-swap. Read transfers validate declared size/hash and the runtime compatibility target. Snapshot identity and the one-way PSP portable-file reader relation use the shared compatibility contract.

OPSV0001 exports use the same magic, little-endian metadata length, bounded UTF-8 JSON metadata and raw payload as Rust. Imports validate the full envelope and create a new local identity/branch without overwriting an existing head. The original core/platform/game-content compatibility remains. Writable file-picker exports await close; browser-download fallbacks retain a visible retry/dismiss link and report download initiation rather than claiming physical file durability.

Transfers use bounded 48 KiB chunks, exact offsets, strict base64, full-payload hashes, inactivity expiry and explicit cancellation. Save uploads keep at most two bounded staging blobs; content downloads hold at most four Blob leases. Deadlines/cancellation propagate into mutations and upload commits. Worker/file reads already in progress can delay cancellation observation, and final acknowledgement uncertainty after a completed storage transaction remains a normal retry/history concern. Fault, cancellation, quota and cross-tab cases remain unexecuted.

## Sessions, settings and measurements

The browser adapter admits only approved bundled runtime identities. Renderer candidate benchmarks are discarded; routing remains advisory and reports device performance as unmeasured. Observed sessions have owner-bound leases, bounded concurrent rows and serialized heartbeats. Playtime uses confirmed monotonic heartbeat intervals with a gap ceiling. Cross-tab lease expiry uses wall time for liveness cleanup, so clock changes and background scheduling still need qualification. A stopped reporting page cannot invent unobserved playtime or change another page's owned lease through this API.

Game settings and local-rule proposals have bounded values, explicit enablement, expiration, compare-and-swap against the previous configuration and next-session application. No external model is invoked. Web performance records validate approved core/content identity and sample bounds, retain at most twenty reports and keep the source unclassified and independently unattested. Reports can be compared under matching conditions, deleted and explicitly exported. Diagnostics omit game names/content, save bytes, paths and browsing history.

## PWA resources and updates

The source assembler uses an explicit Web file list plus the common approved Game Hub inventory. It refuses local edits in its generated output, checks links/ownership before cleanup, streams artifact hashing/copying and emits exact sizes/hashes/MIME types. The generated worker pins the manifest digest and build identity. Only inventoried application assets enter its cache; user content, saves and unknown/account requests do not. Core JavaScript/WASM is cached lazily after exact integrity checks. The storage UI reads which core payloads are actually cached; installing the shell does not imply that every core is available offline.

The worker does not call `skipWaiting` or force reload during another tab's game. An update waits for prior clients to close. Old application caches are retired only on activation. IndexedDB game/progress data remains separate. Offline delivery preserves security headers, including CSP and origin isolation. The static deployment header artifact and loopback server recipe use explicit resource routes without directory listing. Production TLS, immutable/atomic deployment, domain policy, server hardening and deployment verification remain required. The [service-worker lifecycle documentation](https://developer.mozilla.org/en-US/docs/Web/API/Service_Worker_API/Using_Service_Workers) informed the source; no lifecycle behavior has been exercised.

The web manifest currently has an authored SVG app mark. Installation eligibility, raster/platform icon variants, launch behavior and store/browser requirements remain part of the complete platform/release work. WebGPU/WebGL runtime profiles, remote streaming, opt-in encrypted sync, advanced file/disc providers, accessibility/device coverage, source/license distribution and the full final consolidated verification remain outstanding. No requirement is marked complete by this document or source assembly.

The shared remote-play source is now included, with disabled configured relay origins, signed comparison-code pairing, encrypted signaling, WebRTC viewing/capture and bounded QoS. `STREAMING_PROTOCOL.md` defines its exact scope. The PWA's source assembly derives `connect-src` only from validated configured HTTPS origins and carries that policy into offline responses. Its build identity now includes the service-worker template and deployment headers as well as resource hashes, so a worker-only change cannot reuse an old worker's cache identity. No remote-play or update behavior has been run; all native/mobile credential, input/capture/codec/deployment and final verification work remains required.

The current standalone IndexedDB version is 3 in the original database name. Its transactional upgrades add numeric favorite metadata, favorite/system compound indexes and removed-save ID retention. The shared UI combines favorite, system and text filtering with version-2 cursors. Profile appearance now stores fixed palettes and library layouts through strict local preferences; content/save identities remain unchanged. See `LIBRARY_CUSTOMIZATION.md` for source behavior and the required consolidated migration/accessibility checks. No database upgrade or UI has run.

Local history removal now uses revision-bound previews, cross-tab session checks, strict atomic record/head/accounting changes and removed-ID retention. Browser-controlled Blob reclamation is not represented as an assured amount of freed disk space. See `SAVE_MAINTENANCE.md`; direct emergency export, full history paging/backup and broader storage controls remain required. No migration or removal has been executed.

Live progress export now has a separate export-only stream and does not insert history records or consume its logical quota. FSA writers close before acknowledgement; browser download fallback reports only that delivery was started and keeps its retry link in the gameplay dialog. Source behavior, owner/abort bounds and remaining physical-storage/provider qualification are described in `LIVE_SAVE_EXPORT.md`. No export, picker, file stream, download or test has run.

History details now traverse the existing game/generation/time/UUID index through bounded pages with current-head IDs and totals; the UI keeps only one page in the DOM. The history index needed no change for paging; the later firmware vault upgrades the overall standalone schema to version 4. See `SAVE_HISTORY_PAGING.md`; query/interaction evidence, bulk backup and fuller slot/storage UX remain required. No page request or UI has run.

Local per-game input profiles now feed the WASM player’s physical key and standard-gamepad source mapping. The editor uses strict local preference transactions and temporary focused key capture; changing controls applies next launch and does not alter save hashes. Touch and raw/nonstandard/advanced analog paths retain their remaining product work. See `GAME_INPUT_PROFILES.md`; no transaction, input or UI has run.

The shared WASM player source now also admits an explicit persistent-file core contract, combines RAM/RTC with files and opaque/disc state with files, and restores through closed-handle guest restart plus rollback. File-only progress participates in history/autosave/lifecycle/exit/export. No core binary advertises this route yet; core-private barriers and actual toolchain/device recovery evidence remain required. See `WEB_SAVED_FILES.md`. No runtime, filesystem or test has run.

A pinned PS1 software WASM candidate source recipe now connects those existing media/firmware/disc/files protocols to an unthreaded interpreter adaptation, disabled whole-disc precache and a checked card barrier. Corresponding source is materialized; no binary is built or installed, distribution approval stays false and no core/browser/device/test has run. See `WEB_PS1.md`; all remaining scope stays required.

The shared game-detail source now adds declared per-runtime finite core choices, local profile drafts/save/reset and unavailable-profile cleanup. The standalone strict storage route shares the bounded preference schema; WASM validates and hashes effective choices before initialization. No transaction, UI/browser/core or test has run. See `GAME_CORE_OPTIONS.md`; all remaining requirements stay active.

Standalone firmware-vault source now has bounded role-sized immutable import/hash/strict publication, separate metadata/data/selection stores, checked selection/repair/deletion and game/role/expected-hash transfer admission with revision rechecks. Shared Settings and storage counters are connected. No browser, migration/transaction, picker/Blob/hash, core/UI or test has run. See `WEB_FIRMWARE_VAULT.md`; all original remaining product/release work stays active.

Standalone selected-folder disc ingestion now also has bounded FileList capabilities, CUE/M3U/encoding/reference planning, immutable copied/hash snapshots and strict whole-bundle publication using the existing media identity. Shared job/panel ownership and commit-phase cancellation behavior are explicit. No picker/parser/hash/worker/lock/transaction/browser/core or test has run. See `WEB_DISC_IMPORT.md`; all original remaining platform/runtime/storage/release requirements remain active.
