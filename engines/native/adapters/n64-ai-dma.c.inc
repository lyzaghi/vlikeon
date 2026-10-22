#include "n64-ai-timing.h"
#include "device/r4300/interrupt.h"
static int om_n64_ai_ram_extent(struct ai_controller* ai,uint32_t raw,uint32_t length)
{
    return ai && ai->ri && ai->ri->rdram && ai->ri->rdram->dram
        && ai->ri->rdram->dram_size && ai->ri->rdram->dram_size<=0x800000
        && !(ai->ri->rdram->dram_size&3)
        && om_n64_device_range(ai->ri->rdram->dram_size,raw&~UINT32_C(3),length);
}
static int om_n64_ai_duration_for_request(struct ai_controller* ai,uint32_t* result)
{
    if(!om_n64_owner_access() || !ai || !ai->vi || !ai->mi || !ai->mi->r4300
        || !ai->iaout || !ai->iaout->set_frequency || !ai->iaout->push_samples
        || !om_n64_ai_ram_extent(ai,ai->regs[AI_DRAM_ADDR_REG],ai->regs[AI_LEN_REG]&~UINT32_C(7))
        || !om_n64_ai_duration(ai->vi->clock,ai->vi->delay,ai->vi->expected_refresh_rate,
            ai->regs[AI_DACRATE_REG],ai->regs[AI_LEN_REG],ai->dma_modifier,result)){
        om_n64_owner_admission_failed();return 0;
    }
    return 1;
}
static int do_dma(struct ai_controller* ai,struct ai_dma* dma)
{
    uint64_t address,rate;uint32_t carry;
    if(!om_n64_owner_access() || !ai || !dma || !ai->vi || !ai->mi || !ai->mi->r4300
        || !ai->iaout || !ai->iaout->set_frequency || !ai->iaout->push_samples
        || ai->delayed_carry>1 || (dma->length&7) || dma->duration>INT32_MAX){
        om_n64_owner_admission_failed();return 0;
    }
    address=(uint64_t)dma->address+(ai->delayed_carry?0x2000:0);
    if(address>UINT32_MAX || !om_n64_ai_ram_extent(ai,(uint32_t)address,dma->length)){
        om_n64_owner_admission_failed();return 0;
    }
    rate=ai->regs[AI_DACRATE_REG]?(uint64_t)ai->vi->clock/((uint64_t)ai->regs[AI_DACRATE_REG]+1):44100;
    if(!rate || rate>0x1fffffff){om_n64_owner_admission_failed();return 0;}
    carry=((address+dma->length)&0x1fff)==0;
    if(ai->samples_format_changed){
        ai->iaout->set_frequency(ai->aout,(unsigned int)rate);
        if(!om_n64_owner_admitted())return 0;
    }
    cp0_update_count(ai->mi->r4300);
    if(!om_n64_add_interrupt_checked(&ai->mi->r4300->cp0,AI_INT,dma->duration)){
        *r4300_stop(ai->mi->r4300)=1;om_n64_owner_admission_failed();return 0;
    }
    ai->samples_format_changed=0;
    dma->address=(uint32_t)address;ai->last_read=dma->length;ai->delayed_carry=carry;
    return 1;
}
