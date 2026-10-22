/* Actual frontend content admission. A ROM-open receipt belongs to this
 * frontend until a successful ROM_CLOSE, including failed header queries. */
#include "n64-rom-image.h"
static int om_n64_content_rom_open=0;
static int om_n64_content_close_unstarted(void)
{
    if(!om_n64_owner_access() || om_n64_owner_started())return 0;
    if(!om_n64_content_rom_open)return 1;
    if(CoreDoCommand(M64CMD_ROM_CLOSE,0,NULL)!=M64ERR_SUCCESS){
        om_n64_owner_admission_failed();return 0;
    }
    om_n64_content_rom_open=0;return 1;
}
static bool emu_step_load_data(void)
{
    m64p_rom_header staged;
    if(!om_n64_owner_access() || om_n64_content_rom_open || !game_data
        || om_n64_rom_image_format(game_data,game_size)<0)return false;
    if(CoreDoCommand(M64CMD_ROM_OPEN,game_size,game_data)!=M64ERR_SUCCESS){
        free(game_data);game_data=NULL;game_size=0;return false;
    }
    om_n64_content_rom_open=1;
    free(game_data);game_data=NULL;game_size=0;
    memset(&staged,0,sizeof(staged));
    if(CoreDoCommand(M64CMD_ROM_GET_HEADER,sizeof(staged),&staged)!=M64ERR_SUCCESS){
        /* A refused close retains the receipt and poisons this session's
         * admission. Teardown may retry; no new content overwrites ownership. */
        om_n64_content_close_unstarted();return false;
    }
    ROM_HEADER=staged;return true;
}
