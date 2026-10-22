/* Included after actual DD register methods. The pinned supported buffer
 * entry addresses remain exact; MS remains an explicitly unimplemented no-op. */
#include <limits.h>
#include "n64-core-owner.h"
#include "n64-dma-memory.h"
static unsigned int om_n64_dd_dma_cycles(uint32_t length)
{
    uint64_t cycles=(uint64_t)length*63/25;
    return cycles>UINT_MAX?UINT_MAX:(unsigned int)cycles;
}
void read_dd_rom(void* opaque,uint32_t address,uint32_t* value)
{
    struct dd_controller* dd=(struct dd_controller*)opaque;uint32_t at=dd_rom_address(address)*4;
    if(!om_n64_owner_access() || !value)return;
    *value=0;
    if(!dd || !dd->rom || dd->rom_size>DD_ROM_MAX_SIZE || !om_n64_device_range(dd->rom_size,at,4))return;
    memcpy(value,(const uint8_t*)dd->rom+at,4);
}
void write_dd_rom(void* opaque,uint32_t address,uint32_t value,uint32_t mask)
{(void)opaque;(void)address;(void)value;(void)mask;}
unsigned int dd_dom_dma_read(void* opaque,const uint8_t* dram,uint32_t dram_addr,uint32_t cart_addr,uint32_t length)
{
    struct dd_controller* dd=(struct dd_controller*)opaque;unsigned int cycles=om_n64_dd_dma_cycles(length);uint32_t i;
    if(!om_n64_owner_access() || !dd || !dd->r4300 || !length
        || !om_n64_dma_rdram(dd->r4300->rdram,dram,dram_addr,length))return cycles;
    if(cart_addr==MM_DD_MS_RAM)return cycles; /* MS is not emulated upstream. */
    if(cart_addr!=MM_DD_DS_BUFFER || !om_n64_dma_swapped_range(sizeof(dd->ds_buf),0,length))return cycles;
    for(i=0;i<length;++i)dd->ds_buf[i^S8]=dram[(dram_addr+i)^S8];
    return cycles;
}
unsigned int dd_dom_dma_write(void* opaque,uint8_t* dram,uint32_t dram_addr,uint32_t cart_addr,uint32_t length)
{
    struct dd_controller* dd=(struct dd_controller*)opaque;unsigned int cycles=om_n64_dd_dma_cycles(length);
    const uint8_t* source;size_t capacity;uint32_t at,i;
    if(!om_n64_owner_access() || !dd || !dd->r4300 || !length
        || !om_n64_dma_rdram(dd->r4300->rdram,dram,dram_addr,length))return cycles;
    if(cart_addr==MM_DD_C2S_BUFFER){source=dd->c2s_buf;capacity=sizeof(dd->c2s_buf);at=0;}
    else if(cart_addr==MM_DD_DS_BUFFER){source=dd->ds_buf;capacity=sizeof(dd->ds_buf);at=0;}
    else if(cart_addr>=MM_DD_ROM){
        source=(const uint8_t*)dd->rom;capacity=dd->rom_size;at=cart_addr-MM_DD_ROM;
        if(capacity>DD_ROM_MAX_SIZE)return cycles;
    }else return cycles;
    if(!source || !om_n64_dma_swapped_range(capacity,at,length))return cycles;
    for(i=0;i<length;++i)dram[(dram_addr+i)^S8]=source[(at+i)^S8];
    invalidate_r4300_cached_code(dd->r4300,R4300_KSEG0+dram_addr,length);
    invalidate_r4300_cached_code(dd->r4300,R4300_KSEG1+dram_addr,length);
    return cycles;
}
