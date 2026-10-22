#ifndef OMNIPLAY_N64_AI_TIMING_H
#define OMNIPLAY_N64_AI_TIMING_H
#include <stdint.h>
#include <float.h>
/* Retain pinned integer truncation before float modifier. Intermediate wraps,
 * float-to-unsigned overflow and delays outside the signed CP0 horizon reject. */
static inline int om_n64_ai_duration(uint32_t clock,uint32_t delay,uint32_t refresh,
    uint32_t dacrate,uint32_t length,float modifier,uint32_t* duration)
{
    uint64_t rate,counts,base;float scaled;
    if(!duration || !clock || !(modifier>=0 && modifier<=FLT_MAX))return 0;
    rate=(uint64_t)clock/((uint64_t)dacrate+1);
    counts=delay?(uint64_t)delay*refresh:clock;
    if(!rate || rate>UINT32_MAX/4 || !counts || counts>UINT32_MAX)return 0;
    base=(uint64_t)length*(counts/(rate*4));
    if(base>UINT32_MAX)return 0;
    scaled=(float)(uint32_t)base*modifier;
    if(!(scaled>=0 && (double)scaled<=2147483647.0))return 0;
    *duration=(uint32_t)scaled;return 1;
}
#endif
