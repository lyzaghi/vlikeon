/* Actual C2 processing: reject sector subtraction underflow and admit the
 * entire byte-swapped output extent before modifying any buffer byte. */
#include "n64-core-owner.h"
static void read_C2(struct dd_controller* dd)
{
    size_t length,offset,i;unsigned int sector;
    if(!om_n64_owner_access() || !dd)return;
    if(dd->bm_zone>=16){dd->regs[DD_ASIC_BM_STATUS_CTL]|=DD_BM_STATUS_MICRO;return;}
    sector=((dd->regs[DD_ASIC_CUR_SECTOR]>>16)&0xff)%90;
    if(sector<SECTORS_PER_BLOCK){dd->regs[DD_ASIC_BM_STATUS_CTL]|=DD_BM_STATUS_MICRO;return;}
    length=zone_sec_size[dd->bm_zone];offset=0x40*(sector-SECTORS_PER_BLOCK);
    if(offset>sizeof(dd->c2s_buf) || length>sizeof(dd->c2s_buf)-offset){dd->regs[DD_ASIC_BM_STATUS_CTL]|=DD_BM_STATUS_MICRO;return;}
    for(i=0;i<length;++i)dd->c2s_buf[(offset+i)^3]=0;
}
