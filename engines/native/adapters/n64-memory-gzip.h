#ifndef OMNIPLAY_N64_MEMORY_GZIP_H
#define OMNIPLAY_N64_MEMORY_GZIP_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef struct OmniN64Gzip* OmniN64GzFile;
/* Gzip framing through the scratch-only stdio bridge. A single complete member
 * is admitted; CRC/length/truncation/trailing bytes are checked. Compressed
 * storage shares the 64-MiB scratch budget. Each decoded stream is bounded to
 * 256 MiB, without allocating a decoded image. Callers serialize each stream
 * and check every read/write/close before accepting parsed cache records. */
OmniN64GzFile om_n64_gzopen(const char*,const char*);
int om_n64_gzread(OmniN64GzFile,void*,unsigned);
int om_n64_gzwrite(OmniN64GzFile,const void*,unsigned);
int64_t om_n64_gzseek(OmniN64GzFile,int64_t,int);
int om_n64_gzeof(OmniN64GzFile);
void om_n64_gzabort(OmniN64GzFile);
int om_n64_gzclose(OmniN64GzFile);
#ifdef __cplusplus
}
#endif
#endif
