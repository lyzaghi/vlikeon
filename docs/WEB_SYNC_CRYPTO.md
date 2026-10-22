# Browser synchronization credential and wire foundation

This is authored source, not a working or qualified account-sync product. The
Web capability remains `encrypted_sync: false`; account UI, cloud opt-in and
network synchronization remain disabled. No compiler, Cargo resolver, WASM
engine, WebCrypto operation, IndexedDB transaction, worker, browser, account
service or test was run. Prepared source and resource inventories are not
cryptographic, interoperability or release evidence.

## Reuse the native protocol

`platform/web-sync` wraps the existing `omniplay-sync` Rust implementation for
the browser. It retains native OP1 account recovery, HKDF-derived root/data
keys, Ed25519 domains/certificates/requests, XChaCha20-Poly1305 save encryption,
blob headers, authenticated metadata and payload integrity checks. It introduces
no new server cipher or alternate save representation. Actual native/browser
interoperability remains a required final check.

The explicit compilation target is `wasm32-wasip1`, used only as a finite browser
ABI. Rust documents this target's [WASIp1 imports and cross-compilation
support](https://doc.rust-lang.org/rustc/platform-support/wasm32-wasip1.html).
The browser implements selected imports; it does not supply a general WASI
filesystem, process environment or network. The existing getrandom dependency
uses [WASI random_get](https://docs.rs/getrandom/0.2.17/getrandom/), supplied here
by browser cryptographic randomness with no fallback. WASIp1 is a compatibility
target, not a claim that the module is a maintained standalone WASI application.

The command enum is exact and finite:

| Command | Local result |
| --- | --- |
| Create/recover | Public account, root-authorized device certificate, delegated credential bytes; Create additionally returns one recovery string |
| Load | Verify the stored certificate signature/account/device/audience binding and admit delegated credentials |
| Sign | Native typed device request; certificate lifetime and operation bounds are verified before return |
| Root | Native typed root command after explicit matching recovery-key input |
| Renew | A new root-authorized certificate for the existing delegated device |
| Seal/open | Existing authenticated encrypted blob or validated plaintext save plus metadata |
| Clear | Drop the delegated session and owned buffers |

An expired, otherwise correctly bound certificate may be loaded for local root
renewal. It cannot authorize a signed network request: Sign repeats the native
current-time certificate and operation checks. Root authority is reconstructed
only for the explicit request and is not retained with the delegated session.

## Memory, imports and worker ownership

The module exposes a registered-buffer C ABI with no Rust raw-pointer
dereferencing. At most two caller allocations are registered, with total
logical input at most one encrypted save plus the 1 MiB control limit. Commands
consume registered pointers; arbitrary/double-consumed pointers are rejected.
Output pointers belong to the module until copied and explicitly cleared.
Binary payloads do not pass through base64/JSON. The full existing 256 MiB save
limit is preserved with the native metadata/ciphertext allowance.

Rust source uses zeroizing owned buffers for incoming commands, delegated bytes
and decrypted data. Open consumes owned ciphertext and uses the native in-place
authentication/decryption/metadata removal path. Seal currently retains the
caller allocation while native encryption builds its AEAD buffer; it does not
claim a single total allocation. Normal output clearing retires allocation
capacity rather than retaining a previous maximum-size result indefinitely.

The private module has 32 MiB initial memory, a 1 GiB maximum and a requested
2 MiB stack. Static admission rejects imported/shared/64-bit/unbounded memory,
automatic start, unknown imports and unknown exports. The engine's actual module
validation still occurs only when the worker is later run. This memory cap does
not bound total browser RSS, JavaScript snapshots or cryptographic operation
duration; mobile memory/performance qualification remains due.

The only successful ambient import is bounded `random_get`, forwarded in at
most 64 KiB browser randomness calls. Arguments/environment report empty.
Descriptor operations fail; clocks fail; proc_exit terminates the worker path.
There are no preopened directories, stdio output, sockets, dynamic modules,
child workers or service credentials inside WASM. Static admission runs in both
the future package recipe and the browser source before engine compilation.

One foreground worker processes one command at a time. The client transfers a
private binary snapshot, bounds input/output, correlates the request UUID and
enforces a deadline. Focus loss, hidden document, page hide, cancellation,
message failure or deadline terminates the client worker. After a guest trap,
the worker never calls another guest export; best-effort host memory retirement
precedes closure. Normal binary snapshots are wiped after use. Immutable
JavaScript recovery strings, structured-clone internals, terminated worker
heaps and OS memory do not provide a proven zeroization guarantee.

## Local browser credential vault

The delegated credential envelope is exactly 84 bytes: `OPK1`, 16-byte UUID,
32-byte device seed and 32-byte data key. It contains no root/recovery seed.
`WebSyncVault` wraps it with browser AES-256-GCM, a fresh 12-byte nonce and
128-bit tag. The nonextractable wrapping CryptoKey is stored as a CryptoKey in
the existing profile's IndexedDB `config` store, with no plaintext fallback.
This is origin-bound WebCrypto storage, not the operating-system credential
vault used by desktop/mobile. Trusted-origin script execution can use that key;
nonextractability does not make a compromised origin harmless.

Associated data binds the envelope format, schema, profile UUID, generation,
wrapping-key UUID, account/device/root public identities and service audience.
The envelope's device UUID must match its public account. Certificates are
independently signature/binding-checked by the Rust module on load. They can be
renewed without storing root authority.

Expensive cryptography finishes outside IndexedDB transactions. A strict
read/write transaction then compares the current generation/key identity and
publishes wrapped credentials and public account together. Every account update
rewraps against a fresh generation. Disconnect deletes the wrapping key and
retains a new disconnected generation tombstone: an old pending operation cannot
restore the former credentials through an absent-record ABA. Existing local
game/save data is untouched. The vault itself uses the existing config namespace;
the subsequent transport adds the explicit schema-7 stores and atomic cache
cleanup described in `WEB_SYNC_JOURNAL.md`.

`WebSyncCredentials` owns the worker/vault coordination for future account and
transfer callers. It checks durable generation before/after a cryptographic
operation, fences closed/local sessions, and exposes explicit provision, unlock,
sign, root, renewal, seal/open, registration update and disconnect operations.
A disconnect is independent of a pending command. Network callers must also
check the generation before each request and honor server-side device
revocation; local disconnect cannot erase already copied keys or retroactively
cancel an admitted request on another device.

WebPlatform also now connects account status and the shared asynchronous action
protocol, with staged recovery enrollment, explicit service/CSP policy and a
bounded HTTP adapter. See `WEB_SYNC_ACCOUNTS.md`. Full
`encrypted_sync_available` remains false, the shared account UI is hidden and
both actual crypto and network policies remain disabled with no service origins.

## Source production and artifact admission

`tools/build_web_sync.py --prepare-only --project out/web-sync/crypto-v1-fenced`
completed a source-only project containing 151 files / 952,080 bytes. It retains
workspace sources/manifests/lock and selected frontend/build-policy sources,
with a bounded hash inventory and an explicit preparation receipt. The earlier
`crypto-v1` snapshot is preserved. That snapshot includes the final
guest-trap fence. Neither snapshot contains a compiler output or WASM binary.
The subsequent `crypto-v2-account-transport` snapshot retains the expanded
account/network/frontend source described in `WEB_SYNC_ACCOUNTS.md`; it also
remains source-only and unverified.

`security/web-sync-toolchain.json` pins Rust 1.98.0 / compiler prefix 88d9e12ae
and the WASIp1 standard-library distribution archive hash from the installed
Rust channel manifest. That manifest reading is not independent archive/signature
qualification, and no target/SDK was installed by this work.

The separate `--build-prepared` mode is reserved for after product authoring.
It requires explicit Cargo, rustc and offline registry-cache paths, the pinned
compiler, an installed matching target standard library, unchanged prepared
source/recipe and a fresh build destination. It uses frozen/offline Cargo and
fixed memory/stack/remapping options. A future build records actual tool and
standard-library hashes, source/preparation identities and the artifact's finite
ABI, but leaves distribution/verification unapproved. It does not execute WASM.

`security/web-sync-channel.json` is disabled. Future activation requires explicit
distribution and consolidated-verification approval tied to the artifact/source
hashes. The Web assembler additionally compares the retained preparation,
corresponding-source inventory, Cargo lock and actual binary ABI/hash. Its
generated disabled configuration references no executable. A selected WASM is
optional first-use content, not an app-shell precache promise.

## Remaining full sync and final evidence

The browser account/HTTPS/CSP/CORS source now exists as described in
`WEB_SYNC_ACCOUNTS.md`; retained ciphertext journals, shared leases and inbox/
cursor publication are authored in `WEB_SYNC_JOURNAL.md`. Canonical metadata/game
mapping, authenticated local publication/inbox consumption, equivalence and
removal handling, consent epochs and foreground jobs are now authored in
`WEB_SYNC_RECONCILIATION.md`; explicit queued cloud repair is authored in
`WEB_SYNC_RECOVERY.md`. Retained old upload-stage identity/orphan recovery and
real service/device interoperability remain due.
Native/mobile source already covering these areas does not prove the Web path works.

The final consolidated pass must include the actual target build/imports/ABI,
native/browser deterministic fixtures and malformed ciphertext, recovery and
root/device separation, expired/future/wrong-audience certificates, nonce/random
failure, transferred-buffer lifetime, traps/deadlines/lifecycle, wrapping-key
serialization and denial/quota, corrupt envelopes, concurrent generations and
disconnect, immutable-string exposure, peak memory and mobile/browser behavior.
SDK/dependency/license/SBOM/reproducibility qualification and all other original
platform/runtime/streaming/security/store/release requirements remain open.
