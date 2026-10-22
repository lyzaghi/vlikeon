#ifndef OMNIPLAY_N64_AUDIO_OUTPUT_H
#define OMNIPLAY_N64_AUDIO_OUTPUT_H
#include <stddef.h>
#include <stdint.h>
#include <string.h>
enum { OM_N64_AUDIO_QUEUE_FRAMES=262144 };
enum { OM_N64_AUDIO_BATCH_FRAMES=16384 };
struct om_n64_audio_queue {
    int16_t samples[OM_N64_AUDIO_QUEUE_FRAMES*2];
    size_t head,count;int busy,fault;
};
typedef size_t (*om_n64_audio_consumer)(const int16_t*,size_t);
/* No callback remains borrowed after this function returns. At most eight
 * calls per drain. Zero consumption preserves exact samples and ordering. */
static inline int om_n64_audio_queue_drain(struct om_n64_audio_queue* q,om_n64_audio_consumer consumer)
{
    unsigned calls=0;
    if(!q || q->busy || q->fault || !consumer)return -1;
    q->busy=1;
    while(q->count && calls++<8){
        size_t offered=OM_N64_AUDIO_QUEUE_FRAMES-q->head,accepted;
        if(offered>q->count)offered=q->count;
        if(offered>OM_N64_AUDIO_BATCH_FRAMES)offered=OM_N64_AUDIO_BATCH_FRAMES;
        accepted=consumer(q->samples+q->head*2,offered);
        if(accepted>offered){q->fault=1;q->busy=0;return -1;}
        if(!accepted)break;
        q->head=(q->head+accepted)%OM_N64_AUDIO_QUEUE_FRAMES;q->count-=accepted;
    }
    q->busy=0;return q->count?0:1;
}
static inline int om_n64_audio_queue_append(struct om_n64_audio_queue* q,const int16_t* samples,size_t frames)
{
    size_t tail,first;
    if(!q || q->busy || q->fault || (!samples && frames) || frames>OM_N64_AUDIO_QUEUE_FRAMES-q->count)return 0;
    if(!frames)return 1;
    tail=(q->head+q->count)%OM_N64_AUDIO_QUEUE_FRAMES;
    first=OM_N64_AUDIO_QUEUE_FRAMES-tail;if(first>frames)first=frames;
    memcpy(q->samples+tail*2,samples,first*2*sizeof(int16_t));
    if(frames>first)memcpy(q->samples,samples+first*2,(frames-first)*2*sizeof(int16_t));
    q->count+=frames;return 1;
}
#endif
