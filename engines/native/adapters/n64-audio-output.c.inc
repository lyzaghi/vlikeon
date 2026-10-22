#include "n64-audio-output.h"
#ifndef OM_N64_AUDIO_PENDING_DEFINED
static struct om_n64_audio_queue om_n64_audio_pending;
#endif
int om_n64_audio_callback_idle(void)
{return om_n64_owner_access() && !om_n64_audio_pending.busy;}
int om_n64_audio_is_retired(void)
{
    if(!om_n64_audio_callback_idle() || om_n64_audio_ready || om_n64_audio_pending.count || audio_out_buffer_s16)return 0;
#ifdef __APPLE__
    return !audio_converter && !converter_input_buf;
#else
    return !resampler_audio_data && !resampler && !audio_in_buffer_float && !audio_out_buffer_float;
#endif
}
int om_n64_audio_drain_pending(void)
{
    int result;
    if(!om_n64_owner_access() || !om_n64_audio_ready)return -1;
    result=om_n64_audio_queue_drain(&om_n64_audio_pending,audio_batch_cb);
    if(result<0)om_n64_owner_admission_failed();
    return result;
}
static int om_n64_audio_submit(const int16_t* samples,size_t frames)
{
    if(om_n64_audio_drain_pending()<0)return 0;
    if(!om_n64_audio_queue_append(&om_n64_audio_pending,samples,frames)){
        /* Finite queue exhaustion is a session error, never reported as a
         * successful drop. Existing pending frames remain owned until stop. */
        om_n64_owner_admission_failed();return 0;
    }
    return om_n64_audio_drain_pending()>=0;
}
