/* Actual PI request implementation; included after the pinned core headers. */
#include "n64-core-owner.h"
#include "n64-device-memory.h"
#include "device/rdram/rdram.h"
int validate_pi_request(struct pi_controller* pi)
{
    if(!om_n64_owner_access() || !pi)return 0;
    if(pi->regs[PI_STATUS_REG]&(PI_STATUS_DMA_BUSY|PI_STATUS_IO_BUSY)){
        pi->regs[PI_STATUS_REG]|=PI_STATUS_ERROR;return 0;
    }
    return 1;
}
static int om_n64_pi_extent(struct pi_controller* pi,uint32_t address,uint32_t length)
{
    uint32_t physical;
    if(!pi->ri || !pi->ri->rdram || !pi->ri->rdram->dram || !pi->mi || !pi->mi->r4300
        || !pi->dp || !pi->get_pi_dma_handler
        || !om_n64_rdram_extent(pi->ri->rdram->dram_size,address,length,&physical)){
        pi->regs[PI_STATUS_REG]|=PI_STATUS_ERROR;return 0;
    }
    return 1;
}
static void om_n64_pi_dma(struct pi_controller* pi,int to_dram)
{
    uint32_t cart_addr,dram_addr,length;unsigned int cycles;
    const struct pi_dma_handler* handler=NULL;void* opaque=NULL;uint8_t* dram;
    if(!validate_pi_request(pi))return;
    cart_addr=pi->regs[PI_CART_ADDR_REG]&~UINT32_C(1);
    dram_addr=pi->regs[PI_DRAM_ADDR_REG]&UINT32_C(0xfffffe);
    length=(pi->regs[to_dram?PI_WR_LEN_REG:PI_RD_LEN_REG]&UINT32_C(0xffffff))+1;
    /* Preserve pinned PI rounding, including zero-effective short writes. */
    if(length>=0x7f && (length&1))++length;
    if(to_dram && length<=0x80)length=length>=(dram_addr&7)?length-(dram_addr&7):0;
    if(!om_n64_pi_extent(pi,dram_addr,length))return;
    pi->get_pi_dma_handler(pi->cart,pi->dd,cart_addr,&opaque,&handler);
    if(!handler || (to_dram?!handler->dma_write:!handler->dma_read)){
        pi->regs[PI_STATUS_REG]|=PI_STATUS_ERROR;return;
    }
    dram=(uint8_t*)pi->ri->rdram->dram;
    if(to_dram){
        cycles=handler->dma_write(opaque,dram,dram_addr,cart_addr,length);
        if(length)post_framebuffer_write(&pi->dp->fb,dram_addr,length);
    }else{
        /* Every intersecting dirty framebuffer page must reach RAM before the
         * cartridge sees its input, not just the page containing DMA start. */
        if(length)pre_framebuffer_read_range(&pi->dp->fb,dram_addr,length);
        cycles=handler->dma_read(opaque,dram,dram_addr,cart_addr,length);
    }
    pi->regs[PI_STATUS_REG]|=PI_STATUS_DMA_BUSY;
    pi->regs[PI_DRAM_ADDR_REG]=(pi->regs[PI_DRAM_ADDR_REG]+length+7)&~UINT32_C(7);
    pi->regs[PI_CART_ADDR_REG]=(pi->regs[PI_CART_ADDR_REG]+length+1)&~UINT32_C(1);
    cp0_update_count(pi->mi->r4300);
    add_interrupt_event(&pi->mi->r4300->cp0,PI_INT,cycles);
}
static void dma_pi_read(struct pi_controller* pi){om_n64_pi_dma(pi,0);}
static void dma_pi_write(struct pi_controller* pi){om_n64_pi_dma(pi,1);}
