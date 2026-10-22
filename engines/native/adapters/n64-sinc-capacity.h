#ifndef OMNIPLAY_N64_SINC_CAPACITY_H
#define OMNIPLAY_N64_SINC_CAPACITY_H
#include <stddef.h>
#include <stdint.h>
enum { OM_N64_SINC_OUTPUT_FRAMES=131072 };
/* Dry-run the pinned sinc phase walk before it writes any output. No filter
 * samples, phase time or caller outputs change on refusal. */
static inline int om_n64_sinc_output_count(uint32_t phases,uint32_t time,double ratio,
    size_t frames,size_t capacity,size_t* output)
{
    uint32_t step;double raw;size_t count=0;
    if(!output || !phases || (phases&(phases-1)) || phases>0x40000000
        || !(ratio>0) || frames>65536 || capacity>OM_N64_SINC_OUTPUT_FRAMES)return -1;
    raw=phases/ratio;
    if(!(raw>=1 && raw<=(double)(UINT32_MAX-phases)))return -1;
    step=(uint32_t)raw;
    while(frames){
        size_t available=time/phases,take=available<frames?available:frames;
        time-=(uint32_t)((uint64_t)take*phases);frames-=take;
        if(time<phases){
            uint64_t generated=((uint64_t)phases-time+step-1)/step;
            if(generated>capacity-count)return 0;
            count+=(size_t)generated;time+=(uint32_t)(generated*step);
        }
    }
    *output=count;return 1;
}
#endif
