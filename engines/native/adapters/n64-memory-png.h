#ifndef OMNIPLAY_N64_MEMORY_PNG_H
#define OMNIPLAY_N64_MEMORY_PNG_H
#include <stdint.h>
#include "n64-memory-io.h"
#ifdef __cplusplus
extern "C" {
#endif
#define OM_N64_PNG_CODEC_LIMIT (8u*1024u*1024u)
/* RGB/RGBA8, bounded explicit source extent/stride, optional vertical flip.
 * The borrowed scratch stream is never closed here. Any encoder/I/O failure
 * aborts it; callers must still check close before claiming publication.
 * A caller may lower the codec budget, never raise it above 8 MiB. All libpng/
 * zlib codec allocations use that aggregate budget, including header overhead.
 * setjmp/longjmp and cleanup remain entirely within this C translation unit. */
int om_n64_png_encode(OmniN64File*,const unsigned char*,uint64_t source_bytes,
                     uint32_t width,uint32_t height,uint32_t stride,
                     uint32_t channels,uint32_t flip,uint32_t codec_limit);
#ifdef __cplusplus
}
#endif
#endif
