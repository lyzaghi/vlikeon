# Native Remote Play credential lifecycle — item 58

Source authoring only. No vault, signing, native/mobile API, build or test has
been executed for this checkpoint. Item 58 remains incomplete. Existing browser
Remote Play still uses its IndexedDB/WebCrypto store; this module does not
migrate those non-extractable keys or enable a native credential capability.

## Secret material source

`platform/streaming/src/native_credentials.rs` defines a Remote Play provider
separate from encrypted-save synchronization. Desktop keyring service
`org.vlikeon.remote-play.credentials.v1` addresses one profile/device/kind/id/
generation. Identity and pair addresses have different kinds, each new candidate
gets a fresh UUID generation, and all UUIDs must be nonzero. Deserialized
addresses are data, not lifecycle authorization.

The provider's binary record is exactly 137 bytes: eight-byte version magic,
one kind byte, four UUIDs (profile, device, item, generation), a 32-byte binding
and 32 secret bytes. Loads check all address fields and the expected binding;
another generation/profile/device/kind is rejected. Records and temporary secret
buffers use zeroizing ownership. Secret identity/pair types do not implement
Serialize, Clone or Debug and expose no secret export method. This does not
claim the OS/backend makes no transient copies or that software keyring storage
is hardware-backed/non-exportable.

Signing identities use OS randomness and Ed25519. Reloading verifies that the
seed produces the pinned public half. Signing accepts only the existing four
Remote Play domain constants, with their existing body-size bounds and JSON
object parsing. This primitive still needs typed protocol and document/session
authorization in native orchestration; it is not a renderer signing endpoint.
The parser ignores JSON values without retaining a second object tree.

Pair records are bound to the complete signed grant hash and the pinned local
public key. Before load/store, the existing grant verifier checks signature,
audience and lifetime, and the local public key must be a participant. The pair
UUID must equal the address item UUID. A monotonic elapsed-time adjustment
rechecks expiry after synchronous vault work, so slow I/O cannot publish a pair
key using only the earlier timestamp. The caller must still supply native
current time, its own pinned identity and durable active-generation authority.
The raw derived-key constructor is now crate-private. The public native exchange
below produces the wrapper after signed-transcript admission; loaded vault keys
remain subject to their separate grant/journal authority admission.

Candidate storage first refuses an already present address, writes, then reads
back the exact record before returning. The keyring is not a CAS store; a
durable journal must serialize generation operations before using these calls.
A failed write/read-back/expiry recheck can leave an orphan candidate and must
retain cleanup debt. The module deliberately does not retry by inventing a new
identity or deleting an uncertain record automatically.

Desktop providers select the same pinned keyring backends already used for
save-sync (Windows native, Apple native, Linux persistent native keyring), with
a separate service/account namespace and binary secret APIs. Only NoEntry maps
to absence. Locked, denied, malformed-length and other failures remain errors;
no plaintext-file or renderer fallback exists. Android/iOS default desktop
provider calls refuse until a scoped native mobile provider is supplied.

## Durable metadata journal source

`platform/core/src/stream_credentials.rs` now authors a profile/device-bound
journal in the existing SQLite preferences table, with no schema migration or
key bytes in SQL. Each transition runs inside an immediate transaction, compares
the complete journal revision and commits a bounded replacement. Mutations
require WAL and FULL/EXTRA synchronous mode with no enclosing transaction. Read admission
checks the SQL byte length before fetching JSON, its version/bindings, unique
generations, public keys, states and active-pair identity invariant. Limits are
128 retained generations, 192 KiB and ten million revisions; exhaustion refuses
new work. Tombstones are not discarded by age or silently reused.

Reserved -> Writing is committed before any vault writer may start. The trusted
native owner may record MaterialReady only after exact vault read-back and
actual writer retirement, then Active after native opt-in and, for pairs,
authenticated relay approval. These are native metadata APIs, not implementations
of those external operations or verifiable receipts. Pair reservation verifies
the signed grant and pins its hash/expiry and the active local identity. The
same pair UUID cannot be reserved again, even after retirement. Grant expiry
is rechecked after SQL-lock waiting on reservation/activation; orchestration
must still recheck authority after commit before publishing or using a key.

Revocation commits Revoking before side effects. Revoking an identity atomically
fences all its pairs. Writing generations keep their writer debt; repeating a
revocation does not clear previous cleanup observations. An already retired
identity cannot be used to revoke a later identity generation. Separate native
observations track writer retirement, material absence, local session retirement
and relay retirement. Material absence is refused while a writer is unretired.
All four are required for Retired, and identity retirement additionally waits
for every pair tombstone to retire. No rollback to Active exists. Uncertain SQL
commit requires reloading the authoritative revision rather than repeating
side effects under an old state.

This journal has no current product caller, startup recovery worker or vault/
network orchestration. Loading metadata is not a live authority lease. Existing
in-memory key objects are not yet fenced by journal changes. A native owner must
serialize side effects, retire all old writers before declaring cleanup, keep
the grant required for relay revocation and enforce current epochs across
processes. The journal now also retains the exact signed grant and its canonical
HTTPS relay audience as non-secret native recovery metadata, in addition to the
hash/expiry. Every journal admission verifies the signature, original grant
shape, UUID, local participant, audience, hash and expiry bindings. Historical
validation uses the grant's signed creation time to retain expired revocation
metadata; this does not authorize use now. Native operations/activation still
require current time and live authority. A pair record missing that metadata is
refused for recovery, never filled from an untrusted renderer or silently reset.
Metadata consumes the existing 192-KiB bound, so large grants can exhaust the
journal before its 128-generation ceiling. No private seed, pair AES key or
invitation secret is written to the journal.

`Journal::recovery_plan` is now a read-only bounded plan for each retained
generation. Incomplete candidate states and expired active pairs first request
a durable fence; valid active records request revalidation before use, not
automatic activation. Revoking records request actual writer retirement, local
session retirement, relay revocation, material removal and finalization in that
order. An identity waits for every pair to retire before its material cleanup;
acknowledging identity material absence while pair debt remains is refused.
`finalize_retirement` can finish an identity after its earlier receipts and pair
cleanup are complete, without inventing new evidence. The caller must reload
after every transition/side effect: a plan is a snapshot, not an authority lease
or executor. Vault lock, unknown writer and failed relay requests keep debt;
elapsed time alone does not acknowledge writer or session retirement.

## One-step recovery executor source

`stream_credential_recovery.rs` now authors `advance`, which reloads the bounded
journal, checks the expected revision/address/profile/device, obtains the
current plan and executes at most one step. A candidate is durably fenced before
any cleanup. Active records return RevalidationRequired and are not silently
opened/used. Waiting on pair debt does not claim cleanup progress. There is no
retry loop, detached thread, automatic key regeneration or renderer endpoint.

The vault-removal step connects to `delete_material`, including exact-generation
removal and read-back absence, then records the observation using the original
journal revision. Writer/local/relay steps call a required `NativeRecoveryOwner`
and record only explicit Confirmed results. Pending, backend error, deadline
expiry, owner-loss or revision conflict leaves the fence/debt in place. Unknown
SQL commit requires reload and idempotent re-observation; deletion is not rolled
back or inferred successful from a failed/unknown metadata commit.

The supplied native owner must hold real cross-process exclusion against all
credential writers/recovery workers throughout each call, including vault I/O
and the final commit. Its check is repeated before effects and receipt commit.
The trait has no default implementation; a mutable Rust borrow does not prove
that exclusion. The Windows cooperative lease below now supplies the retained
OS lock in source, but writer containment, local-session ownership and
authenticated relay revocation providers are still missing. Consequently
this executor has no product/startup caller yet and has not performed an action.

An original monotonic deadline of at most 30 seconds is required and checked
between operations. The owner receives that deadline for cancellable backend
work. Synchronous vault calls and SQLite/OS calls may block beyond it; no hard
wall-clock termination is claimed. A backend result received after the deadline
is not positively published as a cleanup observation. Journal fence/finalization
commits can themselves block and may already be durable when a caller times out;
the next operation must reload rather than retry with a stale revision.

## Windows cooperative writer lease source

`stream_credential_lease.rs` now authors a `CredentialLease` required by every
journal mutation and by the one-step recovery executor. The caller must retain
the same lease across write-intent commit, vault write/read-back, publication or
cleanup receipt commit; reacquiring separately between these operations is not
equivalent. The private lease owns its file/directories, profile/device/root
binding and has no Clone, serialized constructor or handle/unlock export.

On Windows it opens a fixed zero-byte profile-root lock file with no truncate
and uses nonblocking `File::try_lock`. WouldBlock reports Busy. Reparse points,
symlinks, non-regular files, nonzero lock contents, relative/parent-traversal and
UNC-shaped roots are refused. Root ancestors are opened and retained without
FILE_SHARE_DELETE before the lock file is opened, keeping that admitted path
namespace from being renamed/replaced during the lease. The lock remains until
the owned file is dropped; release/crash does not clear journal cleanup debt.

This is cooperative exclusion for admitted native processes sharing that same
profile root. It does not prove filesystem ACL provenance, an entire machine's
absence of other writers, or that a mapped drive has qualified local locking
semantics. Copied/restored profile roots with duplicated vault addresses still
need coordinated admission/recovery; separate path lock files do not arbitrate
those copies. Raw secret-provider APIs remain internal primitives whose callers
must honor the lease contract. Provider/native owner integration and real Windows
lock/reparse/contender/crash qualification are still due. A Unix candidate is
now authored below; other unsupported platforms refuse acquisition. No plaintext
or process-local mutex fallback is supplied.

## Candidate writer orchestration source

`stream_credential_writer.rs` now connects the journal, retained lease and vault
primitives. `NativeCredentialWriter::open` acquires the scoped lease and refuses
corrupt/mismatched journal history. It owns that same lease through every
synchronous operation, including provider errors or blocking calls. There is no
detached writer whose exclusion disappears while its vault call remains live.

`prepare_identity` refuses any existing non-retired identity, generates an
in-memory candidate, commits Reserved then Writing, writes/read-backs the exact
generation and only then commits MaterialReady. It returns public key/address/
revision metadata, never a seed or signer. Missing or locked existing identities
are not replaced. `activate_identity_after_local_consent` requires the exact
MaterialReady generation/revision and reloads its pinned key from the vault
before marking Active. The native caller must actually obtain local consent;
this method does not show a dialog or supply a consent capability.

`prepare_pair` requires an active native identity and pinned vault reload,
validates the signed grant/current lifetime and local participant, and uses a
key supplied by the future authenticated native exchange. Reserved and Writing
commit before the key enters the vault; matching read-back precedes MaterialReady.
The grant is rechecked after vault work and after ready commit using original
time plus monotonic elapsed duration. Pair approval/activation is deliberately
not inferred from preparation; no relay-approved activation orchestration exists
yet. The wrapper accepts no browser-extracted key or renderer secret endpoint.

These operations require original monotonic deadlines of at most 30 seconds.
Late/error results retain their last authoritative Reserved/Writing/MaterialReady
state; no automatic deletion, rollback, retry with a fresh identity or false
activation is performed. A SQL commit can be durable even when a caller receives
an error/timeout, including identity activation. Reload before deciding the next
action. OS/SQLite/provider calls may block beyond a deadline and still need
runtime supervision/qualification. Current APIs are native-only source with no
broker/startup/UI/mobile caller; no key, vault or SQL operation was executed.

## Native signed-transcript exchange source

`native_pair_exchange.rs` now authors a P-256 ephemeral exchange using pinned
`ring` 0.17.14, already present in the lockfile. Each instance generates its own
public point and 32-byte nonce, pins the local Ed25519 public identity and
expires after five minutes of original monotonic time. It exposes public
exchange/nonce metadata, has no Clone/serialization/private-key export, and
consumes its private scalar on finish even if validation fails.

Host and client finish methods require the expected peer public key from the
native consent/pairing controller. Invite, hello and grant signatures are
verified under their respective pinned endpoint keys. Their IDs, audiences,
participant identities, hello-document hash, ECDH points and nonces must match
the signed transcript; the local point/nonce must equal this exact exchange.
The selected peer point must be uncompressed 65-byte P-256, different from the
local point; `ring` performs curve/point validation during agreement. Grant/hello
times are checked against the original invitation, and invite/grant validity
is rechecked after derivation using elapsed native time. No peer key is accepted
merely because a received grant claims it.

The KDF follows the existing browser source: 256-bit ECDH shared material,
HKDF-SHA256 with salt SHA256 of the exact `{payload,signature}` signed-document
JSON and info `omniplay.streaming.signal-key.v1`, yielding 32 bytes. It does not
substitute parsed/reserialized grant payloads for that document. Shared material
is passed directly from the agreement callback into HKDF; the owned output is
zeroizing. No claim is made that backend internals make no transient copies.

This is source compatibility, not evidence of matching browser/native output.
It does not show comparison-code consent, prove the relay invitation secret,
implement invitation/hello HTTP requests or confirm relay approval. Those
controller steps and native encrypted signaling remain unfinished. Native
candidate storage can accept the resulting opaque key; product orchestration,
mutual WebCrypto/native vectors, wrong-peer/transcript/nonce/point/time cases,
single-use rejection and platform qualification are still required.

API review follows the pinned package's
[agreement contract](https://docs.rs/ring/0.17.14/ring/agreement/fn.agree_ephemeral.html),
[P-256 exchange API](https://docs.rs/ring/0.17.14/ring/agreement/index.html)
and [HKDF API](https://docs.rs/ring/0.17.14/ring/hkdf/index.html).
No RNG, agreement, HKDF, signing, vault or native API was executed here.

## Native signal AEAD and replay metadata source

`native_signal.rs` now authors AES-256-GCM with 12-byte OS-random nonces and
16-byte tags, using the existing 48-KiB ciphertext bound. The AAD is the same
JSON array/order as browser `signalAad`: domain, pair UUID, sender public key,
message UUID, session UUID, sequence and expiry. Values require canonical UUID
serialization and JS-safe bounded integers. The cipher pins the signed grant's
local/peer identities and expiry; it has no raw-key export. Send metadata must
already have a durable sequence reservation. Send lifetime is at most 60 seconds
and bounded by the pair certificate, with a post-crypto expiry check. Random
nonce collision resistance is inherited from the RNG design, not a proof of
counter-based uniqueness.

Receive accepts only the pinned peer, validates lengths/metadata and AES-GCM
authentication, parses a bounded JSON object and returns zeroizing plaintext.
The existing browser receive tolerance of 30 seconds past packet expiry and
90 seconds future is retained, while the pair certificate itself must be live.
`open_uncommitted` explicitly does not acknowledge replay acceptance or authorize
delivery; its caller must still check the journal/current document and commit
acceptance before dispatch. The cipher object must be retired on revocation;
its own certificate check is not a live authority lease.

The native journal now retains per-generation send/receive sequences, relay
cursor and the last 256 accepted message UUIDs. `reserve_signal_sequence`
requires the active pair and writer lease, checks original expiry after SQL
waiting and durably burns the next number before encryption/send. Ambiguous or
failed sends cannot roll that number back; transport retries must reuse the
same already sealed packet. One million sends requires explicit re-pairing.

`accept_signal` is native-only and requires prior authentication of the exact
packet. It rechecks the active generation/expiry, pinned peer, increasing cursor
and sequence, bounded message/session IDs/time and duplicate-message history,
then commits them in the same revision-checked journal transaction. Delivery
must wait for commit and recheck live authority. Required counter fields are
not defaulted to zero if absent; older/incomplete journal records are refused
for reviewed recovery instead of silently resetting replay history. This is
not protection against restoring an entire old profile snapshot; backup/restore
anti-rollback and copied-profile admission remain required.

The native operation pipeline below now connects AEAD and journal sequencing in
source. Relay HTTP, product delivery, native authority providers, revocation
wiring and actual WebCrypto/native vectors/replay/crash cases remain unfinished.
No encryption, decryption, database, vault, network, build or test operation was
run for these checkpoints.

## Native per-operation signal pipeline source

`stream_signal_operations.rs` now connects scoped credential exclusion, active
generation admission, vault key loading, AEAD and durable sequence acceptance.
Each call owns one lease through all SQL/vault/crypto work; key/cipher temporaries
are dropped before that lease. An original operation deadline of at most 30
seconds and native wall time plus monotonic elapsed checks apply. OS/provider/
SQL calls can still block beyond a deadline; there is no hard-preemption claim.

`prepare_send` admits the exact active generation/revision and session authority,
commits the next send sequence before key loading, reloads the exact signed-grant-
bound pair key and seals a bounded packet. It checks authority around slow work
and pair/packet expiry before returning immutable outgoing metadata/ciphertext.
The returned relay Send operation uses that same packet for retries. Errors or
late outcomes after reservation burn the number; the caller reloads the revision.
No send is performed and no new sequence is invented for a network retry here.

`accept_received` requires the locally expected session, rejects already accepted
cursor/sequence before vault work, decrypts/authenticates under the pinned peer,
then checks authority before committing replay acceptance. It rechecks current
authority and pair/packet expiry after the commit before returning zeroizing
JSON bytes. Failed/uncertain commits never deliver bytes. A successful commit
followed by failed authority admission may consume a message without delivering
it; the transport needs explicit reconnect/session recovery, not replay rollback.

`NativeSignalAuthority` is required and has no default implementation. Its
native observation binds exact credential generation, session and original epoch
and must be authorized, nonfuture and at most 100 ms old at admission. The real
provider must observe the live native document/session/pair and installed relay
policy. A journal Active bit, loaded key or fabricated timestamp is not such a
provider. The lifecycle registry below now supplies a concrete source observer,
but the real host document/session hooks and consent controller are not wired.
Epoch changes reject in-flight results.

These outputs are not indefinite capabilities: the relay owner must recheck
authority/deadline before every network attempt, and queued/delayed receive
delivery must recheck before dispatch. Per-call exclusion is not a long-lived
receiver ownership lease or a cross-profile/profile-clone arbiter. Paired HTTP
signing/send source is now connected below; receiver lifecycle, real native authority, session dispatch, startup/mobile
integration and runtime verification remain due. Neither operation has a current
product caller or has been executed.

## Native paired HTTPS source

`native_relay.rs` now authors a reqwest 0.12.28 blocking client for Send, Receive,
IceCredentials and RevokePair. Invitation/approval operations are explicitly
unsupported by this client. Its at-most-eight canonical HTTPS origins must come
from admitted installed/native policy; no invitation/renderer/environment URL
defines the allowlist. Requests use only `/v1/relay`, with standard TLS validation,
no redirect, ambient proxy, cookies or caller headers. Connect timeout is five
seconds, total request timeout is the lesser of original remaining time and
15 seconds, and idle connections per host are bounded to two.

The supplied native signing identity must participate in the verified grant;
normal send/receive/ICE additionally require it to be live. Retirement-only
historical validation below cannot authorize normal operations.
The existing RelayRequest body is signed in its original domain, then re-admitted
through `verify_request` before HTTP. Each attempt has a fresh request nonce;
there is no hidden transport retry. Request envelopes are bounded to 192 KiB,
response bodies to 1 MiB including chunked replies, with JSON-object and content-
type admission and no content-encoding fallback. Error text/URLs/body are not
included in returned diagnostics. The request operations do not contain a
pairing invitation secret; native invite/join controller integration is still due.

Cancellation/deadline checks occur before/after HTTP, between bounded response
reads and before success. A blocking DNS/connect/read/backend operation cannot
be synchronously interrupted merely by the AtomicBool; request timeout and the
future native supervisor must cover that interval. A cancelled, late, failed or
expired ordinary grant result is never positive operation acknowledgment, even
if the remote side effect happened. Original state must be reconciled explicitly.
This code must run on its native blocking-I/O lane, not inside an async/UI thread.

`stream_signal_operations::send_prepared` now connects the original prepared
packet to this transport. It reacquires the profile lease, verifies current
active generation/revision, signed-grant hash, sender, audience and previously
reserved sequence, reloads the pinned active identity from the vault, and checks
native session authority before HTTP. The same lease is retained through the
request and result checks. Only a bounded exact `{accepted:true,cursor}` response
plus post-network authority/certificate/packet/cancellation checks returns a send
receipt. A send receipt is not durable session delivery; no ack/outbox journal
or receive-poll dispatcher is introduced here. Retry reuses the exact prepared
message ID/sequence/expiry/IV/ciphertext while signing a fresh HTTP envelope.

No client was constructed, request signed or network contacted. Real native
authority, approved-pair activation, receive/ICE/revocation controllers,
background/cancellation supervision, product/startup/mobile callers and all
HTTP/crypto/replay interoperability/platform qualification remain unfinished.
This is not item-59 relay/TURN deployment and does not enable streaming policy.

## Bounded receive polling and ordered acceptance source

`poll_received` now connects the native active pair and pinned identity vault
reload to a signed HTTPS Receive request, starting from the durable journal
cursor and requesting at most eight packets. It retains the lease through HTTP,
rejects unknown response fields and oversized batches, and admits each packet's
pinned peer, increasing cursor/sequence, nonzero message/session IDs, bounded
time/IV/ciphertext and within-batch message uniqueness. Post-network native
authority, pair expiry and cancellation are rechecked. This poll does not decrypt,
advance a cursor or accept any remote session as locally authorized.

The returned `PolledSignals` is an encrypted batch bound to the original
credential generation, epoch, journal revision and operation deadline. It has
no skip/advance setter. `accept_next` requires the locally admitted session to
match its head packet and calls the connected authentication -> current-authority
-> journal acceptance -> post-commit authority pipeline. Only successful return
advances its private batch position/revision. Later packets cannot skip a failed
or foreign-session head. Cancellation before delivery refuses publication; if
acceptance already committed, reload/re-poll rather than rolling replay state
back. A deadline cannot be extended by supplying a new one to the batch.

The original per-call lease is released after poll; each accepted head reacquires
it and checks the exact revision, so concurrent native journal work can invalidate
the batch. That conflict needs reload/re-poll, not blind delivery. This is not a
long-lived receiver ownership lease, a reconnect loop or a native session router.
Unknown/closed-session recovery and deliberate discard policy remain unfinished;
there is no automatic cursor jump to hide them. ICE/revocation, native authority,
approval/product/startup/mobile integration and all actual HTTP/crypto/replay/
cancel/platform verification are still required. No poll or acceptance was run.

## Bound relay retirement source

The shared `verify_grant_for_retirement` now verifies canonical audience,
signature, grant shape/IDs and nonfuture creation while retaining expired
metadata. It is used only for cleanup. Live grant verification remains required
for send, receive, ICE, approval and activation. The native paired client permits
historical validation only for RevokePair; it cannot use an old grant for other
operations and does not report ordinary grant-expiry failures as success.

The signaling store's RevokePair transaction verifies that the signed request
sender is one of the exact grant's participants. A matching stored grant is
marked revoked and its messages deleted. Repeating that same revocation can
return the same bound result. If the row is absent, only an expired exact grant
can receive an `expired` retirement result: expired rows were collected inside
the same transaction, and normal approval refuses that expired grant. An absent
live grant returns NotFound. An existing pair UUID bound to a different grant
hash returns Conflict and is not modified.

The response now contains `retirement_protocol:1`, `revoked:true`, exact pair
UUID/grant hash and reason `revoked` or `expired`. It remains an authenticated
HTTPS response from the configured relay, not a cryptographically signed
server receipt. Existing browser code that only checks request success can
ignore the added fields; native cleanup requires the exact bound response.

`stream_credential_recovery::retire_pair_remote` connects that response to the
durable relay-retired observation. It requires the current journal revision,
pair Revoking state and previously confirmed writer/local retirement under the
same retained lease. The matching identity must be Active or Revoking, with its
writer retired and vault material still retained. It reloads the pinned identity
only for the exact revoke request, checks the response pair/hash/protocol/reason,
then records cleanup with the original revision after cancellation/deadline
checks. A revoking identity is not reopened for media or normal signaling.

An unbound legacy response, 404, unknown network/SQL result or local elapsed
time does not acknowledge relay retirement. Lost/late responses retain debt and
can retry the same grant. Old relay deployments need this response protocol
before native cleanup can confirm it; no automatic fallback is supplied. The
actual native startup/controller still needs to dispatch this concrete branch
alongside writer/local retirement and vault removal. No relay was deployed,
request made, transaction executed, key loaded, build or test run.

## Required durable ordering and integration

The journal authors the state transitions below. Their external effects and
product/recovery integration remain required work:

1. Persist a reserved generation and expected public/grant binding in a durable
   profile/device journal before writing material. Bound pair count and serialize
   profile/device writers; record candidate and cleanup debt through crash.
2. Publish active state only after matching vault read-back and successful
   authenticated pairing/relay acceptance. Retain approval uncertainty rather
   than claiming a pair active after an ambiguous network reply.
3. Before each native operation, consult the original generation/state and
   current document/pair/session authority. Loaded key objects alone cannot
   authorize use after revocation; close all in-flight producers and leases.
4. Commit a permanent local revocation fence before removing key material, keep
   a bounded relay-revocation tombstone, and require actual local retirement and
   remote acknowledgment/expiry before clearing the corresponding debt.
5. Reconcile reserved/orphan/cleanup generations after restart without reviving
   deleted or revoked credentials. Keyring deletion and SQL commits are not one
   atomic transaction; handle both crash windows explicitly.

`delete_material` removes only the exact addressed generation, then checks
absence. Its success is a material-cleanup observation, not confirmation of
relay revocation, active session retirement or absence of concurrent writers.
The caller must fence and retire writers first. Failed cleanup keeps debt.

## Bound native ICE admission source

`native_ice.rs` now admits the current relay's temporary TURN credentials.
Signaling responses add `ice_protocol:1`, exact pair UUID and signed-grant hash
alongside existing ICE fields. The native admission requires those bindings,
the local participant, at most eight server entries/32 total distinct URLs,
canonical configured URL allowlist, live pair lifetime and at most ten minutes
remaining credential lifetime. Older unbound responses are refused by this path.
Existing browser consumers can ignore the added response fields.

The current TURN REST format is checked explicitly: username must equal the
integral-second expiry plus this native identity's SHA-256 fingerprint, password
type must be `password`, and credential must be canonical base64 of a 20-byte
HMAC-SHA1 output. This validates the configured service's shape/bindings, not the
HMAC itself: the client does not possess the TURN shared secret. Effective native
expiry is rounded down to username seconds, never extended by that conversion.
Secrets have no Serialize/Clone/Debug export and owned username/password buffers
are zeroizing; native ICE adapters may read them solely to configure ICE.

The shared TURN URI admission deliberately supports a bounded canonical subset
of [RFC 7065](https://www.rfc-editor.org/rfc/rfc7065.html): `turn`/`turns`, lower-case
DNS labels or canonical IPv4/bracketed IPv6, optional nonzero u16 port, and only
`transport=udp`/`transport=tcp`. Userinfo, paths, fragments, percent escapes,
trailing/empty labels, arbitrary transport extensions and noncanonical ports/IPs
are refused. It does not feed these opaque URIs into a hierarchical URL parser.
The service's startup configuration now uses the same validator. Backend support
for TLS/DTLS/transports still needs actual platform qualification; syntax alone
does not prove a TURN allocation will work. See also
[RFC 7350](https://www.rfc-editor.org/rfc/rfc7350.html) for secure UDP semantics.

`stream_signal_operations::request_ice` connects the exact active pair/revision,
retained profile lease, pinned identity vault reload and signed HTTPS ICE request.
TURN URL policy and relay-only preference come from the trusted native caller;
the relay cannot broaden that URL allowlist. Native authority, certificate,
credential expiry, cancellation and deadline are checked after network/admission.
Relay unavailable with a relay-only requirement is refused, never downgraded to
direct networking. A correctly bound response with no servers can describe an
explicit direct-capable configuration when local policy permits it.

Returned configuration is bound to credential generation/session/epoch and
requires a live timestamp on native adapter access. The adapter must also
observe live authority, refresh/restart ICE before expiry and retire existing
allocations on revocation. Already-issued TURN allocations are not erased merely
by deleting pairing material or receiving relay retirement; real backend stop
and TURN lifetime/credential rotation policy remain required.

No ICE adapter, allocation, DNS lookup, client, network request, signing/vault/SQL,
build or test was executed. Native authority/controller, WebRTC configuration,
background/refresh/stop behavior, mobile integration, actual URI/credential fault
qualification and item-59 TURN deployment remain unfinished. Streaming stays
disabled and original 51–109 completion scope remains open.

## Native document/session authority source

`stream_signal_authority.rs` now implements `NativeSignalAuthority` using a
bounded native registry, document lifetime guard, revocable session permit and
current journal ownership. Guards have no deserializer or Clone; close/drop
permanently flips their atomic liveness fence. Registry entries retain only weak
references. Native host code must create the document guard after actual
document/profile admission and close it on the real document event; the guard
constructor itself is not an origin attestation or a native consent dialog.

Registration admits only the fixed installed streaming policy and its canonical
relay origins, exact active credential generation/profile/device and original
pair certificate. Installed policy remains disabled, so registration currently
refuses instead of enabling streaming. The signaling gate does not grant capture,
host or remote input permission. At most 16 in-memory sessions are retained.

Under the credential file lease, registration now durably reserves one native
session owner per pair. Epoch is that committed journal revision, so subsequent
registration consumes a later durable revision rather than resetting an in-memory
counter. Session/epoch fields are required and validated; missing fields do not
silently create an owner. All send/receive/poll/ICE operation paths check those
exact persistent session/epoch fields in addition to their native observation.
The maximum journal revision still bounds the epoch; profile rollback/clone
admission remains an outstanding separate requirement.

Observation checks actual guard liveness, monotonic session deadline, installed
relay policy and freshly loaded Active generation/current session/epoch/certificate.
It keeps the original timestamp from before SQL/policy work; operation admission
rejects a slow observation older than 100 ms. Closing a document/permit while
HTTP is running causes post-network observation to refuse publication. These
observations do not promise atomicity with a later OS/network effect.

`release` closes the permit before attempting the nonblocking credential lease.
Acquiring that lease excludes participating per-operation vault/HTTP/commit lanes;
only a matching durable session/epoch can be cleared. Busy/error/unknown commit
keeps debt and the permit stays closed. Releasing ownership does not itself
acknowledge input/capture/ICE allocation stop; real backend retirement remains
required. Existing journal LocalRetired cleanup also clears the session binding
only after its trusted native owner has confirmed retirement.

Expired/closed in-memory entries can be pruned, but pruning/drop/time never clears
their journal owner. Crash or a late reservation result leaves ownership for
reviewed native release/recovery, not automatic takeover or epoch reuse. There
is no current native startup/controller hookup, origin admission callback,
consent UI, backend-stop integration or mobile provider for this registry. No
registration, guard event, SQL/OS/network operation, build or test was executed.

## Dedicated mobile provider ABI and iOS candidate source

`platform/mobile/src/pairing_credentials.rs` and the C header now define a
dedicated Remote Play credential table, ABI 1, separate from save-sync ABI 2.
Its address is four fixed 16-byte UUID arrays plus a u32 kind (68 bytes), and
records are exactly 137 bytes. Rust binds the consumed native context to the
actual profile/device, serializes synchronous callbacks, validates address and
record envelope and checks native access before/after each operation. Context
ownership is consumed on rejection and released after the final owner drops.
Callbacks must remain valid across native worker threads, not retain pointers,
overrun buffers or reenter Rust; the raw table is a trusted native ABI, not an
untrusted renderer/IPC surface.

Only an explicit read -1 maps to absence; -2 stays locked, other failures and
wrong lengths/envelopes remain errors. Locked/error responses do not initialize
replacement keys. Write/remove success is still subject to post-access checks
and the existing native read-back/lifecycle journal. Header/UUID/kind admission
does not prove the secret's public/grant binding; the typed load path still
performs that verification. No secret bytes are serialized in the C address.

`apps/ios/OmniPlay/PairingCredentials.swift` now authors a separate Keychain
provider for `org.vlikeon.remote-play.credentials.v1`, with account names matching
profile/device/kind/item/generation. It checks each nonzero UUID/scope, expected
record magic/prefix and fixed length. Read admission requires matching returned
service/account and accessibility. Entries use non-synchronizing generic-password
items, noninteractive calls and `WhenUnlockedThisDeviceOnly`. Writes add a new
generation only; duplicates are errors rather than updates. Removal targets one
exact account. Temporary Data mutation is best effort and does not claim all
Swift/OS copies are guaranteed zeroized or that generic-password keys are Secure
Enclave non-exportable objects.

The iOS protected-data flag starts false and is checked before/after Keychain
calls. Actual UIApplication protected-data events must be wired by the native
host; this new class is not registered with those events, added to project target
membership or consumed by a mobile open/attach entry point yet. Its callback
factory must only be invoked immediately for a real consuming native call to
avoid leaking its retained context. Android/JNI provider integration and a
mobile credential file-lease implementation also remain due; the existing core
lease still refuses non-Windows acquisition.

Apple's documented
[WhenUnlockedThisDeviceOnly](https://developer.apple.com/documentation/security/ksecattraccessiblewhenunlockedthisdeviceonly)
and [accessibility behavior](https://developer.apple.com/documentation/security/restricting-keychain-item-accessibility)
inform this candidate. Device-only data is not migrated to another device;
restore/lost-key handling needs explicit recovery or re-pairing and must not
silently replace a peer-pinned identity. No C callback, Swift/Keychain API,
mobile open, vault/SQL/native operation, build or test was executed. Native
provider/product lifecycle and device qualification remain incomplete.

## Android Keystore provider candidate source

`apps/android/java/org/chromium/chrome/browser/omniplay/PairingCredentials.java`
now authors a Remote Play provider separate from save synchronization. Its
native-only Java address encoding is 68 bytes: four UUID byte arrays followed
by a big-endian u32 kind. JNI must encode that integer explicitly, not memcpy
the native C struct's platform-endian bytes. Scope checks require exact native
profile/device, nonzero UUIDs, valid kind and identity item == device UUID.

Each address hashes to a distinct Keystore alias under
`org.vlikeon.remote-play.wrap.v1` and a ciphertext file inside the app's
credential-protected no-backup `VlikeonRemotePlayCredentialsV1` directory. A
device-protected storage context is refused. AES-256-GCM wraps the 137-byte
secret envelope with a randomized 12-byte IV and 16-byte tag; AAD binds its
domain, complete address and wrapping version. The file is exactly 173 bytes.
Canonical/nonlinked directory/regular-file checks, restrictive permissions,
bounded no-follow I/O and an exact-generation nonblocking file lock are authored.
The shared app namespace still requires filesystem/contender qualification; this
is not a sandbox against compromised same-UID native code or profile rollback.

Reads report absence only when both alias and record are absent. Missing-key
ciphertext, orphan alias, malformed envelope, authentication failure or storage
error remain failures rather than regenerating keys. Writes require a fresh
generation with neither existing file nor alias, create the wrapping alias,
write a new file without replacement, force/read-back the exact ciphertext and
sync its parent before success. Crashes may leave alias-only or partial-file
debt; no automatic rollback/delete or overwrite hides it. The native journal
must record intent first and keep debt on any uncertain result.

Removal deletes only the addressed alias first, then its ciphertext, verifies
both absent and syncs the directory. The zero-byte lock file stays named to
avoid splitting exclusion across concurrent open handles. This is material
cleanup, not peer revocation or native session retirement. Existing journal
limits/producer retirement are still required; these Java locks alone are not
the core mobile profile-wide credential lease.

User/profile unlock and keyguard checks run before/after access. Android 15+
generation sets `setUnlockedDeviceRequired(true)`; earlier supported versions
use explicit app checks because Android documents unlock/key-loss issues on
older releases. No hardware-backed or immediate hard-cancellation guarantee is
claimed. Plaintext/temporary byte arrays are cleared best effort; failed reads
clear their output as well. Java/OS/Keystore internal copies are outside that
guarantee. Error messages contain no secret, alias or path details.

ProGuard retains the four native callback methods. The Android source assembly
recipe already includes Java sources dynamically, but has not been run. JNI
callback ownership, native provider attachment, true device/profile lookup,
mobile credential lease and lifecycle/backup/reinstall recovery remain missing.
No Java constructor, filesystem/Keystore/crypto API, JNI call, build or test was
executed. This source does not enable a Remote Play capability.

The Android
[unlocked-device-required API documentation](https://developer.android.com/reference/android/security/keystore/KeyGenParameterSpec.Builder#setUnlockedDeviceRequired(boolean))
informs this candidate; real lock/unlock/invalidation/crash/fault/restore and
concurrent process qualification remain due.

## Native mobile attachment and access lifetime source

The C ABI now has `om_mobile_pairing_scope` and a dedicated
`om_mobile_attach_pairing_credentials_v1`. Scope is read from the already-open
native host's actual profile/device. Attachment consumes its retained callback
context on every return, including stale/closing host, invalid table and duplicate
attachment. It rechecks host identity after constructing the provider, admits
only one provider per host and releases rejected/final contexts outside the
global host-registry lock. No vault/secret callback is invoked by attachment.
The response describes attachment only, not pairing approval or a streaming
capability. No renderer command or secret-returning endpoint is added.

Each provider starts unavailable. Native `om_mobile_set_foreground` now updates
the attached provider access gate alongside existing sync lifecycle notification.
Closing the native host permanently closes that gate before host teardown; a
late foreground event cannot reopen it. Every availability transition increments
an in-memory epoch without wrapping. The before/after callback checks require
the exact same epoch, so background -> foreground while a vault call was in
flight cannot make that old result publishable again. Counter exhaustion closes
the provider. Post-transition write/remove results may already have changed the
OS vault; their journal debt must still be reconciled, not retried as absent.

Foreground access is not OS unlock, native user consent, document admission or
active pair/session authority. Swift protected-data hooks and Android Keystore
checks remain separate. Native operations must retain the provider Arc while
callbacks are in flight; host close fences results but does not forcibly abort
an OS call or release a still-owned callback context. Existing close collection
drops retired native contexts outside the global registry lock.

The app-side Swift/JNI factories do not yet call these new attachment functions,
and no pairing operation/controller in LocalHost consumes the attached store.
No attachment, scope query, foreground/close callback, vault/native operation,
build or test was executed. Mobile core lease, startup recovery, app-side glue
and actual lifecycle/race/ABI/device qualification remain required.

## Android native attachment caller source

`NativeCore.java` now calls dedicated native scope/attach symbols after its
profile host opens, on the existing serialized I/O lane. It admits the scope
reply size/status, exact canonical profile and nonzero canonical device UUID,
constructs the separate pairing provider, then checks attachment ABI/status.
Temporary scope/attachment response arrays are wiped. The local attached flag
does not advertise Remote Play or authorize any pair/session. Attachment failure
leaves that feature unavailable and does not fail local play or fall back to
save-sync credentials. No automatic same-context retry is introduced after an
ambiguous result; native close/recovery must reconcile retained attachment.

`mobile_jni.cc` now adapts the dedicated table to its Java provider methods.
It encodes four UUID arrays plus big-endian u32 kind explicitly, with a C ABI
size assertion; it never copies the C struct's native integer byte order into
Java. Read/write buffers are exactly 137 bytes, accepted read counts are exact
or known negative statuses, callback exceptions become failures, and temporary
Java secret arrays are wiped on every path. Local references use synchronous
JNI frames; a retained global provider reference is transferred to Rust only
after method resolution succeeds. Rust consumes it on every attachment return,
and the common native release callback drops it when its final owner retires.
JVM shutdown/attach failure still requires platform qualification; no guarantee
is made that a VM already unavailable can execute cleanup callbacks.

ProGuard now retains both native method signatures. These are private native
entry points, not JavaScript interfaces. Actual pairing/key operations are not
called by attach, and LocalHost still has no pairing controller that consumes
the attached store. The existing native foreground/close bridge gates access
once attached. Actual JNI/Keystore/classloader/exception/release/build/device
qualification, mobile core lease and iOS attachment caller remain due. No Java
startup, JNI/native callback, vault, crypto, build or test was executed.

## iOS attachment and protected-data caller source

`NativeCore.swift` now attempts the dedicated pairing attachment after native
host open, on its serialized queue. It reads profile/device scope from Rust,
requires the exact canonical profile and nonzero canonical device UUID, and
constructs the retained Keychain callback table immediately inside its consuming
native call. Attachment result must match ABI 1. Failure leaves the local
attached flag false without failing local play, substituting sync credentials,
reading keys or advertising streaming. A response parsing failure can occur
after native attachment already succeeded; retry is not automatic and close/
recovery must reconcile that retained context.

`BackgroundSync.swift` now initializes the separate pairing protected-data flag
from UIApplication and forwards protected-data unavailable/available events.
The pairing provider increments a nonwrapping access epoch on every event and
checks the same epoch across actual Keychain calls. A lock/unlock cycle during
read/write/remove cannot make the old result valid again. Failed late reads
clear output; uncertain writes/deletes retain native lifecycle debt. This flag
does not replace foreground, native consent, session authority or journal gates.

`PairingCredentials.swift` is now referenced by the checked-in Xcode source
group/build phase with the same deterministic identifiers as the existing source
assembly recipe. Only its source-membership changes belong to this checkpoint;
other scheme/build-setting work is preserved. No Xcode generation, compilation,
UIKit notification, host open, callback, Keychain or test was executed. Actual
device lock/unlock/background/close/ABI behavior, core mobile lease and LocalHost
pairing-controller consumption remain missing. Streaming policy stays disabled.

## Unix/mobile credential lease source

`CredentialLease` now also authors a Unix branch used by Android/iOS/macOS/Linux
targets, retaining the same required lease type for journal/writer/recovery/
signal operations. It requires an admitted absolute canonical profile root,
nonzero native profile/device and bounded path depth. The root directory opens
without following its final symlink and must be private (no group/other mode
bits). A fixed zero-byte lock file is created with create-new or opened existing-
only if creation reports AlreadyExists. Existing names are checked before open;
no-follow/close-on-exec/nonblocking flags are retained and no truncate/unlink or
create fallback after disappearance is used. Only single-link regular zero-byte
files owned by the profile-directory owner and with private mode are admitted.

Nonblocking `File::try_lock` reports Busy for a live contender. The held file
and root-directory descriptors remain inside the noncloneable lease. Each
admission compares held/named device, inode, owner, file kind, length, link count
and private mode, and rechecks the canonical root. Path replacement or unlink
detected at these checks refuses further journal publication. Root permissions
are admitted, not automatically weakened/repaired by this lease.

Unlike Windows deny-delete handles, Unix open descriptors do not prohibit
rename/unlink. The protocol is cooperative inside the admitted app-private
namespace; it does not claim atomic exclusion against hostile same-UID or
privileged namespace mutation between checks. Copied profile/vault addresses,
network filesystem locking, sandbox/provider permissions and root provenance
still need their separate admission/qualification. No existing owner is reclaimed
by age and dropping a lease never removes durable session/cleanup debt.

Android's trusted host now resolves its OS storage aliases before passing the
profile root to Rust. iOS resolves/standardizes its native container URL after
directory creation and passes that path to open. Renderer paths cannot opt into
this normalization. This avoids treating normal Android/iOS storage aliases as
an admitted final symlink or opening split lock namespaces.

The implementation follows the documented
[Rust file-lock contract](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock)
available since the project's Rust 1.89 minimum. No Unix file, lock, path, native
host, vault, SQL, build or test was executed. Actual Android/iOS filesystem,
contender/crash/replacement/permissions tests, LocalHost pairing-controller
consumption and native runtime qualification remain unfinished.

## Native LocalHost preparation entry points

LocalHost now provides native-only status and identity-preparation methods. They
are absent from the renderer Command enum. Status returns bounded journal
metadata, revision and installed-policy availability; it does not return private
key material, signed grants or a signing capability.

The mobile C ABI snapshots the actual open foreground host and its attached,
profile/device-scoped provider, then releases the global registry lock before
waiting for the host mutex or doing journal/vault work. Identity preparation
requires an expected journal revision and an original 1–25 second budget,
including time waiting for the host mutex. It uses the existing retained-lease
writer and returns only public MaterialReady candidate metadata. There is no
activation, pairing approval, implicit retry or replacement of an existing key.

Provider transition epochs are checked before work and again before publication,
including a native access observation for the prepared address. A background,
close or observed protected-data failure rejects the result. These observations
are not an atomic guarantee against a subsequent OS state change. A synchronous
vault call is not forcibly interrupted at the deadline. An error may follow
durable journal/vault effects, so the native controller must reload status and
perform reviewed recovery instead of assuming that nothing happened.

The installed streaming policy still refuses preparation before RNG/vault
side effects. No Swift/JNI UI caller, consent flow, activation action, recovery
dispatcher or transport controller is connected to these new entry points yet.
No endpoint, native callback, vault, SQL, build or test was executed. Only source
review and whitespace inspection were performed; item 58 remains incomplete.

## Native identity review and confirmation binding

LocalHost now retains at most one pending identity review, bound to the exact
MaterialReady generation, current journal revision and mobile provider's
foreground epoch. Replacement, detach and close clear the earlier review. The native ABI
returns a fresh opaque review ID, exact public key/address and the local-only
activation action for a trusted native dialog. Review lifetime is at most 60
seconds on the monotonic clock, including journal read time.

The trusted native UI must show that identity/action and call confirmation only
after explicit approval. A token does not prove a dialog was shown. These entry
points are absent from renderer dispatch. Confirmation accepts only a bounded
review-ID object and a 1–25 second original operation budget; it never accepts a
caller-selected replacement address or public key. LocalHost consumes its pending
review before validation/vault work, checks the captured lifecycle epoch and
original revision, then uses the retained-lease writer to reload the pinned key
before activation. The review deadline also bounds confirmation. Cancellation
clears only the transient review; it does not revoke or delete durable material.

Foreground transitions invalidate the review's epoch even after resume. Native
vault checks still enforce protected-data access during activation; this review
is not a separate attestation of every OS lock/unlock transition. Lifecycle or
deadline failure after a committed activation can suppress the response without
rolling back the journal. Reload status before recovery; do not automatically
retry, infer absence or silently create another identity. Local activation does
not approve any remote peer, grant, signaling session or transport capability.

The installed policy remains disabled. Actual Swift/JNI dialog presentation,
native caller integration and startup/revocation recovery remain unfinished.
No dialog, ABI endpoint, native callback, vault, SQL, build or test was executed;
source review and whitespace checks are the only verification of this change.

## iOS native identity dialog caller

GameHub now authors a native identity entry button above WebKit content, hidden
while a game uses fullscreen. Its Swift caller reads native status first and
reports the installed disabled policy without trying to create a key. Once an
eligible installed policy exists, an explicit button action may prepare a new
identity only if there is no nonretired identity; an existing MaterialReady
identity can be reviewed. Active or uncertain states require separate recovery
and are never silently replaced.

The native alert displays the exact device, generation and public key, explains
the local-only activation action, and has separate cancel/approve actions. Only
the approve handler calls confirmation. Swift checks protocol, revision, scope,
action, key shape and bounded expiry before display, then checks the returned
activation generation. Calls remain on the serialized native queue; review
metadata/tokens are never returned through HubBridge or JavaScript.

A separate thread-safe flow lifetime suppresses stale callbacks. Resigning
active, protected-data loss, suspension, view departure, fullscreen, load failure,
cancel or monotonic expiry invalidates the flow and schedules native review
cancellation. UI cancellation does not claim to preempt an already-running vault
call or roll back committed journal changes. Confirmation errors require a fresh
status read; there is no automatic retry. Rust revision/epoch/deadline and pinned
vault admission remain authoritative.

This is authored iOS caller/UI source, not qualified product behavior. No Swift
compiler, UIKit dialog, Keychain callback, native endpoint, build or test ran.
Android callers, recovery, peer consent/exchange/approval, transport and all
platform qualification remain unfinished. Policy remains disabled.

## Android native identity dialog caller

The dedicated JNI identity entry dispatches only status, preparation, review,
confirmation and cancellation. It is separate from renderer `nativeCall` and its
Java declaration is private and retained for shrinking. Confirmation copies at
most 128 bytes of review-ID JSON onto the synchronous native call stack; budgets
are fixed at 25 seconds and no secret is returned through JNI.

NativeCore serializes these calls through its existing bounded work lane. It
reads status/policy first, refuses replacement of active or uncertain identities,
and checks public profile/device/generation/action/revision metadata. A typed
review is created on the worker before UI dispatch, so main-thread delays do not
restart its monotonic UI expiry. Rust's original expiry is authoritative. Public
response arrays are bounded and cleared after decoding. The review token and
metadata are not sent to the renderer channel.

The current native entry is reached by selecting the Chrome Game Hub menu again
when its existing tab is open. The resulting native actions menu offers the
library or Remote Play identity. The confirmation alert displays exact public
key/device/generation and explains local-only activation; only its positive
action calls confirmation. A separate atomic lifetime suppresses stale callbacks
after cancel, expiry, pause, tab hiding/navigation disconnect or close. Native UI
admission checks the current non-incognito Game Hub, resumed activity, unlocked
user and device. This does not attest to every intervening OS lock transition.

Cancellation schedules transient review cleanup and does not preempt a blocked
vault call or roll back a committed activation. Uncertain results require reading
status again, with no automatic retry. The installed policy remains disabled and
the menu reports unavailability before preparing a key. Peer consent/exchange,
startup recovery, rotation, relay approval and transport are still unfinished.

No Java/C++ compiler, Chromium menu, JNI callback, Android vault, native endpoint,
build or test was executed. Only source review and whitespace inspection were
performed. Menu discoverability, localization, lifecycle/lock/deadline behavior
and all device/ABI qualification remain due.

## Native recovery status and metadata dispatch

LocalHost status now includes recovery protocol 1, the native observation time
and each nonretired generation's exact next step. These are derived from the
same validated journal snapshot/revision as the public records. They are an
advisory read, not a lease, producer receipt or instruction to execute a plan.
In particular, MaterialReady can belong to an ongoing consent flow; its recovery
plan entry does not by itself prove abandonment.

A separate native-only metadata recovery entry accepts the exact status address
and expected revision, obtains the real profile credential lease and dispatches
at most one metadata transition. Only FenceGeneration or FinalizeRetirement can
commit; active valid generations require separate revalidation and all external
writer/session/relay/vault steps remain pending. Existing finalization invariants
still require all retained cleanup receipts. No fallback NativeRecoveryOwner or
caller-supplied retirement flags were introduced.

Explicit identity fencing also fences its pairs under the existing journal
transaction. LocalHost clears any pending identity consent review before this
dispatch. Therefore this is not a status operation or an automatic startup loop;
a real native recovery controller must select it deliberately and present any
required user decision. Streaming-disabled policy does not block cleanup.

The mobile entry bounds input to 1024 bytes and the original operation budget
to 1–25 seconds, and rechecks foreground/provider epoch and deadline before
publication. No secrets or vault callbacks are used by this metadata step. A
late/error response may follow a durable fence or finalization; reload status
instead of assuming rollback. Pending does not mean that a backend was stopped.

App recovery UI/JNI callers, actual retirement owners and startup reconciliation
remain unfinished. No native endpoint, lock/file/SQL mutation, build or test was
executed. Source review and whitespace checks are the only verification here.

## iOS native recovery inspection and explicit metadata action

The native identity button now offers separate setup and status/recovery actions.
Recovery status is read through the dedicated native API even while streaming
policy is disabled. Swift admits protocol/revision/profile/device, bounded
records and recovery steps, unique generation addresses and a one-to-one match
between each nonretired record and its planned action. Retired records have no
remaining action. Public metadata stays outside WebKit messaging.

A native list opens each record's exact generation, key, human-readable phase
and next step. External writer/session/relay/vault cleanup is displayed as waiting
and has no execution button. Only journal fencing or fully receipted finalization
has a native action. The fence confirmation explicitly states its permanence,
identity-to-pair effects and that it does not prove connection stop/key deletion.
Selection of a list row only opens details and never executes recovery.

Swift binds the action to the selected snapshot record/address/phase/step and
expected revision, with a 30-second monotonic snapshot lifetime established before
status I/O. The operation budget is capped by the remaining snapshot lifetime and
the native 25-second limit. Core reselects the actual current step under the real
lease. Every outcome/error requires a fresh status read; no subsequent cleanup
step, new key or retry is scheduled automatically.

Recovery uses the same transient UI lifetime cancellation as identity consent,
including background/protected-data loss and expiry. UI cancellation cannot
preempt a synchronous in-flight operation or undo a committed fence. Native
sheet/presentation/accessibility behavior, device lifecycle, ABI/vault/SQL and
compilation remain unverified. No UI/native call, build or test was executed.
Android recovery UI and actual external retirement owners remain unfinished.

## Android native recovery inspection and explicit metadata action

The native Game Hub actions menu now includes status/recovery, separately from
identity setup. Status remains readable when streaming policy is disabled. A
typed immutable Java snapshot admits the native protocol/profile/device/revision,
at most 128 records/actions, unique canonical generation addresses, known phases
and recovery steps, and an exact record/action address match. Retired records
have no next action. Snapshot lifetime starts before native status I/O and is not
restarted by main-thread delivery delays.

The native list/detail dialogs show exact public key, device, generation, phase
and next step. Merely opening a row never executes recovery. External producer,
relay and vault cleanup remains waiting with no execution action. Journal fencing
requires a native approval action after displaying its permanence, pair effects
and the distinction from actually stopping connections/deleting keys. Fully
receipted finalization has a separate finish action.

The private, shrinker-retained recovery JNI method accepts at most 1024 borrowed
JSON bytes and a 1–25 second budget. Its Java caller sends only the exact selected
immutable snapshot record and expected revision, with a budget capped by the
remaining 30-second snapshot lifetime. Rust reselects the current plan under its
credential lease; Java admits the exact returned address/protocol/outcome and
requires status reload. No retirement flags or secret bytes cross this entry.

Recovery shares the existing atomic native dialog lifetime and pause/disconnect/
close/cancel/expiry invalidation. Cancellation cannot preempt synchronous native
work or undo an already committed fence. There is no automatic follow-on step,
replacement key, producer-retired assumption or cleanup retry.

No Java/C++ compilation, Android dialog/menu, JNI endpoint, lease/SQL mutation,
vault, build or test was executed. Actual device/lifecycle/presentation/localization
qualification, startup reconciliation and external retirement owners remain due.

## Exact-generation vault removal product path

A concrete removal branch now connects LocalHost and the mobile provider to
actual exact-generation vault deletion/read-back. It requires the current plan
to be RemoveMaterial under the retained real credential lease and exact expected
journal revision. Existing writer, local and relay retirement receipts and the
identity-child ordering must already be present. This branch does not infer or
create any of those prerequisite receipts.

The scoped provider removes only the admitted generation and then reads it back.
Only explicit missing is absence evidence; locked, corrupt, failed or late
observations keep cleanup uncertain. After read-back and lease/deadline checks,
the journal records MaterialAbsent and applies its existing retirement invariant.
A post-commit error may leave deletion/retirement committed, so callers reload
status rather than assuming rollback or generating another key.

The dedicated mobile C entry bounds the exact-address/revision request, retains
host/provider ownership and observes provider access/foreground epoch before and
after work. It is not renderer dispatch. iOS and Android typed recovery callers
now route a selected RemoveMaterial step to this entry; other recovery steps
retain their separate metadata path. Android's private JNI declaration and
shrinker signature are updated together for the native removal selector.

Both native dialogs display permanent exact-generation key removal and require
explicit approval. Unobserved writer/session or relay cleanup still has no
execution button and cannot be skipped to reach deletion. Installed streaming
policy remains disabled; cleanup does not require enabling new sessions.

No native/UI/vault/lease/SQL operation, compiler, build or test was executed.
Actual producer retirement owners, startup reconciliation, relay cleanup product
integration and all provider/device/failure qualification remain unfinished.

## Installed-policy relay retirement product path

LocalHost now connects the concrete authenticated pair-retirement branch to the
mobile recovery API. It builds the bounded native HTTPS client only from validated
installed origins, retains the real credential lease through pinned identity
reload/network/receipt publication, and uses the journal's exact signed grant.
Prior writer/local retirement remains required. The response must bind the
expected pair and grant hash with the retirement protocol acknowledgment; network
failure, HTTP status alone, local expiry or a missing live grant is not success.

Installed configuration validation is shared with normal signaling, including
canonical bounded distinct origins. Cleanup may use those configured origins
while new sessions remain disabled. An empty origin list refuses retirement
before client/lease/vault/network work. The checked-in list is still empty and
streaming is still disabled; no relay was configured or contacted.

Mobile providers now retain a cancellation token for each availability epoch.
Every foreground transition cancels the old token permanently and creates a
separate token for the new epoch; terminal close also cancels. Resuming never
uncancels an older HTTP operation. The retirement call captures the matching
token and rechecks provider access/epoch afterward. HTTP cancellation remains
cooperative and cannot forcibly preempt a blocked OS call. A change racing the
last observation/commit can still leave a committed receipt with a suppressed
response; reload remains mandatory. UI timeout/cancel alone is not a native
network-preemption guarantee.

Status exposes whether configured retirement origins are available. Android/iOS
native recovery dialogs offer explicit remote revocation only when this is true,
and otherwise explain the missing service configuration. They send no URL/key or
receipt flags. Android uses the renamed private `nativePairingRecoveryV2` JNI
entry for its expanded action selector, so mixed old native/Java components fail
linking instead of interpreting the old boolean selector as a different action.

No compiler, UI/JNI/native/provider/lease/SQL operation, HTTP request, deployment,
build or test was executed. Actual writer/session retirement owners, startup
reconciliation, relay deployment and device/failure qualification remain due.

## Protected-data and explicit-cancel epoch bridge

An acknowledged native epoch invalidation entry now bypasses the LocalHost
request mutex, making it usable while synchronous relay/vault work is pending.
It rotates only the scoped provider's existing availability epoch, permanently
cancels the old token and preserves the host's current foreground setting.
Registry failure, zero or never-issued handles are not acknowledged. An already
closed/removed issued handle is terminal; an unattached provider has no in-flight
pairing work. Mobile handle allocation now refuses overflow permanently instead
of wrapping and potentially reusing a handle.

iOS provider contexts retain their actual native handle and register weakly for
protected-data notifications already wired by BackgroundSync. A transition first
closes Swift vault access, then invalidates every live context's native epoch
without holding the Swift access lock. Unlock publishes availability only after
all invalidations are acknowledged and the same Swift transition is still
current. A later notification cannot be overwritten by an earlier completion;
epoch exhaustion terminally closes access. This binds observed lock/unlock
cycles across HTTP waits, not just across individual Keychain calls.

Native Android/iOS dialog cancellation now also invalidates the epoch before
queuing transient LocalHost review cleanup. Android has a separate private,
shrinker-retained JNI invalidation call. Waiting behind the request mutex cannot
delay the cancellation token until after that request has returned. Registry
contention may still delay the bridge; it is not a hard real-time OS hook.

These changes do not prove an unobserved OS event, preempt synchronous native
work, stop backends or roll back commits. Revocation may have reached the relay
or a journal commit may already exist when UI cancellation suppresses its reply.
Reload/reconcile the same generation; no retirement receipt is manufactured.

No notification/UI/JNI/native/keychain/relay operation, compiler, build or test
was executed. Event ordering, blocked registry/request lanes, rapid lock/unlock,
late results, context release and cancellation races remain device-qualification
requirements. All earlier incomplete scope remains open.

## Startup inspection and retained session debt

Native status now exposes only a boolean indicating retained journal session
ownership, without exporting a session capability. Typed Android/iOS recovery
records preserve it. An active record with a RevalidateBeforeUse plan can still
carry abandoned ownership; absence of another cleanup action must not hide that
debt or imply that a new receiver can replace it. Native detail dialogs explain
the unresolved earlier connection and do not clear ownership by restart/age.

Each Game Hub controller authors one read-only startup inspection after its
trusted foreground UI/channel is ready. iOS marks the native identity menu as
needing attention or not yet inspected; opening status again refreshes that
indicator. Android presents a native notice pointing to status/recovery when
attention is needed or status cannot be inspected. Plain active records without
retained ownership and retired history alone do not trigger cleanup attention.
Incomplete candidate setup does trigger review, not automatic abandonment.

Startup inspection has its own transient lifetime, cancelled on suspension or
disconnect, so it cannot deliver a stale notice into another document/activity.
It does not run a recovery step, activate a candidate, replace a key, contact a
relay, query the vault or synthesize a producer-retired receipt. Foreground
admission failures remain "not inspected" rather than "clean". Actual startup
reconciliation/retirement dispatch still requires the missing native owners.

No startup callback, dialog, native/SQL read, compiler, build or test was executed.
Foreground scheduling, interruption, corrupt/locked journals, retained session
recovery and real device presentation remain qualification requirements.

## Lease source correction found during writer-retirement review

Static source review found the Unix held/named namespace revalidation block in
the wrong function: it followed acquisition inside the associated acquire()
function while referring to self. In addition to being invalid Unix source, that
placement left check() without the intended Unix per-admission namespace checks.

The block is now inside check(&self, ...), after profile/device/root binding and
alongside the Windows branch. Unix acquisition still retains the actual file and
directory handles, acquires the nonblocking lock, then invokes this check before
returning. Subsequent existing writer/recovery calls to check now have the same
canonical-root, held/named inode/device/owner, kind/link-count/private-mode
revalidation in their intended source path.

No compiler, file/lock operation, build or test was run. This correction is source
evidence only and does not qualify Unix locking or close writer retirement. In
particular, obtaining a lock in one profile root is not by itself proof against
an uncoordinated copied profile in another root. Actual retirement owner/root
provenance and platform qualification remain required before positive receipts.

## Cooperative lease namespace provenance

The journal now records a version-domain-separated SHA-256 binding to the admitted
profile/device/root and retained lock namespace. Unix includes held directory and
lock-file device/inode/owner identities; Windows includes the admitted path and
held root/lock creation identities under its existing deny-delete handles. The
binding is metadata, not a key, hardware identity or OS attestation.

The first successful mutation of a truly empty revision-zero history binds that
history. Every later journal mutation admits the existing binding before changing
state and checks it again before commit. Operational scoped loads now cover the
candidate writer, recovery/vault deletion/relay retirement, signal/ICE operations
and session registration. A copied/moved/recreated namespace cannot silently
adopt its current lock or relabel an existing history. Read-only authority
observation still uses its native registered entry; this change does not add
cross-host attestation or a new transport/session retirement proof.

Legacy histories without a binding remain inspectable through native status but
are refused by operational loads/mutations. They are not overwritten, reset,
re-paired or automatically upgraded. Native status exposes missing provenance;
Android/iOS attention and detail views surface it and withhold recovery actions.
Presence alone is not a live namespace-match attestation in a read-only status
response. The operational lease admission remains authoritative.

Explicit migration/root provenance review remains unfinished. Path aliases,
backup/in-place rollback, filesystem identity reuse, volume remapping, cross-host
copies and malicious same-user/admin namespace changes require separate admission
and qualification; this is a cooperative same-installation protocol. It does not
turn acquiring an arbitrary copied root's lock into WriterRetired evidence.

No hashing/metadata/lease/SQL/native/key operation, compiler, build or test was
executed. Only source review and whitespace inspection were performed. Actual
writer/session retirement owners and all original scope remain unfinished.

## iOS retained retirement marker instead of delete/add gap

Writer-retirement review identified that a process-owned file lease alone is not
an OS-service completion receipt. An unresolved Keychain request is treated as
potential late work; this threat assumption is not evidence that such a race was
observed on a device. No WriterRetired receipt is added by this change.

iOS cleanup now retains a public generation-bound `VLKRF001` marker in the same
non-synchronizing device-only account instead of deleting that account. The
marker is 73 bytes: a distinct eight-byte tag followed by the existing public
kind/profile/device/item/generation prefix. It contains no key/seed or grant.
Normal secret writes remain SecItemAdd-only and never update existing accounts.

Cleanup first attempts to add the marker. A duplicate updates the exact account's
value in place to the marker, with the required accessibility policy. It never
deletes and re-adds. It then reads the account back and admits exact service,
account, policy and marker bytes. NotFound during this sealing operation is an
error, not a positive retirement fence. Lock/epoch/OS errors retain uncertainty.

This ordering is intended to keep the account occupied against a late add-only
candidate: an add winning first is overwritten by cleanup, while an add arriving
after the retained marker is a duplicate. This relies on the admitted native
provider's add-only protocol and Keychain primary-account semantics, not on
arbitrary external writers respecting an application flag. Actual race behavior
and synchronization still require qualification.

Provider reads report no secret material only for an exact admitted marker or
normal missing item; malformed markers remain errors. The generic store contract
now clarifies that material absence does not require absence of a public OS
tombstone. This is logical secret removal, not a forensic media-erasure claim.
Marker retention/profile-maintenance policy and owner/fence integration remain
unfinished; no native tombstones were created or deleted in this source turn.

The API choice follows Apple's documented
[same-account update and duplicate-add behavior](https://developer.apple.com/documentation/security/updating-and-deleting-keychain-items).
No Swift compiler, Keychain call, OS operation, build or test was executed.
Android/desktop late-write containment and all producer retirement proof remain
open; item 58 is not complete.

## Android retained retirement barrier

Android cleanup now creates a separate public `.retired` record for the exact
generation under the existing per-record lease. Its fixed 76 bytes are
`VLKRPF01` followed by the dedicated JNI address (four UUIDs and big-endian kind).
It contains no seed/key/grant. Creation is exclusive/no-follow, file contents are
forced, the private directory is synchronized and exact contents are read back
before any alias/ciphertext deletion. Cleanup never unlinks this marker.

Candidate writes reject any retirement-file presence before Keystore creation
and again before ciphertext creation. Reads validate a present marker first;
remaining ciphertext or alias is incomplete cleanup and cannot be decrypted or
reported missing. Only observed absence of both secret artifacts reports missing.
Cleanup verifies the retained marker again after deleting and synchronizing.
Reads treat partial, corrupt, wrong-address and oversized markers as errors.
An explicit cleanup retry under the retained record lease may complete only an
exact public prefix of the expected marker, including an interrupted empty
create. It appends the missing bytes without truncating, unlinking or overwriting
the existing prefix, forces and reads back the full marker before secret deletion.
Wrong-prefix or oversized content remains refused and requires reviewed repair.

This contains cooperating provider candidate writes, not arbitrary external
writers or future AndroidKeyStore service work. A late alias or ambiguous OS
result remains unresolved; the marker is not a WriterRetired/session receipt.
Actual owner/fence integration, corrupt-marker repair/retention policy, crash
ordering, lock/unlock and desktop containment remain unfinished.

No Android file/Keystore call, native callback, compilation, build or test was
executed. Only source review and whitespace inspection were performed.

## Desktop provider retirement marker

The generic desktop keyring provider now retains a public marker in the separate
`org.vlikeon.remote-play.retirement.v1` service at the exact credential account.
Its fixed 73-byte record binds tag/kind/profile/device/item/generation and holds
no secret. Cleanup admits any existing marker, creates only when explicitly
missing, reads it back before secret deletion and retains it after deletion.
Corrupt, mismatched, locked or failed marker reads never authorize replacement.

Candidate writes validate the full secret envelope, refuse an existing secret
or retirement marker, and check the marker again around set_secret. This is
check/write under the caller's retained native credential lease, not backend
compare-and-swap or atomic create-only semantics. Reads refuse a marked generation
with remaining secret material; cleanup returns success only after explicit secret
absence and exact marker read-back. Public marker absence is distinct from a vault
error. Temporary secret results remain zeroizing on rejected paths.

This prevents normal cooperating provider use of marked generations. It does
not prevent a generic OS vault from physically completing a previously submitted
write after process death. Such late material remains quarantined by the separate
marker and requires reconciliation; this change supplies no WriterRetired receipt,
forensic erasure guarantee or complete cross-process/backend ownership proof.
Desktop-specific late-operation containment and marker retention/repair remain due.

No vault/OS operation, compiler, build or test was executed. Only source review
and whitespace checks were performed; item 58 and all original scope remain open.

## Explicit revocation of active generations

Native recovery detail views now offer explicit revocation for an Active identity
or pair, rather than exposing only its RevalidateBeforeUse advisory plan. The new
native entry is separate from metadata plan execution: it requires the exact
active generation, expected revision, scoped namespace/lease, foreground epoch
and original bounded deadline. It accepts no cleanup receipts, URL or secret.
Installed streaming enablement and vault availability are not prerequisites to
this journal-only fence (the scoped native provider must still be attached).

The existing journal revoke transaction fences the selected pair or identity and
its pairs. It retains session ownership and outstanding cleanup observations;
the operation does not claim backend stop, relay acknowledgment or key deletion.
Both native detail dialogs explain permanence and identity-to-pair effects before
the approval action. Expired-but-still-Active records also use this explicit path.
Java/Swift retain exact snapshot address/revision/lifetime binding, and Android
JNI's bounded action selector now accepts the new explicit revocation action.

All responses/errors require status reload; a late epoch/deadline failure may
follow a committed fence. No automatic retry, replacement or session release is
introduced. No native/UI/SQL/lease/vault operation, compiler, build or test ran.
Actual producer retirement and interrupted-session recovery remain unfinished.

## Session release requires the actual backend owner

Source review found that registry release previously closed its permit then
cleared durable session ownership without requiring a backend retirement call.
The native contract documented retirement but did not enforce that prerequisite.
Release now requires a native owner implementing NativeSessionRetirement and an
original deadline of at most 30 seconds. There is no default implementation or
renderer acknowledgment path. Actual ICE/media/input/gamepad/signaling owner
integration remains unfinished and cannot be replaced by a constant confirmation.

The registry closes the permit first, admits the exact registered Arc identity,
generation/session/epoch/revision and scoped credential lease, then invokes the
owner to cancel/join and confirm all scoped backend work. Pending/error/timeout
retains journal ownership and the registry entry. After confirmation, the journal
mutation re-admits exact ownership/revision/namespace before clearing it. The raw
release mutation is crate-private. A late post-commit error requires status reload.

Closed or expired registry entries are no longer pruned automatically. They count
toward the existing 16-entry bound until actual retirement, preventing elapsed
time or a dropped weak guard from silently discarding native cleanup tracking.
Dropping the whole registry/permit still leaves durable recovery debt. This change
does not implement crash recovery or prove that any backend has actually stopped.

The owner callback executes under the retained credential lease and must not
reacquire it or reenter the registry. Existing in-flight cooperative operations
must leave their lease before release can acquire it; unrelated retained backend
work requires its own cancellation/join path. Device/service qualification of
those paths remains due. No backend/native/SQL operation, compiler, build or test
was executed; only source review and whitespace checks were performed.

## Worker retirement requires I/O completion

Review of the concrete Windows worker clients found stop could report success
after helper process exit while retaining unfinished I/O thread handles. Input,
video and gamepad client stop/close now refuse a positive retirement result until
all owned I/O threads have actually finished and joined. A thread panic is retained
as a terminal join failure, including subsequent stop attempts. Process termination
or a cancellation request alone does not satisfy this check.

Input retains only an actually received released-state Stop observation; video
retains only an exact successful Stop receipt. A caller retaining these worker
objects may reap threads on a later call without resending the Stop operation or
manufacturing a lost acknowledgment. Missing/failed acknowledgments remain errors.
Gamepad transport stop still is not a HID release receipt; its existing independent
device cleanup proof remains necessary. No session owner is implemented by this
worker-only correction, and dropped objects still require external recovery when
their result was uncertain.

No helper, process/thread cancellation/join, OS input/video/device operation,
compiler, build or test ran. Only source review/whitespace inspection was done.
Deadline propagation, full owner aggregation and crash qualification remain due.

## Broker retains uncertain input/video retirement

Broker review found input stop removed its worker before observing the result,
and terminal status/error paths also discarded it. The broker now retains an
uncertain worker together with artifact/process witnesses in CleanupRequired.
Only confirmed released input plus helper/I/O retirement transitions it to
Revoked and removes the worker. A later explicit revoke can reap that same worker;
new input/capture admission remains blocked by CleanupRequired. close() attempts
retirement and retains uncertainty while the broker object is alive.

The input client also preserves an actually received revoked/released observation
from any matched RPC. This avoids losing a real terminal receipt when the helper
exits before a subsequent Stop request; it does not substitute process exit for
the receipt. Pending I/O or panic still refuses confirmation. Video adapters now
permit repeated explicit revoke on their retained closed worker, allowing the
worker's existing exact Stop receipt and later completed joins to be rechecked.

Object destruction/process crash can still lose in-memory owner tracking and
requires the separate durable recovery controller; retention while alive is not
crash-proof cleanup. No broker/helper/native/thread operation, compiler, build or
test ran. Actual owner aggregation, deadline propagation and qualification remain
unfinished, as does the original 51–109 objective.

## Native client invitation transcript preparation

NativeClientPairing now authors the missing client-side preparation from an
`OPRP1` invitation code. It bounds/decodes the outer document, refuses unknown
fields, verifies the signed invite against a separately pinned host/audience,
admits its lifetime/UUID and checks the canonical 32-byte invitation secret hash.
Only then does it generate its own ephemeral exchange/nonce and sign ClientHello
with the supplied native identity. Product code must load that identity through
the existing Active/scoped vault path; this primitive does not do that admission.

The pending object retains the exact invite, signed hello, exchange and pinned
peer/audience. Its six-digit comparison code uses the browser's JSON array of
domain/invite-payload/hello-payload and the first SHA-256 word modulo one million.
The original remaining invite lifetime is bounded by a monotonic deadline as well
as native wall time; preparation/finishing cannot renew it. Finish consumes this
exact transcript and the existing authenticated ECDH/KDF admission, including on
rejection. It does not accept a replacement hello from the caller.

Invitation proof is kept in a zeroizing owner with no Debug/Clone/Serialize or
secret getter. Decoded outer bytes and raw secret bytes also use zeroizing owners;
the successfully parsed temporary secret is explicitly cleared on Drop. Caller
input, parser partial allocations and library-internal copies are outside that
zeroization guarantee. Native UI must clear its own scanned/pasted code storage.

This sends no request and supplies no peer approval. Native relay JoinInvite,
comparison-code consent UI, grant collection, pair writer activation and host-side
invitation controller remain unfinished. No parse/crypto/native/vault/network
operation, compiler, build or test ran. Source review compared the format with
the checked-in browser pairing.js; runtime interoperability is not proven.

## Native client join transport and bound receipt

The pending native client transcript now dispatches an explicit JoinInvite through
the existing allowlisted HTTPS transport. A crate-private operation borrows the
retained invitation proof and signed hello, checks the supplied identity against
that hello and the pinned signed invite, and signs a fresh request nonce. Request
serialization uses borrowed proof fields and zeroizing buffers; the signed
envelope's proof-bearing base64 payload is cleared on Drop. HTTP/library internal
copies are outside this guarantee. No renderer signing/proof-export API is added.

Paired requests and join share one bounded POST/response path: installed/native
origin allowlist, no redirect/ambient proxy, fixed response bounds and original
deadline/cancellation rules remain. Join additionally caps the request by the
pending invite's monotonic expiry. Only an exact protocol-1 pending receipt for
invite ID, client public key and signed-hello hash marks the object joined. Finish
now requires that acknowledgment in addition to local consent and valid signed
grant/transcript. Pending is not host approval or activation.

Signaling source returns this bound receipt for both first insertion and identical
hello resubmission. Conflicting hellos still refuse; existing browser callers can
ignore the additive receipt fields. Legacy unbound pending responses are refused
by native. The server must be deployed before native join can succeed. Ambiguous
failure keeps the same transcript for an explicit retry, never a new exchange or
automatically published pair.

No HTTP/crypto/parser/native/vault/SQL operation, server deployment, compiler,
build or test was executed. Product Active identity/lease/foreground admission,
native peer-consent UI, grant polling, pair activation and host invitation flow
remain unfinished; protocol and interoperability qualification remain due.

## Native client approval polling

The pending client now authors one explicit PairingStatus poll, signed by the
same identity and limited by the original invite and operation deadlines. The
server source binds status to protocol 1, invite/client and stored signed-hello
hash, alongside pending/declined/approved state and an optional grant. Native
refuses unbound legacy status, unknown states and a grant outside approved state.
The first approved document is pinned; regressions, later conflicting grants or
approval after observed decline are rejected instead of replacing local trust.

Approval verifies the grant against the separately pinned host public key, then
checks audience, participant keys, invite ID, exact hello hash, client exchange
and nonce, creation bounds and expiry. The final deadline/cancellation/invite
checks occur before publishing pending-object state. Native UI receives only
pair ID, input permission and expiry metadata; public signed grant access is
available to the later native writer. Finish consumes the stored admitted grant
rather than accepting a caller-substituted document.

Poll status is not local comparison-code consent or activation. No automatic
poll loop, pair persistence, foreground/lease controller or UI caller is added.
The additive server receipt source still needs deployment and interoperability
qualification. No HTTP/crypto/parser/SQL/native/vault operation, compiler, build
or test was executed. Product grant-to-writer activation and the host invitation
controller remain unfinished, together with producer/crash recovery scope.

## Approved client transcript to native candidate writer

The retained-lease writer now accepts an opaque NativeClientPairing after actual
native comparison/peer consent. It admits the exact expected journal revision and
Active identity generation, reloads the pinned vault identity, and verifies the
pending hello/client key against that identity before consuming the ephemeral
exchange. The caller supplies no alternate grant, audience or key; the exact
approved grant is copied from the pending object and independently rechecked by
its consumed transcript finish.

Derived material flows directly into the existing reserve/write-intent/vault
write/read-back/MaterialReady path under the same credential lease. The original
operation deadline is capped by the pending invite's original monotonic deadline,
so network/consent/derivation cannot renew the vault-publication budget. The
candidate remains inactive; late or failed publication keeps existing journal
recovery debt and does not restore the consumed scalar.

LocalHost provides a native-only controller entry for this operation and enforces
the installed pairing policy before vault/exchange work. The checked-in disabled
policy still refuses it. No mobile JSON/C ABI accepts a pending transcript or
secret material; actual pending-controller ownership and native peer-consent UI
must still be integrated before this entry has a product caller.

No identity/vault/crypto/SQL/native operation, compiler, build or test ran. Pair
activation with persisted approval/recovery semantics, client UI/controller and
host invitation flow remain unfinished, together with backend/crash recovery.

## Client candidate activation with fresh pair confirmation

The native writer now authors explicit client candidate activation after actual
native local consent. It requires the exact MaterialReady pair/revision under
the retained scoped lease, a live grant naming this client, an Active matching
identity, pinned identity reload and exact pair-material reload. Missing/locked/
retired material cannot become a new identity or silently regenerated pair.

Before Active publication it sends ConfirmPair, authenticated by that identity,
and requires protocol-1 approval for the exact pair ID, signed-grant hash and
expiry. Signaling source checks the current unrevoked live pair row and signed
participant grant, independently of invite/join rows that expire after five
minutes. It allocates no ICE credentials and does not mutate approval state.
Legacy/unknown responses, missing/revoked/expired pairs and network uncertainty
refuse activation. This is a fresh observation, not an atomic guarantee against
a later remote revocation; subsequent operations still need live admission.

Journal activation re-admits revision/namespace under the same lease. Deadline,
cancellation and grant expiry are checked again before publishing its result.
An error may follow an already committed Active transition, requiring reload and
reconciliation; no automatic retry is introduced. This client-only path is not
a session/capture/input capability or a host approval implementation.

LocalHost gates the operation on enabled installed pairing policy and constructs
transport only from that policy's origins. Retirement origins while disabled
cannot enable activation. Current disabled policy still refuses; native pending/
consent UI and mobile foreground controller integration remain unfinished.

No relay deployment, network/native/vault/SQL operation, compiler, build or test
was executed. Protocol deployment/interoperability, activation races, owner/crash
recovery and the full original scope remain unqualified and incomplete.

## Native host invitation and persist-before-approval transcript

NativeHostInvitation now authors a signed five-minute invite with fresh UUID and
32-byte OS-random secret. The hash enters the signed invite; proof remains in a
zeroizing owner. Explicit share_code returns a zeroizing OPRP1 bearer code for
the native share UI, not diagnostics/renderer storage. The share's borrowed
serialization uses a zeroizing buffer; caller/OS sharing copies still need their
own handling. The host object has no Clone/Debug/Serialize implementation.

Native peer review verifies the hello using the separately approved client key,
checks invite/time bounds and produces label/key/comparison metadata. Host and
client use the same checked-in browser-layout comparison function. After actual
native comparison/input-permission consent, approval preparation creates a fresh
P-256 exchange/nonce and signed grant bound to that exact hello. Pair lifetime
must be explicitly supplied within protocol bounds; input permission has no
automatic opt-in. Each invite allows at most eight distinct attempted client keys,
one grant attempt per client. Uncertain persistence must retain the returned
approval/candidate, not generate a competing grant from the same invite object.

NativeHostApproval owns the exact invite/hello/grant/exchange until consumed.
The retained-lease writer reloads the Active pinned host identity before deriving
material, then uses existing reserve/write-intent/vault read-back publication.
It preserves the original invite deadline through MaterialReady. The signed grant
is retained in journal recovery metadata before any future ApprovePair request.
MaterialReady does not mean relay acknowledgment or Active host authority.

Relay invite registration/poll/approval dispatch, LocalHost/native share/peer UI,
host activation and durable retry orchestration remain unfinished. No RNG/signing/
ECDH/parser/vault/SQL/native operation, compiler, build or test was executed.
Cross-browser vectors, consent/persistence races, crash recovery and the complete
original scope remain unqualified and open.

## Persisted host approval and activation dispatch

Native host activation now reloads the exact MaterialReady pair and Active pinned
host identity/material under the same scoped lease as client activation. It first
checks whether the configured relay already holds this exact live grant. An exact
confirmation can reconcile a previously lost approval response without the expired
invite/join rows. Network/protocol/receipt-conflict errors do not trigger approval.

Only an explicit missing/unauthorized pair observation proceeds to ApprovePair
using the retained signed grant, original invite ID and client public key. No
alternative grant, generation or exchange is created. Server source returns a
bound protocol-1 approval receipt for first and identical approvals. Native checks
pair/grant/invite/client binding and obtains another fresh ConfirmPair before
allowing journal Active. A revoked existing pair cannot be resurrected by this
path: the server's revoked row/hash and existing approval checks still refuse it.

The host/client writer paths share revision/namespace/vault/deadline/cancellation
admission. Relay effects can survive a later local error, so MaterialReady plus
retained grant is recovery state, not permission to regenerate or auto-activate.
LocalHost gates either role on enabled installed origins and actual native consent
must precede dispatch. Current policy is disabled. No session/input capability is
created by activation; subsequent transport/authority admission remains required.

No server deployment, HTTP/native/vault/SQL operation, compiler, build or test ran.
Invite registration/poll/share UI, full native controller/foreground binding and
recovery/owner qualification remain unfinished. Additive approval protocol source
still needs deployment and browser/native interoperability verification.

## Native host registration and bounded hello polling

The host invitation now explicitly registers through the configured native HTTPS
client. Only a protocol-1 receipt binding invite ID, host key, exact signed-invite
hash and expiry marks it registered. share_code refuses an unregistered invite.
Signaling source makes identical signed-invite registration idempotent before
quota admission; a conflicting document for that ID is still refused. An uncertain
response keeps the original invite/proof for an explicit retry, without creating
a competing invite or extending its five-minute deadline.

Host polling requires registration and a bound protocol-1 invite/host/hash response,
with at most eight signed hellos. The native object verifies every hello's client
signature, invite/time/key constraints and comparison code before publishing a
complete snapshot. Duplicate clients or any invalid entry reject the entire new
batch. The previous snapshot is not partially replaced. Labels/keys remain peer
claims for actual native user verification, not automatic trust or permission.

Approval preparation now requires the exact signed hello from that polled
snapshot, alongside native comparison/input consent and the existing one-attempt
per-client rule. A caller cannot substitute another hello for the same UI-selected
client. Only public signed requests/review metadata are available outside the
pending object; bearer proof is sent only by the explicit share operation.

No HTTP/crypto/parser/native/SQL operation, deployment, compiler, build or test ran.
Host decline/close handling, Active-identity/foreground controller ownership,
native sharing/peer UI and durable crash reconciliation remain unfinished. Server
receipt changes need deployment and browser/native interoperability qualification.

## Native host decline and terminal invite close

Host pending state now fences a client from local approval before dispatching
decline. It requires the exact polled hello and refuses decline once this object
has prepared a grant for that client; approved pairs instead need revocation.
An explicit retry keeps the same signed hello, including across polls that omit
a remotely declined request. Retained pending/decline evidence stays bounded to
eight clients and conflicting hello replacement is refused.

Server decline now reads the actual join state in the transaction: pending is
changed to declined, identical declined requests are acknowledged, missing joins
return NotFound and approved/unknown states return Conflict. Previously its
conditional UPDATE was already pending-only, but a zero-row update still returned
declined:true. The new protocol-1 receipt binds invite/client/exact hello hash;
native requires that binding before reporting remote decline confirmation.

Explicit close immediately fences the native invite, clears bearer proof and
pending requests, and prevents subsequent registration/share/poll/approval through
that object. Its authenticated CloseInvite request requires a protocol-1 receipt
binding the exact invite/host/hash. Server verifies owner and actual deletion
before acknowledging. Existing approved pair rows are unaffected by invite close.

Lost close responses or missing/expired invite rows are not converted into a
positive acknowledgment. The local object stays closed and can attempt bounded
reconciliation only while its original invite is live; durable close receipts,
post-expiry reconciliation and process-crash tracking remain unfinished. Native
terminal state does not prove remote cleanup or stop any live transport backend.

No relay/SQL/native/crypto/parser operation, deployment, compiler, build or test
ran. Real controller/UI/foreground wiring, receipt persistence and qualification
remain required; item 58 and the original full objective remain open.

## LocalHost owns the pending client controller

LocalHost now retains at most one opaque pending client transcript, with a fresh
native ticket bound to the exact identity generation, journal revision and native
lifecycle epoch. Begin checks enabled installed origins and reloads the Active
pinned signer under the credential lease before creating the transcript. A live
pending object is not silently replaced. Public output contains ticket, peer key/
fingerprint, comparison code and expiry; proof/exchange never enters that output.

Join and poll accept only ticket/epoch and reload that same identity/revision
under the scoped lease before each signed network operation. They cannot accept
replacement invite/hello/grant/key material. Explicit cancellation drops the exact
pending transcript; detach/close clear it with other transient capabilities. These
local actions do not revoke an already approved remote pair or prove remote exit.

The native consent entry takes and consumes the pending object before handing it
to the existing approved-client writer. Errors after candidate publication require
journal inspection, not recreation of the ephemeral exchange. It returns only
MaterialReady metadata and does not automatically activate a pair. Host identity
revocation or any journal revision change makes old pending operations refuse.

Actual mobile ABI/UI wiring must supply/check the real foreground epoch and
cancellation token around these calls, independently approve the peer key and
comparison/input permission, and clear scanned-code input. These methods are not
renderer commands and no UI dialog/OS attestation is implied by calling them.
No native/vault/crypto/SQL/network call, compiler, build or test ran. Mobile/host
controller completion and full recovery/qualification scope remain unfinished.

## Mobile pending client ABI

Dedicated native-only C entries now connect pending-client begin and ticket-based
join/poll/consented candidate persistence/cancel to LocalHost. Begin bounds JSON
to 16 KiB and keeps its copied input and parsed bearer-code owner zeroizing. The
caller must clear original input/clipboard/share copies; parser-internal partial
allocations are not covered. Host-key confirmation remains the native UI's task.

Ticket actions accept at most 128 bytes, derive epoch and cooperative cancellation
from the actual attached foreground provider and retain original bounded deadlines
including mutex wait. No supplied epoch, replacement transcript or remote consent
assertion is accepted. Persist requires actual native peer/comparison/input approval
and returns MaterialReady only. The ABI itself does not attest a dialog was shown.

Cancel is available after backgrounding and requires the exact ticket; the native
caller first invalidates epoch outside the I/O lane, then queues local cancellation.
It does not revoke an approved remote pair. Native status exposes pending ticket,
revision/expiry metadata for reconciliation if begin's response was lost or stale,
without exporting proof/key/transcript. Late errors may follow committed candidates.

No ABI invocation, vault/crypto/SQL/network call, compiler, build or test ran.
Swift/JNI UI callers, peer confirmation, host controller, activation orchestration
and full recovery/qualification remain unfinished. Current policy stays disabled.

## Android client JNI and serialized caller

A dedicated private versioned JNI entry now bridges native client begin and
ticket actions. It bounds begin to 16 KiB and actions to 128 bytes, checks the
action/budget, copies proof into an RAII-wiped native byte vector and never
converts it to a Java string. Its exact signature is retained by shrinking rules.
The entry is separate from renderer nativeCall and does not create UI consent.

NativeCore's package-local caller consumes the supplied owned byte array and
clears it on validation, admission, queue failure, native failure and normal
completion. Only one normal client operation is admitted at a time, alongside
the existing total native work bound. Cancel invalidates epoch before queueing
and can enter while another client operation is pending, subject to that bound.
The original 25-second caller budget starts before queueing and passes only
remaining time into Rust; a queued request cannot renew its budget.

The eventual native UI must own/clear its original input and independently verify
host/comparison/input consent, parse bounded public replies, suppress stale UI
callbacks and reconcile lost begin tickets through native status. Caller-owned
aliases of the byte array, UI strings and library copies are outside the wipe
guarantee. The package-local caller must never be exposed via renderer messaging.

No Java/C++ compiler, JNI/native/crypto/vault/network operation, build or test ran.
There is no product UI caller yet. Android peer/share UI, iOS client caller and
host controller/recovery/qualification remain unfinished.

## iOS client ABI caller and owned input

NativeCore now authors a dedicated typed-action client caller for begin/join/poll/
consented persistence/cancel. It takes an inout Data variable into a private owner,
leaving that variable empty, bounds the payload and clears its owned storage on
every completion/admission failure. Swift copy-on-write aliases and OS/UI/parser
copies remain outside the best-effort wipe guarantee. No proof-bearing JSON is
converted to a Swift String by this caller or forwarded through HubBridge.

Only one ordinary client operation is admitted; a separate two-slot bound permits
local cancellation to queue behind one in-flight operation. Cancel invalidates
native epoch before entering the serialized I/O lane. The original 25-second
deadline starts before queueing and only remaining time reaches the C ABI. Native
foreground/provider epoch checks remain authoritative. Completion still needs UI
lifetime/ticket/metadata admission on the main thread, not raw response forwarding.

The persist action is named persistAfterPeerConsent and must only be called after
actual native host-key/comparison/input permission approval. A typed enum does not
prove that consent occurred. No UI flow calls this method yet; scan/paste/share
handling and typed public response models remain due. No Swift compiler, native
ABI/vault/crypto/SQL/HTTP operation, build or test ran. Current policy remains off.

## Relay closed-invite replay fence

Server review found an identical CreateInvite retry could recreate an invite
after CloseInvite deleted it. The server now authors a public closed-invite table
binding invite ID, original host, signed-invite hash and original expiry. Close
inserts that fence and deletes the live invite in the same Immediate transaction.
CreateInvite checks the retained fence before idempotent registration or quota
admission, refusing a still-valid closed ID instead of resurrecting it.

The same authenticated host can retry close and receive the exact stored receipt;
another signer is refused. Missing state without a fence remains NotFound. Fences
expire only at the original invitation expiry, when the original signed invite
can no longer pass registration validation. No proof secret is retained. Existing
pair rows remain unaffected. This is an additive schema-1 table/index, not a
destructive migration or claim that older deployments already enforce the rule.

Fence storage is bounded to 4096 total and 32 per host. If reservation cannot be
made, close fails with quota and leaves the live invite unchanged; unexpired
fences are never evicted to make room. Expiry cleanup runs in the existing request
transaction. Pre-upgrade closed invites without a receipt cannot be reconstructed
from absence and still require conservative handling.

No SQL/server/network operation, deployment, compiler, build or test ran. Source
review and whitespace inspection only. Race/retry/quota/expiry/schema qualification
and native durable post-crash reconciliation remain open with the original scope.

## Mobile client reply admission (source only)

Android and iOS callers now parse non-begin requests as a single canonical,
nonzero UUID ticket and require matching ticket/protocol metadata in normal
successful replies. Persisted candidate replies receive basic profile, kind and
MaterialReady phase checks. This is not exhaustive typed validation or a new
consent/activation authority; the native controller remains responsible for
the transaction and credential scope.

Successful callbacks recheck the original queue-inclusive deadline and dialog
lifetime on the main thread. Late results require native status reconciliation:
rejecting delivery does not undo a committed operation. Cancel may outlive the
dialog but still respects its original operation deadline. Actual consent UI,
complete typed candidate admission and runtime qualification remain open.
No compiler, build, test, vault, crypto, SQL or network operation was run for
this checkpoint.

## Remaining implementation and qualification

iOS host review source now wraps the existing validated native poll reply into immutable peer review objects scoped to the originating core, document lifetime, delivery fence and original 30-second deadline (including polling time). Native menus distinguish untrusted labels from public-key/comparison evidence. A separate decline confirmation consumes one review before admission, invokes host-peer action 2 without permission fields, and checks exact ticket/key/hello hash plus typed declined/pairs-revoked flags. Stale or uncertain results require status reconciliation rather than automatic replay. Permission selection, candidate persistence and host activation UI on iOS remain unfinished. This source checkpoint has not been compiled, executed or device-qualified.

iOS host proof reveal is now wired in source after explicit registration. The native raw-buffer callback creates a selectable UIKit text surface; its Rust allocation remains scoped to the callback. An over-full-screen presentation preserves the hub's owner lifetime. The surface clears visible text on hide/disappearance; the existing owner cancellation also clears and dismisses it on expiry, background, protected-data loss, or full-screen gameplay. Capture-state checks refuse reveal while capture is reported active and clear the surface on capture notification. This cannot prevent screenshots or erase UIKit/String/OS clipboard aliases. No automatic clipboard write, renderer exposure, runtime claim, or device qualification is implied. Host peer review/consent and activation UI remain open on iOS.

iOS host creation UI source now calls the existing native setup/begin/ticket APIs. Only installed relay origins are selectable. The host sees its public identity before local creation; registration requires a separate action. Labels are bounded to 64 UTF-8 bytes without control characters. Setup keeps its original deadline, then the created invitation installs an expiry bounded by its original native expiry. Lifecycle ownership checks guard callbacks; closing uses the existing cleanup flow. Uncertain registration requires status/recovery. Host sharing and peer consent/activation UI on iOS are still pending. No build, tests, relay traffic or runtime verification accompanied this checkpoint.

Android host save success can now issue one immutable activation snapshot from
the consumed peer review. Exact original ticket/client/hello context and returned
MaterialReady address/revision/hash/permission/expiry are admitted before issue.
Separate confirmation displays peer, pair/generation, input permission and expiry,
then consumes activation once and calls host candidate action 7. Successful reply
must bind full address/hash and advancing bounded revision/Active phase. Shared
queue, original review deadline, owner/cancel fences and byte clearing apply.
Relay approval occurs only after that explicit action; transport is not started.
Expired/uncertain results require native status reconciliation, not another grant.
No build/test/UI render/native operation ran. Saved-host rereview/resume UI, iOS
host product flow and end-to-end qualification remain unfinished.

Android host peer permissions now require an unchecked independent-peer/code
verification box, default-off input choice and explicit displayed lifetime (one
hour, one day or seven days). Save consumes the same one-use decision snapshot
as decline and constructs retained ticket/client/hello hash plus chosen permission/
lifetime. The bounded caller validates exact reviewed context, MaterialReady scope,
grant hash, permission, expiry and nonapproval flags. LocalHost reloads the saved
host candidate under the existing lease before returning its grant metadata;
failure can follow commit and requires status reconciliation. No relay approval
or connection starts here. No build/test/UI render/native operation ran. Host
activation UI, iOS product flow and end-to-end qualification remain unfinished.

Android peer details now lead to a separate decline confirmation showing retained
client key/comparison context. The native caller consumes a one-use decision
flag, constructs only retained ticket/client/hello hash, and uses bounded ordinary
pairing admission with original review/operation deadline. Successful response
must match ticket, client and hash, with actual declined:true/pairs_revoked:false.
Late/uncertain results require fresh requests/status; no automatic retry or revoke
is inferred. No build/test/UI render/native operation ran. Approval/persistence/
activation UI and corresponding iOS product flow remain unfinished.

Android host invitation screen now offers explicit peer-request checks. Validated
poll output becomes immutable per-peer metadata retaining private ticket/hello
hash and original owner/cancel fence with a queue-inclusive 30-second lifetime.
The bounded list labels peer names unverified; details display exact client key
and comparison code for independent verification. Selecting a stale snapshot
refuses and details expire back to the invitation screen. No automatic polling,
peer approval/input permission or new relay action is added by browsing. No
build/test/UI render/native operation ran. Host consent/persistence/activation
UI and corresponding iOS product flow remain unfinished.

Android host invitation setup is now reachable from the native menu. Active
identity setup supplies relay choices; user creates locally then separately
registers. Only another explicit action fetches/reveals proof. The selectable
proof view disables state saving, requests FLAG_SECURE and clears displayed text
on dismissal/lifecycle expiry. Its Java String/UI/clipboard aliases are not
reliably erasable; the view explains copied text may remain in clipboard history.
Raw callback bytes still wipe after rendering. Original setup/invitation timers
cover the flow and explicit close uses existing cleanup. No automatic peer
approval, connection or renderer route is added. No build/test/UI render/native
operation ran. Host peer selection/consent, iOS views and qualification remain due.

New host invitation creation now checks the current scoped journal under its
retained lease and refuses any durable invitation cleanup debt before generating
proof or signing an invite. Mobile setup snapshots retain this advisory state
and host-begin helpers direct users to cleanup before consuming their one-use
setup. Client pairing is not blocked by this host-specific flag. Native revision
and journal checks remain authoritative if state changes after setup. This closes
the restart gap where missing transient owner previously allowed new host invites
despite old durable debt. No build/test/runtime operation ran.

Android/iOS durable cleanup callers no longer reject expired records solely by
local time. Confirmation views label expiry reconciliation explicitly and state
that the service must confirm expiry/live absence; unsupported/error responses
keep the record. Existing snapshot/provenance/retirement-origin, epoch/cancel and
deadline checks remain. Native writer chooses live close versus expiry receipt
using its verified original metadata; UI labels grant no completion authority.
No build/test/UI render/network operation or deployment ran. Relay rollout and
actual client/server compatibility qualification remain required.

Native relay cleanup now verifies the original expired host-signed invitation
and requires an exact expired_absent receipt binding protocol, invite ID/hash,
host, original expiry and pairs_revoked:false. Network/status/shape mismatch is
never accepted as absence. Durable restart-close writer selects this branch for
expired intents, requires installed retirement audience and exact retained
Active/Revoking signer, and holds its scoped lease through HTTP and transactional
completion. Original deadlines/cancellation surround each side effect. Failure
retains intent; postcommit failure requires reload. UI still blocks expired
records pending follow-up wiring. No compiler/build/test/SQL/network operation or
deployment ran; actual server/client compatibility qualification remains due.

RelayOperation ReconcileExpiredInvite now accepts the original host-signed public
invitation. Server verifies it against authenticated request sender and installed
audience at historical creation, then requires expiry by server time and no live
invite row after same-transaction expiry pruning. Conflicting live rows refuse.
Bound receipt includes original invite ID/hash/host/expiry, expired_absent reason
and pairs_revoked:false. No pair, grant or session is revoked; no live invitation
can be resurrected because CreateInvite still validates current expiry. This is
source protocol/server work only: native receipt admission and durable completion
are not wired. No SQL/network/compiler/build/test or deployment ran.

iOS status now lists durable invitation cleanup entries and displays invite ID,
signer generation, service and expiry in a separate confirmation. Unexpired
entries with provenance and retirement configuration can invoke the dedicated
restart-close ABI. Caller matches complete metadata against its snapshot,
invalidates the native epoch, reserves bounded cleanup admission and uses the
earlier status/operation deadline. Exact invite receipt and actual closed/
nonrevocation/reload flags are required; main delivery rechecks owner/fence/time.
Expired entries remain displayed without pretending cleanup is complete. No
compiler/build/test/UI render/network operation ran. Expiry reconciliation,
full host integration and end-to-end qualification remain unfinished.

Android native status now lists each durable invitation cleanup record and shows
invite ID, signer generation, service and expiry in a separate confirmation.
Unexpired records with valid provenance/configured retirement service can invoke
JNI host action 8, mapped to the dedicated recovery ABI. Caller requires exact
record membership/revision, original status deadline, live owner and immediate
native invalidation acknowledgment; bounded cleanup queue remains available
while an ordinary share is finishing. Reply must bind exact invite ID and actual
closed/nonrevocation/reload flags. Main delivery rechecks owner/fence/deadline.
Expired records remain visible but nonactionable pending expiry reconciliation.
No build/test/UI render/network operation ran. iOS equivalent and full runtime
qualification remain unfinished.

iOS cleanup status now retains immutable per-invitation metadata instead of a
count alone. Exact local five-field DeviceSigning address must match a nonretired
identity in the same snapshot, with canonical nonzero generation. Metadata also
requires a valid integral safe-range expiry and HTTPS origin shape; existing
unique invite/hash/boolean checks remain. Count is derived from retained records.
This grants no cleanup destination authority: native retirement allowlist still
applies. No compiler/build/test/runtime operation ran. Explicit recovery caller
and confirmation views remain unfinished.

Android recovery snapshots now retain immutable durable host cleanup records,
not just a count. Each validates invite/hash, integral bounded expiry, actual
expiry flag, HTTPS origin shape and exact five-field local DeviceSigning address.
After credential parsing, the address must match a nonretired identity in the same
snapshot. The list is immutable and contains no proof or activation capability.
Origin format is not network authorization; native installed retirement policy
still controls cleanup destinations. No build/test/runtime operation ran. The
explicit durable-cleanup caller/UI and equivalent iOS record model remain due.

Host registration retry now matches existing durable intent by invite ID, exact
signed-document hash, identity generation and audience under the scoped lease.
Matching intent is reused without another journal mutation. The expected revision
may lag by exactly one when that intent exists, covering an intent commit followed
by lost registration response; other revision changes refuse. The original exact
Active identity is reloaded before HTTP and normal closed/expiry/epoch/cancel
checks remain. This restores only registration retry, not proof/peer authority
after restart. No build/test/SQL/network operation ran for this source follow-up.

Android/iOS snapshots now count at most eight durable invitation cleanup entries,
checking canonical nonzero unique invite IDs, hash text and actual expiry flags.
Any entry triggers native status attention independently of transient host state.
This does not imply cleanup or automatic network activity; full action metadata
and recovery UI remain unwired. No build/test/runtime operation ran.

Native status now exposes durable host_invitation_cleanup metadata (invite ID,
identity generation, audience, signed-document hash, expiry), never proof. Explicit
LocalHost/mobile recovery close uses exact invite ID/expected journal revision,
installed retirement origins, actual provider epoch/cancel and original deadline.
Any matching live invitation is fenced first; confirmed durable completion clears
that transient owner. No share/peer resume authority is reconstructed. Current
path refuses expired invitations and still requires foreground provider; native
mobile callers/views are not wired. No build/test/SQL/HTTP/native operation ran.

restore_for_close now reconstructs only a permanently closed invitation from
pinned signed public metadata. It verifies current signature/audience/lifetime,
retains no bearer proof, and restores no peer request/exchange/registration state.
All normal share/approval/register paths remain refused by closed admission.
Writer recover_host_invitation_close selects exact durable invite ID and expected
revision under lease, then follows existing exact signer/relay close and intent
completion. Only unexpired invitations are supported; expired remote cleanup
still needs separate protocol reconciliation. No controller/ABI/UI or build/test/
crypto/vault/network operation ran. Durable recovery integration remains due.

Confirmed live host close now matches identity/audience/exact signed invitation
hash against current durable intents and completes only that entry transactionally
under the same lease/revision after native relay acknowledgment. No caller boolean
can invoke this crate-private transition. A missing legacy intent does not invent
one; other intents remain untouched. Network, deadline/cancel or journal failure
leaves intent for recovery (late postcommit errors may require reload). In-memory
close confirmation permits exact retry of journal completion. No build/test/SQL/
HTTP operation ran; restart cleanup and expired-invite reconciliation remain due.

Writer registration now invokes transactional retain_host_invitation before
network admission while holding the same credential lease and original deadline.
After intent commit it rechecks lease/deadline/cancellation, then sends register.
Confirmed success returns committed journal revision to LocalHost. Any later
error leaves cleanup intent durable, including failures before actual HTTP; no
false remote acknowledgment is inferred. The old controller revision may then
need explicit reconciliation before retry. No secret/proof is persisted. Durable
intent completion after close and restart recovery are not wired yet, so signer
cleanup can remain blocked until those are implemented. No build/test/SQL/HTTP
or native runtime operation ran for this source checkpoint.

Credential journal now authors up to eight host invitation cleanup intents,
containing original identity generation, signed invitation and audience only.
Validation binds exact retained Active/Revoking identity, signature/audience and
unique invite ID; no proof/private exchange or resume consent is persisted.
The transactional retain API uses existing namespace-bound FULL/EXTRA WAL update
and expected revision, rejecting conflicting same-ID documents and capacity
overflow. Identity recovery waits for retained invitation cleanup before material
removal. Existing journals default to empty new metadata; that does not establish
absence of preexisting unrecorded remote invites. Writer registration does not yet
call this primitive, so durable integration is incomplete. No build/test/SQL or
native operation ran. Actual pre-HTTP wiring, close receipt completion, restart
inspection and migration/qualification remain required.

LocalHost transient cleanup on detach now fences host invitations and clears
proof/request snapshots, while retaining possibly registered public cleanup
ownership in the same object. A new document cannot resume share/approval, but
can inspect status and explicitly close the retained ticket; signer-removal guard
still applies. Never-attempted invitations drop. Terminal close also fences but
closed-host admission prevents further calls; final object/process loss remains
unrecoverable without the unfinished durable invitation journal. No automatic
network cleanup or crash recovery is claimed. No build/test/runtime operation ran.

LocalHost exact material removal now refuses an identity generation referenced
by a retained possibly-registered host invitation. Closing the invitation first
preserves its cleanup signer; new-session policy need not be enabled for close.
Mobile status attention now includes any pending host ticket, surfacing the
existing native close action. This guard is only within the live LocalHost: it
does not protect against independent core callers, detach or crash because
durable invitation ownership is still missing. No build/test/runtime operation
ran; durable retention and expiry reconciliation remain required.

Invitation close now permits exact retained Active or Revoking identity material
on its cleanup-only stack. Current scoped journal must contain the original
DeviceSigning generation, matching pinned invitation host key, with material not
marked absent; vault load revalidates it. The retained lease spans signing/HTTP
and postchecks. No retired/missing/replacement identity fallback exists and no
Revoking signer reaches normal pairing actions. Durable invitation ownership is
still absent, so identity material retention across cleanup/crash is not yet
guaranteed by this path. No build/test/runtime operation ran. Expired invitation
and durable cleanup qualification remain unfinished.

Host close no longer treats the invitation's old peer-consent revision as cleanup
authority. It reloads current journal revision under the already-held scoped lease
and still requires the original exact Active identity address/generation plus
invitation host signature binding. Thus unrelated journal advances do not alone
strand live invitation cleanup. This performs no journal mutation or peer consent
rebase; poll/prepare/activation retain their exact expected revisions. Revoking
identity, crash and expired remote cleanup remain unfinished. No build/test or
runtime operation ran for this source follow-up.

Raw host share now transfers ordinary-operation admission with its proof buffer
to main delivery. Android work count/busy flag and iOS operation/queue permits
release only after proof wiping in callback finally/defer. Worker failures still
clear/release locally. Thus a stalled UI cannot admit repeated successful share
allocations after worker completion; close retains its separate bounded path.
No lock is held during the callback. Callback retention/copy violations remain
outside the borrow contract. No build/test or runtime race qualification ran.

LocalHost host expiry now fences the invitation and zeroes owned bearer proof/
clears request snapshots during status, capability maintenance and new-host
entry. Never-attempted expired invitations can drop locally; possibly registered
ones retain signed public metadata/identity/revision for explicit cleanup rather
than claiming relay absence. No new timer/background task is created, so an idle
host may retain proof until these entries or detach/close. Expired registered
remote cleanup is still not reconciled by this change. No build/test/runtime
operation ran; durable ownership and expiry acknowledgment remain unfinished.

iOS explicit share caller now owns the raw Rust allocation in a dedicated buffer
object instead of copying through String/JSON/Data responses. It validates bounded
OPRP1/base64url bytes, shares existing admission/deadline/owner/cancel fencing and
offers a synchronous UnsafeBufferPointer borrow only within the main callback.
Finally/deinit frees the Rust allocation through its wiping destructor. Consumers
must not retain that pointer. Pending main delivery still retains the allocation;
close fences delivery but does not synchronously erase it. No clipboard/OS share
UI or guarantee over future copies is introduced. No build/test/runtime operation
ran; native sharing views and end-to-end qualification remain unfinished.

Android explicit host-share caller now keeps proof as byte[] through its bounded
serialized lane, validates OPRP1/base64url form and checks owner/fence/deadline
before queue and main delivery. Native reply bytes are borrowed only during the
main callback and wiped in finally, including callback errors; rejected posting
and worker failures clear owned bytes. No string/JSON/clipboard conversion occurs.
While awaiting main delivery the array remains retained; close prevents delivery
but cannot synchronously erase queued data. Future UI must not retain the borrow
and must manage any unavoidable OS copies separately. No build/test/runtime
operation ran; product sharing UI and iOS equivalent remain unfinished.

Android dedicated host-share JNI now calls the raw share ABI, validates <=8192
OPRP1/base64url bytes before copying and frees/wipes the Rust buffer on every
path. Request vector also wipes; a failed JNI output copy attempts to clear the
Java array before returning null. Exact private declaration/shrinker signature
are retained. No jstring, generic JSON, clipboard or renderer route is introduced.
Java returned bytes still require a future lifecycle-owned caller to clear them;
OS/UI copies are outside these guarantees. No build/test/native operation ran.

Dedicated host_share ABI now returns raw OPRP1 UTF-8 bytes <=8192 only on explicit
native share admission, outside generic JSON serialization/status. Ticket request
is bounded; actual provider epoch/cancel/deadline and registered live invitation
checks apply. Failure returns NULL/0 with no proof-bearing error payload. Original
Zeroizing String drops after copying to the owned C buffer, which om_mobile_free
wipes. Caller must always free it and clear later copies where possible; Swift/
Java/UI/clipboard/share-service copies are not guaranteed erased. No caller/UI or
build/test/native operation ran. Product sharing lifecycle remains unfinished.

iOS host begin now consumes the original core/owner/cancel-bound identity setup,
selects an installed origin and uses shared bounded admission plus the earlier
setup/operation deadline. Successful replies require exact local identity key/
revision, canonical nonzero ticket/invite IDs, future integral expiry and actual
false registered/share flags. Main delivery rechecks owner/setup/deadline and
owned request Data clears on worker return. Setup refuses both pending roles.
No automatic register/share or renderer route. No build/test/runtime operation
ran. Host setup views, sharing, remaining peer/candidate callers and durable
recovery remain unfinished.

Android native host begin now consumes the retained Active-identity setup once,
selects an installed origin and uses shared bounded admission plus the earlier
setup/operation deadline. Successful response must match local identity key and
revision, canonical ticket/invite ID, future integral expiry and explicit false
registered/share flags. Delivery rechecks owner/cancel/deadline; input clears on
worker return. Setup now refuses both pending host and client flows. No automatic
registration/sharing or renderer route. No build/test/native operation ran; host
setup UI, sharing and remaining peer/candidate caller integration remain due.

iOS native status now admits canonical pending-host ticket and actual boolean
expiry/local-close/remote-cleanup flags. It exposes a separate close invitation
action even without credential records, explaining local stop, conditional
service cleanup and nonrevocation of existing pairs. Confirmation invokes the
typed host close caller with owner/lifecycle/expiry admission. Uncertainty directs
back to status; expired or stale-revision cleanup still needs unfinished recovery.
No build/test/UI render/runtime operation ran for this checkpoint.

Android native status now admits canonical pending-host ticket and boolean
expired/local-close/remote-cleanup flags, offering a separate close invitation
action even when no credential records exist. Confirmation explains local stop,
conditional service cleanup and nonrevocation of existing pairs. It invokes the
bounded host close caller; uncertainty directs back to status. Expired remote
invitations can still require unfinished reconciliation; the UI does not pretend
that retry guarantees success. No build/test/UI render/runtime operation ran.

iOS typed host ticket caller now shares bounded pairing admission and original
queue-inclusive deadline. Close rotates cancellation delivery token and requires
immediate native epoch acknowledgment before queueing. Reply checks bind ticket,
registration/close booleans, or bounded unique peer snapshots with exact key/hash/
comparison text and bounded control-free labels. Main delivery rechecks owner,
deadline and token. Owned request Data clears on worker return; no automatic poll,
renderer route or sharing is introduced. Close still needs foreground provider.
No build/test/native operation ran; other host callers/views and durable recovery
remain unfinished.

Android native-only host ticket caller now uses shared ordinary pairing admission,
bounded queue and original 25-second deadline. Close rotates delivery fence and
requires immediate native epoch acknowledgment before queueing. Reply admission
checks exact protocol/ticket, register flags, bounded unique peer snapshots with
canonical keys/hashes/comparison and bounded labels, or explicit close semantics.
Never-registered close is distinguished from actual relay acknowledgment. Main
delivery rechecks owner/fence/deadline; buffers clear on worker completion.
No automatic polling or renderer route. Close still requires foreground provider
and a live owner; this is not background cleanup. No build/test/runtime operation
ran. Other host callers/UI and durable recovery remain unfinished.

Android private nativePairingHostV1 now dispatches begin/register/poll/close,
peer persist/decline and saved host candidate review/activation to separate Rust
ABI entries. Action/budget/input bounds precede array copy; native vector is
cleared through existing RAII on every return. Java declaration and shrinker
signature match. No renderer/nativeCall route or share proof is added. This
bridge has no Java operation caller/UI yet and is not runtime evidence. No
compiler/build/test or native operation ran; caller lifecycle/reply validation,
product host views and durable cleanup remain unfinished.

Confirmed reviewed-host activation now advances an existing invitation's expected
journal revision only when it exactly matched activation's expected revision.
The new revision comes from the successful leased writer, allowing subsequent
live invitation operations after activation. Already-stale invitations and
uncertain activation errors remain unreconciled; no guessed revisions or replay
authority are introduced. Mobile host candidate ABI now uses the mutable host
guard for this update. No build/test or native runtime operation ran.

Dedicated host_candidate ABI now routes strict <=1024-byte review and reviewed
activation requests to the corresponding host-role LocalHost methods. Activation
requires canonical reviewed hash; both derive actual foreground provider epoch,
validate scoped address access before/after and share original bounded deadline
and cancellation checks. Review grants no consent; activation still requires
actual native peer/input approval and grants no transport/session authority.
Late errors require journal reconciliation rather than rollback assumptions.
No JNI/Swift caller/UI, compiler/build/test or runtime operation ran. Full host
integration, durable invitation cleanup and end-to-end qualification remain due.

Dedicated host_peer ABI now accepts bounded native-only ticket/client-key/
reviewed-hello-hash requests. Persist requires explicit input boolean and lifetime;
decline refuses supplied permission choices. Exact client key is parsed, hash
shape checked, actual provider epoch/cancellation derived and original deadline
rechecked after controller work. Late errors explicitly require reconciliation;
success does not activate or remotely approve the candidate. Native UI must still
perform real peer/comparison/input consent. No JNI/Swift UI caller or build/test/
runtime operation ran; full host integration and durable recovery remain due.

Dedicated mobile host_begin/host_action ABI entries now accept strict bounded
native-only JSON: begin exact identity/revision/audience/label <=1024, ticket
actions <=128 for register/poll/close. Actual foreground attached provider supplies
epoch and cancellation; original budgets include mutex wait and post-operation
checks. Begin validates identity access before/after. No renderer route, share
proof or consent/activation is exposed. Close currently requires foreground
provider admission, so it is not a background/crash recovery solution. Late errors
direct native status reconciliation. No compiler/build/test/runtime operation ran;
JNI/Swift/product host UI and durable cleanup remain unfinished.

Saved candidate review now shares leased validation for distinct client/host
roles. Host review requires exact local host key and installed host/input/origin
policy, MaterialReady/no-session state, live signed grant/hash and both vault
records. Public metadata adds client key/fingerprint and explicit local_role to
existing host metadata; missing old comparison remains explicit. LocalHost wraps
host review with original deadline/cancel checks. Review is not relay approval,
consent or activation. No ABI/UI, build/test or runtime operation ran. Durable
recovery and full host product qualification remain unfinished.

Reviewed host activation now has a LocalHost/writer entry requiring exact stored
candidate address, journal revision and reviewed grant hash. The retained writer
lease spans hash admission and existing host-role/installed input policy/vault/
fresh relay approval checks before Active publication. Response binds hash and
address/revision, requiring status reload. Actual native host peer/input consent
is still required; no transport authority is created. No ABI/UI caller is wired
and no build/test/runtime operation ran. Host grant rereview/publication metadata,
durable recovery and full qualification remain unfinished.

Explicit native host_share now reloads the exact Active identity/revision under
lease, verifies invitation host binding and installed policy, then asks the
registered live invitation for share_code. LocalHost checks ticket/epoch and
original lifetime/cancellation before and after. Return type remains Zeroizing
String, outside public JSON/status/diagnostics. This only clears owned Rust
allocations; future OS share/clipboard/UI copies require separate lifecycle
handling and cannot inherit that guarantee. No ABI/share UI is wired and no
build/test, vault/crypto/network operation ran for this source checkpoint.

Host controller now records registration_attempted before entering the writer's
potential HTTP registration path. It remains sticky on every later error. A
never-attempted invitation can be locally fenced/dropped without provider/relay
access; its receipt explicitly has relay_close_confirmed:false, remote_close_required:
false and reason never_registered. Any attempted path still requires exact remote
close confirmation, even if failure actually preceded HTTP. Native status reports
this conservative distinction. This marker is transient, not crash evidence.
No build/test or runtime operation ran; durable recovery and product wiring due.

Host decline controller now validates exact ticket/epoch/live invitation and
reviewed retained hello hash before calling the leased decline path. Bound success
returns peer/hash and pairs_revoked:false. Native status exposes pending-host
ticket/revision, monotonic expired flag and local-close state, without invitation
proof or signed request contents. remote_close_required is conservative even for
never-registered invitations; it does not prove relay state. This metadata is
transient and does not implement crash recovery. No build/test/runtime operation
ran. Mobile status UI does not yet consume pending_host; ABI/UI integration due.

LocalHost host_prepare_peer now admits exact ticket/epoch/original invitation
lifetime and forwards the retained peer hello hash plus explicit input/lifetime
choice to the leased writer. Confirmed success updates the invitation's expected
revision from the actual candidate result and returns exact address/revision,
reviewed hello/client and relay_approved:false. Failure never guesses a revision
or clears the attempted-peer fence; uncertain journal writes still require the
unfinished reconciliation path. Actual native peer/comparison/input consent must
precede this entry. No ABI/UI, build/test or runtime operation ran. Full host
integration, recovery and qualification remain unfinished.

LocalHost host_poll now connects explicit native ticket/epoch admission to the
leased host writer. It returns client public key, untrusted label, comparison
code and exact retained signed hello hash for each bounded request snapshot,
plus original revision and local_consent_required:true. Final deadline/cancel
checks prevent late review publication. It neither signs a grant nor approves
input or a peer. No automatic polling is added. No build/test/runtime operation
ran; host consent/persistence controller, ABI/UI and qualification remain due.

Host controller close now checks exact ticket then irreversibly fences local
sharing/approval and clears invitation proof/request snapshots before cleanup
policy/provider/lease admission can fail. It uses installed retirement origins
and leased Active identity for exact relay close; uncertainty retains the closed
invitation owner for retry. Only confirmed completion clears the controller slot,
with an explicit pairs_revoked:false receipt. No epoch replacement creates a new
invitation during cleanup. Revision changes, expired invitation and identity
retirement can still require unimplemented reconciliation. This is not durable
crash cleanup. No build/test or native/network operation ran; ABI/UI remain due.

LocalHost now authors host_begin and host_register around one retained invitation
with native ticket, identity/revision/epoch and original monotonic lifetime.
Begin creates locally only; register reloads the exact identity under lease and
retains the invitation on uncertain outcomes. A second begin refuses while any
host invitation remains, rather than silently replacing possible relay ownership.
No share proof is returned by these entries. Detach/close currently drops this
transient owner; durable remote close/recovery is not implemented, so controller
registration must not be exposed as a finished product flow yet. No ABI/UI calls
or build/test/runtime operations ran. Close/reconciliation and durable invitation
ownership recovery remain required before accepting full host integration.

prepare_reviewed_host_peer now connects exact retained hello review to host grant
creation and candidate persistence under one lease. It checks reviewed hello
hash, installed host/input/audience policy, Active identity/revision, cancellation
and original deadline. Existing invitation attempted-peer fence precedes key/grant
creation; any subsequent failure requires reconciliation, not regeneration of a
competing grant. Success remains MaterialReady before remote approval. A late
cancel explicitly reports possible commit. This does not prove actual native
peer/comparison/input consent. No build/test/runtime operation ran; product host
controller, consent UI, durable uncertainty recovery and qualification remain due.

Leased writer now provides decline_host_peer and close_host around retained
invitation state and exact Active identity/revision. Decline requires enabled
host/origin policy and uses the existing local-before-remote peer decline fence.
Close uses installed retirement origins (new-session enablement is not required)
and existing terminal local close/proof clearing before remote close admission.
Both recheck lease/deadline/cancellation before reporting success. Caller must
retain uncertain invitation state for exact retries; close does not revoke pairs.
These are live Active-identity paths, not crash/revoking-identity recovery. No
build/test, vault/crypto/network operation ran. Host controller/UI and durable
invitation cleanup remain unfinished.

Writer poll_host now reloads the exact Active signing identity under its retained
lease and requires installed host/origin policy before one bounded invitation
poll. Existing native invitation code verifies signed request snapshots and
comparison codes. Lease/deadline/cancellation are rechecked before publishing
reviews. Peer labels and keys remain unapproved; this does not sign a grant,
persist a candidate or activate anything. Caller must retain invitation state on
uncertainty. No build/test or network/vault/crypto operation ran. Host controller,
consent UI and complete qualification remain unfinished.

NativeCredentialWriter now provides begin_host and register_host under its
retained profile lease. Creation requires installed hosting/origin policy and
exact Active identity/revision, then signs a local invitation without network or
sharing. Explicit registration reloads that identity, checks installed audience,
cancellation and original deadline around relay registration and lease checks.
Caller retains the invitation on uncertain outcomes; share_code still requires
actual registration state. These methods create no peer/input consent or Active
pair. No build/test, crypto/vault/HTTP operation ran. Product host controller,
ABI/UI and qualification remain unfinished.

Host candidate persistence and host activation now require enabled installed
hosting policy, installed grant audience and remote-input permission when the
signed grant includes input_allowed. Refusal occurs before exchange consumption/
pair vault write in host preparation and before relay approval in activation.
This does not grant capture/input authority or replace actual native consent.
Installed policy remains unchanged/disabled. No build/test/runtime operation ran.

iOS MaterialReady pair details now offer saved-pair rereview when provenance
exists and no session recovery debt is present. A fresh review snapshot feeds
two native dialogs: independent host-key verification (explicit old comparison
unavailability), then permission/expiry/generation review and one-use activation.
The same owner and capped monotonic review deadline cover both stages; lifecycle
and protected-data admission remain. No connection starts. No build/test, UI
render or native runtime operation ran. Native host product flow, actual producer
retirement/recovery ownership and full qualification remain unfinished.

iOS saved-peer review helper now requires a matching MaterialReady record from
native status, verified provenance, no session debt and a live snapshot. Validated
reviewSaved output becomes immutable metadata with private address/revision/hash,
original core/owner/cancel binding and monotonic lifetime capped at 60 seconds.
Separate activation consumes it once and uses existing reviewed activation ABI
and admission predicates. Fresh independent host/permission consent is still
required; no old comparison is invented. Resume view remains unwired. No build,
test or native runtime operation ran for this source checkpoint.

Android MaterialReady pair details now expose a separate saved-pair review action
when storage provenance is present and no durable session debt exists. Fresh
leased review opens a native dialog showing host key/fingerprint, service, pair,
generation, permission and expiry. It explicitly says the old comparison code
is unavailable. Separate unchecked independent-host and permission confirmations
gate one-use activation. Owner/expiry/lifecycle checks remain, and failure directs
back to status; no connection starts. This is authored UI, not runtime evidence.
No build/test or native operation ran. iOS saved-pair UI and full qualification due.

Android saved-peer review now requires an exact MaterialReady pair record from
the provided native status snapshot, valid storage provenance, no session debt
and a live snapshot deadline. Validated action-6 output becomes an immutable
SavedPeerReview retaining host/pair/input/expiry/audience and private address/
revision/hash, original owner and cancellation fence. Its separate activation
helper consumes once and rechecks both owners/fence/monotonic review lifetime
(at most 60 seconds). UI must obtain fresh independent host and permission consent;
no old comparison code is available or invented. No build/test/runtime operation
ran. Native resume dialog and iOS equivalent remain unfinished.

iOS native-only reviewSaved now routes the dedicated saved-client review ABI,
not generic ticket actions. Request bounds and canonical address/revision checks
match activation scope. Response checks exact address/pair/revision, MaterialReady,
canonical host/fingerprint/grant hash, actual boolean input permission and fresh
consent/unavailable comparison flags, plus future integral safe-range expiry.
Existing owned Data cleanup, queue/deadline/owner/cancel predicates apply. No
activation capability or user consent is inferred. Typed resume snapshots/views
remain due; no compiler/build/test or runtime operation ran.

Android native-only pairing action 6 now routes the separate candidate-review
ABI. Its bounded serialized lane validates request scope/address/revision and
response exact address/revision/pair, MaterialReady phase, host/hash formats,
boolean permission, future integral expiry and comparison_available:false with
local_consent_required:true. Existing deadline/owner/cancel fences and byte
clearing apply. This does not create an activation snapshot or imply consent.
Typed resume UI and iOS routing remain due; no build/test/native operation ran.

Dedicated om_mobile_pairing_review_client_v1 now accepts exact native-status
address/revision with a bounded original budget. Actual foreground provider epoch
and cancellation are derived natively; address access is checked before/after.
LocalHost requires enabled installed policy and rejects saved grant audiences no
longer configured. The leased writer reloads the candidate and returns only
review metadata; no relay HTTP, activation, consent or session authority results.
Missing old comparison remains explicit. No compiler/build/test or runtime
operation ran. JNI/Swift callers and resume consent UI remain unfinished.

The writer now authors review_client_candidate for interrupted MaterialReady
client records. Under the scoped lease it checks exact revision/address, absence
of durable session ownership, signed live grant/client role, retained grant hash,
Active local identity and both vault records. Public result includes original
host/fingerprint/pair/input/expiry/audience/hash; no secrets or authority escape.
Original invite/hello comparison text is not durably retained, so this explicitly
returns comparison_available:false. A future resume UI must independently verify
the host and obtain fresh consent; it cannot claim the original comparison was
reconstructed. Controller/ABI/UI wiring is still absent. No build/test or vault,
crypto, SQL or network operation ran for this source checkpoint.

Consented client persistence now returns grant_hash alongside pairing_ticket.
LocalHost sources it from the reviewed hash already checked against the retained
signed grant before exchange consumption. Both mobile lanes require exact request
hash equality; activation factories additionally compare the result to their
original peer snapshot. This closes reply correlation across poll, persistence
and activation preparation, without adding consent or transport authority.
No build/test or native runtime operation ran for this follow-up.

iOS successful peer persistence now issues at most one activation snapshot from
the consumed peer review, checking exact ticket/pair/profile, candidate generation,
MaterialReady phase and bounded revision. Snapshot retains copied address/revision
and original grant/owner/core/cancel/expiry. A separate native dialog displays
host, pair, generation, input permission and grant expiry; its explicit positive
action consumes activation once and calls the reviewed ABI. Dismissal leaves
saved state for reconciliation. No connection/session starts. No compiler/build/
test, UI render or native operation ran. Interrupted-candidate rereview, native
host flow, retirement ownership and end-to-end qualification remain unfinished.

iOS native-only client lane now dispatches typed action activate to the separate
reviewed activation ABI, not generic ticket actions. Input <=1024 requires exact
profile/pair-kind canonical address IDs, bounded integral revision and canonical
reviewed hash. Response admission checks exact address/hash, Active phase, true
reload flag and advancing bounded integral revision. It shares existing owned
Data cleanup and queue-inclusive deadline/owner/cancel/snapshot predicates.
No build/test or ABI/vault/crypto/network operation ran. One-use iOS activation
snapshot/UI and full product qualification remain unfinished.

Android successful peer persistence now creates at most one immutable activation
snapshot from the consumed peer review. Factory checks exact ticket/pair/profile,
candidate generation, MaterialReady phase and bounded revision; the snapshot
retains copied address/revision and original reviewed grant/fence/expiry. Native
UI explicitly displays host/pair/generation/input/permission expiry and asks for
activation. Only the positive action consumes its one-use activation capability
and invokes JNI action 5. Later/dismissal leaves persisted state for reconciliation,
not automatic activation. No transport starts. No build/test or runtime operation
ran. iOS activation and rereview/resume of saved candidates remain unfinished.

Android private pairing JNI action 5 now calls the separate reviewed activation
ABI with <=1024-byte input, retaining the existing wipe-on-return native vector.
The Java native-only lane validates request scope/canonical IDs/hash/integral
revision and checks exact five-field reply address, grant hash, Active phase,
reload_required and advancing bounded revision. It shares ordinary-operation
admission, queue-inclusive deadline, owner/cancel delivery fences and input clear
behavior. No arbitrary renderer dispatch is added. Typed one-use activation
snapshot/UI and iOS equivalent remain due; no compiler/build/test/native operation
ran for this source checkpoint.

The dedicated om_mobile_pairing_activate_client_v1 entry accepts native-only
address/expected_revision/reviewed_grant_hash JSON <=1024 and an original bounded
budget. It derives actual foreground provider epoch/cancellation, validates
address access before/after and reports uncertain late commit explicitly.
LocalHost requires enabled installed origins. Under one retained scoped lease,
the writer checks exact journal revision/address/grant hash, then follows existing
MaterialReady/client-role/vault/fresh relay confirmation checks before Active.
Active publication still grants no transport/session authority. No build/test,
vault/crypto/SQL/HTTP operation ran. JNI/Swift activation callers and UI, native
host product flow, retirement ownership and end-to-end qualification remain due.

iOS native menu now wires installed-relay selection and invitation/independently
verified host-key/device-label entry to ClientSetup. UITextField delegate bounds
UTF-8 input; invitation uses secure text entry with correction/spelling disabled.
Fields clear on submit/cancel/lifecycle dismissal; Swift/UIKit/IME/JSON aliases
remain outside guaranteed erasure. Explicit host-verification action precedes
begin, and another action sends join. The waiting view checks approval only on
user request and retains the flow for unconfirmed poll outcomes; local cancel
remains available. Original setup and invitation/review timers remain in effect.
No build/test, UI render, vault/crypto or network operation ran. Activation,
native host product flow, producer/recovery ownership and qualification remain.

iOS now prepares a native-only one-use ClientSetup from validated status: exactly
one nonretired Active identity, verified storage provenance, no live pending
client and bounded installed HTTPS origins. It retains original identity/revision,
core/owner/cancel binding and queue-inclusive 30-second setup deadline. Begin
serializes only retained scope plus UI-confirmed host/invitation/origin/label;
Swift/UIKit/JSON string copies remain outside wipe guarantees. Setup and peer
snapshot predicates are checked before ABI and at post-response/main admission.
An already-issued operation can still commit before late rejection; reconcile
status rather than assuming rollback. No build/test ran; iOS setup UI/join,
activation and full product qualification remain unfinished.

Android's native hub menu now wires ClientSetup to an invitation/independent
host-key/device-label form and installed relay selector. Input is length bounded,
invitation field uses password input, disables state saving/autofill and clears
on send/dismiss. Java/UI/IME strings are not reliably erasable; no stronger wipe
claim is made. Explicit host-key verification precedes begin; a second action
sends JoinInvite. The waiting view polls only on a user action and retains its
owner for pending/unconfirmed poll results; explicit local cancel is available.
Original setup/review/invite deadline fences and lifecycle dismissal remain.
This is authored UI source only: no compilation/build/test, UI render, crypto,
vault or network operation ran. iOS full setup/begin/join, native host controller,
activation and end-to-end qualification remain unfinished.

Native status now returns enabled installed pairing_origins, empty when policy
is disabled/invalid. Android's native-only ClientSetup helper validates status,
requires exactly one nonretired Active identity, refuses missing storage
provenance or another live pending flow, and retains immutable configured relay
choices plus original identity/revision/fence and a 30-second deadline. It never
creates/replaces identity. Its one-use begin helper builds the request from that
retained identity and selected installed origin; UI must independently pin host
and clear the code field. Immutable Java/JSON/UI string copies cannot be wiped
reliably; owned ABI bytes retain the existing clear-on-return behavior. No build,
test, crypto/vault/network operation ran. Setup UI, iOS helper and full flow due.

LocalHost now drops a pending client exchange once its original monotonic
deadline has expired when native status is read or join/poll/persistence is
attempted. This releases the retained zeroizing invitation proof and exchange
owner; it does not mutate the durable journal/vault or acknowledge relay cleanup.
No automatic timer is introduced: an idle live host may retain the expired
object until the next relevant entry or host detach/close. Status's Rust receiver
is now mutable solely for this transient cleanup. No build/test ran.

Both mobile peer-permission dialogs now display the exact retained grant's
wall-clock expiry using local date/time formatting, separate from the existing
monotonic review deadline capped at 60 seconds. iOS retains this Date in the
immutable snapshot; Android uses its retained expires_at_ms. Save/result labels
use pairing language while still explaining activation is separate. The journal
phase remains MaterialReady. No build/test or UI render was performed.

Android/iOS native status menus now offer explicit local cancellation of the
pending ticket, with a separate confirmation that remote-approved pairs and
saved candidates are not revoked/deleted. Confirming invokes the existing
immediate epoch invalidation and queued exact-ticket cancel path. A missing or
uncertain result directs the user back to status. Existing flow expiry/lifecycle
dismissal remains; closing a dialog is not a remote revocation receipt. No build,
test or runtime UI operation ran for this source checkpoint.

iOS status now retains an unexpired pending client ticket and offers a native
review action. A fresh approved poll opens host key/fingerprint/comparison/pair
review; an explicit verification action proceeds to a separate input-permission
confirmation. Only the final action invokes one-use candidate persistence.
Both stages retain the same snapshot/owner and expiry, with foreground,
protected-data and lifecycle dismissal checks. This source UI has not been
compiled, displayed or tested. Full begin/join entry, activation and end-to-end
qualification remain unfinished; no runtime or network operation ran.

Android native status/recovery now offers review of an unexpired pending-client
ticket from native status. An explicit fresh poll creates the one-use approval
snapshot; the native dialog displays its host key/fingerprint/comparison, pair
and input permission. Separate unchecked peer/comparison and permission boxes
gate the save button. Owner/cancel/expiry checks and existing lifecycle dismissal
remain in effect; dismissal is not advertised as remote revocation. Confirmation
persists MaterialReady only and directs the user back to status. This is authored
UI wiring, not evidence of displayed/runtime behavior. No build/test ran. Full
begin/join entry, activation, iOS consent views and end-to-end qualification remain.

Client poll replies now carry the original independently pinned host key,
its native-derived fingerprint and the retained invite/hello comparison code.
These come from the same pending exchange as the approved grant, not mutable
UI input. Android/iOS approved-poll snapshots retain this metadata immutably
alongside pair/input permissions so the eventual view can display the exact
peer/comparison context. Text format validation does not itself prove user
comparison or consent. Actual view wiring and build/runtime tests remain due;
none ran for this source checkpoint.

iOS now mirrors the approved-poll snapshot/confirmation helper using immutable
public metadata, private ticket/hash, original core identity, owner/cancel fence,
monotonic lifetime capped at 60 seconds and a locked one-use consume operation.
Main-actor review/confirm helpers accept only validated approved polls, construct
the consent request from retained fields and recheck snapshot validity before
delivering persistence results. These helpers do not display or prove native
peer/comparison/input consent; actual views and activation remain due. No Swift
compiler/build/test, vault, crypto or HTTP operation ran for this checkpoint.

Android NativeCore now exposes an immutable native PeerApproval snapshot created
only by the validated explicit poll helper. It retains exact ticket/grant hash,
pair ID/input permission, cancellation delivery fence and a monotonic review
deadline capped at 60 seconds and observed grant lifetime. Confirmation checks
the live owner/fence/deadline, atomically consumes the snapshot once, and builds
action 3 from its private retained ticket/hash; failures require rereview or
journal reconciliation instead of replaying the same consent object.
This helper does not prove user consent or display host/comparison information.
Actual native dialog wiring must collect those confirmations; iOS equivalent,
activation and runtime qualification remain unfinished. No build/test ran.

Approved client poll now returns the SHA-256 document_hash of its retained signed
grant. Trusted native action 3 requires ticket and reviewed_grant_hash (strict
JSON <=256 bytes); other ticket actions remain <=128 bytes. Before taking the
pending exchange, LocalHost compares the supplied hash with the current approved
signed grant. Missing or changed approval fails without consuming the exchange.
The hash binds all signed grant fields, including pair ID, input permission and
expiry. A hash is not proof of user consent: native UI must retain it with the
displayed metadata and send it only after actual peer/comparison/input approval.
Both mobile callers admit canonical hash text; JNI bounds match the Rust ABI.
No compiler/build/test, crypto execution, vault or HTTP operation ran. Product
consent UI, activation, recovery and end-to-end qualification remain unfinished.

iOS client pairing now mirrors Android delivery fencing with NSLock-protected
object-identity tokens. Cancel rotates the token before native invalidation and
queue admission; ordinary work checks it before ABI, after response and at main
delivery. No lock is held across ABI or callback execution. Native UI entry
points must serialize UI transitions on main; concurrent cancellation cannot
retract a callback already executing. No build/test or runtime race qualification
ran; this fence neither rolls back persisted candidates nor revokes remote peers.

Android pairing cancel rotates an atomic object-identity delivery fence before
requesting native invalidation. Ordinary operations capture the fence and check
it before JNI work, after the reply and on main delivery. A cancel attempt thus
invalidates already-produced replies even if native acknowledgment or queue
admission fails. This is delivery admission only, not rollback or remote revoke.
Actual UI calls must serialize on the main thread for UI transition atomicity;
an arbitrary concurrent cancel can race a callback already executing. iOS
equivalent fencing and runtime qualification remain due; no build/test ran.

Mobile begin replies now receive comparison-code, host key/fingerprint format,
future safe-range integral expiry and bounded revision checks. These format
checks do not independently pin the host or verify fingerprint derivation;
native transcript validation and the future consent controller retain those
responsibilities. No build/test ran for this follow-up.
Swift now uses full ASCII byte validators for these strings and candidate keys;
terminal newlines, Unicode substitutions and noncanonical pad bits are refused
by the authored predicates. Compiler/runtime qualification remains deferred.

Join response admission requires pending state and `approves_peer:false`.
Poll admission requires `local_consent_required:true` and a known pending,
declined or approved state. Approved metadata requires canonical nonzero pair
UUID, actual boolean input permission and integral future expiry within JSON's
safe integer range. Cancel admission requires `cancelled:true` and
`remote_revoked:false`. These are source-only metadata checks, not native user
consent or relay/transport activation. Build/test remain deferred.

Mobile candidate reply admission additionally checks canonical nonzero address
UUIDs, integral revision 1..10,000,000 and unpadded canonical base64url text for
the 32-byte public key (including final-character pad-bit constraints). These
format checks do not establish expected device/pair/key identity: the eventual
typed consent controller must bind those values to the reviewed transcript.
No compiler/build/test was run for this source-only follow-up.

Android client cancel now checks the boolean native epoch invalidation result
before queuing local ticket cleanup. A missing handle or refused acknowledgment
clears owned input and reports that status reconciliation is required. A queued
cleanup failure still cannot undo an already acknowledged epoch invalidation.
This source change aligns with iOS; runtime cancellation qualification is pending.

Client candidate responses now include `pairing_ticket` from the exact consumed
pending controller entry. Android/iOS require this to match the requested ticket
before accepting MaterialReady metadata. This correlation does not imply relay
activation, transport authority or undo a candidate committed before cancellation.
This follow-up was authored without running build or tests.

- Journal/executor/lease/writer product integration, authority epochs, actual native owner
  and startup recovery execution,
  rotation/re-pair and local/relay revocation orchestration.
- Native pairing-controller integration of the authored ECDH/HKDF transcript
  exchange and authored signal AEAD/replay metadata into a real transport,
  compatible with the current signed protocol; no secret IPC export.
- Broker operations and document-scoped native capabilities, with explicit
  native opt-in and locking/background/expiry behavior.
- Android/iOS attachment and protected-data device qualification,
  scoped callback consumption and mobile lease/product lifecycle qualification. Existing
  save-sync callbacks are not reused as pairing storage.
- Device-lock, protected-data, backup/restore/reinstall, profile deletion,
  missing vault and lost-key behavior. Native identity recovery must not silently
  replace the identity pinned by remote peers.
- UI/transport migration: non-extractable browser keys cannot be read into this
  module. A fresh native identity requires explicit re-pairing or a separately
  specified authenticated migration, without falsely preserving peer trust.
- Source review and actual build/test/platform qualification of all providers,
  wrong bindings, corrupt/missing/locked records, slow I/O, candidate conflicts,
  concurrent writes, failed deletion, crashes and revocation races.

All earlier incomplete items and the original 51–109 scope remain open. The
existing streaming policy is unchanged and disabled.
