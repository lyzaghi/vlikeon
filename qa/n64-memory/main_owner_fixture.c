/* Exact shared storage layout and actual prepared main-owner helpers.
 * The fixture supplies only the four controller slots, not a guest CPU. */
#include "n64-gb-storage-owner.h"
#include "n64-core-owner.h"
#include "n64-dd-storage.h"
#define GAME_CONTROLLERS_COUNT 4
static struct gb_cart_data l_gb_carts_data[GAME_CONTROLLERS_COUNT];
#include "n64-main-storage-owner.c.inc"
struct file_storage* om_n64_case_ram(unsigned int port)
{return port<GAME_CONTROLLERS_COUNT?&l_gb_carts_data[port].ram_fstorage:NULL;}
int om_n64_case_bind_rtc(unsigned int port,struct mbc3_rtc* rtc,const char* path)
{
    if(port>=GAME_CONTROLLERS_COUNT || l_gb_carts_data[port].rtc || !om_n64_rtc_storage_bind(rtc,path))return 0;
    l_gb_carts_data[port].rtc=rtc;return 1;
}
struct mbc3_rtc* om_n64_case_clock(unsigned int port)
{return port<GAME_CONTROLLERS_COUNT?l_gb_carts_data[port].rtc:NULL;}
int om_n64_case_retire_gb(unsigned int port)
{return port<GAME_CONTROLLERS_COUNT && om_n64_gb_retire_storage(&l_gb_carts_data[port]);}
