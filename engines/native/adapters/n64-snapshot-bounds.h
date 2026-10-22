#ifndef OMNIPLAY_N64_SNAPSHOT_BOUNDS_H
#define OMNIPLAY_N64_SNAPSHOT_BOUNDS_H
#include <stddef.h>
#include <stdint.h>
#include <string.h>

/* Pinned M64P event wire: pairs of 32-bit words, then one sentinel.
 * Decode explicitly for preflight, before the stock host-endian conversion.
 * Duplicates other than SPECIAL are legal in the upstream scheduler. */
static inline uint32_t om_n64_snapshot_word(const unsigned char* p,int little)
{
    uint32_t value;
    if(!little){memcpy(&value,p,4);return value;}
    return (uint32_t)p[0]|((uint32_t)p[1]<<8)|((uint32_t)p[2]<<16)|((uint32_t)p[3]<<24);
}
static inline int om_n64_snapshot_queue_valid(const void* data,size_t bytes,int little)
{
    const unsigned char* p=(const unsigned char*)data;
    size_t at=0,total=0,normal=0,special=0;
    if(!p)return 0;
    while(at<=bytes && bytes-at>=4){
        uint32_t type=om_n64_snapshot_word(p+at,little);
        if(type==UINT32_MAX)return normal<16 && special<=1;
        if(bytes-at<8 || total==16 || !type || (type&(type-1)) || type>0x4000)return 0;
        ++total;if(type==0x20)++special;else ++normal;
        at+=8;
    }
    return 0;
}
/* Returns the entire command extent, or zero without reading a short header.
 * The same admission is used for live PIF and restored channel pointers. */
static inline size_t om_n64_pif_command_extent(const uint8_t* data,size_t bytes)
{
    size_t extent;
    if(!data || bytes<2 || !(data[0]&0x3f))return 0;
    extent=2+(size_t)(data[0]&0x3f)+(size_t)(data[1]&0x3f);
    return extent<=bytes?extent:0;
}
/* memcpy preserves the historical raw-word ID semantics without alignment
 * assumptions. The caller supplies the admitted format's actual ID offset. */
static inline int om_n64_snapshot_disk_word(const uint8_t* data,size_t bytes,size_t offset,uint32_t* output)
{
    uint32_t value;
    if(!data || !output || offset>bytes || bytes-offset<4)return 0;
    memcpy(&value,data+offset,4);*output=value;return 1;
}
/* Structural/pointer preflight for the fixed-size native libretro image.
 * This is deliberately not a semantic validator of CPU/peripheral registers.
 * Header is BE, body/queue are LE; PIF bytes have no word conversion. */
enum { OM_N64_SNAPSHOT_BYTES=16788288+1024+4+4096,
       OM_N64_SNAPSHOT_PIF_AT=44+400+0x800000+0x2000,
       OM_N64_SNAPSHOT_QUEUE_AT=16788288,
       OM_N64_SNAPSHOT_EXTRA_AT=16788288+1024+4,
       OM_N64_SNAPSHOT_LUT_AT=OM_N64_SNAPSHOT_PIF_AT+64+24,
       OM_N64_SNAPSHOT_TLB_AT=16788288-16-32*52,
       /* PIF channels through v1.9 CP0/CP2 latches, with 19 DD regs. */
       OM_N64_SNAPSHOT_RTC_AFTER_PIF=5+1+1+4+7*10*4+4+(19*4+256+64+16+5*4)+4+2*4*4+128+2*4+4+2+2+2*8 };
/* Hardware-width fields and the six derived 32-bit quantities emitted by
 * the pinned TLBWrite/PJ64 import. Arbitrary guest PageMask bit patterns are
 * retained; do not substitute a list of preferred page sizes. Addition and
 * PFN shifts intentionally retain the interpreter's uint32 wrap semantics. */
static inline int om_n64_snapshot_tlb_entry_valid(const uint8_t* p)
{
    uint32_t mask=(uint32_t)p[0]|((uint32_t)p[1]<<8);
    uint32_t vpn=om_n64_snapshot_word(p+4,1);
    uint32_t even=om_n64_snapshot_word(p+12,1),odd=om_n64_snapshot_word(p+20,1);
    uint32_t se=om_n64_snapshot_word(p+28,1),ee=om_n64_snapshot_word(p+32,1),pe=om_n64_snapshot_word(p+36,1);
    uint32_t so=om_n64_snapshot_word(p+40,1),eo=om_n64_snapshot_word(p+44,1),po=om_n64_snapshot_word(p+48,1);
    uint32_t expected_se,expected_ee,expected_so,expected_eo;
    if(mask>0xfff || vpn>0x7ffff || p[8]>1 || even>0xffffff || odd>0xffffff
       || p[16]>7 || p[17]>1 || p[18]>1 || p[24]>7 || p[25]>1 || p[26]>1 || p[27]>3)return 0;
    /* Never-written power-on entries have zero derived fields as well.
     * Ignore padding and unused region bits, but not an ASID/validity field. */
    if(!mask && !vpn && !p[8] && !p[9] && !even && !odd && !p[16] && !p[17] && !p[18]
       && !p[24] && !p[25] && !p[26] && !se && !ee && !pe && !so && !eo && !po)return 1;
    expected_se=vpn<<13;
    expected_ee=expected_se+(mask<<12)+UINT32_C(0xfff);
    expected_so=expected_ee+UINT32_C(1);
    expected_eo=expected_so+(mask<<12)+UINT32_C(0xfff);
    return se==expected_se && ee==expected_ee && pe==(even<<12)
        && so==expected_so && eo==expected_eo && po==(odd<<12);
}
static inline int om_n64_snapshot_tlb_valid(const uint8_t* image)
{
    size_t at;
    for(at=0;at<32;++at)if(!om_n64_snapshot_tlb_entry_valid(image+OM_N64_SNAPSHOT_TLB_AT+52*at))return 0;
    /* The selected interpreter writes zero or a page-ending physical mapping.
     * Max = (0x1ffff000 + 0xfff000 + 0xfff) | 0x80000000.
     * This also bounds both cached-interpreter invalid-code lookup aliases.
     * Read/write tables can differ after overlaps; do not rebuild or equate them. */
    for(at=0;at<2u*0x100000u;++at){
        uint32_t value=om_n64_snapshot_word(image+OM_N64_SNAPSHOT_LUT_AT+4*at,1);
        if(value && ((value&0xfff)!=0xfff || value<UINT32_C(0x80000000) || value>UINT32_C(0xa0ffefff)))return 0;
    }
    return 1;
}
/* Page indices avoid a 32-bit byte counter wrapping at 0xfffff000. End is
 * inclusive. Reversed/wrapped halves remain unmapped as in the pinned core.
 * Every admitted half contains at most the hardware's 4096 pages. */
static inline int om_n64_tlb_page_span(uint32_t start,uint32_t end,uint32_t* first,uint32_t* limit)
{
    if(!first || !limit || (start&0xfff) || (end&0xfff)!=0xfff || start>end || end-start>0xffffff)return 0;
    *first=start>>12;*limit=(end>>12)+1;return 1;
}
/* One complete Transfer Pak record, with optional 92-byte GB state. Type is
 * inside the stored 28-byte ROM fingerprint (header 0x147 minus 0x134).
 * Raw RTC/camera data bytes remain legal: guest register writes are unmasked.
 * Unsupported upstream MBC implementations are not certified by this bound. */
static inline int om_n64_snapshot_transferpak_valid(const uint8_t* p,size_t bytes,uint32_t version,size_t* used)
{
    uint32_t rom,ram,enable,mode,latch,max_rom=0x1ff,max_ram=0xff;
    uint8_t type;
    size_t length;
    if(!p || !used || bytes<44 || om_n64_snapshot_word(p,1)>1 || om_n64_snapshot_word(p+4,1)>3
       || om_n64_snapshot_word(p+8,1)>1 || om_n64_snapshot_word(p+12,1)>3)return 0;
    length=p[16]?136:44;
    if(bytes<length)return 0;
    if(length==44){*used=length;return 1;}
    type=p[16+19];
    rom=om_n64_snapshot_word(p+44,1);ram=om_n64_snapshot_word(p+48,1);
    enable=om_n64_snapshot_word(p+52,1);mode=om_n64_snapshot_word(p+56,1);
    latch=om_n64_snapshot_word(p+44+(version==0x10200?21:16),1);
    switch(type){
    case 0x00:case 0x08:case 0x09:max_rom=1;max_ram=0;break;
    case 0x01:case 0x02:case 0x03:max_rom=0x7f;max_ram=3;break;
    case 0x05:case 0x06:max_rom=0xf;max_ram=0;break;
    case 0x0f:case 0x10:case 0x11:case 0x12:case 0x13:max_rom=0x7f;break;
    case 0x19:case 0x1a:case 0x1b:case 0x1c:case 0x1d:case 0x1e:max_ram=0xf;break;
    case 0xfc:max_rom=0x3f;break;
    default:break; /* Keep bounded compatibility with remaining MBC profiles. */
    }
    if(rom>max_rom || ram>max_ram || enable>1 || mode>1 || latch>(version>=0x10a00?255u:1u))return 0;
    if(type==0xfc && p[44+38]>7)return 0;
    *used=length;return 1;
}
/* Stage all v1.10 flags before device mutation or PIF callbacks. A bad later
 * port never partially publishes the earlier ports to the caller. */
static inline int om_n64_snapshot_rtc_flags(const void* data,size_t bytes,uint8_t* output)
{
    const uint8_t* p=(const uint8_t*)data;uint8_t staged[4];size_t at,i;
    if(!p || !output || bytes!=OM_N64_SNAPSHOT_BYTES || memcmp(p,"M64+SAVE",8)
       || p[8]!=0 || p[9]!=1 || p[10]!=10 || p[11]!=0)return 0;
    at=OM_N64_SNAPSHOT_EXTRA_AT+48;
    for(i=0;i<4;++i){size_t used;if(at>bytes || !om_n64_snapshot_transferpak_valid(p+at,bytes-at,0x10a00,&used))return 0;at+=used;}
    at+=OM_N64_SNAPSHOT_RTC_AFTER_PIF;
    if(at>bytes || bytes-at<16)return 0;
    for(i=0;i<4;++i){uint32_t value=om_n64_snapshot_word(p+at+4*i,1);if(value>1)return 0;staged[i]=(uint8_t)value;}
    memcpy(output,staged,sizeof(staged));return 1;
}
/* Known device selectors only. Raw addresses/lengths remain register data and
 * are range-checked by actual DMA handlers at use, including registers changed
 * while a transfer is pending. Ignore absent DD padding; ID zero requires the
 * actual loaded-disk context because zero is also a legitimate disk ID. */
static inline int om_n64_snapshot_devices_valid(const void* data,size_t bytes,int has_dd,uint32_t current_dd_id)
{
    const uint8_t* p=(const uint8_t*)data;size_t at,i,pif;uint32_t version;
    if(!p || bytes!=OM_N64_SNAPSHOT_BYTES || memcmp(p,"M64+SAVE",8))return 0;
    version=((uint32_t)p[8]<<24)|((uint32_t)p[9]<<16)|((uint32_t)p[10]<<8)|p[11];
    if(version<0x10000 || version>0x10a00 || (version&0xff))return 0;
    if(version<0x10200)return 1;
    at=OM_N64_SNAPSHOT_EXTRA_AT+(version==0x10200?46:48);
    for(i=0;i<4;++i){size_t used;if(at>bytes || !om_n64_snapshot_transferpak_valid(p+at,bytes-at,version,&used))return 0;at+=used;}
    pif=at;
    if(at>bytes || bytes-at<11)return 0;
    if(p[at+(version==0x10200?9:5)]>2 || p[at+(version==0x10200?10:6)]>3)return 0;
    at=pif+11+7*10*4;
    if(version>=0x10400){
        uint32_t id;
        if(at>bytes || bytes-at<436)return 0;
        id=om_n64_snapshot_word(p+at,1);
        if(id || (has_dd && id==current_dd_id)){
            if(om_n64_snapshot_word(p+at+44,1)>255 || om_n64_snapshot_word(p+at+416,1)>1
               || om_n64_snapshot_word(p+at+420,1)>1 || om_n64_snapshot_word(p+at+428,1)>=16)return 0;
        }
        at+=436;
    }
    if(version>=0x10500)at+=4;
    if(version>=0x10700){
        if(at>bytes || bytes-at<32 || om_n64_snapshot_word(p+at,1)>1 || om_n64_snapshot_word(p+at+16,1)>1)return 0;
        at+=32;
    }
    if(version>=0x10800){
        if(at>bytes || bytes-at<144 || om_n64_snapshot_word(p+at+136,1)>255
           || ((unsigned int)p[at+142]|((unsigned int)p[at+143]<<8))>5)return 0;
    }
    return 1;
}
static inline int om_n64_snapshot_pointer_preflight(const void* data,size_t bytes)
{
    const uint8_t* p=(const uint8_t*)data;
    const uint8_t* ram;
    uint32_t version;
    size_t at,i;
    if(!p || bytes!=OM_N64_SNAPSHOT_BYTES || memcmp(p,"M64+SAVE",8))return 0;
    version=((uint32_t)p[8]<<24)|((uint32_t)p[9]<<16)|((uint32_t)p[10]<<8)|p[11];
    if(version<0x10000 || version>0x10a00 || (version&0xff))return 0;
    if(!om_n64_snapshot_queue_valid(p+OM_N64_SNAPSHOT_QUEUE_AT,1024,1))return 0;
    if(!om_n64_snapshot_tlb_valid(p))return 0;
    if(!om_n64_snapshot_devices_valid(p,bytes,0,0))return 0;
    ram=p+OM_N64_SNAPSHOT_PIF_AT;
    if(version<0x10200){
        size_t channel=0;
        for(at=0;at<64 && channel<5;){
            uint8_t tag=ram[at];size_t extent;
            if(tag==0xfe)break;
            if(tag==0xff){++at;continue;}
            if(tag==0 || tag==0xfd){++at;++channel;continue;}
            /* Match the pinned parser's known bogus-command-byte skip. */
            if(at+1<64 && ram[at+1]==0xfe){++at;continue;}
            extent=om_n64_pif_command_extent(ram+at,64-at);
            if(!extent)return 0;
            at+=extent;++channel;
        }
        return 1;
    }
    /* v1.2 omits the two-byte AF-RTC alignment pad. Each cartridge has
     * 16 bytes of Transfer Pak registers + a 28-byte fingerprint, and,
     * if present, 92 bytes of banks/RTC/camera state in either layout. */
    at=OM_N64_SNAPSHOT_EXTRA_AT+(version==0x10200?46:48);
    for(i=0;i<4;++i){
        size_t used;
        if(at>bytes || !om_n64_snapshot_transferpak_valid(p+at,bytes-at,version,&used))return 0;
        at+=used;
    }
    if(bytes-at<5)return 0;
    for(i=0;i<5;++i){
        uint8_t offset=p[at+i];
        if(offset>=128)continue; /* signed negative = disabled */
        if(offset>=64 || !om_n64_pif_command_extent(ram+offset,64-offset))return 0;
    }
    if(version>=0x10a00){
        uint8_t staged[4];if(!om_n64_snapshot_rtc_flags(p,bytes,staged))return 0;
    }
    return 1;
}
#endif
