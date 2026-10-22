/* Included after the exact pinned sinc backend definition. */
#include "n64-sinc-capacity.h"
int om_n64_sinc_process_checked(void* opaque,const retro_resampler_t* backend,
    struct resampler_data* data,size_t capacity)
{
    rarch_sinc_resampler_t* state=(rarch_sinc_resampler_t*)opaque;
    size_t predicted;unsigned bits;int admitted;
    if(!state || backend!=&sinc_resampler || !backend->process || !data
        || !data->data_in || !data->data_out)return -1;
    if(state->phase_bits>30 || state->subphase_bits>30-state->phase_bits)return -1;
    if(!state->main_buffer || state->phase_table!=state->main_buffer || !state->om_process
        || !state->taps || state->taps>4096 || state->ptr>state->taps
        || !state->om_phase_elems || state->om_phase_elems>state->om_buffer_elems
        || 4u*(size_t)state->taps>state->om_buffer_elems-state->om_phase_elems
        || state->buffer_l!=state->main_buffer+state->om_phase_elems
        || state->buffer_r!=state->buffer_l+2u*state->taps)return -1;
    bits=state->phase_bits+state->subphase_bits;
    admitted=om_n64_sinc_output_count(UINT32_C(1)<<bits,state->time,data->ratio,
        data->input_frames,capacity,&predicted);
    if(admitted!=1)return admitted;
    state->om_process(opaque,data);
    return data->output_frames==predicted?1:-1;
}
