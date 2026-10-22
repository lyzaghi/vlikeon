/* Included in the actual fb.c. Descriptors and accesses are independently
 * checked; a valid caller does not authenticate plugin-provided dimensions. */
#include "n64-core-owner.h"
#include "n64-device-memory.h"
static int om_n64_fb_range(struct fb* fb,uint32_t raw,uint32_t length,uint32_t* at)
{
    return fb && fb->rdram && fb->rdram->dram
        && om_n64_rdram_extent(fb->rdram->dram_size,raw,length,at);
}
static int om_n64_fb_info(struct fb* fb,const FrameBufferInfo* info,uint32_t* at,uint32_t* length)
{
    return fb && fb->rdram && fb->rdram->dram && info->addr
        && om_n64_framebuffer_extent(fb->rdram->dram_size,info->addr,info->width,
            info->height,info->size,at,length);
}
void pre_framebuffer_read_range(struct fb* fb,uint32_t address,uint32_t length)
{
    uint32_t at;size_t i;
    if(!om_n64_owner_access() || !length || !gfx.fBRead
        || !om_n64_fb_range(fb,address,length,&at) || !fb->infos[0].addr)return;
    for(i=0;i<FB_INFOS_COUNT;++i){
        uint32_t begin,bytes,first,end;
        if(!om_n64_fb_info(fb,&fb->infos[i],&begin,&bytes))continue;
        first=at>begin?at:begin;end=at+length<begin+bytes?at+length:begin+bytes;
        while(first<end){
            uint32_t page=first>>12;
            if(fb->dirty_page[page]){
                gfx.fBRead(first);fb->dirty_page[page]=0;
            }
            first=(page+1)<<12;
        }
    }
}
void pre_framebuffer_read(struct fb* fb,uint32_t address)
{pre_framebuffer_read_range(fb,address,1);}
void post_framebuffer_write(struct fb* fb,uint32_t address,uint32_t length)
{
    uint32_t at,step;size_t i;
    if(!om_n64_owner_access() || !length || !gfx.fBWrite
        || !om_n64_fb_range(fb,address,length,&at) || !fb->infos[0].addr)return;
    step=length%4==0?4:(length%2==0?2:1);
    for(i=0;i<FB_INFOS_COUNT;++i){
        uint32_t begin,bytes,first,end,delta;
        if(!om_n64_fb_info(fb,&fb->infos[i],&begin,&bytes))continue;
        first=at>begin?at:begin;end=at+length<begin+bytes?at+length:begin+bytes;
        if(first>=end)continue;
        /* Same start positions/granularity as the pinned loop, without
         * scanning the whole DMA for every nonintersecting descriptor. */
        delta=first-at;first=at+((delta+step-1)/step)*step;
        for(;first<end;first+=step)gfx.fBWrite(first,step);
    }
}
