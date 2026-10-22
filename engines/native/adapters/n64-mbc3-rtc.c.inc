/* Actual selected MBC3 RTC. Bounded arithmetic for any signed-64-bit host
 * timestamp, including restored extremes. No loop proportional to elapsed time.
 * Timer/latched registers remain distinct; writes do not overwrite a latch. */
#include "n64-core-owner.h"
#include "n64-rtc-storage.h"
#include <limits.h>
typedef char om_n64_rtc_signed_time64[(sizeof(time_t)*CHAR_BIT==64 && (time_t)-1<(time_t)0)?1:-1];

/* Increment an eight-bit counter whose carry/reset occurs on equality to its
 * radix. Out-of-range raw register bytes first wrap at 256; do not canonicalize
 * a restored/guest-written byte until an actual tick changes it. */
static uint64_t om_n64_mbc3_counter(uint8_t* value,uint64_t ticks,unsigned int radix)
{
    uint64_t first=*value<radix?radix-*value:256u-*value+radix;
    if(ticks<first){*value=(uint8_t)((uint64_t)*value+ticks);return 0;}
    ticks-=first;*value=(uint8_t)(ticks%radix);return 1+ticks/radix;
}
static void update_rtc_regs(struct mbc3_rtc* rtc)
{
    time_t now,previous;uint64_t ticks,days;
    if(!rtc)return;
    om_n64_rtc_storage_changed(rtc);
    if(!rtc->iclock || !rtc->iclock->get_time)return;
    now=rtc->iclock->get_time(rtc->clock);previous=rtc->last_time;
    rtc->last_time=now;
    if(now<=previous || (rtc->regs[MBC3_RTC_DAYS_H]&0x40))return;
    /* Modulo unsigned subtraction exactly represents a positive signed-time
     * difference up to UINT64_MAX without evaluating a signed subtraction. */
    ticks=(uint64_t)now-(uint64_t)previous;
    ticks=om_n64_mbc3_counter(&rtc->regs[MBC3_RTC_SECONDS],ticks,60);
    ticks=om_n64_mbc3_counter(&rtc->regs[MBC3_RTC_MINUTES],ticks,60);
    ticks=om_n64_mbc3_counter(&rtc->regs[MBC3_RTC_HOURS],ticks,24);
    days=((uint64_t)(rtc->regs[MBC3_RTC_DAYS_H]&1)<<8)|rtc->regs[MBC3_RTC_DAYS_L];
    days+=ticks; /* At most UINT64_MAX / (60*60*24) + 511. */
    rtc->regs[MBC3_RTC_DAYS_L]=(uint8_t)days;
    rtc->regs[MBC3_RTC_DAYS_H]=(uint8_t)((rtc->regs[MBC3_RTC_DAYS_H]&0xfe)|((days>>8)&1));
    if(days>511)rtc->regs[MBC3_RTC_DAYS_H]|=0x80;
}
void init_mbc3_rtc(struct mbc3_rtc* rtc,void* clock,const struct clock_backend_interface* iclock)
{
    if(!om_n64_owner_access() || !rtc)return;
    om_n64_rtc_storage_changed(rtc);rtc->clock=clock;rtc->iclock=iclock;
}
void om_n64_mbc3_sync(struct mbc3_rtc* rtc)
{if(om_n64_owner_access() && rtc)update_rtc_regs(rtc);}
void poweron_mbc3_rtc(struct mbc3_rtc* rtc)
{
    if(!om_n64_owner_access() || !rtc)return;
    if(om_n64_rtc_storage_poweron(rtc)){
        update_rtc_regs(rtc);memset(rtc->latched_regs,0,sizeof(rtc->latched_regs));
        rtc->latch=0;rtc->latched_valid=0;return;
    }
    memset(rtc->regs,0,sizeof(rtc->regs));memset(rtc->latched_regs,0,sizeof(rtc->latched_regs));
    rtc->latch=0;rtc->latched_valid=0;
    rtc->last_time=(rtc->iclock && rtc->iclock->get_time)?rtc->iclock->get_time(rtc->clock):(time_t)0;
}
uint8_t read_mbc3_rtc_regs(struct mbc3_rtc* rtc,unsigned int reg)
{
    if(!om_n64_owner_access() || !rtc || reg>=MBC3_RTC_REGS_COUNT)return 0xff;
    update_rtc_regs(rtc);
    return rtc->latched_valid?rtc->latched_regs[reg]:rtc->regs[reg];
}
void write_mbc3_rtc_regs(struct mbc3_rtc* rtc,unsigned int reg,uint8_t value)
{
    if(!om_n64_owner_access() || !rtc || reg>=MBC3_RTC_REGS_COUNT)return;
    /* Charge elapsed running time before changing halt, then anchor halted
     * accesses too. Clearing halt never charges the stopped interval. */
    update_rtc_regs(rtc);rtc->regs[reg]=value;
}
void latch_mbc3_rtc_regs(struct mbc3_rtc* rtc,uint8_t data)
{
    if(!om_n64_owner_access() || !rtc)return;
    update_rtc_regs(rtc);
    if(rtc->latch==0 && data==1){memcpy(rtc->latched_regs,rtc->regs,sizeof(rtc->regs));rtc->latched_valid=1;}
    rtc->latch=data;
}
