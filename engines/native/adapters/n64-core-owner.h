#ifndef OMNIPLAY_N64_CORE_OWNER_H
#define OMNIPLAY_N64_CORE_OWNER_H
#include <stddef.h>
#ifdef __cplusplus
extern "C" {
#endif
enum OmniN64Action {OM_N64_PREPARE=1,OM_N64_RUN=2,OM_N64_SAVE=3,OM_N64_LOAD=4,OM_N64_STOP=5,OM_N64_FLUSH=6};
/* Every transition binds the actual calling OS thread and exact root/guest
 * tokens. No mutex remains held while a caller switches fibers or runs I/O.
 * Tokens are compared, never dereferenced. Snapshot buffers remain borrowed
 * until checked guest completion/exit, then root finish clears the operation. */
int om_n64_owner_begin(void);
int om_n64_owner_thread(void);
int om_n64_owner_access(void); /* standalone helpers, or this active owner */
int om_n64_owner_bind(void* root,void* guest);
int om_n64_owner_loaded(void* root);
int om_n64_owner_enter(void* root,int action,void* buffer,size_t bytes);
int om_n64_owner_action(void* guest,void** buffer,size_t* bytes);
int om_n64_owner_boundary(void* guest,int initial);
int om_n64_owner_snapshot_done(void* guest,int success);
int om_n64_owner_ended(void* guest,int success);
int om_n64_owner_finish(void* root);
int om_n64_owner_at_boundary(void* root);
int om_n64_owner_ready(void* root);
int om_n64_owner_started(void);
int om_n64_owner_stopped(void* root);
int om_n64_owner_cancel_unstarted(void* root);
/* Persistent-peripheral flush is admitted at a healthy pause or confirmed
 * stopped guest, including failure. Its receipt never resumes the guest. */
int om_n64_owner_flush_begin(void* root);
int om_n64_owner_flush_end(void* root,int result);
void om_n64_owner_admission_failed(void);
int om_n64_owner_admitted(void);
/* Failed sessions may publish retained peripherals only inside a checked
 * post-stop flush, never while guest execution or snapshot buffers are live. */
int om_n64_owner_persistence_allowed(void);
int om_n64_owner_end(void* root);
int om_n64_owner_quiescent(void);
/* Actual C core checkpoints, provided by n64-core-barrier.c.inc. */
int om_n64_guest_initial_checkpoint(void);
void om_n64_guest_interrupt_checkpoint(void);
int om_n64_native_peripherals_quiet(void);
int om_n64_native_flush_peripherals(void);
int om_n64_native_retire_peripherals(void);
#ifdef __cplusplus
}
#endif
#endif
