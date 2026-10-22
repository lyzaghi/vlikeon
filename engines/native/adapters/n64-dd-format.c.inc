/* Actual pinned disk.c replacement, after retained upstream notices/includes.
 * Failure leaves input ownership, its length and every output unchanged. */
#include "n64-save-files.h"
#include <stddef.h>
#include <string.h>

static int om_dd_range(size_t bytes,size_t offset,size_t length)
{return offset<=bytes && length<=bytes-offset;}
static int om_dd_system_valid(const struct dd_sys_data* sys,unsigned int defect_limit)
{
    unsigned int zone,begin=0;
    uint32_t region=big32(sys->region),address=big32(sys->ipl_load_addr);
    uint16_t load=big16(sys->ipl_load_blk);
    if((sys->type&0xef)>6 || !load || load>MAX_LBA-SYSTEM_LBAS || address<0x80000000u || address>=0x80800000u)return 0;
    if(region!=DD_REGION_JP && region!=DD_REGION_US && region!=DD_REGION_DV)return 0;
    if(defect_limit>sizeof(sys->defect_info))defect_limit=sizeof(sys->defect_info);
    for(zone=0;zone<16;++zone){
        unsigned int end=sys->defect_offset[zone],at;
        if(end<begin || end>defect_limit)return 0;
        for(at=begin;at<end;++at)
            if(sys->defect_info[at]>=ZoneTracks[zone] || (at>begin && sys->defect_info[at]<=sys->defect_info[at-1]))return 0;
        begin=end;
    }
    return 1;
}
static const uint16_t om_dd_ram_start[7]={0x5a2,0x7c6,0x9ea,0xc0e,0xe32,0x1010,0x10dc};
static const uint32_t om_dd_ram_bytes[7]={0x24a9dc0,0x1c226c0,0x1450f00,0xd35680,0x6cfd40,0x1da240,0};

uint8_t* scan_and_expand_disk_format(uint8_t* data,size_t* length,
    unsigned int* format,unsigned int* development,
    size_t* offset_sys,size_t* offset_id,size_t* offset_ram,size_t* size_ram)
{
    static const unsigned int blocks[8]={0,1,2,3,8,9,10,11};
    size_t bytes,sys_at=0,id_at=0,ram_at=0,ram_bytes=0,new_bytes;
    unsigned int fmt,dev=0,selected=0,found=0,i,type;
    const struct dd_sys_data* sys;
    uint8_t* candidate=data;
    if(!data || !length || !format || !development || !offset_sys || !offset_id || !offset_ram || !size_ram)return NULL;
    bytes=*length;
    if(bytes<D64_OFFSET_DATA || bytes>OM_N64_SAVE_MAX_BYTES)return NULL;
    if(bytes==MAME_FORMAT_DUMP_SIZE || bytes==SDK_FORMAT_DUMP_SIZE){
        fmt=bytes==MAME_FORMAT_DUMP_SIZE?DISK_FORMAT_MAME:DISK_FORMAT_SDK;
        for(i=0;i<8;++i){
            unsigned int block=blocks[i],is_dev=block==2 || block==3 || block==10 || block==11;
            size_t at=(size_t)BLOCKSIZE(0)*block;
            unsigned int sector_bytes=is_dev?SECTORSIZE_SYS_DEV:SECTORSIZE_SYS,j,repeat=1;
            if(!om_dd_range(bytes,at,sizeof(struct dd_sys_data)) || !om_dd_range(bytes,at,(size_t)sector_bytes*SECTORS_PER_BLOCK))continue;
            sys=(const struct dd_sys_data*)(data+at);
            if(!om_dd_system_valid(sys,sector_bytes-(unsigned int)offsetof(struct dd_sys_data,defect_info)))continue;
            for(j=1;j<SECTORS_PER_BLOCK;++j)if(memcmp(data+at,data+at+(size_t)j*sector_bytes,sector_bytes)){repeat=0;break;}
            if(repeat){sys_at=at;dev=is_dev;selected=block;found=1;break;}
        }
        if(!found)return NULL;
        found=0;
        for(i=14;i<16;++i){
            size_t at=(size_t)BLOCKSIZE(0)*i;unsigned int j,repeat=1;
            if(!om_dd_range(bytes,at,(size_t)SECTORSIZE_SYS*SECTORS_PER_BLOCK))continue;
            for(j=1;j<SECTORS_PER_BLOCK;++j)if(memcmp(data+at,data+at+(size_t)j*SECTORSIZE_SYS,SECTORSIZE_SYS)){repeat=0;break;}
            if(repeat){id_at=at;found=1;break;}
        }
        if(!found)return NULL;
        (void)selected;
        sys=(const struct dd_sys_data*)(data+sys_at);type=sys->type&15;
        if(fmt==DISK_FORMAT_SDK){
            uint32_t at=LBAToByteA((uint8_t)type,0,om_dd_ram_start[type]);
            if(at==UINT32_MAX || !om_dd_range(bytes,at,om_dd_ram_bytes[type]))return NULL;
            ram_at=at;ram_bytes=om_dd_ram_bytes[type];
        }
        new_bytes=bytes;
    }else{
        uint16_t rom_end,ram_start,ram_end;
        uint32_t rom_bytes,stored_ram=0;
        unsigned int no_ram;
        fmt=DISK_FORMAT_D64;dev=1;sys_at=D64_OFFSET_SYS;id_at=D64_OFFSET_ID;
        if(!om_dd_range(bytes,sys_at,sizeof(struct dd_sys_data)))return NULL;
        sys=(const struct dd_sys_data*)(data+sys_at);
        if(!om_dd_system_valid(sys,sizeof(sys->defect_info)))return NULL;
        type=sys->type&15;rom_end=big16(sys->rom_lba_end);ram_start=big16(sys->ram_lba_start);ram_end=big16(sys->ram_lba_end);
        if(rom_end>=om_dd_ram_start[type]-SYSTEM_LBAS || big16(sys->ipl_load_blk)>rom_end+1u)return NULL;
        no_ram=ram_start==0xffff && ram_end==0xffff;
        if(!no_ram){
            if(type==6 || ram_start==0xffff || ram_end==0xffff || ram_start!=om_dd_ram_start[type]-SYSTEM_LBAS || ram_end<ram_start || ram_end>MAX_LBA-SYSTEM_LBAS)return NULL;
            stored_ram=LBAToByteA((uint8_t)type,SYSTEM_LBAS+ram_start,(uint32_t)ram_end+1u-ram_start);
            if(stored_ram==UINT32_MAX || stored_ram>om_dd_ram_bytes[type])return NULL;
        }
        rom_bytes=LBAToByteA((uint8_t)type,SYSTEM_LBAS,(uint32_t)rom_end+1u);
        if(rom_bytes==UINT32_MAX || (uint64_t)D64_OFFSET_DATA+rom_bytes+stored_ram!=bytes)return NULL;
        ram_at=(size_t)D64_OFFSET_DATA+rom_bytes;ram_bytes=om_dd_ram_bytes[type];
        if(ram_at>OM_N64_SAVE_MAX_BYTES || ram_bytes>OM_N64_SAVE_MAX_BYTES-ram_at)return NULL;
        new_bytes=ram_at+ram_bytes;
        if(new_bytes!=bytes){
            candidate=(uint8_t*)calloc(1,new_bytes);
            if(!candidate)return NULL;
            memcpy(candidate,data,bytes);
        }
        /* Every rejecting check/allocation precedes mutation or input release. */
        {
            struct dd_sys_data* normalized=(struct dd_sys_data*)(candidate+sys_at);
            normalized->format=0x10;normalized->type|=0x10;
            normalized->ram_lba_start=type<6?big16((uint16_t)(om_dd_ram_start[type]-SYSTEM_LBAS)):0xffff;
            normalized->ram_lba_end=type<6?big16((uint16_t)(MAX_LBA-SYSTEM_LBAS)):0xffff;
        }
        if(candidate!=data)free(data);
    }
    *length=new_bytes;*format=fmt;*development=dev;
    *offset_sys=sys_at;*offset_id=id_at;*offset_ram=ram_at;*size_ram=ram_bytes;
    return candidate;
}
