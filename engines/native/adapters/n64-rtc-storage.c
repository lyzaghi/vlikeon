#include "n64-rtc-storage.h"
#include "n64-core-owner.h"
#include "n64-save-files.h"
#include "device/gb/mbc3_rtc.h"
#include "backends/api/clock_backend.h"
#include <stdint.h>
#include <string.h>
#include <limits.h>
typedef char om_n64_battery_time64[(sizeof(time_t)*CHAR_BIT==64 && (time_t)-1<(time_t)0)?1:-1];
enum { RTC_PORTS=4,RTC_BYTES=32,RTC_PATH_BYTES=54 };
struct OmRtcStorage {struct mbc3_rtc* rtc;char path[RTC_PATH_BYTES];uint8_t image[RTC_BYTES];int saved,live,retire_ready;};
static struct OmRtcStorage slots[RTC_PORTS];
static unsigned int find(const struct mbc3_rtc* rtc)
{unsigned int i;for(i=0;i<RTC_PORTS;++i)if(slots[i].rtc==rtc && rtc)return i;return RTC_PORTS;}
static uint32_t word(const uint8_t* p)
{return (uint32_t)p[0]|((uint32_t)p[1]<<8)|((uint32_t)p[2]<<16)|((uint32_t)p[3]<<24);}
static void put_word(uint8_t* p,uint32_t value)
{unsigned int i;for(i=0;i<4;++i)p[i]=(uint8_t)(value>>(8*i));}
static uint32_t checksum(const uint8_t* image)
{
    uint32_t value=UINT32_MAX;unsigned int i,bit;
    for(i=0;i<28;++i){value^=image[i];for(bit=0;bit<8;++bit)value=(value>>1)^((value&1)?UINT32_C(0xedb88320):0);}
    return ~value;
}
static int path_valid(const char* path)
{
    unsigned int i;
    if(!path)return 0;
    for(i=0;i<RTC_PATH_BYTES && path[i];++i){}
    if(i!=RTC_PATH_BYTES-1 || memcmp(path,"save/N64/tpak/",14))return 0;
    for(i=14;i<46;++i)if(!((path[i]>='0' && path[i]<='9') || (path[i]>='a' && path[i]<='f')))return 0;
    return path[46]=='-' && path[47]=='p' && path[48]>='1' && path[48]<='4' && !memcmp(path+49,".rtc",5);
}
static int valid(const uint8_t* image)
{return !memcmp(image,"OMGBRTC1",8) && !image[13] && !image[14] && !image[15] && !word(image+24) && word(image+28)==checksum(image);}
static void restore(struct mbc3_rtc* rtc,const uint8_t* image)
{
    uint64_t bits=(uint64_t)word(image+16)|((uint64_t)word(image+20)<<32);
    int64_t stamp=(bits>>63)?INT64_MIN+(int64_t)(bits-UINT64_C(0x8000000000000000)):(int64_t)bits;
    memcpy(rtc->regs,image+8,5);rtc->last_time=(time_t)stamp;
}
static void encode(uint8_t* image,const struct mbc3_rtc* rtc)
{
    uint64_t bits=(uint64_t)(int64_t)rtc->last_time;
    memset(image,0,RTC_BYTES);memcpy(image,"OMGBRTC1",8);memcpy(image+8,rtc->regs,5);
    put_word(image+16,(uint32_t)bits);put_word(image+20,(uint32_t)(bits>>32));put_word(image+28,checksum(image));
}
int om_n64_rtc_storage_bind(struct mbc3_rtc* rtc,const char* path)
{
    struct OmRtcStorage candidate;unsigned int i,slot=RTC_PORTS;int result;
    if(!om_n64_owner_access() || !rtc || !rtc->iclock || !rtc->iclock->get_time || !path_valid(path) || find(rtc)!=RTC_PORTS)return 0;
    for(i=0;i<RTC_PORTS;++i){if(slots[i].rtc && (!strcmp(slots[i].path,path) || slots[i].path[48]==path[48]))return 0;if(!slots[i].rtc && slot==RTC_PORTS)slot=i;}
    if(slot==RTC_PORTS)return 0;
    memset(&candidate,0,sizeof(candidate));
    result=om_n64_save_read(path,candidate.image,sizeof(candidate.image));
    if(result!=OM_N64_SAVE_OK && result!=OM_N64_SAVE_MISSING){om_n64_owner_admission_failed();return 0;}
    if(result==OM_N64_SAVE_OK && !valid(candidate.image)){om_n64_owner_admission_failed();return 0;}
    candidate.saved=result==OM_N64_SAVE_OK;candidate.rtc=rtc;memcpy(candidate.path,path,RTC_PATH_BYTES);
    slots[slot]=candidate;return 1;
}
int om_n64_rtc_storage_poweron(struct mbc3_rtc* rtc)
{
    unsigned int i;int retained;
    if(!om_n64_owner_access() || !rtc)return 0;
    i=find(rtc);if(i==RTC_PORTS)return 0;
    retained=slots[i].live || slots[i].saved;
    if(!slots[i].live && slots[i].saved)restore(rtc,slots[i].image);
    slots[i].live=1;slots[i].retire_ready=0;
    return retained;
}
void om_n64_rtc_storage_changed(struct mbc3_rtc* rtc)
{unsigned int i;if(!om_n64_owner_access())return;i=find(rtc);if(i<RTC_PORTS)slots[i].retire_ready=0;}
static int flush(unsigned int i)
{
    uint8_t image[RTC_BYTES];int result;
    if(!slots[i].live)return -1;
    slots[i].retire_ready=0;
    om_n64_mbc3_sync(slots[i].rtc);encode(image,slots[i].rtc);
    result=om_n64_save_write(slots[i].path,image,sizeof(image));
    if(result!=OM_N64_SAVE_OK)return 0;
    memcpy(slots[i].image,image,sizeof(image));slots[i].saved=1;slots[i].retire_ready=1;return 1;
}
int om_n64_rtc_storage_prepare_retire(struct mbc3_rtc* rtc)
{
    unsigned int i;
    if(!om_n64_owner_access())return 0;i=find(rtc);if(i==RTC_PORTS)return 1;
    if(!slots[i].live){slots[i].retire_ready=1;return 1;}
    return flush(i)==1;
}
int om_n64_rtc_storage_detach(struct mbc3_rtc* rtc)
{
    unsigned int i;
    if(!om_n64_owner_access())return 0;i=find(rtc);if(i==RTC_PORTS)return 1;
    if(!slots[i].retire_ready)return 0;
    memset(&slots[i],0,sizeof(slots[i]));return 1;
}
int om_n64_rtc_storage_flush_all(void)
{
    unsigned int i;int result,complete=1;
    if(!om_n64_owner_access() || !om_n64_owner_persistence_allowed())return -1;
    for(i=0;i<RTC_PORTS;++i)if(slots[i].rtc){result=flush(i);if(result<0)return -1;if(!result)complete=0;}
    return complete;
}
int om_n64_rtc_storage_quiescent(void)
{unsigned int i;if(!om_n64_owner_access())return 0;for(i=0;i<RTC_PORTS;++i)if(slots[i].rtc)return 0;return 1;}
