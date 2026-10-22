/* Actual selected backend lifecycle. Partial allocation is independently
 * retired; a ready flag is published only after all required buffers exist. */
#include "n64-core-owner.h"
#include "n64-sinc-capacity.h"
#ifndef OM_N64_AUDIO_ALLOC
#define OM_N64_AUDIO_ALLOC malloc
#define OM_N64_AUDIO_FREE free
#endif
#ifndef OM_N64_AUDIO_CAN_RETIRE
#define OM_N64_AUDIO_CAN_RETIRE 1
#define OM_N64_AUDIO_CANCEL() ((void)0)
#endif
static int om_n64_audio_ready=0;
int om_n64_audio_is_ready(void){return om_n64_owner_access() && om_n64_audio_ready;}
void deinit_audio_libretro(void)
{
    if(!om_n64_owner_access())return;
    if(!OM_N64_AUDIO_CAN_RETIRE){om_n64_owner_admission_failed();return;}
    om_n64_audio_ready=0;
#ifdef __APPLE__
    if(audio_converter){
        if(AudioConverterDispose(audio_converter)!=noErr){om_n64_owner_admission_failed();return;}
        audio_converter=NULL;
    }
    OM_N64_AUDIO_FREE(converter_input_buf);converter_input_buf=NULL;input_buf_frames=0;converter_input_rate=0;
#else
    if(resampler_audio_data){
        if(!resampler || !resampler->free){om_n64_owner_admission_failed();return;}
        resampler->free(resampler_audio_data);resampler_audio_data=NULL;
    }
    resampler=NULL;
    OM_N64_AUDIO_FREE(audio_in_buffer_float);audio_in_buffer_float=NULL;
    OM_N64_AUDIO_FREE(audio_out_buffer_float);audio_out_buffer_float=NULL;
#endif
    OM_N64_AUDIO_FREE(audio_out_buffer_s16);audio_out_buffer_s16=NULL;
    OM_N64_AUDIO_CANCEL();
}
void init_audio_libretro(unsigned frames)
{
    if(!om_n64_owner_access())return;
    /* Repeated live initialization must not replace a running resampler. */
    if(om_n64_audio_ready || !frames || frames>65536){om_n64_owner_admission_failed();return;}
    deinit_audio_libretro();
    if(!om_n64_owner_admitted())return;
    MAX_AUDIO_FRAMES=frames;
#ifdef __APPLE__
    audio_out_buffer_s16=(int16_t*)OM_N64_AUDIO_ALLOC((size_t)frames*8);
    converter_input_buf=(int16_t*)OM_N64_AUDIO_ALLOC((size_t)frames*8);
    if(!audio_out_buffer_s16 || !converter_input_buf)goto failed;
    input_buf_frames=0;converter_input_rate=0;
#else
    if(!retro_resampler_realloc(&resampler_audio_data,&resampler,"sinc",RESAMPLER_QUALITY_DONTCARE,1.0)
        || !resampler_audio_data || !resampler || !resampler->free || !resampler->process)goto failed;
    audio_in_buffer_float=(float*)OM_N64_AUDIO_ALLOC((size_t)frames*2*sizeof(float));
    audio_out_buffer_float=(float*)OM_N64_AUDIO_ALLOC((size_t)OM_N64_SINC_OUTPUT_FRAMES*2*sizeof(float));
    audio_out_buffer_s16=(int16_t*)OM_N64_AUDIO_ALLOC((size_t)OM_N64_SINC_OUTPUT_FRAMES*2*sizeof(int16_t));
    if(!audio_in_buffer_float || !audio_out_buffer_float || !audio_out_buffer_s16)goto failed;
    convert_s16_to_float_init_simd();convert_float_to_s16_init_simd();
#endif
    om_n64_audio_ready=1;return;
failed:
    deinit_audio_libretro();om_n64_owner_admission_failed();
}
