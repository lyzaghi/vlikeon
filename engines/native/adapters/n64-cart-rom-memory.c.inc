/* Actual cartridge ROM methods. Preserve zero padding beyond logical EOF,
 * without overflowing source arithmetic or invalidating all code for len=0. */
#include <string.h>
#include "n64-core-owner.h"
#include "n64-dma-memory.h"
static int om_n64_cart_rom_view(const struct cart_rom* cart)
{return cart && (uint64_t)cart->rom_size<=UINT64_C(0x100000000) && (!cart->rom_size || cart->rom);}
void read_cart_rom(void* opaque,uint32_t address,uint32_t* value)
{
    struct cart_rom* cart=(struct cart_rom*)opaque;uint32_t at=rom_address(address);
    if(!om_n64_owner_access() || !value)return;
    *value=0;
    if(!om_n64_cart_rom_view(cart) || !cart->pi)return;
    if(cart->pi->regs[PI_STATUS_REG]&PI_STATUS_IO_BUSY)*value=cart->last_write;
    else if(om_n64_device_range(cart->rom_size,at,4))memcpy(value,cart->rom+at,4);
}
void write_cart_rom(void* opaque,uint32_t address,uint32_t value,uint32_t mask)
{
    struct cart_rom* cart=(struct cart_rom*)opaque;(void)address;
    if(!om_n64_owner_access() || !cart || !cart->pi || !cart->r4300)return;
    /* Pinned write latch changes even when PI refuses a busy request. */
    cart->last_write=value&mask;
    if(!validate_pi_request(cart->pi))return;
    cart->pi->regs[PI_STATUS_REG]|=PI_STATUS_IO_BUSY;
    cp0_update_count(cart->r4300);
    add_interrupt_event(&cart->r4300->cp0,PI_INT,0x1000);
}
unsigned int cart_rom_dma_read(void* opaque,const uint8_t* dram,uint32_t dram_addr,uint32_t cart_addr,uint32_t length)
{
    struct cart_rom* cart=(struct cart_rom*)opaque;
    if(!om_n64_owner_access() || !cart || !cart->r4300
        || !om_n64_dma_rdram(cart->r4300->rdram,dram,dram_addr,length))return 0x1000;
    /* Writes to ROM stay ignored. No source/destination pointer is formed. */
    (void)cart_addr;return 0x1000;
}
unsigned int cart_rom_dma_write(void* opaque,uint8_t* dram,uint32_t dram_addr,uint32_t cart_addr,uint32_t length)
{
    struct cart_rom* cart=(struct cart_rom*)opaque;uint32_t i;
    if(!om_n64_owner_access() || !om_n64_cart_rom_view(cart) || !cart->r4300
        || !om_n64_dma_rdram(cart->r4300->rdram,dram,dram_addr,length))return 0x1000;
    cart_addr&=CART_ROM_ADDR_MASK;
    for(i=0;i<length;++i){
        uint64_t logical=(uint64_t)cart_addr+i,physical=logical^S8;
        /* Partial last words cannot borrow bytes beyond the declared view. */
        dram[(dram_addr+i)^S8]=(logical<cart->rom_size && physical<cart->rom_size)?cart->rom[(size_t)physical]:0;
    }
    if(length){
        invalidate_r4300_cached_code(cart->r4300,R4300_KSEG0+dram_addr,length);
        invalidate_r4300_cached_code(cart->r4300,R4300_KSEG1+dram_addr,length);
    }
    return length/8+add_random_interrupt_time(cart->r4300);
}
