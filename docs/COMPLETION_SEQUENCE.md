# User-ordered completion sequence

## Current sequence — 1–109 outstanding tasks

Source checkpoint: progressive H.264 reference marking now implements short/long-term entries, sliding eviction, IDR and MMCO 1–6 with staged, version/parameter-bound one-use commit. It is a frame-reference component, not full DPB/output/decode admission; field/MBAFF, reference lists/POC/gap handling and backend wiring remain open. No build/test executed.

Source checkpoint: receiver admission now has a concrete parameter/syntax pipeline joining Annex B, wire-order SPS/PPS installation, versioned dependency resolution, picture/slice/auxiliary parsing and exact queued-AU proof binding. The native backend's negotiated codec/DPB/owner implementation remains mandatory and unfinished; there is no default-success route. Build/test remain deferred.

Source checkpoint: primary-picture parsing now consumes actual auxiliary NAL syntax instead of skipping it. AUD type is matched to slices, SEI envelopes retain bounded payloads, filler/end-marker syntax and picture ordering are enforced. SEI semantics, DPB, actual decoder admission and product transport integration remain open. Build/test stay deferred.

Source checkpoint: receive-bridge pending frames now retain original AU metadata plus a native admission proof. A required provider recheck runs before each decoder attempt/retry, followed by original-age and decoder-authority checks. It must not repeat DPB/reference mutations or revive replaced parameter versions. No admission provider, transport enablement or runtime qualification is supplied; build/test remain deferred.

Source checkpoint: native video receiver now forwards caller retirement deadlines into the decoder and allows explicit reaping retries of its same CleanupRequired owner. Buffered data is fenced first and never reopened; only a timely confirmed Stop/process/I/O result publishes Revoked. This integrates the previously authored decoder deadline at the receive-bridge boundary, not aggregate session retirement. Build/test remain deferred.

Source checkpoint: H.264 primary-picture header grouping now rejects mixed picture/parameter identities, conflicting reference-marking/FMO/SP state, duplicate starts and inconsistent slice-type/IDR metadata. It checks in-band parameter bytes against retained versions. This is header consistency only, not complete AU coverage, entropy/DPB or decoder admission; redundant/separate-plane pictures need a later coordinator. `NATIVE_H264_PICTURE_HEADERS.md` records limitations. Build/test remain deferred.

Source checkpoint: ordinary H.264 slice-header parsing now continues the version-bound prefix through bounded reference edits/weights/marking, CABAC initialization/alignment, QP/QS, deblocking and FMO cycle syntax. It does not establish DPB reference availability, full picture coverage or entropy validity. `NATIVE_H264_SLICE_HEADER.md` records limits and remaining decoder admission work. Build/test remain deferred.

Source checkpoint: ordinary H.264 slice identity prefixes now resolve exact retained PPS/SPS versions and parse bounded macroblock/type/frame/field/IDR/POC/redundancy metadata. A lazy borrowed reader consumes only needed header-prefix bytes; it does not validate entropy data or the remaining header. Parameter bindings now compare table identity plus versions. `NATIVE_H264_SLICE_PREFIX.md` records missing full header/AU/reference/decoder work. Build/test remain deferred.

Source checkpoint: native H.264 parameter storage now retains exact encoded SPS/PPS with bounded versions, immutable resolution, SPS-dependent PPS invalidation and table-local stale-binding checks. Aggregate encoded/map/accounting quotas are preflighted before state changes; exact repeats do not advance versions. `NATIVE_H264_PARAMETER_VERSIONS.md` records scope and remaining AU/slice/IDR/negotiation work. No decoder admission or build/test execution is claimed.

Source checkpoint: ordinary H.264 PPS parsing now binds the exact supplied SPS ID and retains bounded slice-group types 0–6, entropy/quantization/prediction/reference defaults and optional transform/scaling syntax. `NATIVE_H264_PPS.md` records geometry/allocation limits and missing table/version/slice/semantic work. No complete receiver admission or decoder wiring is claimed. Build/test/parser/media execution remain deferred.

Source checkpoint: ordinary H.264 SPS parsing now retains bounded geometry/crop, chroma/depth/scaling, POC/reference state and VUI/HRD/timing/restriction metadata through exact trailing-bit consumption. `NATIVE_H264_SPS.md` records recognized syntax, resource caps and missing semantic/negotiation checks. This is not full decoder admission; PPS/slice/dependency checks and actual bridge wiring remain open. No build/test/parser/decoder execution ran.

Source checkpoint: native H.264 parameter-set parsing now has a bounded SPS/PPS RBSP bit-reader primitive with checked emulation-prevention removal, sticky read failures, caller-bounded Exp-Golomb fields and exact trailing-bit consumption. It does not yet parse full SPS/PPS/VUI semantics or implement the mandatory receiver admission interface. `NATIVE_H264_PARAMETER_BITS.md` records remaining integration and qualification. No build/test/parser/decoder/network operation ran.

Source checkpoint: gamepad parent transport now exposes deadline-bearing stop and supervisor revoke paths. Stop exchange, helper termination and I/O reaping share the original deadline; late/failed containment remains an error. Journal/arbitration writes are checked before/after and may still finish late, requiring reload/reconciliation rather than a success receipt. No slot/device ownership is released merely because the helper exits. Build/test and actual driver/device qualification remain deferred.

Source checkpoint: keyboard/mouse helper retirement now accepts one original caller deadline across Stop/release observation, graceful exit, forced process containment and I/O reaping. Broker revoke/close expose deadline-bearing paths and retain `CleanupRequired` on late/partial/missing-release results. Repeated cleanup may reap the same owner but cannot replace a missing release receipt with process exit. Build/test remain deferred; this is one component, not full session retirement.

Source checkpoint: Windows video helper retirement now accepts one caller deadline across Stop RPC, process wait and I/O reaping. Raw-frame, owned-capture and viewer-decoder broker wrappers expose `revoke_until`. Missing Stop acknowledgement, expired completion, pending threads or a failed join remain errors; a retry can reap the same owner but cannot manufacture an acknowledgement. This prepares component retirement only, not full session retirement or transport integration. Build/test remain deferred.

Source checkpoint: saved-pair recovery no longer asks mobile users to guess the local role. Native status derives optional host/client metadata from the historically verified signed grant, matching pair ID, document hash and the record's public identity. Android/iOS select only the corresponding re-review path; missing role metadata leaves cleanup readable without enabling resume. This supersedes the earlier manual role selector. Fresh vault/role/grant checks and explicit peer/permission consent remain mandatory. No build/test or transport enablement occurred.

Source checkpoint: Android saved-pair recovery now offers explicit host/client role selection. Host review/activation uses existing JNI actions 6/7 with bounded queue admission and original status/review deadlines, validates the exact record/address/revision and native role, and displays the receiving peer key/fingerprint for renewed independent verification. Permission confirmation remains required and activation consumes the typed review once. Client review now explicitly checks client role. No build/test, relay request, device qualification or release enablement was performed.

Source checkpoint: iOS recovery UI now exposes separate host/client re-review actions for material-ready pairs. Host re-review reloads through native candidate action 1, binds the originating record/public identity and exact journal revision, and displays the recipient key/fingerprint for fresh independent verification. Activation requires separate permission confirmation and a one-use review, then validates the exact address/grant hash/advanced revision. Old comparison codes are not recreated. Build/test and device restart qualification remain deferred.

Source checkpoint: iOS host permission flow now explicitly selects view-only/control and 1-hour/24-hour/7-day lifetime, confirms independent comparison, persists the exact reviewed hello, then separately offers activation bound to the returned address/revision/grant hash. Typed one-use activation validates native receipts and retains the original review deadline. Activation does not start streaming. Re-review/activation of saved host candidates after UI dismissal or restart remains open. Build/test remain deferred; none of this is runtime-qualified.

Source checkpoint: iOS host manual polling now produces typed, owner/fence-bound peer reviews with an original 30-second deadline. Native UI lists untrusted peer labels, shows public keys and comparison codes, and requires a separate confirmation before declining the exact reviewed hello. Decline consumes the review once, shares bounded native admission, and validates the matching relay receipt before reporting success. No permission persistence or activation UI was added in this checkpoint; those remain open. Build/test are deferred by user instruction.

Source checkpoint: iOS registered host invitations now offer explicit reveal through the borrowed native share API. A separate native text surface clears visible content on hide, invitation expiry, lifecycle cancellation, disappearance, and capture-state notification; it refuses initial reveal while screen capture is reported active. UIKit selection can copy the code; screenshots, clipboard history and immutable string aliases are outside wipe guarantees. No WebKit delivery or automatic pasteboard write was added. iOS host peer consent/activation UI remains unfinished. Build/test and device lifecycle qualification remain deferred.

Source checkpoint: iOS native host setup now exposes installed relay selection, bounded device label and independently shareable public identity, explicit local invitation creation, explicit relay registration, and invitation close. Setup and invitation retain their original deadlines and foreground ownership. Registration uncertainty directs the user to recovery rather than automatic retry. Host proof sharing, peer review, and activation UI on iOS remain unfinished. No build or test was run at the user's request; this is not runtime qualification.

Item 58 Android host persistence now leads to a separate one-use activation
snapshot/dialog, exact candidate/revision/grant checks and host JNI activation.
Only explicit confirmation publishes relay approval; no connection starts.
Source-only; interrupted host rereview, iOS flow and build/test remain due.

Item 58 Android host peer review now leads to explicit comparison consent,
default-off input choice and selected permission lifetime before one-use candidate
persistence. Native replies bind reviewed hello/client and retained grant hash.
Source-only; activation UI/iOS host flow and build/test remain due.

Item 58 Android host peer details now offer explicit exact-request decline,
consuming the review snapshot once and checking bound receipt metadata. Queue,
owner/cancel/deadline fences remain. Source-only; host approval/activation UI,
iOS flow and build/test remain unfinished.

Item 58 Android host UI now exposes manual peer polling and bounded immutable
request snapshots tied to ticket/hello hash/owner/fence/30-second review lifetime.
Unverified labels, keys and comparison are displayed without automatic consent.
Source-only; host approval actions/iOS views and build/test remain due.

Item 58 Android native menu now authors host label/relay setup, explicit local
creation, separate registration and explicit invitation-code reveal. Proof view
uses FLAG_SECURE, no state saving and text clearing on dismissal. Source-only;
peer approval UI, iOS host views and build/test/qualification remain due.

Item 58 new host creation now refuses retained durable invitation cleanup under
the scoped lease, including after process restart. Both mobile host-begin helpers
surface the same gate before consuming setup; client setup remains usable.
Source-only; no build/test/runtime operation ran.

Item 58 both mobile durable-cleanup views now allow explicit expired invitation
reconciliation through the native receipt branch. UI expiry only selects wording;
server-bound evidence remains required to clear intent. Source-only; no build/
test/deploy ran. Actual compatibility and full qualification remain due.

Item 58 durable restart cleanup now authors expired-invitation receipt validation
and exact intent completion under the same lease, after authenticated relay
absence confirmation. Native UI expiry guards remain to update. Source-only;
no build/test/network/SQL ran and relay changes are not deployed.

Item 58 relay source now authors authenticated expired-invitation reconciliation:
original host-signed invite, server expiry and transaction-verified live absence
produce an exact bound receipt without revoking pairs. Native receipt consumer
and UI remain unwired. Source-only; no SQL/network/build/test ran.

Item 58 iOS now connects durable invitation records to explicit restart-cleanup
confirmation and bounded recovery ABI, with exact snapshot/receipt admission and
immediate delivery fencing. Expired records remain visible without a completion
action. Source-only; no build/test/UI/network operation ran.

Item 58 Android now connects durable invitation cleanup entries to explicit
native confirmation and bounded JNI restart-close calls. Exact snapshot entry/
revision/receipt admission and immediate delivery invalidation apply. Expired
records remain visible without false completion. Source-only; no build/test ran.

Item 58 iOS status now retains typed immutable durable invitation cleanup records
bound to the same snapshot's nonretired signing identity, HTTPS audience and
integral expiry. Source-only; explicit recovery caller/views and build/test due.

Item 58 Android status now retains typed immutable durable invitation cleanup
metadata with exact local signing-generation binding, configured-shape HTTPS
audience and integral expiry admission. No action authority inferred. Source-only;
cleanup action UI and build/test remain due.

Item 58 registration retry now recognizes an exact durable invitation intent
without writing another revision. It can reconcile one potentially committed
intent revision only, while reloading the original Active signer. Arbitrary later
changes still refuse. Source-only; no build/test/network execution ran.

Item 58 mobile status attention now includes durable invitation cleanup entries
after restart, even without a transient host ticket. Bounded unique invite IDs,
hashes and expiry booleans are admitted. Source-only; cleanup UI/build/test due.

Item 58 native status now publishes bounded durable invitation cleanup metadata;
LocalHost/mobile ABI expose exact invite/revision restart close without resuming
pairing. Source-only; expired invites, mobile recovery UI and build/test remain due.

Item 58 writer now authors restart cleanup from exact durable invite ID/revision:
reconstructed invitation is permanently closed with no proof/request/exchange
state and uses existing bound close/completion. Only unexpired invitations are
supported yet. Source-only; controller/ABI/UI and build/test remain due.

Item 58 confirmed host close now removes only its exact durable invitation
intent under the retained scoped lease, after bound relay acknowledgment and
deadline/cancel checks. Uncertain results retain recovery debt. Source-only;
restart cleanup/expiry reconciliation and build/test remain due.

Item 58 host registration now commits signed invitation cleanup intent under the
same scoped lease before any HTTP and returns the committed revision to its
controller. Failure retains durable intent; source-only, no build/test. Intent
completion/restart cleanup/retry reconciliation remain unfinished.

Item 58 credential journal now authors bounded signed host invitation cleanup
metadata and transactional retain intent, with exact identity/signature/hash and
namespace validation. Identity recovery waits for retained invitation cleanup.
Not yet wired before HTTP; source-only, no build/test. Migration/recovery due.

Item 58 detach now fences host proof/reviews but retains possibly registered
cleanup ownership inside the same LocalHost, preserving signer-removal guard.
Never-attempted invitations drop. Process/terminal-host loss still needs durable
recovery. Source-only; no build/test/runtime qualification ran.

Item 58 LocalHost identity material removal now refuses while a retained host
invitation may require that exact signer. Both mobile status attention predicates
include pending host cleanup. Source-only; durable cross-crash retention and
full recovery/build/test remain due.

Item 58 invitation close now accepts the original retained Revoking identity
only for cleanup, requiring exact generation/pinned host key and non-absent vault
material under lease. No new pairing authority. Source-only; build/test/crash/
expired-invite reconciliation and durable identity retention remain due.

Item 58 host close now reloads current scoped journal revision under its lease
while requiring the original exact Active identity generation and invitation
signature binding. Unrelated revision advances no longer alone block cleanup;
peer consent paths remain exact-revision. Source-only; build/test/recovery due.

Item 58 raw host-share admission now stays reserved until the UI callback wipes
the retained proof, on both mobile platforms. Main-queue proof retention is thus
covered by operation bounds. Source-only; callback/race/build/test qualification due.

Item 58 host expiry now fences sharing and clears proof/request snapshots on
native status, capability maintenance and new-host entry. Possibly registered
public cleanup owner stays retained; never-attempted expired invitations drop.
Source-only; no build/test. Durable expiry/relay reconciliation remains due.

Item 58 iOS now authors explicit raw host-share caller retaining the Rust buffer
until a scoped UI byte borrow, then wiping/freeing it. Queue/deadline/owner/cancel
admission applies. Source-only; actual sharing UI/OS copy lifecycle/build/test due.

Item 58 Android now authors explicit raw-share caller with bounded queue/deadline,
owner/cancel admission and callback-borrowed proof wiped after UI callback.
Source-only; actual share UI/OS copy lifecycle and build/test remain due.

Item 58 Android now has separate raw share JNI with ticket/budget/output bounds,
OPRP1/base64url byte admission and RAII native buffer wiping. No String/clipboard
or renderer route. Source-only; lifecycle-owned caller/UI and build/test due.

Item 58 explicit host sharing now has separate raw-byte mobile ABI, avoiding
proof-bearing generic JSON results. Failure is empty; owned returned bytes wipe
on om_mobile_free. Source-only; OS/UI copy lifecycle and build/test remain due.

Item 58 iOS now authors bounded one-use host begin from retained Active identity,
matching local key/revision and explicit unregistered reply flags. Setup refuses
pending host/client flows. Source-only; UI/share/durable recovery/build/test due.

Item 58 Android now authors bounded one-use host begin from retained Active
identity setup, with exact local-key/revision and unregistered reply admission.
Setup refuses existing pending host/client flows. Source-only; UI/share/build/test due.

Item 58 iOS native status now retains validated pending-host ticket/flags and
offers explicit host invitation closure with existing bounded typed caller.
Source-only; no build/test. Full host views/durable recovery/qualification due.

Item 58 Android native status now retains pending-host ticket metadata and offers
explicit invitation-close confirmation alongside client/record actions. Source-
only; no build/test. Full host flow, durable cleanup and qualification remain due.

Item 58 iOS now authors typed host register/poll/close calls with bounded shared
admission, original deadline, close invalidation/fence and exact reply checks.
Source-only; no build/test. Remaining host callers/views and durable recovery due.

Item 58 Android now authors bounded serialized host ticket register/poll/close
calls, exact reply admission and immediate close invalidation/delivery fencing.
Source-only; no build/test. Begin/share/peer/candidate callers and host UI due.

Item 58 Android now has private versioned JNI host dispatch for bounded begin,
ticket, peer and candidate ABI actions with retained byte wipe and exact keep
signature. No Java caller/UI/proof sharing yet; source-only, no build/test ran.

Item 58 confirmed reviewed-host activation now advances a matching live invitation
revision from the actual writer result, preserving subsequent close/poll admission.
Stale invitations/uncertain failures are not rebased. Source-only; build/test due.

Item 58 host candidate review/activation now has a dedicated bounded mobile ABI
with exact native address/revision/hash and actual provider epoch/cancel checks.
No transport authority. Source-only; host callers/UI, durable recovery/build/test due.

Item 58 host peer mobile ABI now routes exact reviewed-hello persistence/decline,
requiring explicit input/lifetime choices for persistence and actual provider
epoch/cancel/deadline. No automatic consent/activation; source-only. Native UI,
durable uncertainty recovery and build/test remain due.

Item 58 mobile ABI now authors native host begin/register/poll/close with strict
bounded requests and actual foreground provider epoch/cancel/deadline admission.
No proof sharing/peer approval; source-only, no build/test. JNI/Swift/UI and
background/crash cleanup remain due.

Item 58 saved candidate review now supports host role under the same leased
scope/vault/grant checks, with installed host/input policy and explicit peer
fingerprints/local role. LocalHost exposes read-only review with cancellation.
Source-only; ABI/UI, durable recovery and build/test remain due.

Item 58 reviewed host activation now binds exact persisted address/revision/grant
hash under one writer lease before host-role/policy/vault/relay checks and Active
publication. Source-only; host review/ABI/UI and build/test remain due.

Item 58 explicit host sharing now connects controller to leased identity reload,
registered/live invitation and ticket/epoch/deadline/cancel checks. It returns
Zeroizing text separately from JSON/status. Source-only; actual native sharing
UI/ABI and copy-lifetime qualification remain due. No build/test ran.

Item 58 host controller now distinguishes never-attempted registration from any
possibly-sent attempt. Never-registered invitations close locally without false
relay acknowledgment; attempted ones retain exact remote cleanup requirement.
Source-only; durable crash recovery/ABI/UI/build/test remain due.

Item 58 host controller now declines exact reviewed peer requests by ticket/
epoch/hello hash and reports transient pending-host cleanup metadata in native
status. No proof is exposed; durable crash recovery/ABI/UI/build/test remain due.

Item 58 host controller now connects explicit peer consent to exact-ticket/
epoch/hello-hash candidate persistence. Revision advances only on confirmed
writer success; no relay approval/activation is implied. Source-only;
uncertain-write reconciliation, ABI/UI and build/test remain unfinished.

Item 58 LocalHost host poll now returns bounded retained peer review metadata
and exact hello hashes, scoped to ticket/epoch/revision with final cancel/expiry
admission. No approval inferred. Source-only; consent controller/ABI/UI/build/test due.

Item 58 LocalHost explicit host close now fences local invitation sharing/approval
before relay/provider/lease admission and retains the exact owner on failure.
Only confirmed close clears it; pairs are not revoked. Source-only; crash/expiry
reconciliation, ABI/UI and build/test remain due.

Item 58 LocalHost now retains a native host invitation with exact ticket,
identity/revision/epoch and explicit registration. No silent replacement or
automatic sharing/approval. Source-only; close/reconcile controller, ABI/UI and
durable remote ownership recovery remain unfinished. No build/test ran.

Item 58 leased host writer now binds consented grant creation/persistence to the
exact retained polled hello hash, Active identity and installed input policy.
One lease spans the operation; uncertainty never authorizes grant regeneration.
Source-only; host controller/UI and build/test remain due.

Item 58 leased host writer now authors exact-peer decline and live invitation
close. Close uses configured retirement origins so disabled new-session policy
does not alone block cleanup. Active identity/revision still required; crash and
revoking-identity cleanup remain due. Source-only; no build/test/network run.

Item 58 now authors leased explicit host request polling using exact Active
identity/revision and installed host/origin admission, with post-network lease,
deadline/cancel checks. No approval inferred; host controller/UI and build/test due.

Item 58 now authors leased host invitation create/register writer methods using
the exact Active identity/revision and installed host/origin policy. Invitation
sharing remains separate; registration keeps caller-owned state on uncertainty.
Source-only; host controller/ABI/UI and build/test remain due.

Item 58 host candidate persistence and host activation now enforce installed
hosting/input policy and configured grant audience, in addition to native consent.
Source-only; policy remains disabled and no build/test/network operation ran.

Item 58 iOS saved MaterialReady pair details now link fresh leased rereview to
independent host verification then explicit permission/activation confirmation.
Original owner/review expiry remains; missing old comparison is explicit.
Source-only; no build/test/UI run. Host flow/retirement qualification remain due.

Item 58 iOS now authors immutable saved-peer review and one-use activation
helpers bound to native status record, core/owner/cancel and capped expiry.
No old comparison is reconstructed. Source-only; resume view/build/test due.

Item 58 Android MaterialReady pair details now offer saved-peer rereview and
explicit one-use activation with fresh host/permission checks and original
review expiry. Missing old comparison is explicit. Source-only; no build/test;
iOS resume UI and full qualification remain due.

Item 58 Android now authors immutable saved-peer review snapshots bound to a
validated status record, original owner/cancel fence and capped monotonic expiry.
Explicit one-use activation retains exact address/revision/hash without inventing
old comparison data. Source-only; resume dialog/iOS snapshot/build/test due.

Item 58 iOS private lane now routes saved-client review with bounded scope/
revision request and exact candidate/permission/expiry/hash response admission.
It requires comparison unavailable and fresh consent flags. Source-only;
resume snapshots/views and build/test remain due.

Item 58 Android private JNI/caller now routes interrupted-candidate review with
1024-byte bounds and exact address/revision, live expiry, grant/host text and
explicit unavailable-comparison checks. Source-only; typed resume snapshot/UI,
iOS caller and build/test remain due.

Item 58 interrupted-candidate review now connects LocalHost and a dedicated
mobile C ABI with foreground epoch/cancel/deadline and installed-origin checks.
No activation or consent is inferred. Source-only; mobile caller/UI resume wiring
and build/test remain due.

Item 58 now authors a leased writer primitive to re-review an interrupted saved
client candidate: exact revision/address, live signed grant, Active local identity
and vault material are reloaded before returning public review metadata. It
explicitly reports comparison unavailable; no activation/consent is inferred.
Source-only; controller/ABI/UI resume wiring and build/test remain due.

Item 58 client persistence replies now return the exact reviewed grant hash.
Both mobile lanes and activation snapshot factories require it to match the
retained consent hash before accepting the saved candidate. Source-only;
build/test, interrupted-candidate recovery and full integration remain due.

Item 58 iOS now authors one-use activation snapshots from exact successful peer
persistence and an explicit activation dialog. Original peer owner/core/fence/
expiry and exact candidate/revision/grant are retained. No transport starts.
Source-only; no build/test. Interrupted-candidate rereview and qualification due.

Item 58 iOS private client lane now calls reviewed activation ABI with 1024-byte
request bound, exact address/grant/revision response checks and existing owner/
deadline/cancel admission. Source-only; no build/test. Typed activation snapshot
and explicit iOS activation UI remain due.

Item 58 Android now authors one-use activation snapshots from exact successful
peer persistence replies and an explicit activation dialog. It retains reviewed
host/pair/input/expiry, candidate address/revision/hash and original review fence.
Active does not start transport. Source-only; build/test, iOS equivalent and
re-review of saved candidates after interruption remain due.

Item 58 Android private JNI/caller now dispatches reviewed client activation
through its bounded serialized lane. Caller checks exact scoped address, grant
hash, Active phase and advancing bounded revision in successful replies. No
renderer route; source-only, no build/test. Typed activation review/UI remains due.

Item 58 now authors reviewed client activation from mobile C ABI through LocalHost
to the leased writer. Exact candidate/revision/grant hash is checked before fresh
relay confirmation and Active publication; native epoch/deadline checks surround
the operation. Source-only; JNI/Swift activation callers/UI and build/test due.

Item 58 iOS now authors installed-relay selection, bounded invitation/host/label
form, explicit host verification, begin, explicit join and manual approval checks.
Code fields clear on submit/dismiss/lifecycle cleanup; string aliases cannot be
guaranteed wiped. Source-only; no build/test/UI/network run. Activation, native
host product flow, recovery ownership and full qualification remain incomplete.

Item 58 iOS now has immutable one-use setup from validated native Active identity
and installed relay origins. Setup/peer snapshot admission is rechecked before
ABI and response delivery. Source-only; setup UI/join/activation and build/test due.

Item 58 Android native menu now authors client setup form, independent host-key
confirmation, installed relay selection, bounded invitation input, explicit join
and manual approval polling. Code field clears on send/dismiss; ordinary strings
remain outside wipe guarantees. Source-only; no build/test/UI/network run.
iOS setup/begin/join, activation and end-to-end qualification remain unfinished.

Item 58 native status now exposes enabled installed pairing origins. Android has
a read-only setup helper selecting one Active identity from validated native
status, immutable relay choices and a one-use bounded begin helper. Disabled
policy refuses; no identity replacement or policy enablement. Source-only;
actual setup UI, iOS equivalent, join/activation and build/test remain due.

Item 58 LocalHost now drops expired transient client exchange/proof on native
status reads and before join/poll/consented persistence admission. Status remains
read-only for durable journal/vault/relay state. No timer/background cleanup or
runtime claim; build/test and full native pairing integration remain due.

Item 58 mobile peer-permission dialogs now display retained grant expiry in the
device locale and explain the separate one-minute review cap. User-facing save
labels say pairing rather than candidate; persistence remains MaterialReady only.
Source-only; no build/test or UI rendering qualification ran.

Item 58 both native status views now offer explicit pending-client local cancel
with a confirmation explaining remote approvals/candidates are unaffected.
They use immediate epoch invalidation then exact-ticket cleanup via action 4.
Source-only; build/test, full begin/join, activation and qualification remain due.

Item 58 iOS native status now links live pending-client tickets to a two-stage
peer/comparison then input-permission consent flow. It uses the immutable one-use
snapshot, owner/foreground/protected-data checks and original review expiry.
Source-only; no build/test/UI run. Begin/join, activation and qualification due.

Item 58 Android status now exposes a live pending-client review action and native
peer-consent dialog. It displays retained host/comparison/pair/input metadata,
requires separate peer-comparison and permission checks, and sends the one-use
snapshot only on explicit confirmation. Source-only; no UI/build/test ran.
Begin/join entry, activation, iOS views and full qualification remain incomplete.

Item 58 approved-poll review snapshots now retain the transcript's pinned host
key, derived fingerprint and comparison code alongside pair/input metadata.
LocalHost sources these from the retained native exchange, and both mobile
callers validate text formats. Source-only; consent view wiring/build/test due.

Item 58 iOS now has immutable approved-poll peer snapshots, core/owner/cancel
binding, capped monotonic review lifetime and one-use confirmation. Requests
retain exact reviewed ticket/grant hash. Source-only; build/test, actual consent
views and activation remain incomplete.

Item 58 Android now has an immutable approved-poll review object and single-use
native confirmation helper. It retains exact ticket/grant hash, displayed pair/
input permission, delivery fence and at-most-60-second monotonic review lifetime.
Only a current approved poll creates it. Source-only; actual dialog wiring,
iOS equivalent, activation and build/test remain due.

Item 58 now binds consented client persistence to an exact reviewed grant hash.
Approved poll returns grant_hash; action 3 requires ticket plus reviewed_grant_hash
and checks it against the retained signed grant before consuming the exchange.
Rust ABI, Android JNI/caller and iOS caller bounds/parsing are updated together.
Source-only: no build/test; actual native consent UI and activation remain due.

Item 58 iOS now mirrors Android's immediate cancel delivery fence, using locked
object-identity tokens checked before ABI, after response and on main delivery.
Source-only; build/test and actual consent UI/transcript binding remain due.

Item 58 Android cancel now rotates an in-memory reply fence before native
invalidation. Earlier queued/in-flight/main-pending ordinary replies fail caller
admission even if the dialog remains alive. No counter wrap or fence reuse.
Source-only: build/test and matching iOS delivery fencing remain due.

Item 58 begin-response metadata now checks comparison code, host key/fingerprint
format, future safe-range integral expiry and bounded journal revision in mobile
callers. Source-only; build/test and native consent UI remain due.
Follow-up replaces Swift regex checks with exact ASCII byte validators for
comparison/key/fingerprint text, including canonical base64url final pad bits.

Item 58 mobile callers now admit only known join/poll/cancel response states.
Approved poll metadata requires a canonical pair UUID, actual boolean input
permission and unexpired integral safe-range expiry; local consent remains
mandatory. Cancel explicitly remains local, not remote revocation. No build/test
ran; native consent UI and complete transcript binding remain unfinished.

Item 58 mobile candidate admission now validates canonical nonzero device/pair/
generation UUIDs, bounded integral journal revision and canonical 32-byte public
key text. iOS explicitly refuses JSON booleans as revisions. Source review and
diff whitespace checks only; build/test and actual product UI remain pending.

Item 58 Android cancellation now requires the immediate native epoch invalidation
acknowledgment before queuing ticket cleanup, matching the iOS caller contract.
Missing handles/refused invalidation return uncertain-state guidance and clear
owned input. Source-only change; no build/test or cancellation qualification ran.

Item 58 persisted client candidate responses now carry the consumed pairing
ticket; both mobile callers require the exact requested ticket before admitting
the candidate metadata. This closes the previously missing response correlation.
Source review only; build/test and actual consent UI remain deferred/incomplete.

Item 58 mobile callers now check canonical request/reply tickets, protocol
metadata and candidate profile/kind/phase before accepting successful replies.
Both callers recheck the original deadline and dialog lifetime on main-thread
delivery. These are source changes only: no compiler/build/test ran. Full typed
reply validation, actual consent UI and end-to-end qualification remain due.

Item 58 relay review now authors transactionally retained closed-invite fences
so old registration retries cannot resurrect a still-valid closed invite. Same-
host close retry receives the bound stored receipt; quota never evicts live
fences and expiry remains original. No SQL/server/deployment/compiler/build/test
ran. Upgrade/expiry/race qualification and native durable recovery remain due;
full original scope remains incomplete.

Item 58 now authors iOS typed client ABI calls with consumed Data input, bounded
ordinary/cancel admission and queue-inclusive original deadline. Immediate native
epoch invalidation precedes queued cancel; owned buffers are best-effort cleared.
No Swift/compiler/native/vault/HTTP/build/test ran. Actual peer-consent UI and
typed reply/ticket admission remain due; full original scope remains incomplete.

Item 58 now authors private versioned Android JNI client pairing dispatch and
an owned-byte-array serialized caller with size/admission/queue/deadline bounds.
Cancellation invalidates native epoch before queued cleanup; proof bytes are
cleared on all caller/native vector paths. No Java/C++/JNI/native/HTTP/build/test
ran. Actual Android consent UI and iOS client/host wiring remain due; original
scope is still incomplete.

Item 58 now connects dedicated mobile client begin/ticket-action ABI to the
LocalHost controller with native-derived epoch/cancellation, bounded proof input
and exact-ticket local cancellation. Native status exposes pending ticket metadata
for lost-response reconciliation. No ABI/native/vault/crypto/SQL/HTTP/compiler/
build/test ran. Actual Swift/JNI consent callers and host/recovery qualification
remain due; full original scope stays incomplete.

Item 58 now authors LocalHost-owned single pending client ticket bound to exact
identity/revision/native epoch, scoped signer reload for begin/join/poll and
consume-on-consent candidate publication. Cancel/detach/close discard transient
proof; no peer/transport authority is automatically activated. No native/vault/
crypto/SQL/HTTP/compiler/build/test ran. Mobile ABI/UI foreground/peer consent
wiring, host controller and recovery qualification remain due; full scope is open.

Item 58 now authors host decline with immediate local approval fence and retained
exact hello evidence, plus terminal native invite close that clears proof. Server
decline checks actual pending/declined state and no longer acknowledges missing/
approved joins; close acknowledges exact owned deletion. Bound receipts are
required and unknown close remains unknown. No HTTP/SQL/native/deployment/compiler/
build/test ran. Durable receipts, controller/UI and qualification remain due.
Full original scope remains unfinished.

Item 58 now authors exact host invite registration receipts and idempotent same-
invite server registration, with sharing gated on acknowledgment. Bounded host
polling verifies all signed hellos before replacing a snapshot; approval requires
its exact stored request. No HTTP/native/crypto/SQL/deployment/compiler/build/test
ran. Host close/decline, real native identity/foreground/UI controller and crash
qualification remain due; full original scope is still open.

Item 58 now connects persisted host candidate activation through exact relay
confirmation/reconciliation, retained-grant approval with bound protocol receipt,
then fresh confirmation before journal Active. Client/host share scoped writer
admission; native consent and enabled installed origins remain required. No
deployment/HTTP/native/vault/SQL/compiler/build/test ran. Invite registration/poll,
actual UI/controller and owner/recovery qualification remain due; scope is open.

Item 58 now authors native host invitation/share proof, signed peer review and
exact consented grant/exchange preparation, bounded to eight distinct client
attempts and original invite expiry. Host writer reloads its Active pinned key
and persists MaterialReady/grant metadata before future relay approval. No
RNG/crypto/native/vault/SQL/compiler/build/test ran. Host relay dispatch/native UI/
activation/recovery and qualification remain due; full original scope stays open.

Item 58 now authors client candidate activation with scoped pinned identity/pair
reload and fresh exact ConfirmPair relay receipt before journal Active. Server
confirmation uses the live unrevoked pair row independently of expired invites,
without ICE allocation. LocalHost requires enabled installed origins; current
policy refuses. No HTTP/deployment/native/vault/SQL/compiler/build/test ran.
Actual UI/foreground consent controller, host flow, races and recovery remain
due; full original 51–109 scope stays unfinished.

Item 58 now connects the opaque approved native client transcript to the retained
lease writer and a policy-gated LocalHost entry. Active pinned identity possession
is rechecked before consuming ECDH, and exact stored grant/material flow through
existing candidate write/read-back with original invite deadline retained. Result
is MaterialReady only. No native/vault/crypto/SQL/compiler/build/test ran. Product
pending/consent controller, activation and host flow remain due; full scope is open.

Item 58 now authors native client approval polling with bound invite/client/
hello-hash status receipts, pinned-host grant admission and monotonic terminal
state/first-grant binding. Finish consumes only the stored approved transcript;
local consent and product persistence remain separate. Signaling receipt changes
are source only. No HTTP/native/crypto/SQL/vault/deployment/compiler/build/test
ran; UI/controller/writer activation and host flow remain due. Full scope is open.

Item 58 now authors explicit pending-transcript JoinInvite transport and bound
protocol-1 invite/client/hello-hash receipt admission. Signaling source returns
the same binding for first/identical joins; legacy unbound replies cannot mark
native joined. Borrowed proof serialization shares bounded allowlisted HTTPS
handling; finish requires the joined receipt. No HTTP/native/vault/SQL/deployment/
compiler/build/test ran. Product admission/consent/poll/activation and host flow
remain due; full original scope stays incomplete.

Item 58 now authors NativeClientPairing preparation from a bounded OPRP1 code,
separately pinned signed invite/secret admission, native exchange-bound signed
hello and browser-layout comparison code. Exact transcript and original invite
deadline remain owned until consumed finish; proof has no public export getter.
No parse/crypto/native/vault/HTTP/compiler/build/test ran. Relay join, native peer
consent UI, grant collection/activation and host controller remain due; full
original scope stays open.

Item 58 broker review now retains uncertain input workers and artifact/process
witnesses in CleanupRequired, including close and terminal-status/error paths.
Actual terminal input receipts are preserved separately from helper/I/O exit;
video revoke permits reaping its retained closed worker. No broker/helper/native/
thread/compiler/build/test ran. Process-crash recovery, owner aggregation and
deadline qualification remain due; full scope stays incomplete.

Item 58 backend review found Windows worker stop could succeed before all owned
I/O threads retired. Input/video/gamepad now require completed successful joins,
retain panic failure and preserve only actual input/video Stop receipts for later
reaping. This does not implement the session owner or prove HID cleanup. No
helper/native/thread/compiler/build/test ran; owner aggregation, deadlines and
platform qualification remain due. Full original scope stays incomplete.

Item 58 source review found session release could clear journal ownership after
permit close without an enforced backend-retirement call. Release now requires
the actual native owner, bounded deadline and exact registered permit/session/
epoch/revision under the scoped lease. Pending/error retains ownership; closed
entries are no longer automatically pruned. The actual backend owner is still
unimplemented. No backend/native/SQL/compiler/build/test was executed; full
original scope remains unfinished.

Item 58 now authors explicit native revocation of Active identity/pair generations,
separate from advisory recovery plan execution. Android/iOS detail approval routes
through exact address/revision/namespace/foreground/deadline admission to the
existing journal fence. Identity-to-pair effects are shown; session/backend/relay/
key cleanup debt remains. No UI/native/SQL/compiler/build/test was executed.
Producer retirement and runtime qualification remain due; full scope stays open.

Item 58 now authors separate desktop vault retirement markers, exact marker
admission/read-back around cleanup and candidate-write refusal around generic
keyring set operations. Marked generations with remaining material stay errors.
This quarantines cooperating provider use, not late OS writes themselves, and
does not provide WriterRetired evidence. No OS/vault/compiler/build/test was
executed. Backend retirement/repair and qualification remain due; scope stays open.

Item 58 now authors an Android exact-generation public retirement barrier,
durably created/read back before wrapped-material deletion and retained after
cleanup. Candidate writes refuse the marker and reads do not classify partial
cleanup as missing. This does not prove AndroidKeyStore service retirement or
stop a session. No file/Keystore/native/compiler/build/test was executed.
An explicit cleanup retry can append only a matching interrupted public marker
prefix; corrupt bytes are never overwritten. Owner integration, corrupt-marker
recovery and qualification remain due; full
original scope and item 58 stay incomplete.

Item 58 writer-retirement review keeps unresolved OS-service work as recovery
debt. iOS cleanup now authors an exact public retirement marker retained in the
same Keychain account, add/update without a delete/add gap and exact read-back;
normal candidate writers remain add-only. No WriterRetired/session receipt is
inferred from this. No compiler/Keychain/native/build/test was executed. Actual
race qualification, provider fence/owner integration and Android/desktop late
write containment remain due; original scope stays open.

Item 58 now authors a cooperative lease namespace binding in fresh journal
history, admitted before all mutations and scoped writer/recovery/signal/ICE
loads. Existing copied/moved or legacy unbound histories are not silently rebound;
native inspection remains available and mobile UI surfaces missing provenance.
This is not hardware/cross-host attestation or a producer-retired receipt. No
hash/filesystem/lease/SQL/native/compiler/build/test was executed. Explicit
migration, owner integration and qualification remain due; full scope stays open.

Item 58 writer-retirement source review found the Unix lease namespace block
misplaced in acquire() instead of check(&self). It now runs in the intended
per-admission method, including acquisition's final check. No compiler/native/
file/lock/build/test was executed. This does not prove copied-profile exclusion
or producer retirement; actual owner integration and qualification remain due.
Item 58 and the original 51–109 scope remain incomplete.

Item 58 now authors read-only Game Hub startup inspection and native attention
indicators/notices, with separate cancellation lifetimes. Status exposes retained
session-ownership debt so active records are not misclassified as ready for a new
receiver after restart. No automatic cleanup, activation, key replacement or
ownership reclamation is performed. No startup/UI/native/SQL/compiler/build/test
was executed. Actual reconciliation owners and qualification remain due; item 58
and the complete original 51–109 scope remain open.

Item 58 now authors acknowledged native epoch invalidation independent of the
request mutex. iOS closes protected-data access until live provider handles have
been invalidated, preserving transition ordering across HTTP waits; explicit
Android/iOS cancellation also invalidates before queued review cleanup. Mobile
handle allocation no longer wraps. No UI/notification/JNI/native/vault/HTTP,
compiler/build/test was executed. Lifecycle races, producer owners, startup and
all qualification remain due; full original scope stays unfinished.

Item 58 now connects explicit native relay retirement through LocalHost/mobile
to pinned-key signed revocation and exact pair/grant receipts. Only validated
installed origins are used; empty current configuration refuses network work.
Foreground epochs own separate cancellation tokens, never reset for older work.
Native dialogs admit configured retirement availability and Android JNI uses a
versioned selector entry. No UI/native/vault/SQL/HTTP/deployment/compiler/build/
test was executed. Producer owners, startup reconciliation and qualification
remain due; full original scope and item 58 remain unfinished.

Item 58 now connects exact-generation vault removal/read-back from the native
recovery dialogs through scoped mobile providers and LocalHost. The real lease,
current RemoveMaterial plan and retained writer/local/relay receipts are required;
unknown external cleanup cannot be skipped. Missing-only read-back precedes the
journal receipt; uncertain results require reload. No UI/native/vault/file/SQL,
compiler/build/test was executed. Producer owners, relay/startup integration and
qualification remain due; full original 51–109 scope is still incomplete.

Item 58 now authors Android native recovery list/detail/approval UI, immutable
bounded status/plan snapshots and a private recovery JNI caller. Exact selected
address/revision and remaining snapshot budget bind metadata actions; external
producer/relay/vault steps stay waiting. Policy remains disabled and status is
still readable. No compiler/UI/JNI/native/file/SQL/build/test was executed.
Startup reconciliation, actual retirement owners and qualification remain due;
item 58 and the complete original 51–109 scope remain unfinished.

Item 58 now authors iOS native recovery status/list/detail UI and an explicit
metadata-action caller. Record/plan/address/revision admission and a bounded
snapshot lifetime prevent automatic plan execution; waiting external cleanup has
no execution button. Fencing shows its permanent identity/pair effects before
approval. Streaming policy remains disabled. No UI/ABI/vault/SQL/compiler/build/
test was executed; Android recovery, external owners and qualification remain
due. Original item 58 and the full 51–109 objective remain incomplete.

Item 58 now connects LocalHost status to the journal's exact recovery plan and
authors a native one-step metadata recovery entry with real credential lease,
revision/generation admission and foreground/deadline checks. Only journal fence
or fully receipted finalization can commit; unobserved producer/relay/vault steps
stay pending. No automatic startup cleanup or fabricated owner was added. App
recovery UI and actual retirement owners remain due. No native/file/SQL/build/test
was executed; item 58 and the full original 51–109 scope remain incomplete.

Item 58 now authors Android dedicated JNI identity operations, serialized typed
review callers and a native Game Hub actions/consent dialog. Admission checks
current trusted tab/activity and unlocked device; one-use review metadata remains
outside renderer messaging. Pause/disconnect/expiry/cancel suppress stale UI
callbacks; uncertain states are not replaced or retried automatically. Policy
remains disabled. No compiler/native/vault/build/test was executed; device/UI
qualification, recovery, peer pairing and transport remain due. Full scope is open.

Item 58 now authors an iOS native identity button/dialog and serialized status,
prepare, review, confirm and cancel callers. Public scope/revision/action/expiry
checks and a separate flow lifetime suppress stale callbacks; existing uncertain
identities are not replaced. Disabled policy is shown without key creation.
No UIKit/vault/native endpoint, compiler, build or test was executed. Android UI,
recovery, peer pairing/transport and qualification remain due; full scope is open.

Item 58 now authors a native one-use identity review/confirmation binding with
exact candidate/revision/foreground epoch and monotonic expiry. Confirmation
consumes the review before vault work and uses pinned-key activation; cancellation
only clears transient review state. A token is not consent: actual native dialog
and Swift/JNI callers remain due. Policy remains disabled; no UI/native/vault/SQL,
build or test was executed. Item 58 and original 51–109 scope stay incomplete.

Item 58 now connects LocalHost source to native public-status and MaterialReady
identity-preparation entry points. Mobile operations require the actual foreground
host/provider, original bounded deadline and lifecycle epoch checks; preparation
also requires the installed policy, which remains disabled. No renderer command,
activation, app UI caller or recovery dispatcher was added. No native/vault/SQL,
build or test was executed. Item 58 and the full original scope stay incomplete.

Item 58 now authors a Unix/mobile credential lease using a canonical private
profile root, bounded no-follow lock-file admission and nonblocking exclusion,
with held/named inode/device/owner checks. Trusted Android/iOS open paths resolve
OS container aliases before Rust admission. No filesystem/lock/native/vault/SQL,
build or test was executed; actual platform qualification and LocalHost pairing
controller remain due. Full original objective and item 58 stay incomplete.

Item 58 now connects iOS host-open source to scoped Keychain attachment and
protected-data events with a nonwrapping epoch across calls. The provider source
is included in the checked-in Xcode target; unrelated settings are preserved.
No generation/build/test, UIKit/Keychain/native callback or runtime was executed.
Core mobile lease, pairing controller and full device verification remain due;
item 58 and original 51–109 scope remain unfinished.

Item 58 now connects Android host-open source to dedicated native scope/provider
attachment, with canonical native profile/device checks and separate failure
state. JNI authors explicit UUID/kind byte encoding, exact bounded secret arrays,
callback exception handling and retained-provider ownership transfer. No startup,
JNI/vault/build/test was executed; LocalHost pairing consumption, mobile lease,
iOS attachment and all runtime qualification remain due. Original scope is open.

Item 58 now authors native mobile scope/provider attachment tied to the actual
open host profile/device, with duplicate/closing rejection and consumed callback
ownership. Foreground/terminal-close provider gates use transition epochs to
reject stale in-flight results across background/resume. App-side Swift/JNI
attachment callers, LocalHost pairing controller and mobile lease remain due.
No native callback/OS/vault/build/test was executed; original scope stays open.

Item 58 now authors an Android Keystore wrapping provider candidate with separate
generation namespace, authenticated profile/device address, bounded no-backup
ciphertext, record locking and durable new-file read-back. Missing, locked and
orphan/corrupt states differ; failed-read output is cleared. JNI/lifecycle/core
mobile lease integration and device qualification remain missing. No native,
crypto, storage, build or test was executed; original 51–109 scope stays open.

Item 58 now authors a separate mobile Remote Play callback ABI/Rust scoped
provider and unintegrated iOS Keychain candidate, retaining profile/device/
generation envelopes and missing-vs-locked/error distinction. New source uses
device-only/non-synchronizing Keychain entries and default-closed protected-data
access; actual iOS target/bridge/hooks, Android/JNI, mobile lease and product
integration remain due. No callback/native/vault/SQL/build/test was executed;
original 51–109 scope and item 58 remain open.

Item 58 now authors a concrete native document/session authority registry using
revocable lifetime guards, fixed installed policy and current journal state.
Session ownership is durably exclusive per pair and epoch comes from committed
journal revision; all native signaling/ICE paths check it. Drop/prune/crash/time
never silently clears that owner. Installed policy stays disabled and real host
hooks/consent/startup/backend/mobile wiring remain due. No runtime, SQL/OS/network,
build or test was executed; item 58 and original objective remain incomplete.

Item 58 now authors exact pair/grant-bound ICE responses, shared canonical TURN
URI admission and temporary-credential/fingerprint/allowlist validation. Native
ICE request source connects retained lease/vault signing, post-network authority
and original certificate/credential expiry, refusing relay-only downgrade. No
ICE agent/connection, relay deployment, network/crypto/SQL/vault/build/test was
run. Actual native controller/backend/mobile integration and full scope stay open.

Item 58 now authors retirement-only historical grant admission, a relay
transaction response bound to exact pair/grant hash, and concrete native remote
retirement-to-journal orchestration after prior writer/local retirement. Absent
live and mismatched grants are refused; expired retirement does not authorize
normal signaling. No service was deployed or HTTP/SQL/vault/build/test executed.
Actual native owner/controller/mobile integration and original scope remain open.

Item 58 now connects bounded native HTTPS receive polling to a non-skippable
encrypted batch and sequential authenticated/journal-committed acceptance. Poll
does not advance the cursor; original epoch/revision/deadline and native session
authority are checked before delivery. Unknown-session recovery, actual receiver
ownership/authority, ICE/revocation and product/mobile dispatch remain unfinished.
No HTTP/vault/crypto/SQL/build/test was executed; original scope remains open.

Item 58 now authors a bounded allowlisted paired HTTPS relay client and connects
prepared-send packets to pinned vault identity/request signing and exact send
acknowledgment admission under the retained credential lease. Original session
authority, cancellation and expiry are rechecked after network work in source.
No HTTP/client/crypto/vault operation was executed. Actual native authority,
pair approval/receive/ICE/revocation/product/mobile callers remain due; this is
not deployed item-59 infrastructure and full original completion scope stays open.

Item 58 now connects a native per-operation signal pipeline: retained lease,
active generation/vault admission, durable send reservation before encryption,
and authenticated receive acceptance before returning plaintext. Required native
observations bind exact generation/session/epoch around slow work; late or
uncertain receipts never publish payloads. Actual authority providers, relay HTTP
and session/product/mobile dispatch remain missing. No crypto/SQL/vault/network,
build or test was executed; item 58 and original completion scope stay open.

Item 58 now authors bounded native signal AES-GCM matching browser AAD layout,
plus journal send-sequence reservation and authenticated receive cursor/sequence/
message acceptance metadata. Missing counters refuse recovery instead of
resetting history. Cipher plaintext is explicitly uncommitted and the two APIs
still need real native transport/epoch integration; no crypto, SQL, network,
vault, build or test was executed. Full original 51–109 scope remains open.

Item 58 now authors a consumed-on-finish native P-256/HKDF exchange with pinned
peer/identity signatures and exact invite/hello/grant, ECDH/nonce/hash/lifetime
checks, matching the browser KDF layout in source. Raw pair-key construction is
crate-private. Relay invitation-secret proof, comparison consent, native pairing
controller and signaling remain missing; no interoperability or crypto API was
executed. Build/test still deferred and full original completion scope stays open.

Item 58 now connects native candidate preparation through one retained writer
lease, durable reserve/write intent, actual vault API calls in source and exact
read-back before MaterialReady publication. Identity activation reloads its
pinned key after required native caller consent; pair preparation verifies the
active identity and grant lifetime without assuming relay approval. Errors and
late/uncertain commits keep journal recovery state. No caller or native exchange
is integrated, and no API/build/test was run; original completion scope stays open.

Item 58 now authors a Windows cooperative credential lease with a fixed zero-
byte nonblocking lock file, retained non-reparse ancestor handles and exact
profile/device/root binding. Every journal mutation and recovery step requires
that retained lease; mutations also admit WAL/FULL durability. Actual writers,
copied-profile admission, non-Windows leases and product/startup integration are
still missing. No file lock, SQL/vault/native call, build or test was executed;
the source candidate does not complete item 58 or the original 51–109 objective.

Item 58 now also authors a one-step recovery executor connecting current journal
plans/revisions to required native writer/local/relay receipts and exact-vault
delete/read-back. Pending/unknown/late backend outcomes retain cleanup debt;
active records require separate revalidation. Real cross-process exclusion and
owner implementations plus startup/product dispatch are absent, so no recovery
operation has run. No SQL/vault/network/native API, build or test was executed;
the full item-58 and original 51–109 scope remain unfinished.

The next item-58 source checkpoint retains signed grant/audience recovery
metadata with signature and exact record-binding checks, including historical
admission of expired grants without reviving permission. A bounded read-only
restart plan orders generation fences, writer/local retirement, relay cleanup
and vault removal; identity material cannot be acknowledged absent while pair
cleanup debt remains. Finalization requires all retained receipts. No recovery
executor, vault/relay call, SQL operation, build or test was run; actual product
and mobile integration plus original item-58 scope remain unfinished.

The subsequent item-58 checkpoint authors a bounded SQLite generation journal
with immediate/revision-checked transitions, write intent before vault I/O,
permanent revocation fences and separate writer/material/local/relay cleanup
observations. Identity revocation fences every pair atomically; retirement waits
for pair cleanup. No external operation or receipt is implemented by these
metadata calls: product authority, durable grant metadata, startup reconciliation
and actual vault/relay/mobile integration remain due. No SQL/vault/build/test
was executed for this checkpoint and item 58 remains unfinished.

Source work now also reaches item 58: a separate native Remote Play secret-store
candidate authors generation/profile/device bindings, pinned Ed25519 identities,
signed-grant-bound pair material, read-back admission and desktop keyring binary
providers. Vault deletion is not lifecycle revocation; durable journal, native
pairing/authority, mobile providers and product wiring remain absent. See
`NATIVE_PAIRING_CREDENTIALS.md`. No vault/crypto/native API, build or test was
executed. Item 57 qualification and all original incomplete 51–109 work remain
open; streaming policy stays disabled and build/test execution remains deferred.

On the owner's explicit "build and test later; next" steering, source work moves
to item 57 while all incomplete item-53–56 obligations remain open. Windows
installed launch now authors creation-time job ownership, group-drain lifetime
and host-only selected process-handle admission. No descendant capture chooser
is integrated, native API executed or launcher compatibility claimed. See
`WINDOWS_LAUNCHER_OWNERSHIP.md`; item 57 and original 51–109 scope remain open.

The next item-57 checkpoint connects input authorization to the actual retained
capture process instead of reacquiring the root launcher. Exact-job/identity
admission is repeated after consent and helper admission, then original page,
capture and certificate deadlines are checked before computing worker lifetime.
The descendant chooser/capture binding still needs implementation; no build,
test, dialog, helper or native API was executed for this checkpoint.

Item 57 now also authors bounded native window discovery and retained choices
for exact game-job members, with caption bounds, original observation expiry
and re-admission of the selected window/process. Core host-only methods expose
these typed choices, not a raw PID/handle command. Broker/UI selection and
descendant capture registration remain unconnected; no native API, build or
test execution was performed and the item is still incomplete.

Item 57 now connects a two-phase broker/Game Hub selection path: prepare retains
bounded native window choices, select consumes an opaque original-lifetime
choice into a capture's exact process/window, and the component worker retains
native identities privately. Chromium consent is still required; a mismatched
returned HWND is refused before publishing a media token. Document ownership,
late/cancel cleanup, explicit choice UI and matching video/input HWND checks are
authored. This is source integration only: build/tests/runtime remain deferred,
policy is disabled, and launcher/platform/final qualification is unfinished.

On the owner's continued source-work/next steering, item 56 now has initial
private native codec-intersection and bounded raw-frame mailbox source in
`native_video.rs`. `NATIVE_VIDEO_SERVICE.md` records the independent executable,
OS encoder/capture backend, transport, authority and qualification still due.
This does not complete item 55 or 56 and does not enable streaming. All build/test
execution stays deferred; the original 51–109 scope is retained.

Item 56 now also contains an unintegrated macOS VideoToolbox H.264/HEVC encoder
candidate and its CMake source recipe, with bounded callback ownership/output,
explicit hardware-required creation and permanent delivery fences. This is not
a macOS build, service/capture connection, codec compatibility or runtime pass.
Windows/Linux encoders and every remaining item-56 service obligation stay open.

The subsequent item-56 checkpoint authors a Windows Media Foundation H.264 NV12
encoder candidate with sync/async MFT handling, bounded pending/output memory,
keyframe/timestamp/deadline fences and failed-stop reference retention. It is a
private static library, not the completed independent service or an enabled
product feature. No configure/build, MFT activation or runtime/test was run.
Capture/conversion, supervision/transport, other codecs, Linux and full original
item-56 service/qualification requirements remain open.

The next item-56 source checkpoint adds an independent Windows encoder-worker
executable recipe and pipe protocol, connecting finite NV12 commands to MF and
bounded output replies. Reader/writer/actor ownership, parent keepalive, original
lifetime and superseding Stop receipts are authored. This has not been built or
run; installed admission, external process supervision, native capture/conversion,
receiver/transport/product integration and remaining platform scope stay due.

Item 56 also now has an unintegrated Windows Rust parent transport/watchdog,
typed 512-MiB Video helper launch, original game/helper witnesses, non-reviving
native authority lease, accepted-frame/output matching, bounded packet ownership
and Stop-receipt-plus-retirement confirmation. No compile, helper launch or test
ran. Product admission/capture/observation/receiver integration and every original
remaining service/platform requirement are still open.

The next item-56 source checkpoint connects the raw mailbox, explicit SDR
BGRA-to-NV12 conversion and Windows worker inside a broker-private module. It
preserves raw receipt age and pending keyframe intent, requests matching MF
colour metadata and returns only unnegotiated packets. Eight colour/buffer/
identity regression cases are authored, not executed. Native capture producer,
installed/native admission, actual receiver/transport/UI integration, other
platform completion and all build/runtime/final evidence remain due.

Item 56 now also has separate WGC/D3D11 owned-window capture source: original
process/native item retention, default-No local confirmation, bounded callbacks,
SDR display checks and GPU aspect-fit opaque BGRA readback. It remains unconnected
to the encoder worker/Rust frame bridge and unbuilt/unexecuted. Native capture
transport, host admission/lifecycle/UI, application audio, other platforms and
every original item-56 product/qualification requirement remain open.

The subsequent item-56 checkpoint connects WGC capture, C++ SDR conversion and
MF encoding inside a distinct owned-v2 worker mode. It requires an inherited
original process handle/local confirmation and preserves capture age/sequence,
keyframe fences and both native Stop results. Rust typed v2 launch/client and
product admission/UI/receiver/transport remain unconnected; no build, native
prompt/media operation or test ran. Full remaining scope stays open.

The next item-56 source checkpoint adds Rust owned-v2 launch/client and a private
owned-video broker wrapper. Only a query/synchronize duplicate of the original
game handle is inherited beside stdio; native sequence/age/RTT, first keyframe
and requested keyframe fences are checked before queuing unnegotiated output.
No actual installed/native observation/service/UI/receiver path invokes it yet.
No build/test, native dialog, helper launch or capture/media operation ran;
all original remaining requirements remain open.

Item 56 now has typed exact-owner native authority observations and a singleton
independent observation-lane source. Foreign/stale/lost evidence closes the lease;
blocked source/cleanup retains the lane permit and cannot revive expired authority.
Four source admission tests are authored, not executed. Current service still
lacks the actual native document/capture/pair-revocation observer implementation
and caller; installed/product/receiver/UI and all original scope remain open.

The next item-56 source checkpoint adds fixed installed-worker inventory/hash
admission, retained ancestor/file guards, and a private prepared-capture handoff
matching full owner/original process identity with a consumed single launch
attempt and lifetime recheck after hashing. No current service command calls it;
real packaging/native observation/lifecycle/receiver/UI remain due. No install,
admission/launch/capture, compiler or test execution ran. Full scope stays open.

The next item-56 source checkpoint adds explicit Windows video snapshot/build/
staging and release-inventory PE architecture/mitigation checks. Static runtime/
asInvoker settings and seven inert PE fixture tests are authored, unexecuted.
No configure/build/staging/package/parser/test or worker execution ran. Real
installed packages, native service/observer/UI, receiver/transport and all
original remaining platform/qualification requirements stay open.

Item 56 also now contains explicit bounded H.264 NAL framing and native RTP mode
0/1 packetization with FU-A/marker/clock/sequence handling. Eight inert source
fixtures are authored, unexecuted. No broker/native injection/receiver/SRTP/network
integration or execution is implied; actual codec/VUI admission and all original
platform/service/transport/final qualification requirements remain open.

The next item-56 source checkpoint adds an internal bounded H.264 RTP receive
stage (single NAL/FU-A/STAP-A) with source/header/sequence/timestamp/assembly-age
fences. Seven inert source cases, including sender/receiver round trip, are
authored and unexecuted. It has no product/network/decoder caller and requires
authenticated SRTP/reorder/codec authority before entry. All original missing
receiver/service/platform/transport and final execution evidence remain due.

The subsequent item-56 checkpoint connects bounded RTP reordering to H.264
assembly, with original receipt age, duplicate/source/gap fences and finite
packet/unit budgets. Six inert source fixtures are authored, unexecuted. This
does not implement SRTP/network, adaptive playout/loss recovery, decoder or a
product caller. All original remaining scope and real evidence stay open.

The next item-56 source checkpoint adds a private Windows H.264 decoder candidate
using the fixed OS MFT, bounded compressed/NV12 ownership, preserved receiver age,
timestamp order and padded-surface/crop/2D-row checks. It is not invoked by any
current worker/product path and is unbuilt/unexecuted. Native decoder IPC/secure
receiver/bitstream admission/presentation plus all original platform/product and
final evidence requirements remain open.

The subsequent item-56 checkpoint connects the C++ native decoder to a distinct
private worker-v3 pipe mode with original receiver-clock age, finite compressed/
NV12 responses and Stop results. Current Rust/service/product has no v3 caller;
typed viewer supervision/admission, clock/codec/secure receiver and presentation
remain unfinished. No compile, protocol/decoder/media operation or test ran;
all original remaining requirements stay open.

The next item-56 checkpoint adds Rust decoder-v3 parent supervision, native
receive-clock receipts, exact input/output/tight-NV12 admission and a private
unpresented-frame broker wrapper. Viewer decode omits a local game handle but
still requires independent exact-owner native authority. Actual secure receiver/
ingress/reorder/codec/native service/display wiring remains due. No build,
helper/decoder/media/clock or test ran; original full scope remains open.

The next item-56 checkpoint connects original native receipt metadata through
ordered RTP assembly, required actual-codec admission and retained decoder submit.
It adds bounded modular RTP clock rebasing and four unexecuted source cases.
There is still no actual SRTP/native ingress, full admission implementation or
product/display caller. No build/parser/clock/helper/decoder/media/test ran;
all original remaining scope and final evidence stay open.

The receive bridge now retains explicit Active/confirmed-Revoked/CleanupRequired
state instead of discarding failed native Stop results during error handling.
Repeated revoke cannot turn unconfirmed cleanup into success. Source-only review
also rejects impossible future buffered receipt clocks. No build/test or native
execution occurred; product state/recovery integration remains due.

Item 56 static review now fences all decoder crop metadata, square-pixel/SDR
colour/stride semantics and actual current output types after sample production.
An inert media-type fixture target is authored and excluded from normal builds;
no build/test/API/media operation ran. Full codec/VUI/source/transport/product
and supported-device qualification remain unfinished.

Decoder source now explicitly normalizes legacy IMF2DBuffer through bounded
GetContiguousLength/ContiguousCopyTo before visible-plane copy, keeping canonical
width stride separate from native padded surface stride. Plain Lock remains
only for non-2D buffers. No copy/provider/build/test ran; actual buffer/device
qualification and all original remaining product requirements stay open.

This fork's current assignment: items 51 and 52 are accepted with evidence in
`SAVE_ARCHIVE_INTERCHANGE.md` and `STORAGE_FAULT_QUALIFICATION.md`. On 2026-10-23,
the owner deferred Linux build/runtime work for item 53, then deferred build/test
for macOS capture (54), and moved this fork to item 55 source work (virtual
gamepads and device permissions). Build/test execution is now deferred; do not
dispatch new Antigravity batches until resumed by the owner. Items 53–55 remain
unchecked. Other chats' assignments and remaining 1–109 obligations are retained.

The 2026-10-24 item-55 source checkpoint also moves the legacy keyboard/mouse
helper onto the creation-time Windows job/handle allowlist launcher, using a
fixed typed helper argument. Stop has finite process waits and joins only an
already-finished IPC thread. An unfinished thread retains the process-local
launch permit; unknown process retirement retains its original job/handle and
blocks relaunch. Cancellation/termination is not input-release proof. Build,
runtime and failure-injection qualification remain deferred; item 55 stays open.

The next static review adds gamepad authority/input-gap checks after each
provider write, before another slot is written or a successful frame refreshes
the input clock. Slow creation/output cannot earn fresh authority after the old
deadline. This does not interrupt a blocked OS call or replace the independent
helper watchdog, and fresh native observations still belong to that watchdog.
No build, timing test or device qualification was executed.

Item 55 source checkpoints: private owner/slot lifecycle, Linux uinput and stable
standard-browser sampling are authored. A default-off Windows WinUHid candidate
now adds a hash-pinned static client recipe, generic HID reports and retained
cleanup records until exact-container disappearance. This is not integrated
with broker/native consent, signed-driver admission or a supervised helper;
macOS gamepad implementation remains due. `VIRTUAL_GAMEPAD_OWNERSHIP.md` records
the source and remaining scope. No item-55 build/test or driver install has run.
The subsequent source checkpoint adds two-step device reservation/activation,
pre-creation Windows container identities, a separate parent-heartbeat lease,
real Windows target/activity observations and a default-off private v2 helper
actor/reader. Parent supervision, durable cleanup records and product integration
remain unfinished; this is not an additional accepted item.
The next source checkpoint adds a bounded SQLite ownership/cleanup journal,
pre-activation durable intent, parent-side reply/authority orchestration and an
original-process-handle Windows retirement witness. No current service path
constructs them; real supervised transport, global arbitration, startup recovery
and signed/native admission remain due. Build/test is still deferred.
The latest checkpoint authors the Windows private pipe transport, creation-time
job association, independent authority lease, prioritized heartbeat writer and
reader/watchdog termination path, plus its concrete broker channel adapter.
These sources are not launched by current service commands. Native authority
observation/service wiring, global arbitration, startup recovery, provider/native
admission and all build/runtime qualification remain open.
On 2026-10-24, source work adds read-only Windows startup recovery using a fully
enumerated process snapshot, saved creation identity and an exact-object handle
when still live. The candidate broker recovery composes that witness with the
durable journal/HID query; it does not resume control or terminate PID matches.
Current service startup still does not invoke it. Build/test remains deferred.
The subsequent source checkpoint connects optional Windows-feature startup to
a dedicated, process-bounded recovery lane and an existing-database-only SQLite
connection. Default builds do not start it; close cooperatively cancels without
blocking on unfinished OS calls. Eight journal/connection regression cases are
authored, not executed. Global arbitration and input activation remain open.
The latest source checkpoint adds a singleton shared per-user SQLite owner store,
requires it in parent prepare/activation, mirrors committed profile transitions
and retains profile retirement proof until shared ownership is released. Optional
startup recovery respects shared debt. Twelve regression cases are authored,
not run. Shared-root/ACL admission, lost-store/foreign-profile recovery, actual
concurrency/OS qualification and product input activation remain unfinished.
The following source checkpoint makes ordinary coordination open existing-only,
separates debt-free profile-inventory bootstrap with create-new semantics, and
retains Windows non-delete-sharing file/directory guards. Fifteen regression
cases are authored (one Windows-only); none ran. Native bootstrap/repair and
actual ACL/SQLite/OS validation remain due, with gamepad capability still false.
The next checkpoint changes product-source revocation to wait for the native
acknowledgment, retain document authority for pending/unknown cleanup and block
rearm. The v1 keyboard broker preserves failed/missing Stop observations instead
of reporting unconditional success. Seven async browser-forwarder cases are
authored separately from the fifteen journal cases; all execution and generated
resource assembly remain deferred. Actual gamepad activation is still unwired.

The owner now requests the individually numbered tasks in
[`OUTSTANDING_WORK.md`](OUTSTANDING_WORK.md), completed in order. Task 1 source
integration is complete at commit `35f3271`; task 2 alignment is recorded in
[`DOCUMENTATION_ALIGNMENT.md`](DOCUMENTATION_ALIGNMENT.md). Task 3, the Windows
Chromium development build, is accepted in [`WINDOWS_CHROMIUM_BUILD.md`](WINDOWS_CHROMIUM_BUILD.md).
Task 4 follows in the build chat's sequence; this fork's source assignment above
is unchanged. [`SOURCE_INTEGRATION.md`](SOURCE_INTEGRATION.md)
records the executed scope and open gates. No full product area is closed by
source integration alone.

All sections below are historical records using the older 1–17 product-area
numbers. Phrases such as "current", "latest" and "now" in those records describe
their assignment at the time. Their unclosed obligations remain active, but
their ordering and source-only execution timing do not override the current
1–109 list or the owner's Antigravity execution-only policy.

## Historical transition — product areas 15 then 16

On 2026-10-23 the owner requested noting the Android/iOS gaps and moving this
chat to security/licensing/release preparation (15), then consolidated
verification (16). `DEFERRED_ITEMS_9_10.md` retains the missing source and
external/platform gates. Neither mobile item is complete. Exact frozen test
batches are now released under Antigravity's execution-only policy; Codex owns
review/fixes/acceptance. `ITEM15_SECURITY_LICENSE_RELEASE.md` and
`ITEM16_VERIFICATION.md` track this assignment. Other concurrent assignments
and their outstanding implementation below remain preserved.

## This chat's latest transition — item 13, then item 14

On 2026-10-23 the owner explicitly requested retaining the item-6 gaps and moving
this chat to performance/routing/optional assistance (13), then installation,
repair/uninstallation/updates (14). `ITEM6_REMAINING_WORK.md` records the missing
implementation and deferred evidence. This does not complete item 6, close
intervening items or replace the concurrent chat assignment recorded below.
Source authoring remains first; no new build/test batch is released.

## Current assignment in this chat — items 11 then 12

The owner now defers the outstanding item-7/8 work and directs source work on
Web/PWA (11), then accounts and encrypted save synchronization (12). Retained
gaps are listed in `DEFERRED_ITEMS_7_8.md`; neither item is marked complete.
Source work for 11/12 is tracked in `ITEM11_WEB_PWA_PRODUCT.md` and
`ITEM12_ACCOUNTS_ENCRYPTED_SYNC.md`. Build/test execution remains deferred.

## Earlier assignment — items 9 then 10

The owner now requests source completion of Android (9) then iOS/iPadOS (10).
Current changes and outstanding implementation are recorded in
`ITEM9_ANDROID_PRODUCT.md` and `ITEM10_IOS_IPADOS_PRODUCT.md`. This does not
close items 7/8 or release a new build/test batch. The following 7/8 assignment
note is historical; its uncompleted obligations remain tracked.

The owner requests completing item 7 (saves/library/storage) and then item 8
(Remote Play/installed PC bridge). Their source changes and remaining gates
are tracked in `ITEM7_SAVE_LIBRARY_STORAGE.md` and `ITEM8_REMOTE_PC_BRIDGE.md`.
Other ongoing item-4/item-6 source below is preserved and is not marked complete.
Practical verification remains deferred: no new build/test/Antigravity batch is
released by this assignment. Missing implementation stays in its own item.

The user requested work on items 1 through 17 and a report when each
item is finished. On 2026-10-23 the owner explicitly deferred the remaining
item-2 work and directed continuation with items 3 through 17. Item 17 is the
owner-selected overall review and closure of outstanding work. Deferral does
not mark item 2 complete or remove it from the final review.
Source authoring and the final comprehensive qualification
remain separate; the newer concurrent-test instruction below supersedes the
earlier deferral for explicitly released batches. Source work, executed verification and release
approval are tracked separately. Source completion requires the entire item's
implementation; practical acceptance remains separately deferred by the owner.

The owner assigns Antigravity test execution only. Codex and the owner retain
result review, diagnosis, fix decisions and source edits. The owner now requests
concurrent work: Antigravity may run exact released test batches on frozen
copies while Codex writes source; full acceptance remains item 16. See
`QA_EXECUTION_POLICY.md`.

Current work: **item 6 source fixes; item 4 source activation authorized**.
The latest requested sequence is to finish the remaining item-4 work, then item 6;
item 5 stays pending. Following the earlier automatic rejection, the user
explicitly approved activation before qualification. The diskless guest gate is
now true in source; no binary was built, installed or run. Verified item-4
completion is not claimed. See `MACOS_DISKLESS_GUEST.md` and
`GRAPHICS_AUDIO_INPUT_ITEM6.md`. The following older ordering note is historical:
The owner's newest correction is explicit: finish all required source within
the current item before moving to the next. Do not park missing source in item
17 to advance the sequence. `MACOS_ITEM4_CLOSURE.md` records the current open
implementation obligations; item 4 is not source complete.
The owner's latest direction is source authoring first, practical verification
later. No new build/test/workflow batch is released in this phase. The former
concurrent-test timing is historical and superseded by `QA_EXECUTION_POLICY.md`.
Item 3's remaining source obligations are retained for the overall closure;
moving to item 4 does not mark Linux complete.
Item 2 is deferred by the owner; preserve `WINDOWS_ITEM2_CLOSURE.md` for return
and item 17. `LINUX_ITEM3_CLOSURE.md` tracks current source work and evidence.

The following item-2 notes are historical context from before that work-order
change; their statement that no transition occurred is superseded above.
The current substep investigates real AppContainer storage and loader behavior
after adding retained-work reasons, operator guidance and bounded broker workers.
Selected concurrent Antigravity batches have compiled the broker/worker and
passed the dedicated 13 lifecycle and 19 wire cases on real Windows processes.
These use fixture DLLs/dummy drivers and do not replace the full production
broker/process/core/save/device qualification required by item 16.
The recovery worker also retains a process-wide single slot across broker
replacement. `WINDOWS_ITEM2_CLOSURE.md` is the remaining source-closure list;
no item-2 completion or transition to item 3 has been recorded.
Batch034 on source04cbd74 now passes one actual device-free actor media case
under production Win32k/AppContainer/job launch with a fixture core and headless
presentation peer. Static actor imports are KERNEL32 only. Batch035 rebuilds
the direct SDL mode on the same source and passes its 35 existing regression
cases. Production helper/core/device and the remaining closure obligations stay
open; these results do not complete item 2.
The five reason counters do not grant deletion authority or complete legacy
migration. The three retained-work regression fixtures passed in batch 005;
actual batch028 now passes the two assigned storage probes; this establishes
only their explicit observations, not whole-core storage compatibility.
Earlier N64 source work adds bounded snapshot event queues, PIF pointer
preflight, format-selected DD ID reads, TLB field/lookup admission and finite
last-page map/unmap walks with an interpreter random-index guard.
Transfer Pak selector admission, complete GB storage/packet extents, full-width
high RAM banks, rumble bank separation and bounded camera startup/output are
also authored. Bounded MBC3 time/halt/carry and persistent latch source now has
a separate gl3/1.10 snapshot candidate with legacy imports. RTC battery binding,
staged boot/offline time and checked RAM/RTC retirement are now source-authored.
Actual persistence/device timing and remaining upstream cartridge implementations
still require completion/qualification.
Flash/SI complete-range memory checks, DD C2 sector/zone bounds, active PIF
opcode admission and known snapshot device selectors are also source-authored;
actual PI/CPU/graphics/device and all original qualification remain open.
PI adjusted RDRAM admission, per-page framebuffer readback, bounded descriptor/
mapping/CPU-wrapper source and the six-slot graphics cache fix are now authored.
Direct SRAM/ROM/DD complete-range DMA, physical short-SRAM-write notifications,
checked source words and nonzero-only code invalidation are now source-authored.
Fixed exact dirty-byte coverage and bounded CPU color conversion/upload geometry
are now source-authored. Image-loader admission, whole GPU resource accounting
and submission/driver failure acknowledgement remain work; actual CPU/core/
graphics/device qualification remains due.
ROM boot-extent/signature/word/size admission, byte-wise format conversion and
bounded header-name diagnostics are now authored. Full frontend loader ordering,
metadata and actual ROM/boot/core compatibility qualification remain due.
Frontend early ROM/path admission, companion allocation checks, staged header
query and retained ROM-open/failed-close ownership are now authored. Complete
setup/graphics/audio/subsystem lifecycle and actual qualification remain due.
Checked partial audio allocation/cleanup and explicit frontend readiness are
now authored. Full AI/sample/resampler/output/backpressure, audio-device and
graphics-context failure closure remain unfinished and unqualified.
Finite audio output buffering, bounded zero/partial consumer handling and
root backpressure are now authored. Producer resampler/AI bounds, snapshot
audio transitions and actual stalled-device/worker qualification remain due.
Audio callback borrow now gates root reentry/reset/deinit/callback replacement
and audio retirement gates final owner/table detach. Actual export retry and
CoreShutdown/context/device qualification and producer bounds remain due.
AI complete-RAM publication and immutable private backend sample copies are
now authored. AI duration/delayed-carry/snapshot timing and resampler output
admission remain source work; real audio/driver/worker qualification remains due.
Wide AI duration admission and delayed-carry checks are now authored, preserving
admitted arithmetic order. FIFO failure rollback/overflow, zero events, snapshot
timing and real scheduling compatibility remain open, alongside resampler bounds.
Non-Apple sinc pre-write phase/output admission and bounded input subdivision
are now authored. Actual C/SIMD phase/sample equivalence, filter internals,
Apple accumulation and low-rate queue pressure remain unfinished/unqualified.
Sinc filter-table/tap allocation bounds and per-instance processor capture are
now authored, with an actual pinned-sinc/allocator qualification target. All
new cases remain unexecuted; real ISA/sample/device qualification remains due.
AI first-slot start and pending-slot promotion now stage descriptor/status
publication until admission succeeds. Void event insertion receipts, fatal
CPU stop, snapshot/zero/full-FIFO semantics and actual timing remain open.
AI checked pool insertion and failure-stop source are now authored; actual
event delivery/CPU-return/save retirement and other event producers remain due.
Explicit post-stop peripheral save recovery now has source admission distinct
from failed CPU readiness. Actual guest unwind/worker capture/crash persistence
and all original Windows completion requirements remain open.
Common native graphics callback release now retains pre-invocation failures,
latches unknown callback exceptions and prevents DLL unload on failed release.
Real callback/driver/process and final-save qualification remain due.
Native context reset now also gates callback reentry and publishes readiness
only after return; unknown reset failure prevents core/DLL unload. Actual
reset/release/driver and process-exit acceptance remains unexecuted.
Native audio ring receipts now report copied frames, and N64 offers match the
worker batch cap; actual partial/zero admission, latency and device qualification
remain open.
Native consumer slots/priming/underruns now commit only after a complete sink
receipt; rejection latches failure and clears SDL's potentially partial queue.
Device-control receipts, deliberately discarded paused producer flushes,
atomic presentation batch admission and same-epoch media-clear generations are
now source-authored. Actual production-ring cases and full device/core/transport
failure acceptance remain unexecuted in `qa/native-audio` and Windows case 24.
Checked guest-pause protocol 1 is now authored across worker, selected native
adapters, build receipts, catalog admission and Windows/Linux caller identity.
Unconfirmed pause and unload/deinit callbacks prevent replay or DLL destruction.
Windows fatal actor termination now bypasses DLL detach through the dedicated
owned-process primitive; actual worker/DLL/pipe/producer and detach-witness
fixture source is in `qa/native-lifecycle`. Case 25 and all real process/core/
save/driver qualification remain unexecuted. Legacy barrier packages need rebuild.
Complete semantic snapshot/resource
admission, remaining writer closure and actual Windows qualification are open.
Item 1's authored integration is retained for actual item-16 qualification;
verified completion has not been claimed.

| Item | Scope | Source state | Verified completion |
| --- | --- | --- | --- |
| 1 | Desktop Chromium, Game Hub, profiles, permissions and build/update integration | Source authored; current desktop overlay and build/upgrade recipes retained | Pending final pass; remains open |
| 2 | Windows native runtime lifecycle and containment | Deferred by owner; remaining source obligations retained | Partial fixture evidence; full acceptance pending |
| 3 | Linux native runtime lifecycle and containment | Remaining source obligations retained; owner directed next item | Eight socket cases, runtime/broker check and bootstrap build passed; full acceptance pending |
| 4 | macOS native runtime lifecycle and containment | Diskless guest source authored; source activation explicitly authorized and enabled | Deferred per latest owner direction |
| 5 | v1 retro systems and core adapters | Pending; latest requested next item is 6 | Pending final pass |
| 6 | Graphics, audio and input | Adapter fixes plus Web raw calibration, Web/SDL axis tuning and custom touch editor/runtime authored; native raw providers/overlays and broader backend scope remain open | Pending final build/device/fault pass; see ADVANCED_INPUT_CUSTOMIZATION.md |
| 7 | Saves, library and storage | Remaining work deferred by owner; authored backup/recovery source retained; see DEFERRED_ITEMS_7_8.md | Source unverified; remains open |
| 8 | Remote Play and installed PC/AAA bridge | Remaining work deferred by owner; authored Windows binding/lifecycle source retained; see DEFERRED_ITEMS_7_8.md | Source unverified; remains open |
| 9 | Android product | Deferred by owner; remaining source in DEFERRED_ITEMS_9_10.md | Android SDK/device qualification pending |
| 10 | iOS/iPadOS product | Deferred by owner; remaining source in DEFERRED_ITEMS_9_10.md | Apple SDK/device qualification pending |
| 11 | Web/PWA product | Cached-resource verification and reviewed offline-core preparation authored; see ITEM11_WEB_PWA_PRODUCT.md | No new build/test; remains open |
| 12 | Accounts and encrypted save synchronization | Startup cancellation and guarded credential/vault publication authored; see ITEM12_ACCOUNTS_ENCRYPTED_SYNC.md | No new build/test; remains open |
| 13 | Performance, routing and optional assistance | Transactional advice acceptance, policy checks and measurement-origin fixes authored; full scope remains open (ITEM13_PERFORMANCE_ROUTING_ASSISTANCE.md) | Source review only; final pass pending |
| 14 | Install, repair, uninstall and updates | Extraction cancellation, inventory verify CLI, path checks and schema-floor fixes authored; repair/uninstall/release scope open (ITEM14_INSTALL_REPAIR_UPDATES.md) | Source review only; final pass pending |
| 15 | Security, licensing and release preparation | Release attribution/privacy gate and dependency review inventory authored; production reviews open | Scoped release/updater cases passed; full release approval pending |
| 16 | Consolidated build, integration, device and fault verification | Active frozen verification batches; original source gaps retained | Initial Windows/shared-source cases passed; platform/device/fault qualification open |
| 17 | Overall review and closure of outstanding work | Owner-selected final review, including deferred item 2 | Not started |

Existing implementation in every item is preserved. This table records the
requested work order and does not reset or remove earlier implementation.

## Item 1 current source changes

- Shared exact live Game Hub document admission for browser and native commands.
- Lazy trusted-storage profile identity with rejection of corrupt/nil records.
- Bounded authentication/replay admission and generation-aware native ports.
- Document-owned native sessions, selected file grants, imports, transfer
  directions, media and input handles across a shared profile broker.
- Cleanup handling for late native session/preparation creation results.
- Obsolete generated component-resource retirement during source staging.
- Windows/Linux/macOS development/release GN selection.
- Frozen original/overlay/resource/recipe inventories and an explicit later
  full Chromium dependency/build procedure with resumable phase journal.
- Derived component-key/allowlist/native-origin agreement and explicit upstream
  snapshot migration admission.
- Three-sided source-only review for an explicitly newer upstream commit,
  preserving current originals/product edits and active revision selection.
- Authored document-boundary unit cases and a real desktop acceptance matrix,
  both reserved for the final consolidated pass.

See `DESKTOP_BROWSER_BUILD.md` for concrete source/verification completion gates.
No build, browser or test has been executed for these changes.

## Item 1 requirement-to-source trace

| Requirement | Authored source | Still required to verify |
| --- | --- | --- |
| Minimal revision-bound Chromium integration | `browser/upstream.lock.json`, `browser/patches/series.json`, `tools/generate_browser_patch.py` | Exact target builds and upstream-upgrade rehearsal |
| Normal/private profiles and product isolation | Component loader/allowlist/product-path patches, `apps/game-hub/manifest.json` | Real new-tab, private-window and two-profile behavior |
| Live-document and transient capability authority | `component-context.js`, `component-ownership.js`, `background.js`, independent broker profile/origin checks | Real multi-tab/worker/native-message/fault cases |
| Ordinary browsing/dashboard | `browser-service.js`, `browser-dashboard.js`, native Chromium controls | Live routing, tab changes, metadata privacy and native-host failure behavior |
| Exact source staging and target build procedure | `stage_browser_integration.py`, `prepare_desktop_browser.py`, `build_desktop_browser.py`, three desktop GN pairs | Actual compiler/SDK, dependency, artifact and rebuild comparison |
| Maintained upstream migration | `prepare_chromium_upgrade.py`, unchanged active pin and three-sided review output contract | Candidate adaptation, actual candidate build and security freshness review |
| Final evidence plan | `qa/browser/DESKTOP_ACCEPTANCE.md`, authored document/context unit sources | Execute the full matrix in item 16 |

Item 1 remains open until that final evidence exists. Core artifacts, packaging,
performance and release policy are covered by their original later items and
must still be connected to the final desktop build.

## Item 2 current source work

- Creation-time Job Object association for both Windows actor and presentation
  helper, verified before journal identity publication and resume.
- Serialized AppContainer grant/launch/retirement lifecycle with no new grants
  after launch.
- External confirmed-retirement proof, separate collector lease and retiring
  ledger admission for resumable partial work deletion.
- Bounded rotating startup recovery cursor.
- Creation-lease/owner-lease handoff, immutable schema-15 Windows pre-launch
  reservation marker and bounded ledger-only recovery without legacy promotion.
- Authored actual Windows sharing/partial-creation/legacy-retention cases and a
  schema-14 to schema-15 debt-preservation case, reserved for item 16.
- Fresh default AppContainer filesystem sealing and exact actor token/low
  integrity/no-write-up admission, with an authored real-actor final-pass probe.
- Exact-token single-use private registry lookup, matching effective SID grant,
  bounded link-refusing preflight and registry/filesystem sealing before process
  publication/resume; expanded actual-actor probe-v2 and caller-context cases.
- Windows Settings status/inspect/recover/stop flow with typed reservation
  totals, a single off-control-thread startup/manual owner, cooperative
  cancellation and document-owned job IDs.
- Bounded memory-file ABI/store, common save capture/atomic staged replacement,
  and an exact Windows MD core/worker/package route without actor OS save/scratch
  write grants; standalone contract and actual core/OS acceptance sources.
- Native3 PS1 FrontIO/async/checked-card memory publication, read-failure
  preservation, pre-swap writer drain and exact no-OS-write package admission;
  real C bridge/store qualification sources, all unexecuted.
- N64 scratch-only C stdio/C++ stream/gzip adapters with atomic complete-image
  publication, page/stream/decoded-byte limits and unresolved-close retention;
  actual adapter/store/vendor-codec qualification source, all unexecuted. N64
  core/cache integration and package/OS-write admission remain in progress.
- Separate pinned N64 gl2 configuration/database/custom-settings and shader
  integration source; bounded complete pre-GL cache decoding, checked coroutine
  init/deinit and owned Windows root/guest fiber retirement. Texture/file/dump/
  peripheral write closure, exact package admission and real qualification
  remain due; the candidate is not enabled or built.
- Actual N64 texture-memory gzip and file-index cache source now use bounded
  staged records, checked nested expansion/index/offsets, snapshot-backed pending
  data and complete-image publication. Actual adapted-cache and independent
  format/fault fixture sources are authored. Dumps/other core/peripheral writes,
  real shared/thread pool bounds and full package/OS qualification remain due.
- N64's actual shared/workspace pool now has checked 4096/one-lane admission,
  finite live/transient payload, staged allocation and owned borrow/retirement
  scopes. Filter init/output/OOM acknowledgement and standalone deposterize are
  authored; cache cases now use that real pool. Common worker source exits its
  actor before unload/context destruction on refused table uninstall. Pipeline
  temporaries, remaining writers and actual full core/OS evidence remain due.
- Selected N64 texture PNG dumps now use a C encoder with bounded source/stride,
  8-MiB codec allocation and checked scratch publication. Texture diagnostics
  use finite scratch logs; native messages use literal-format callbacks. Actual
  encoder/store/codec and prepared-diagnostic qualification sources are authored,
  all unexecuted. Other state/peripheral writes and original final scope remain.

- Selected N64 persistent file-storage/utility and Transfer Pak RAM source now
  uses a separate save namespace, exact staged read/close admission, dirty full-
  image retry and readonly legacy import. Actual storage/store fault/quota case
  sources are authored, not executed. DD/state/producer acknowledgement and the
  full package/OS/final scope remain open.

- Selected 64DD source now stages bounded MAME/SDK/D64 admission, expansion
  length, real geometry/sector ranges, owned memory saves and readonly legacy
  import. Failed close retains its descriptor and blocks deinit/uninstall;
  unstarted IPL rejection does not manufacture a default save. Actual-source
  synthetic/fault cases are authored, not executed; producer/capture/package/
  OS and every original final requirement remain due.

- Selected N64 source now has thread/fiber-owned real prepare/paused guest
  checkpoints, checked snapshots/peripheral capture and retained dirty-close
  ownership. Snapshot mutex/early-rejection lifetime is explicit. The common
  worker requires actual table detach before restoring memory files. Actual
  owner/storage and Windows-fiber qualification sources are authored, not run;
  full core/package/OS/restore/device verification and remaining scope stay open.

Item 2 remains unfinished and is now deferred by the owner.
`WINDOWS_ITEM2_CLOSURE.md` lists source gaps and scoped executed evidence;
`WINDOWS_NATIVE_COMPLETION.md` retains the historical detailed record.
