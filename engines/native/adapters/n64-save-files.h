#ifndef OMNIPLAY_N64_SAVE_FILES_H
#define OMNIPLAY_N64_SAVE_FILES_H
#include <stddef.h>
#ifdef __cplusplus
extern "C" {
#endif
enum OmniN64SaveResult {
    OM_N64_SAVE_OK=0, OM_N64_SAVE_MISSING=1, OM_N64_SAVE_OPEN=2,
    OM_N64_SAVE_READ=3, OM_N64_SAVE_WRITE=4, OM_N64_SAVE_SIZE=5,
    OM_N64_SAVE_MEMORY=6
};
#define OM_N64_SAVE_MAX_BYTES (256u*1024u*1024u)
/* All functions admit save/ names only. Load/read commit caller outputs only
 * after exact read AND close. Successful load transfers a malloc allocation
 * (also for a zero-byte image); the caller releases it with free. */
int om_n64_save_path(const char*);
int om_n64_save_load(const char*,void**,size_t*);
int om_n64_save_read(const char*,void*,size_t);
int om_n64_save_size(const char*,size_t*);
int om_n64_save_write(const char*,const void*,size_t);
int om_n64_save_patch(const char*,const void*,size_t,size_t);
#ifdef __cplusplus
}
#endif
#endif
