/* Included in actual SI controller code, before request validation. */
#include "n64-core-owner.h"
#include "n64-device-memory.h"
static int om_n64_si_memory_ready(struct si_controller* si)
{
    return si && si->pif && si->pif->ram && si->ri && si->ri->rdram && si->ri->rdram->dram
        && om_n64_si_dram_extent(si->ri->rdram->dram_size,si->regs[SI_DRAM_ADDR_REG]);
}
static int copy_pif_rdram(struct si_controller* si)
{
    uint32_t* dram;uint32_t* pif;size_t i;
    if(!om_n64_owner_access() || !si)return 0;
    if(!om_n64_si_memory_ready(si) || (si->dma_dir!=SI_DMA_READ && si->dma_dir!=SI_DMA_WRITE)){
        si->regs[SI_STATUS_REG]|=SI_STATUS_DMA_ERROR;return 0;
    }
    dram=si->ri->rdram->dram+rdram_dram_address(si->regs[SI_DRAM_ADDR_REG]&~UINT32_C(3));
    pif=(uint32_t*)si->pif->ram;
    for(i=0;i<PIF_RAM_SIZE/4;++i){if(si->dma_dir==SI_DMA_WRITE)pif[i]=fromhl(dram[i]);else dram[i]=tohl(pif[i]);}
    return 1;
}
