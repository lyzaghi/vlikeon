#ifndef OMNIPLAY_BEETLE_MEMORY_FILES_H
#define OMNIPLAY_BEETLE_MEMORY_FILES_H
#include <stdint.h>
#include "writable_files_v1.h"
#ifdef __cplusplus
extern "C" {
#endif
/* Install/uninstall only with every card producer stopped. These functions
 * never access OS file APIs. Read: 1 complete, 0 absent, -1 rejected/failed.
 * Write: 1 complete atomic publication, 0 retained prior content/error. */
int om_ps1_files_install(const struct OmniWritableFilesV1*);
int om_ps1_files_installed(void);
int om_ps1_read_card(const char*,uint8_t*,uint32_t);
int om_ps1_write_card(const char*,const uint8_t*,uint32_t);
int om_ps1_card_read_failed(void);
void om_ps1_card_read_begin(void);
int om_ps1_card_write_failed(void);
void om_ps1_card_write_begin(void);
#ifdef __cplusplus
}
#endif
#endif
