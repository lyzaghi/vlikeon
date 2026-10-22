#ifndef OMNIPLAY_N64_DMA_MEMORY_H
#define OMNIPLAY_N64_DMA_MEMORY_H
#include "n64-device-memory.h"
#include "device/rdram/rdram.h"
#include "osal/preproc.h"
typedef char om_n64_dma_byte_order[(S8==0 || S8==3)?1:-1];
/* Direct handler calls receive an actual RAM base, not a pointer to a slice.
 * PI register masking belongs to PI; the leaf must not alias a bad offset. */
static inline int om_n64_dma_rdram(const struct rdram* ram,const uint8_t* base,uint32_t at,uint32_t length)
{
    return ram && base && base==(const uint8_t*)ram->dram && ram->dram_size
        && ram->dram_size<=0x800000 && !(ram->dram_size&3)
        && om_n64_device_range(ram->dram_size,at,length);
}
/* Logical admission alone is insufficient for a short non-word-sized source:
 * XOR byte order can move a logically present byte beyond the backing view. */
static inline int om_n64_dma_swapped_range(size_t bytes,uint32_t at,uint32_t length)
{
    uint64_t last,base,first;
    if(!om_n64_device_range(bytes,at,length))return 0;
    if(!length || S8==0)return 1;
    last=(uint64_t)at+length-1;base=last&~UINT64_C(3);
    first=(uint64_t)at>base?at:base;
    return (first^UINT64_C(3))<bytes;
}
#endif
