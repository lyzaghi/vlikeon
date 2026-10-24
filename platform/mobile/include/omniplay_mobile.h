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
OmniBuffer om_mobile_attach_pairing_credentials_v1(uint64_t handle,OmniPairingCredentialCallbacks callbacks);
/* Trusted native controller only, never renderer commands. Status returns
 * bounded public lifecycle metadata. Preparation requires foreground, attached
 * scoped provider, current journal revision, budget 1000..25000 ms and enabled
 * installed policy; current policy refuses it. Result is MaterialReady public
 * metadata only, not an activated identity/pair. Errors may follow durable
 * side effects: reload status before retry. No keys/seeds leave native memory.
 */
OmniBuffer om_mobile_pairing_status(uint64_t handle);
OmniBuffer om_mobile_pairing_prepare_identity_v1(uint64_t handle,uint64_t expected_revision,uint32_t budget_ms);
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
