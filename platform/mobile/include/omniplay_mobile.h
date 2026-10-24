#ifndef OMNIPLAY_MOBILE_H
#define OMNIPLAY_MOBILE_H
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef struct {uint8_t *data;size_t length;} OmniBuffer;
/* Trusted native-only credential ABI 2. profile is exactly 16 UUID bytes.
 * read returns 1..4096 bytes, -1 absent, -2 locked, -3 failure. write returns 0
 * only after native secure storage acknowledges; remove also permits -1 absent.
 * Buffers are borrowed for the synchronous callback and must never be retained.
 * The host must support calls from worker threads and must not reenter Rust.
 * Context ownership is consumed by open_with_credentials even on error. release
 * must be callable on rejection and is called once after the final owner drops.
 * No callback, context, secret or path is part of the renderer protocol.
 * check is a noninteractive lock/profile access check, 0 permitted, -2 locked,
 * -3 failure. It must work when no credential exists and must not return secrets.
 * v2 uses a distinct open symbol; an old/smaller callback table is not compatible.
 */
typedef struct {
  uint32_t abi_version;
  void *context;
  int64_t (*read)(void *context,const uint8_t *profile,uint8_t *output,size_t capacity);
  int32_t (*write)(void *context,const uint8_t *profile,const uint8_t *secret,size_t length);
  int32_t (*remove)(void *context,const uint8_t *profile);
  void (*release)(void *context);
  int32_t (*check)(void *context,const uint8_t *profile);
} OmniCredentialCallbacks;
/* Dedicated Remote Play ABI 1, NOT compatible with save-sync callbacks above.
 * Four UUID arrays are network-order UUID bytes; kind 1=identity, 2=pair.
 * Scope is the actual native profile/device. read capacity/write length is
 * exactly 137 bytes (VLKRP001 secret envelope). read returns 137, -1 absent,
 * -2 locked, -3 failure/corrupt; write/check return 0, -2 locked, -3 failure.
 * remove permits 0 or -1 absent. Never turn a locked/error vault into absence.
 * Separate OS-vault service org.vlikeon.remote-play.credentials.v1 is required.
 * Same synchronous/thread/reentry/context-consumption contract as above.
 * This table is source ABI only; no renderer or mobile-open endpoint is added.
 */
typedef struct {
  uint8_t profile[16],device[16],item[16],generation[16];
  uint32_t kind;
} OmniPairingAddress;
typedef struct {
  uint32_t abi_version;
  void *context;
  int64_t (*read)(void *,const OmniPairingAddress *,uint8_t *,size_t);
  int32_t (*write)(void *,const OmniPairingAddress *,const uint8_t *,size_t);
  int32_t (*remove)(void *,const OmniPairingAddress *);
  int32_t (*check)(void *,const OmniPairingAddress *);
  void (*release)(void *);
} OmniPairingCredentialCallbacks;
/* Native-only attachment/scope, not a renderer method. Context is consumed on
 * EVERY attach return, including stale host and duplicate provider. Foreground
 * gates access; terminal host close cannot be revived by a late callback.
 * Attached is not a pairing/streaming capability and does not unlock the vault.
 */
OmniBuffer om_mobile_pairing_scope(uint64_t handle);
/* Trusted native lock/protected-data/cancel transition only. Invalidates the current
 * provider epoch and permanently cancels its old network token, without waiting
 * for LocalHost I/O or enabling foreground. Returns 1 for observed invalidation,
 * unattached or terminally removed handle; 0 for invalid/unavailable registry.
 * No key/OS callback is made. This is not a backend-stop or rollback receipt.
 */
uint8_t om_mobile_pairing_invalidate_epoch(uint64_t handle);
OmniBuffer om_mobile_attach_pairing_credentials_v1(uint64_t handle,OmniPairingCredentialCallbacks callbacks);
/* Native UI only, never renderer dispatch. begin JSON <=16384 bytes contains
 * identity (exact status address), expected_revision, code (bearer proof),
 * host_public_key (independently confirmed), audience and label. Caller clears
 * its input/share/clipboard copies; native parsing cannot guarantee clearing
 * parser-internal allocations. Budget 1000..25000 ms includes mutex wait.
 * action JSON <=128 bytes: {"ticket":"UUID"}; action 1 join, 2 poll, 4 cancel.
 * Action 3 JSON <=256 bytes: {"ticket":"UUID","reviewed_grant_hash":"64 hex"}.
 * Persist AFTER explicit native peer/comparison/input consent to the exact
 * grant_hash from approved poll; missing/mismatched hash refuses persistence.
 * Epoch is sourced from attached foreground host, never supplied by caller.
 * Cancel first calls invalidate_epoch to signal in-flight work, then queues
 * action 4; cancel is local only and does not revoke relay-approved pairs.
 * Errors may follow candidate commit or retained pending state: inspect/reconcile
 * before retry. Action 3 yields MaterialReady only; no activation/session grant.
 */
OmniBuffer om_mobile_pairing_client_begin_v1(uint64_t handle,const uint8_t* data,size_t length,uint32_t budget_ms);
OmniBuffer om_mobile_pairing_client_action_v1(uint64_t handle,const uint8_t* data,size_t length,uint32_t action,uint32_t budget_ms);
/* Native UI only after explicit consent to the exact persisted candidate/grant.
 * JSON <=1024: {address,expected_revision,reviewed_grant_hash}; budget 1000..25000.
 * Reloads under the scoped credential lease and requires fresh bound relay
 * approval before Active publication. No transport/session authority is granted.
 * Errors may follow commit: inspect native status, never assume rollback. */
OmniBuffer om_mobile_pairing_activate_client_v1(uint64_t handle,const uint8_t* data,size_t length,uint32_t budget_ms);
/* Native-only interrupted-candidate review. JSON <=1024: {address,expected_revision}.
 * Budget 1000..25000 ms includes waiting. Reloads vault/journal/signed live grant;
 * no relay call, activation or consent. comparison_available is false: UI must
 * independently verify host and obtain fresh consent, not invent old code. */
OmniBuffer om_mobile_pairing_review_client_v1(uint64_t handle,const uint8_t* data,size_t length,uint32_t budget_ms);
/* Native host controller only. begin <=1024: {identity,expected_revision,audience,label}.
 * action <=128: {ticket}; 1 register, 2 explicit poll, 3 close. Budget 1000..25000.
 * Foreground provider epoch/cancel is native-derived. No proof sharing or peer
 * approval. Late errors may leave pending/registered/closed state: inspect status.
 * Close currently requires foreground provider; background/crash recovery is due. */
OmniBuffer om_mobile_pairing_host_begin_v1(uint64_t handle,const uint8_t* data,size_t length,uint32_t budget_ms);
/* Explicit native share only, ticket JSON <=128, budget 1000..25000.
 * SUCCESS: raw OPRP1 UTF-8 bytes <=8192 containing bearer proof, NOT JSON.
 * FAILURE: {NULL,0}; read native status for reconciliation, never auto-retry.
 * Never log/forward to renderer. Clear UI/OS copies when possible; always call
 * om_mobile_free (which wipes the owned native buffer). No OS copy wipe guarantee. */
OmniBuffer om_mobile_pairing_host_share_v1(uint64_t handle,const uint8_t* data,size_t length,uint32_t budget_ms);
OmniBuffer om_mobile_pairing_host_action_v1(uint64_t handle,const uint8_t* data,size_t length,uint32_t action,uint32_t budget_ms);
/* Native peer review only, <=1024: {ticket,client_public_key,reviewed_hello_hash}.
 * action 1 adds explicit input_allowed/lifetime_ms and persists after actual
 * peer/comparison/input consent. action 2 declines; no permission fields allowed.
 * Budget 1000..25000. No activation; late errors require status reconciliation. */
OmniBuffer om_mobile_pairing_host_peer_v1(uint64_t handle,const uint8_t* data,size_t length,uint32_t action,uint32_t budget_ms);
/* Native candidate action <=1024: 1 review {address,expected_revision}; 2 activate
 * adds reviewed_grant_hash after actual native peer/input consent. Budget 1000..25000.
 * Exact host-role grant/vault/policy checks apply. Review grants no consent;
 * activation grants no transport. Late errors require status reconciliation. */
OmniBuffer om_mobile_pairing_host_candidate_v1(uint64_t handle,const uint8_t* data,size_t length,uint32_t action,uint32_t budget_ms);
/* Explicit native restart cleanup <=256: {invite_id,expected_revision} from status.
 * Budget 1000..25000. Foreground provider required; expired invites require exact
 * server expiry/absence receipt. Bound close precedes durable intent completion; reload status
 * after every result. Does not resume sharing/approval or revoke existing pairs. */
OmniBuffer om_mobile_pairing_recover_host_close_v1(uint64_t handle,const uint8_t* data,size_t length,uint32_t budget_ms);
/* Trusted native controller only, never renderer commands. Status returns
 * bounded public lifecycle metadata. Preparation requires foreground, attached
 * scoped provider, current journal revision, budget 1000..25000 ms and enabled
 * installed policy; current policy refuses it. Result is MaterialReady public
 * metadata only, not an activated identity/pair. Errors may follow durable
 * side effects: reload status before retry. No keys/seeds leave native memory.
 */
OmniBuffer om_mobile_pairing_status(uint64_t handle);
/* Explicit native recovery only. Input is borrowed JSON at most 1024 bytes:
 * {"address":<exact status address>,"expected_revision":<status revision>}.
 * Budget is 1000..25000 ms. May permanently fence the selected generation;
 * identity fencing also fences its pairs. Never dispatch a plan automatically
 * or treat MaterialReady as abandoned merely because it appears in a plan.
 * Only metadata fencing/finalization is executed; unobserved writer/session/
 * relay/vault steps stay pending. No retirement flags are accepted. Allowed
 * while installed streaming policy is disabled. Reload after every outcome or
 * error, which may follow a durable commit. This does not stop native backends.
 */
OmniBuffer om_mobile_pairing_advance_metadata_v1(uint64_t handle,const uint8_t* data,size_t length,uint32_t budget_ms);
/* Explicit native user approval of revoking an Active generation. Same bounded
 * address/revision request and budget as metadata recovery. Identity revocation
 * also fences its pairs. Requires current revision/namespace, but no enabled
 * streaming policy or accessible vault. Does not stop backends, revoke remotely
 * or delete keys; all cleanup/session debt remains. Reload after success/error.
 */
OmniBuffer om_mobile_pairing_revoke_active_v1(uint64_t handle,const uint8_t* data,size_t length,uint32_t budget_ms);
/* Same request/budget contract, but explicit native approval of permanent
 * exact-generation vault removal. Current plan MUST be RemoveMaterial, after
 * durable writer/local/relay retirement and identity-child cleanup. Performs
 * actual provider deletion/read-back under the retained credential lease.
 * Locked, corrupt, failed or late observations do not prove absence. Errors may
 * follow successful deletion/commit: reload instead of assuming rollback.
 */
OmniBuffer om_mobile_pairing_remove_material_v1(uint64_t handle,const uint8_t* data,size_t length,uint32_t budget_ms);
/* Same bounded recovery request; explicit native approval of relay revocation.
 * Requires prior writer/local retirement, retained native signing identity,
 * installed HTTPS origin allowlist and exact pair/grant receipt. New sessions
 * may stay disabled; empty installed allowlist refuses all relay traffic.
 * Foreground/close transitions cancel the operation's original token, which is
 * never reset on resume. HTTP/OS cancellation is cooperative, not preemption;
 * uncertain errors require reload of the same grant. No URL/receipt flags input.
 */
OmniBuffer om_mobile_pairing_revoke_relay_v1(uint64_t handle,const uint8_t* data,size_t length,uint32_t budget_ms);
OmniBuffer om_mobile_pairing_prepare_identity_v1(uint64_t handle,uint64_t expected_revision,uint32_t budget_ms);
/* Native UI must display the returned exact identity/action, then invoke
 * confirmation ONLY after explicit user approval. Token alone is not consent.
 * One review per host; replacement/cancel/detach/close consumes it. Review expires
 * within 60 seconds and is bound to revision, generation and foreground epoch.
 * confirm input is borrowed JSON {"review_id":"UUID"}, at most 128 bytes;
 * budget is 1000..25000 ms. Confirmation consumes the review before vault work.
 * An error after activation requires status reload, never automatic retry.
 * Activation approves this local identity only, not any remote peer/session.
 */
OmniBuffer om_mobile_pairing_review_identity_v1(uint64_t handle,uint64_t expected_revision);
OmniBuffer om_mobile_pairing_confirm_identity_v1(uint64_t handle,const uint8_t* data,size_t length,uint32_t budget_ms);
OmniBuffer om_mobile_pairing_cancel_identity_review(uint64_t handle);
/* Trusted native document-tree reader ABI 1. open receives 16 bound profile
 * bytes and a borrowed normalized relative UTF-8 path (<=2048 bytes, <=8 parts).
 * It must authorize each part within the user-selected tree and return a unique
 * nonzero token, length 1..4 GiB or -1 if unknown. A token may defer opening its
 * actual data stream until read so up to 129 planned tokens need no data handles.
 * read writes at most capacity (<=64 KiB), returns 0 at EOF or a positive count.
 * Error codes: -1 missing, -2 cancelled, -3 denied, -4 deadline, -5 storage.
 * open returns 0 only on success; on failure it owns cleanup and returns no token.
 * cancel must only mark cancellation and schedule native IO cleanup, returning
 * without waiting for a read/close/provider or reentering Rust. The import job
 * uses it to interrupt supported native IO. close releases exactly that stream,
 * including lazy unopened streams; release
 * releases the context and any remaining native resources once. Callbacks must
 * be thread-safe, borrow pointers synchronously, never overrun/retain buffers or
 * reenter Rust, and supply native cancellation/IO coordination/cleanup. Rust's
 * count checks cannot repair an invalid native callback's memory writes.
 * Context ownership is consumed by authorize_media on success or rejection.
 * No callback/context, provider URI or host path is part of renderer IPC.
 */
typedef struct {uint64_t token;int64_t byte_length;} OmniMediaStream;
typedef struct {
  uint32_t abi_version;
  void *context;
  int32_t (*open)(void *context,const uint8_t *profile,const uint8_t *relative,size_t length,OmniMediaStream *stream);
  int64_t (*read)(void *context,uint64_t token,uint8_t *output,size_t capacity);
  void (*close)(void *context,uint64_t token);
  void (*cancel)(void *context);
  void (*release)(void *context);
} OmniMediaCallbacks;
/* Inputs are borrowed UTF-8 JSON bytes for this call only, bounded to 1 MiB.
 * Returned buffers are owned by Rust and must be freed exactly once by the caller.
 * Never call these native-only entry points with filesystem paths from a web page.
 */
OmniBuffer om_mobile_open(const uint8_t *json,size_t length);
OmniBuffer om_mobile_open_with_credentials_v2(const uint8_t *json,size_t length,OmniCredentialCallbacks callbacks);
OmniBuffer om_mobile_request(uint64_t handle,const uint8_t *json,size_t length);
OmniBuffer om_mobile_authorize(uint64_t handle,const uint8_t *json,size_t length);
OmniBuffer om_mobile_authorize_media(uint64_t handle,const uint8_t *json,size_t length,OmniMediaCallbacks callbacks);
/* Native picker preparation may retire elapsed grants before allocating another
 * bounded provider context, even when no renderer RPC occurred in between. */
OmniBuffer om_mobile_expire_capabilities(uint64_t handle);
/* Native lifecycle only: 0 cancels the current foreground sync operation, 1
 * permits a subsequent explicit start. It never resumes a cancelled sync job.
 * Call on a dedicated native lifecycle queue, independently of request I/O.
 */
void om_mobile_set_foreground(uint64_t handle,uint8_t foreground);
/* Native OS scheduler only. Policy needs the serialized request lane. Start
 * admits only an inactive app, explicit persisted consent, registered account
 * and an idle operation lease. Budget is 1000..25000 ms, independently watched.
 * The returned status includes a UUID id. Status/cancel borrow exactly 36 lower
 * case ASCII UUID bytes; cancel is 0 or 1. They bypass the host request mutex.
 * Expiry requests cooperative cancellation; live credential/HTTP owners remain
 * retained until the worker finishes, even after the OS completes its task.
 * These APIs never activate foreground access or perform an account mutation.
 */
OmniBuffer om_mobile_background_policy(uint64_t handle);
OmniBuffer om_mobile_background_start(uint64_t handle,uint32_t budget_ms);
OmniBuffer om_mobile_background_status(uint64_t handle,const uint8_t *id,size_t length,uint8_t cancel);
/* Pause/flush are queued independently after the native foreground notification;
 * they do not overwrite a newer resume notification while awaiting file I/O.
 */
OmniBuffer om_mobile_suspend(uint64_t handle);
/* Revoke transient renderer grants, transfers and sessions when its channel ends. */
OmniBuffer om_mobile_detach(uint64_t handle);
void om_mobile_close(uint64_t handle);
void om_mobile_free(OmniBuffer buffer);
#ifdef __cplusplus
}
#endif
#endif
