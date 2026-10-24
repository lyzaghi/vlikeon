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
lock/reparse/contender/crash qualification are still due. Other platforms refuse
lease acquisition until their scoped provider is implemented; no plaintext or
process-local mutex fallback is supplied.

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

## Remaining implementation and qualification

- Journal/executor/lease/writer product integration, authority epochs, actual native owner
  and startup recovery execution,
  rotation/re-pair and local/relay revocation orchestration.
- Native pairing-controller integration of the authored ECDH/HKDF transcript
  exchange and authored signal AEAD/replay metadata into a real transport,
  compatible with the current signed protocol; no secret IPC export.
- Broker operations and document-scoped native capabilities, with explicit
  native opt-in and locking/background/expiry behavior.
- Android candidate JNI/bridge wiring, iOS candidate target/bridge/protected-data wiring,
  scoped callback consumption and mobile lease/product lifecycle. Existing
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
