/* Included by the actual selected tlb.c. Keep pinned mapping/overlap behavior,
 * including ignored wrapped virtual halves and the KSEG/physical exclusions. */
#include "n64-snapshot-bounds.h"
static void om_n64_tlb_unmap_half(struct tlb* tlb,uint32_t start,uint32_t end,int valid,int dirty)
{
    uint32_t page,limit;
    if(!valid || !om_n64_tlb_page_span(start,end,&page,&limit))return;
    for(;page<limit;++page){tlb->LUT_r[page]=0;if(dirty)tlb->LUT_w[page]=0;}
}
void tlb_unmap(struct tlb* tlb,size_t entry)
{
    const struct tlb_entry* e;
    if(!tlb || entry>=32)return;
    e=&tlb->entries[entry];
    om_n64_tlb_unmap_half(tlb,e->start_even,e->end_even,e->v_even,e->d_even);
    om_n64_tlb_unmap_half(tlb,e->start_odd,e->end_odd,e->v_odd,e->d_odd);
}
static void om_n64_tlb_map_half(struct tlb* tlb,uint32_t start,uint32_t end,uint32_t phys,int valid,int dirty)
{
    uint32_t page,first,limit;
    if(!valid || !om_n64_tlb_page_span(start,end,&first,&limit)
       || (start>=UINT32_C(0x80000000) && end<UINT32_C(0xc0000000))
       || phys>=UINT32_C(0x20000000) || (phys&0xfff))return;
    for(page=first;page<limit;++page){
        uint32_t value=UINT32_C(0x80000000)|(phys+((page-first)<<12)+UINT32_C(0xfff));
        tlb->LUT_r[page]=value;if(dirty)tlb->LUT_w[page]=value;
    }
}
void tlb_map(struct tlb* tlb,size_t entry)
{
    const struct tlb_entry* e;
    if(!tlb || entry>=32)return;
    e=&tlb->entries[entry];
    om_n64_tlb_map_half(tlb,e->start_even,e->end_even,e->phys_even,e->v_even,e->d_even);
    om_n64_tlb_map_half(tlb,e->start_odd,e->end_odd,e->phys_odd,e->v_odd,e->d_odd);
}
