/* Actual selected flash device: complete admitted ranges before writes. */
#include "n64-core-owner.h"
#include "n64-device-memory.h"
typedef char om_n64_flash_byte_order[(S8==0 || S8==3)?1:-1];
static uint8_t* om_n64_flash_memory(struct flashram* f,int writing)
{
    if(!f || !f->storage || !f->istorage || !f->istorage->data || !f->istorage->size
       || (writing && !f->istorage->save) || f->istorage->size(f->storage)<FLASHRAM_SIZE)return NULL;
    return f->istorage->data(f->storage);
}
static int om_n64_flash_dram(const struct flashram* f,const uint8_t* dram,uint32_t offset,uint32_t bytes)
{
    return f && f->rdram && dram && dram==(const uint8_t*)f->rdram->dram
        && f->rdram->dram_size && f->rdram->dram_size<=RDRAM_MAX_SIZE
        && !(f->rdram->dram_size&3) && om_n64_device_range(f->rdram->dram_size,offset,bytes);
}
static void flashram_command(struct flashram* f,uint32_t command)
{
    uint8_t* mem;uint32_t offset,length,i;
    if(!om_n64_owner_access() || !f)return;
    switch(command&UINT32_C(0xff000000)){
    case 0x3c000000:f->mode=FLASHRAM_MODE_CHIP_ERASE;break;
    case 0x4b000000:f->mode=FLASHRAM_MODE_SECTOR_ERASE;f->erase_page=(uint16_t)command;break;
    case 0x78000000:
        if(f->mode==FLASHRAM_MODE_SECTOR_ERASE){offset=((uint32_t)f->erase_page&0xff80)*128;length=128*128;}
        else if(f->mode==FLASHRAM_MODE_CHIP_ERASE){offset=0;length=FLASHRAM_SIZE;}
        else return;
        if(!om_n64_device_range(FLASHRAM_SIZE,offset,length) || !(mem=om_n64_flash_memory(f,1)))return;
        f->status|=2;memset(mem+offset,0xff,length);f->istorage->save(f->storage,offset,length);
        f->status=(f->status&~UINT32_C(2))|8;f->mode=FLASHRAM_MODE_STATUS;break;
    case 0xa5000000:
        offset=(command&0xffff)*128;
        if(!om_n64_device_range(FLASHRAM_SIZE,offset,128) || !(mem=om_n64_flash_memory(f,1)))return;
        f->status|=1;for(i=0;i<128;++i)mem[(offset+i)^S8]=f->page_buf[i];
        f->istorage->save(f->storage,offset,128);f->status=(f->status&~UINT32_C(1))|4;f->mode=FLASHRAM_MODE_STATUS;break;
    case 0xb4000000:f->mode=FLASHRAM_MODE_PAGE_PROGRAM;break;
    case 0xd2000000:f->mode=FLASHRAM_MODE_STATUS;break;
    case 0xe1000000:f->mode=FLASHRAM_MODE_READ_SILICON_ID;f->status|=1;break;
    case 0xf0000000:f->mode=FLASHRAM_MODE_READ_ARRAY;break;
    default:break;
    }
}
void init_flashram(struct flashram* f,uint32_t id,void* storage,const struct storage_backend_interface* api,struct rdram* rdram)
{
    if(!om_n64_owner_access() || !f)return;
    f->silicon_id[0]=FLASHRAM_TYPE_ID;f->silicon_id[1]=id;f->storage=storage;f->istorage=api;f->rdram=rdram;
}
void poweron_flashram(struct flashram* f)
{
    if(!om_n64_owner_access() || !f)return;
    f->mode=FLASHRAM_MODE_READ_ARRAY;f->status=0;f->erase_page=0;memset(f->page_buf,0xff,sizeof(f->page_buf));
}
/* The retained raw-format API requires its caller's full FLASHRAM_SIZE buffer. */
void format_flashram(uint8_t* flash){if(om_n64_owner_access() && flash)memset(flash,0xff,FLASHRAM_SIZE);}
void read_flashram(void* opaque,uint32_t address,uint32_t* value)
{
    struct flashram* f=(struct flashram*)opaque;
    if(!om_n64_owner_access() || !f || !value)return;
    *value=((address&0x1ffff)==0 && f->mode==FLASHRAM_MODE_STATUS)?f->status:0;
}
void write_flashram(void* opaque,uint32_t address,uint32_t value,uint32_t mask)
{
    struct flashram* f=(struct flashram*)opaque;
    if(!om_n64_owner_access() || !f)return;
    if((address&0x1ffff)==0 && f->mode==FLASHRAM_MODE_STATUS)f->status=(value&mask)&0xff;
    else if((address&0x1ffff)==0x10000)flashram_command(f,value&mask);
}
unsigned int flashram_dma_write(void* opaque,uint8_t* dram,uint32_t dram_addr,uint32_t cart_addr,uint32_t length)
{
    struct flashram* f=(struct flashram*)opaque;uint8_t* mem;uint32_t i;
    if(!om_n64_owner_access() || !f)return 0x1000;
    if((cart_addr&0x1ffff)==0 && length==8 && f->mode==FLASHRAM_MODE_READ_SILICON_ID){
        uint32_t offset=dram_addr&~UINT32_C(3);
        if(om_n64_flash_dram(f,dram,offset,8))memcpy(dram+offset,f->silicon_id,8);
    }else if((cart_addr&0x1ffff)<0x10000 && f->mode==FLASHRAM_MODE_READ_ARRAY){
        uint32_t offset=cart_addr&0xffff;
        if(f->silicon_id[1]==MX29L1100_ID || f->silicon_id[1]==MX29L0000_ID || f->silicon_id[1]==MX29L0001_ID)offset*=2;
        if(!length || !om_n64_device_range(FLASHRAM_SIZE,offset,length) || !om_n64_flash_dram(f,dram,dram_addr,length) || !(mem=om_n64_flash_memory(f,0)))return 0x1000;
        for(i=0;i<length;++i)dram[(dram_addr+i)^S8]=mem[(offset+i)^S8];
    }
    return 0x1000;
}
unsigned int flashram_dma_read(void* opaque,const uint8_t* dram,uint32_t dram_addr,uint32_t cart_addr,uint32_t length)
{
    struct flashram* f=(struct flashram*)opaque;uint8_t staged[128];uint32_t i;
    if(!om_n64_owner_access() || !f)return 0x1000;
    if((cart_addr&0x1ffff)==0 && length==128 && f->mode==FLASHRAM_MODE_PAGE_PROGRAM && om_n64_flash_dram(f,dram,dram_addr,128)){
        for(i=0;i<128;++i)staged[i]=dram[(dram_addr+i)^S8];
        memcpy(f->page_buf,staged,sizeof(staged));
    }
    return 0x1000;
}
