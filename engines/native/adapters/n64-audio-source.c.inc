/* The audio backend receives an immutable RAM view. Channel conversion owns
 * a private bounded copy; no temporary fake AI registers or pointer subtraction. */
#include "n64-device-memory.h"
static int om_n64_audio_source_active=0;
void push_audio_samples_via_libretro(void* opaque,const void* buffer,size_t bytes)
{
    struct ai_controller* ai=(struct ai_controller*)opaque;
    uintptr_t base,address;size_t offset;uint8_t* owned;
    if(!om_n64_owner_access())return;
    if(om_n64_audio_source_active || !om_n64_audio_callback_idle() || !om_n64_audio_ready
        || !ai || !ai->ri || !ai->ri->rdram || !buffer || !ai->ri->rdram->dram
        || !ai->ri->rdram->dram_size || ai->ri->rdram->dram_size>0x800000
        || (ai->ri->rdram->dram_size&3) || bytes>0x800000 || (bytes&3)){
        om_n64_owner_admission_failed();return;
    }
    base=(uintptr_t)ai->ri->rdram->dram;address=(uintptr_t)buffer;
    if(address<base || address-base>ai->ri->rdram->dram_size){om_n64_owner_admission_failed();return;}
    offset=(size_t)(address-base);
    if(bytes>ai->ri->rdram->dram_size-offset || (offset&3)){om_n64_owner_admission_failed();return;}
    if(!bytes)return;
    owned=(uint8_t*)OM_N64_AUDIO_ALLOC(bytes);
    if(!owned){om_n64_owner_admission_failed();return;}
    memcpy(owned,buffer,bytes);om_n64_audio_source_active=1;
    aiLenChanged(ai,owned,bytes);
    om_n64_audio_source_active=0;OM_N64_AUDIO_FREE(owned);
}
