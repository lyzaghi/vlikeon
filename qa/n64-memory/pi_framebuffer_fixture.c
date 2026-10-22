/* Real PI launch/protection source; scheduler, raw RAM and graphics callbacks
 * are explicit observing substitutes. No emulated CPU, GL or OS is started. */
#include <string.h>
#include "pi_framebuffer_fixture.h"
#include "device/memory/memory.h"
#include "device/r4300/r4300_core.h"
#include "device/r4300/interrupt.h"
#include "device/rcp/mi/mi_controller.h"
#include "api/callbacks.h"
#include "plugin/plugin.h"
#include "n64-device-memory.h"
static struct r4300_core cpu;
static struct mi_controller mi;
static struct memory mem;
static struct rdram* active_ram;
static FrameBufferInfo staged[FB_INFOS_COUNT];
static struct om_n64_pi_fb_observed observed;
static unsigned int handler_mode;
gfx_plugin_functions gfx;
void DebugMessage(int level,const char* message,...){(void)level;(void)message;}
void cp0_update_count(struct r4300_core* value){if(value!=&cpu)++observed.bad;}
void add_interrupt_event(struct cp0* value,int type,unsigned int delay)
{if(value!=&cpu.cp0 || type!=PI_INT)++observed.bad;++observed.scheduled;observed.cycles=delay;}
void invalidate_r4300_cached_code(struct r4300_core* value,uint32_t address,size_t length)
{if(value!=&cpu || address || length)++observed.bad;++observed.invalidations;}
void apply_mem_mapping(struct memory* value,const struct mem_mapping* mapping)
{
    if(value!=&mem || mapping->begin>mapping->end || mapping->end>=active_ram->dram_size)++observed.bad;
    observed.map_begin=mapping->begin;observed.map_end=mapping->end;++observed.mappings;
}
void read_rdram_dram(void* opaque,uint32_t address,uint32_t* value)
{
    if(opaque!=active_ram || !om_n64_device_range(active_ram->dram_size,address,4)){++observed.bad;return;}
    memcpy(value,(uint8_t*)active_ram->dram+address,4);++observed.raw_reads;
}
void write_rdram_dram(void* opaque,uint32_t address,uint32_t value,uint32_t mask)
{
    uint32_t before;
    if(opaque!=active_ram || !om_n64_device_range(active_ram->dram_size,address,4)){++observed.bad;return;}
    memcpy(&before,(uint8_t*)active_ram->dram+address,4);before=(before&~mask)|(value&mask);
    memcpy((uint8_t*)active_ram->dram+address,&before,4);++observed.raw_writes;
}
static void fb_read(unsigned int at)
{if(at>=active_ram->dram_size)++observed.bad;if(!observed.reads)observed.first_read=at;observed.last_read=at;++observed.reads;}
static void fb_write(unsigned int at,unsigned int length)
{if(!om_n64_device_range(active_ram->dram_size,at,length))++observed.bad;if(!observed.writes)observed.first_write=at;observed.last_write=at;observed.last_size=length;++observed.writes;}
static void fb_info(void* out){memcpy(out,staged,sizeof(staged));}
static unsigned int dma_read(void* opaque,const uint8_t* data,uint32_t at,uint32_t cart,uint32_t length)
{
    (void)cart;if(opaque!=&observed || data!=(uint8_t*)active_ram->dram
        || !om_n64_device_range(active_ram->dram_size,at,length))++observed.bad;
    ++observed.transfers;observed.transfer_address=at;observed.transfer_length=length;
    observed.reads_at_transfer=observed.reads;return 1234;
}
static unsigned int dma_write(void* opaque,uint8_t* data,uint32_t at,uint32_t cart,uint32_t length)
{
    dma_read(opaque,data,at,cart,length);memset(data+at,0x6b,length);return 4321;
}
static void get_handler(struct cart* cart,struct dd_controller* dd,uint32_t at,void** opaque,const struct pi_dma_handler** out)
{
    static const struct pi_dma_handler handlers[]={{dma_read,dma_write},{NULL,dma_write},{dma_read,NULL}};
    (void)cart;(void)dd;(void)at;++observed.getter;*opaque=&observed;
    *out=handler_mode<3?&handlers[handler_mode]:NULL;
}
#include "n64-pi-actual.c.inc"
void om_n64_case_pi_fb_reset(struct pi_controller* pi,struct ri_controller* ri,struct rdp_core* dp,struct rdram* ram)
{
    memset(&observed,0,sizeof(observed));memset(staged,0,sizeof(staged));handler_mode=0;active_ram=ram;
    memset(&cpu,0,sizeof(cpu));mi.r4300=&cpu;ri->rdram=ram;
    gfx.fBRead=fb_read;gfx.fBWrite=fb_write;gfx.fBGetFrameBufferInfo=fb_info;
    init_fb(&dp->fb,&mem,ram,&cpu);poweron_fb(&dp->fb);
    init_pi(pi,get_handler,NULL,NULL,&mi,ri,dp);poweron_pi(pi);
}
void om_n64_case_pi_fb_stage(const FrameBufferInfo* infos){memcpy(staged,infos,sizeof(staged));}
void om_n64_case_pi_fb_handler(unsigned int mode){handler_mode=mode;}
void om_n64_case_pi_start(struct pi_controller* pi,int to_dram){if(to_dram)dma_pi_write(pi);else dma_pi_read(pi);}
struct om_n64_pi_fb_observed om_n64_case_pi_fb_observed(void){return observed;}
