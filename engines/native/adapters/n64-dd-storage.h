#ifndef OMNIPLAY_N64_DD_STORAGE_H
#define OMNIPLAY_N64_DD_STORAGE_H
#include <stdint.h>
struct dd_disk;
struct storage_backend_interface;
#ifdef __cplusplus
extern "C" {
#endif
/* Borrowed source path. Mode 0=full, 1=RAM (MAME uses full), -1=readonly.
 * Success publishes a complete owned descriptor; failure changes no output.
 * Disk must initially be empty. All calls belong to the serialized core owner.
 * Readonly original/legacy paths are never used as write destinations. */
int om_n64_dd_open(struct dd_disk*,const struct storage_backend_interface**,const char*,int);
int om_n64_dd_flush(struct dd_disk*);
int om_n64_dd_close(struct dd_disk*);
int om_n64_dd_flush_all(void);
int om_n64_dd_quiescent(void);
int om_n64_dd_retire_all(void);
/* Bounded raw-word snapshot ID at the admitted format's offset. Output stays
 * unchanged on failure. This is compatibility matching, not authentication. */
int om_n64_dd_snapshot_id(const struct dd_disk*,uint32_t*);
/* Release only before any guest producer has received the descriptor, e.g.
 * rejected IPL admission. Does not create/rewrite a save on failed startup. */
void om_n64_dd_discard_unstarted(struct dd_disk*);
#ifdef __cplusplus
}
#endif
#endif
