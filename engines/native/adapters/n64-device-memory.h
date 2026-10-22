#ifndef OMNIPLAY_N64_DEVICE_MEMORY_H
#define OMNIPLAY_N64_DEVICE_MEMORY_H
#include <stddef.h>
#include <stdint.h>
static inline int om_n64_device_range(size_t bytes,uint32_t offset,uint32_t count)
{return offset<=bytes && count<=bytes-offset;}
/* SI uses the low 24 address bits and word alignment, then transfers 64 bytes.
 * Validate before forming a pointer or invoking PIF processing callbacks. */
static inline int om_n64_si_dram_extent(size_t bytes,uint32_t raw)
{return bytes && bytes<=0x800000 && !(bytes&3) && om_n64_device_range(bytes,(raw&0xffffff)&~UINT32_C(3),64);}
/* Only physical RDRAM and its KSEG0/KSEG1 aliases. Do not turn arbitrary
 * segmented/MMIO addresses into RAM by discarding their upper bits. */
static inline int om_n64_rdram_extent(size_t bytes,uint32_t raw,uint32_t count,uint32_t* physical)
{
    uint32_t at;
    if(!bytes || bytes>0x800000 || (bytes&3) || !physical)return 0;
    if(raw>=0x20000000 && (raw<0x80000000 || raw>=0xc0000000))return 0;
    at=raw&UINT32_C(0x1fffffff);
    if(!om_n64_device_range(bytes,at,count))return 0;
    *physical=at;return 1;
}
static inline int om_n64_framebuffer_extent(size_t bytes,uint32_t raw,uint32_t width,
    uint32_t height,uint32_t pixel_bytes,uint32_t* begin,uint32_t* length)
{
    uint32_t at;size_t row;
    if(!begin || !length || !width || !height || (pixel_bytes!=1 && pixel_bytes!=2 && pixel_bytes!=4)
        || !om_n64_rdram_extent(bytes,raw,1,&at) || !at)return 0;
    if(width>(bytes-at)/pixel_bytes)return 0;
    row=(size_t)width*pixel_bytes;
    if(height>(bytes-at)/row)return 0;
    *begin=at;*length=(uint32_t)(row*height);return 1;
}
#endif
