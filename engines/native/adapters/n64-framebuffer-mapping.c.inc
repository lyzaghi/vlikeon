/* Actual protect/unprotect lifecycle, paired around display-list processing. */
void protect_framebuffers(struct fb* fb)
{
    FrameBufferInfo staged[FB_INFOS_COUNT];size_t i,used=0;
    struct mem_mapping mapping={0,0,M64P_MEM_RDRAM,{fb,read_rdram_fb,write_rdram_fb}};
    if(!om_n64_owner_access() || !fb || !fb->mem || !fb->r4300 || !fb->rdram
        || !gfx.fBGetFrameBufferInfo || !gfx.fBRead || !gfx.fBWrite
        || fb->r4300->emumode==EMUMODE_DYNAREC)return;
    memset(staged,0,sizeof(staged));
    gfx.fBGetFrameBufferInfo(staged);
    memset(fb->infos,0,sizeof(fb->infos));
    if(!staged[0].addr)return; /* pinned API's empty-list sentinel */
    for(i=0;i<FB_INFOS_COUNT;++i){
        uint32_t begin,bytes,page;
        if(!om_n64_fb_info(fb,&staged[i],&begin,&bytes))continue;
        staged[i].addr=begin;fb->infos[used++]=staged[i];
        mapping.begin=begin;mapping.end=begin+bytes-1;
        apply_mem_mapping(fb->mem,&mapping);
        for(page=begin>>12;page<=mapping.end>>12;++page)fb->dirty_page[page]=1;
        if(fb->once){
            fb->once=0;
#ifndef NEW_DYNAREC
            fb->r4300->recomp.fast_memory=0;
#endif
            /* This zero-length invalidation intentionally means all code. */
            invalidate_r4300_cached_code(fb->r4300,0,0);
        }
    }
}
void unprotect_framebuffers(struct fb* fb)
{
    size_t i;
    struct mem_mapping mapping={0,0,M64P_MEM_RDRAM,{NULL,read_rdram_dram,write_rdram_dram}};
    if(!om_n64_owner_access() || !fb || !fb->mem || !fb->infos[0].addr)return;
    mapping.handler.opaque=fb->rdram;
    for(i=0;i<FB_INFOS_COUNT;++i){
        uint32_t begin,bytes;
        if(!om_n64_fb_info(fb,&fb->infos[i],&begin,&bytes))continue;
        mapping.begin=begin;mapping.end=begin+bytes-1;
        apply_mem_mapping(fb->mem,&mapping);
    }
    /* Keep old descriptors for CPU notifications during display-list work. */
}
