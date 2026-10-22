/* Actual SRAM implementation. A complete transaction is admitted on both
 * sides before mutation or the backend's dirty-range notification. */
#include "n64-core-owner.h"
#include "n64-dma-memory.h"
static uint8_t* om_n64_sram_memory(struct sram* s,int writing)
{
    if(!s || !s->storage || !s->istorage || !s->istorage->data || !s->istorage->size
        || (writing && !s->istorage->save) || s->istorage->size(s->storage)<SRAM_SIZE)return NULL;
    return s->istorage->data(s->storage);
}
void format_sram(uint8_t* memory){if(om_n64_owner_access() && memory)memset(memory,0xff,SRAM_SIZE);}
void init_sram(struct sram* s,void* storage,const struct storage_backend_interface* api,struct rdram* ram)
{
    if(!om_n64_owner_access() || !s)return;
    s->storage=storage;s->istorage=api;s->rdram=ram;
}
unsigned int sram_dma_read(void* opaque,const uint8_t* dram,uint32_t dram_addr,uint32_t cart_addr,uint32_t length)
{
    struct sram* s=(struct sram*)opaque;uint8_t* memory;uint32_t i;
    if(!om_n64_owner_access() || !s || !length)return 0x1000;
    cart_addr&=SRAM_ADDR_MASK;
    if(!om_n64_dma_rdram(s->rdram,dram,dram_addr,length)
        || !om_n64_dma_swapped_range(SRAM_SIZE,cart_addr,length) || !(memory=om_n64_sram_memory(s,1)))return 0x1000;
    for(i=0;i<length;++i)memory[(cart_addr+i)^S8]=dram[(dram_addr+i)^S8];
    /* Backend offsets are physical byte offsets. A short XOR-ordered write
     * can touch the other end of its word; publish that entire physical word
     * cover so a later incremental save cannot miss the changed bytes. */
    if(S8==3){uint32_t begin=cart_addr&~UINT32_C(3),end=(cart_addr+length+3)&~UINT32_C(3);s->istorage->save(s->storage,begin,end-begin);}
    else s->istorage->save(s->storage,cart_addr,length);
    return 0x1000;
}
unsigned int sram_dma_write(void* opaque,uint8_t* dram,uint32_t dram_addr,uint32_t cart_addr,uint32_t length)
{
    struct sram* s=(struct sram*)opaque;const uint8_t* memory;uint32_t i;
    if(!om_n64_owner_access() || !s || !length)return 0x1000;
    cart_addr&=SRAM_ADDR_MASK;
    if(!om_n64_dma_rdram(s->rdram,dram,dram_addr,length)
        || !om_n64_dma_swapped_range(SRAM_SIZE,cart_addr,length) || !(memory=om_n64_sram_memory(s,0)))return 0x1000;
    for(i=0;i<length;++i)dram[(dram_addr+i)^S8]=memory[(cart_addr+i)^S8];
    return 0x1000;
}
void read_sram(void* opaque,uint32_t address,uint32_t* value)
{
    struct sram* s=(struct sram*)opaque;const uint8_t* memory;
    if(!om_n64_owner_access() || !value)return;
    *value=0;address&=SRAM_ADDR_MASK;
    if(!om_n64_device_range(SRAM_SIZE,address,4) || !(memory=om_n64_sram_memory(s,0)))return;
    memcpy(value,memory+address,4);
}
void write_sram(void* opaque,uint32_t address,uint32_t value,uint32_t mask)
{
    struct sram* s=(struct sram*)opaque;uint8_t* memory;uint32_t before;
    if(!om_n64_owner_access() || !mask)return;
    address&=SRAM_ADDR_MASK;
    if(!om_n64_device_range(SRAM_SIZE,address,4) || !(memory=om_n64_sram_memory(s,1)))return;
    memcpy(&before,memory+address,4);before=(before&~mask)|(value&mask);memcpy(memory+address,&before,4);
    s->istorage->save(s->storage,address,4);
}
