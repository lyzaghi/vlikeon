#ifndef OMNIPLAY_N64_MEMORY_IO_H
#define OMNIPLAY_N64_MEMORY_IO_H
#include <stddef.h>
#include <stdint.h>
#include "writable_files_v1.h"
#ifdef __cplusplus
extern "C" {
#endif
typedef struct OmniN64File OmniN64File;
/* Separate scratch and save entry points, no OS FILE/disk fallback. The core owner installs/uninstalls
 * quiescently; replacement is refused while any stream remains open. Callback
 * entries must not throw. Borrowers serialize close against every use of that
 * same stream. A write stream publishes its complete private image only on a
 * healthy close; flush makes memory visible without publishing the name.
 * Uncertain callback close poisons further opens/table replacement until this
 * actor retires: it never licenses reuse of an unconfirmed callback context. */
int om_n64_files_install(const struct OmniWritableFilesV1*);
int om_n64_files_installed(void);
int om_n64_files_idle(void); /* no live streams and no unresolved close */
int om_n64_files_writes_settled(void); /* readonly cache snapshots may remain */
int om_n64_mkdir(const char*);
OmniN64File* om_n64_fopen(const char*,const char*);
/* Persistent save namespace only. Scratch callers cannot use this namespace
 * through om_n64_fopen. Both namespaces share stream/lifetime ownership. */
OmniN64File* om_n64_save_fopen(const char*,const char*);
int om_n64_fclose(OmniN64File*);
size_t om_n64_fread(void*,size_t,size_t,OmniN64File*);
size_t om_n64_fwrite(const void*,size_t,size_t,OmniN64File*);
int om_n64_fseek(OmniN64File*,long,int);
long om_n64_ftell(OmniN64File*);
void om_n64_rewind(OmniN64File*);
int om_n64_fflush(OmniN64File*);
int om_n64_ferror(OmniN64File*);
void om_n64_abort(OmniN64File*);
int om_n64_feof(OmniN64File*);
int om_n64_fgetc(OmniN64File*);
char* om_n64_fgets(char*,int,OmniN64File*);
int om_n64_fputs(const char*,OmniN64File*);
int om_n64_fprintf(OmniN64File*,const char*,...);
#ifdef __cplusplus
}
#endif
#endif
