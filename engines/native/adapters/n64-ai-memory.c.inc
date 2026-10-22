/* Actual AI source-range publication, before forming any RAM slice pointer. */
#include "n64-core-owner.h"
#include "n64-device-memory.h"
static int om_n64_ai_publish(struct ai_controller* ai,uint32_t remaining)
{
    uint32_t start,offset,count;
    if(!om_n64_owner_access() || !ai || !ai->ri || !ai->ri->rdram || !ai->ri->rdram->dram
        || !ai->iaout || !ai->iaout->push_samples || ai->last_read>ai->fifo[0].length
        || remaining>ai->last_read || ((ai->fifo[0].length|ai->last_read|remaining)&3)){
        om_n64_owner_admission_failed();return 0;
    }
    start=ai->fifo[0].address&~UINT32_C(3);
    offset=ai->fifo[0].length-ai->last_read;count=ai->last_read-remaining;
    if(!ai->ri->rdram->dram_size || ai->ri->rdram->dram_size>0x800000
        || (ai->ri->rdram->dram_size&3) || (count&3)
        || !om_n64_device_range(ai->ri->rdram->dram_size,start,ai->fifo[0].length)){
        om_n64_owner_admission_failed();return 0;
    }
    if(count)ai->iaout->push_samples(ai->aout,(const uint8_t*)ai->ri->rdram->dram+start+offset,count);
    ai->last_read=remaining;return 1;
}
