/* Actual selected DD DMA and cart domain bodies. CPU invalidation, scheduler
 * and PI validation below are observing substitutes, not CPU execution. */
#include <string.h>
#include "cartridge_dma_fixture.h"
#include "n64-core-owner.h"
#include "n64-device-memory.h"
#include "device/cart/cart.h"
#include "device/dd/dd_controller.h"
#include "device/device.h"
#include "device/memory/memory.h"
#include "device/r4300/r4300_core.h"
#include "device/r4300/interrupt.h"
#include "device/rcp/pi/pi_controller.h"
#include "device/rdram/rdram.h"
#include "api/callbacks.h"
static struct r4300_core cpu;
static struct om_n64_dma_observed observed;
void DebugMessage(int level,const char* message,...){(void)level;(void)message;}
void cp0_update_count(struct r4300_core* value){if(value!=&cpu)++observed.bad;}
void add_interrupt_event(struct cp0* value,int type,unsigned int delay)
{if(value!=&cpu.cp0 || type!=PI_INT || delay!=0x1000)++observed.bad;++observed.scheduled;}
unsigned int add_random_interrupt_time(struct r4300_core* value)
{if(value!=&cpu)++observed.bad;++observed.random_calls;return 7;}
void invalidate_r4300_cached_code(struct r4300_core* value,uint32_t address,size_t length)
{
    uint32_t at=address&UINT32_C(0x1fffffff);
    if(value!=&cpu || !cpu.rdram || !length || length>UINT32_MAX
        || (address&UINT32_C(0xe0000000))<R4300_KSEG0
        || (address&UINT32_C(0xe0000000))>R4300_KSEG1
        || !om_n64_device_range(cpu.rdram->dram_size,at,(uint32_t)length))++observed.bad;
    if(!observed.invalidations){observed.first_address=address;observed.first_length=length;}
    ++observed.invalidations;observed.last_address=address;observed.last_length=length;
}
int validate_pi_request(struct pi_controller* pi)
{
    ++observed.validated;
    if(!om_n64_owner_access() || !pi)return 0;
    if(pi->regs[PI_STATUS_REG]&(PI_STATUS_DMA_BUSY|PI_STATUS_IO_BUSY)){pi->regs[PI_STATUS_REG]|=PI_STATUS_ERROR;return 0;}
    return 1;
}
#include "n64-dd-dma-memory.c.inc"
#include "n64-cart-dma-actual.c.inc"
struct r4300_core* om_n64_case_dma_cpu(struct rdram* ram)
{memset(&cpu,0,sizeof(cpu));cpu.rdram=ram;om_n64_case_dma_clear();return &cpu;}
void om_n64_case_dma_clear(void){memset(&observed,0,sizeof(observed));}
struct om_n64_dma_observed om_n64_case_dma_observed(void){return observed;}
