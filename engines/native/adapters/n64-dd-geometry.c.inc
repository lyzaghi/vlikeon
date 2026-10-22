static uint8_t* storage_disk_data(const void* opaque){const struct dd_disk* disk=(const struct dd_disk*)opaque;return disk->istorage->data(disk->storage);}
static size_t storage_disk_size(const void* opaque){const struct dd_disk* disk=(const struct dd_disk*)opaque;return disk->istorage->size(disk->storage);}
static void storage_disk_save_dummy(void* opaque,size_t start,size_t size){(void)opaque;(void)start;(void)size;}
static void storage_disk_save_full(void* opaque,size_t start,size_t size)
{
    struct dd_disk* disk=(struct dd_disk*)opaque;
    if(disk && disk->isave_storage && disk->save_storage && om_dd_range(storage_disk_size(disk),start,size))disk->isave_storage->save(disk->save_storage,start,size);
}
static void storage_disk_save_ram_only(void* opaque,size_t start,size_t size)
{
    struct dd_disk* disk=(struct dd_disk*)opaque;size_t end,begin,limit,available;
    if(!disk || !disk->isave_storage || !disk->save_storage || !om_dd_range(storage_disk_size(disk),start,size))return;
    available=disk->isave_storage->size(disk->save_storage);
    if(!om_dd_range(storage_disk_size(disk),disk->offset_ram,available))return;
    end=start+size;begin=start>disk->offset_ram?start:disk->offset_ram;limit=disk->offset_ram+available;
    if(end>limit)end=limit;
    if(end>begin)disk->isave_storage->save(disk->save_storage,begin-disk->offset_ram,end-begin);
}
const struct storage_backend_interface g_istorage_disk_read_only={storage_disk_data,storage_disk_size,storage_disk_save_dummy};
const struct storage_backend_interface g_istorage_disk_full={storage_disk_data,storage_disk_size,storage_disk_save_full};
const struct storage_backend_interface g_istorage_disk_ram_only={storage_disk_data,storage_disk_size,storage_disk_save_ram_only};

uint32_t LBAToVZoneA(uint8_t type,uint32_t lba)
{
    uint32_t zone;type&=15;if(type>6 || lba>MAX_LBA)return UINT32_MAX;
    for(zone=0;zone<16;++zone)if(lba<VZoneLBATable[type][zone])return zone;
    return UINT32_MAX;
}
uint32_t LBAToVZone(const struct dd_sys_data* sys,uint32_t lba){return sys?LBAToVZoneA(sys->type,lba):UINT32_MAX;}
uint32_t LBAToByteA(uint8_t type,uint32_t lba,uint32_t count)
{
    uint64_t bytes=0;uint32_t at;type&=15;
    if(type>6 || lba>SIZE_LBA || count>SIZE_LBA-lba)return UINT32_MAX;
    for(at=0;at<count;++at){
        uint32_t zone=LBAToVZoneA(type,lba+at),physical;
        if(zone>=16)return UINT32_MAX;
        physical=DiskTypeZones[type][zone];if(physical>7)physical-=7;
        if(physical>=9)return UINT32_MAX;
        bytes+=(uint64_t)zone_sec_size_phys[physical]*SECTORS_PER_BLOCK;
    }
    return bytes>UINT32_MAX?UINT32_MAX:(uint32_t)bytes;
}
uint32_t LBAToByte(const struct dd_sys_data* sys,uint32_t lba,uint32_t count){return sys?LBAToByteA(sys->type,lba,count):UINT32_MAX;}
uint16_t LBAToPhys(const struct dd_sys_data* sys,uint32_t lba)
{
    uint32_t type,vzone,pzone,head,zone,relative,track,base,begin,end;
    if(!sys || !om_dd_system_valid(sys,sizeof(sys->defect_info)) || lba>MAX_LBA)return 0xffff;
    type=sys->type&15;vzone=LBAToVZoneA((uint8_t)type,lba);if(vzone>=16)return 0xffff;
    pzone=DiskTypeZones[type][vzone];if(pzone>=16)return 0xffff;
    head=pzone>=8;zone=pzone-head*8;
    relative=(lba-(vzone?VZoneLBATable[type][vzone-1]:0))>>1;
    base=TrackZoneTable[0][zone];
    /* Address the two actual rows explicitly. Pinned code indexed row zero
     * beyond its eight elements to reach head one's reverse start row. */
    if(head){if(relative>TrackZoneTable[1][zone])return 0xffff;track=TrackZoneTable[1][zone]-relative;}
    else track=base+relative;
    begin=pzone?sys->defect_offset[pzone-1]:0;end=sys->defect_offset[pzone];
    while(begin<end && base+sys->defect_info[begin]<=track){++track;++begin;}
    if(track<base || track>=base+ZoneTracks[pzone])return 0xffff;
    return (uint16_t)(track|(head<<12)|(((lba&3)==1 || (lba&3)==2)?0x2000:0));
}
int GenerateLBAToPhysTable(struct dd_disk* disk)
{
    uint16_t staged[SIZE_LBA];uint8_t seen[0x800]={0};uint32_t lba;const uint8_t* data;size_t bytes;const struct dd_sys_data* sys;
    if(!disk || !disk->istorage || !disk->storage)return 0;
    if(disk->format==DISK_FORMAT_MAME)return 1;
    data=storage_disk_data(disk);bytes=storage_disk_size(disk);
    if(!data || !om_dd_range(bytes,disk->offset_sys,sizeof(struct dd_sys_data)))return 0;
    sys=(const struct dd_sys_data*)(data+disk->offset_sys);
    for(lba=0;lba<SIZE_LBA;++lba){
        uint16_t code=LBAToPhys(sys,lba);
        if(code>=0x4000 || (seen[code>>3]&(1u<<(code&7))))return 0;
        seen[code>>3]|=(uint8_t)(1u<<(code&7));staged[lba]=code;
    }
    memcpy(disk->lba_phys_table,staged,sizeof(staged));return 1;
}
uint32_t PhysToLBA(const struct dd_disk* disk,uint16_t head,uint16_t track,uint16_t block)
{
    uint32_t lba;uint16_t code;
    if(!disk || head>1 || block>1 || track>=TrackZoneTable[0][7]+ZoneTracks[7])return 0xffff;
    code=(uint16_t)(track|(head<<12)|(block<<13));
    for(lba=0;lba<SIZE_LBA;++lba)if(disk->lba_phys_table[lba]==code)return lba;
    return 0xffff;
}
unsigned int get_zone_from_head_track(unsigned int head,unsigned int track)
{
    unsigned int zone=7;
    if(head>1 || track>=TrackZoneTable[0][7]+ZoneTracks[7])return UINT32_MAX;
    while(zone && track<TrackZoneTable[0][zone])--zone;
    return zone+head;
}
static int om_dd_protected(size_t lba,size_t system,size_t id)
{return (lba<PROTECT_LBA && lba!=system) || (lba>PROTECT_LBA && lba<DISKID_LBA+2 && lba!=id);}
uint8_t* get_sector_base(const struct dd_disk* disk,unsigned int head,unsigned int track,unsigned int block,unsigned int sector,size_t requested)
{
    static const uint32_t start[16]={0,0x5f15e0,0xb79d00,0x10801a0,0x1523720,0x1963d80,0x1d414c0,0x20bbce0,0x23196e0,0x28a1e00,0x2df5dc0,0x3299340,0x36d99a0,0x3ab70e0,0x3e31900,0x4149200};
    uint8_t* data;size_t bytes;uint32_t physical,zone,sector_bytes,lba,type;uint64_t offset;const struct dd_sys_data* sys;
    if(!disk || !disk->storage || !disk->istorage || !disk->istorage->data || !disk->istorage->size || block>1 || sector>=SECTORS_PER_BLOCK || !requested || requested>256)return NULL;
    physical=get_zone_from_head_track(head,track);if(physical>=9)return NULL;zone=physical-head;
    data=storage_disk_data(disk);bytes=storage_disk_size(disk);
    if(!data || bytes>OM_N64_SAVE_MAX_BYTES || !om_dd_range(bytes,disk->offset_sys,sizeof(struct dd_sys_data)))return NULL;
    sys=(const struct dd_sys_data*)(data+disk->offset_sys);
    if(!om_dd_system_valid(sys,sizeof(sys->defect_info)))return NULL;
    sector_bytes=(disk->development && head==0 && track<6 && disk->format!=DISK_FORMAT_D64)?SECTORSIZE_SYS_DEV:zone_sec_size_phys[physical];
    if(requested>sector_bytes)return NULL;
    if(disk->format==DISK_FORMAT_MAME){
        uint32_t combined=zone+head*8;
        offset=(uint64_t)start[combined]+(uint64_t)(track-TrackZoneTable[0][zone])*TRACKSIZE(combined)+(uint64_t)block*BLOCKSIZE(combined)+(uint64_t)sector*sector_bytes;
        if(sector==0 && track<SYSTEM_LBAS/2 && om_dd_protected((size_t)offset/BLOCKSIZE(0),disk->offset_sys/BLOCKSIZE(0),disk->offset_id/BLOCKSIZE(0)))return NULL;
    }else{
        lba=PhysToLBA(disk,(uint16_t)head,(uint16_t)track,(uint16_t)block);if(lba>MAX_LBA)return NULL;
        type=sys->type&15;
        if(disk->format==DISK_FORMAT_SDK){
            uint32_t prefix=LBAToByteA((uint8_t)type,0,lba);if(prefix==UINT32_MAX)return NULL;
            offset=(uint64_t)prefix+(uint64_t)sector*sector_bytes;
            if(sector==0 && lba<SYSTEM_LBAS && om_dd_protected(lba,disk->offset_sys/BLOCKSIZE(0),disk->offset_id/BLOCKSIZE(0)))return NULL;
        }else if(disk->format==DISK_FORMAT_D64){
            uint16_t rom_end=big16(sys->rom_lba_end),ram_begin=big16(sys->ram_lba_start),ram_end=big16(sys->ram_lba_end);uint32_t prefix;
            if(rom_end>=om_dd_ram_start[type]-SYSTEM_LBAS)return NULL;
            if(lba<DISKID_LBA)offset=disk->offset_sys;
            else if(lba<SYSTEM_LBAS)offset=disk->offset_id;
            else if(lba<=SYSTEM_LBAS+rom_end){prefix=LBAToByteA((uint8_t)type,SYSTEM_LBAS,lba-SYSTEM_LBAS);if(prefix==UINT32_MAX)return NULL;offset=(uint64_t)D64_OFFSET_DATA+prefix+(uint64_t)sector*sector_bytes;}
            else if(ram_begin!=0xffff && ram_end!=0xffff && ram_begin==om_dd_ram_start[type]-SYSTEM_LBAS && ram_end<=MAX_LBA-SYSTEM_LBAS && lba-SYSTEM_LBAS>=ram_begin && lba-SYSTEM_LBAS<=ram_end){prefix=LBAToByteA((uint8_t)type,SYSTEM_LBAS+ram_begin,lba-SYSTEM_LBAS-ram_begin);if(prefix==UINT32_MAX)return NULL;offset=(uint64_t)disk->offset_ram+prefix+(uint64_t)sector*sector_bytes;}
            else return NULL;
        }else return NULL;
    }
    if(offset>bytes || requested>bytes-(size_t)offset)return NULL;
    return data+(size_t)offset;
}
const char* get_disk_format_name(unsigned int format)
{switch(format){case DISK_FORMAT_MAME:return "MAME";case DISK_FORMAT_SDK:return "SDK";case DISK_FORMAT_D64:return "D64";default:return "<unknown>";}}
