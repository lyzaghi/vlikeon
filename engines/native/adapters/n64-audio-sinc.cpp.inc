   /* Non-Apple: fixed output storage with pre-write phase admission. */
   while(frames){
      size_t chunk=frames<MAX_AUDIO_FRAMES?frames:MAX_AUDIO_FRAMES;
      struct resampler_data data={0};int result;
      data.data_in=audio_in_buffer_float;data.data_out=audio_out_buffer_float;
      data.ratio=44100.0/GameFreq;
      for(;;){
         data.input_frames=chunk;data.output_frames=0;
         convert_s16_to_float(audio_in_buffer_float,raw_data,chunk*2,1.0f);
         result=om_n64_sinc_process_checked(resampler_audio_data,resampler,&data,OM_N64_SINC_OUTPUT_FRAMES);
         if(result!=0 || chunk==1)break;
         chunk=(chunk+1)/2;
      }
      if(result!=1){om_n64_owner_admission_failed();return;}
      convert_float_to_s16(audio_out_buffer_s16,audio_out_buffer_float,data.output_frames*2);
      if(!om_n64_audio_submit(audio_out_buffer_s16,data.output_frames))return;
      raw_data+=chunk*2;frames-=chunk;
   }
