/* Exact prepared SI methods, with explicit non-CPU callback counters. */
#include <string.h>
#include "device/rcp/si/si_controller.h"
#include "device/pif/pif.h"
#include "device/memory/memory.h"
#include "device/r4300/r4300_core.h"
#include "device/r4300/interrupt.h"
#include "device/rcp/mi/mi_controller.h"
#include "device/rcp/ri/ri_controller.h"
#include "device/rdram/rdram.h"
#include "api/callbacks.h"
static struct r4300_core cpu;
static struct mi_controller mi;
static unsigned int scheduled,pif_calls,raised;
void DebugMessage(int level,const char* message,...){(void)level;(void)message;}
void cp0_update_count(struct r4300_core* value){(void)value;}
unsigned int add_random_interrupt_time(struct r4300_core* value){(void)value;return 0;}
void add_interrupt_event(struct cp0* cp0,int type,unsigned int delay){(void)cp0;(void)type;(void)delay;++scheduled;}
void update_pif_ram(struct pif* pif){(void)pif;++pif_calls;}
void process_pif_ram(struct pif* pif){(void)pif;++pif_calls;}
void raise_rcp_interrupt(struct mi_controller* value,uint32_t interrupt){(void)value;(void)interrupt;++raised;}
#include "n64-si-actual.c.inc"
void om_n64_case_si_init(struct si_controller* si,struct pif* pif,struct ri_controller* ri)
{mi.r4300=&cpu;scheduled=0;pif_calls=0;raised=0;init_si(si,10,&mi,pif,ri);poweron_si(si);}
int om_n64_case_si_copy(struct si_controller* si){return copy_pif_rdram(si);}
void om_n64_case_si_start(struct si_controller* si,int read){if(read)dma_si_read(si);else dma_si_write(si);}
unsigned int om_n64_case_si_scheduled(void){return scheduled;}
unsigned int om_n64_case_si_pif_calls(void){return pif_calls;}
unsigned int om_n64_case_si_raised(void){return raised;}
