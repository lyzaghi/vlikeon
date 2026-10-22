// Actual pinned sinc, actual aligned allocator, checked API. Authored, not run.
#include "n64-sinc-capacity.h"
extern "C" {
#include "audio/audio_resampler.h"
int om_n64_sinc_process_checked(void*,const retro_resampler_t*,resampler_data*,size_t);
}
#include <algorithm>
#include <cmath>
#include <iostream>
#include <limits>
#include <stdexcept>
#include <vector>
static void require(bool good,const char* why){if(!good)throw std::runtime_error(why);}
static void compare(unsigned mask,resampler_quality quality){
    auto* baseline=sinc_resampler.init(nullptr,1.0,quality,mask);require(baseline,"real baseline init");
    auto direct=sinc_resampler.process;
    auto* checked=sinc_resampler.init(nullptr,1.0,quality,mask);require(checked,"real checked init");
    std::vector<float> input(256*2),left(OM_N64_SINC_OUTPUT_FRAMES*2+16,1234.0f),right=left;
    for(size_t i=0;i<input.size();++i)input[i]=static_cast<float>(static_cast<int>(i%31)-15)/16;
    for(double ratio: {1.0,44100.0/32000.0,44100.0/48000.0,44100.0/22050.0,0.125,16.0}){
        resampler_data a{},b{};a.data_in=b.data_in=input.data();a.data_out=left.data()+8;b.data_out=right.data()+8;
        a.input_frames=b.input_frames=256;a.ratio=b.ratio=ratio;
        std::fill(left.begin(),left.end(),1234.0f);std::fill(right.begin(),right.end(),1234.0f);
        direct(baseline,&a);require(om_n64_sinc_process_checked(checked,&sinc_resampler,&b,OM_N64_SINC_OUTPUT_FRAMES)==1,"checked actual backend admission and prediction");
        require(a.output_frames==b.output_frames&&a.output_frames<=OM_N64_SINC_OUTPUT_FRAMES,"real phase output agrees");
        for(size_t i=0;i<a.output_frames*2;++i)require(std::isfinite(left[8+i])&&left[8+i]==right[8+i],"checked samples identical to direct pinned processing");
        require(std::all_of(left.begin(),left.begin()+8,[](float v){return v==1234;})&&std::all_of(right.begin(),right.begin()+8,[](float v){return v==1234;}),"prefix guards");
        require(std::all_of(right.begin()+8+b.output_frames*2,right.end(),[](float v){return v==1234;}),"actual backend writes only predicted output span");
    }
    sinc_resampler.free(baseline);sinc_resampler.free(checked);
    auto* denied=sinc_resampler.init(nullptr,1.0,quality,mask);auto* reference=sinc_resampler.init(nullptr,1.0,quality,mask);
    require(denied&&reference,"refusal states init");
    resampler_data a{},b{};a.data_in=b.data_in=input.data();a.data_out=left.data()+8;b.data_out=right.data()+8;a.input_frames=b.input_frames=256;a.ratio=b.ratio=1;
    std::fill(left.begin(),left.end(),1234.0f);const auto before=left;
    require(om_n64_sinc_process_checked(denied,&sinc_resampler,&a,0)==0&&left==before,"capacity rejection before actual filter output");
    require(om_n64_sinc_process_checked(denied,&sinc_resampler,&a,OM_N64_SINC_OUTPUT_FRAMES)==1&&om_n64_sinc_process_checked(reference,&sinc_resampler,&b,OM_N64_SINC_OUTPUT_FRAMES)==1,"retry after rejected capacity");
    require(a.output_frames==b.output_frames&&std::equal(left.begin()+8,left.begin()+8+a.output_frames*2,right.begin()+8),"rejection preserved real filter state");
    sinc_resampler.free(denied);sinc_resampler.free(reference);
}
int main(){try{
    for(double bandwidth: {0.0,-1.0,1e-100,std::numeric_limits<double>::quiet_NaN(),std::numeric_limits<double>::infinity()})
        require(!sinc_resampler.init(nullptr,bandwidth,RESAMPLER_QUALITY_NORMAL,0),"invalid/excessive filter allocation rejected");
    compare(0,RESAMPLER_QUALITY_NORMAL);compare(0,RESAMPLER_QUALITY_LOWEST);compare(0,RESAMPLER_QUALITY_HIGHER);
    {
        auto* older=sinc_resampler.init(nullptr,1.0,RESAMPLER_QUALITY_NORMAL,0);auto selected=sinc_resampler.process;
        auto* reference=sinc_resampler.init(nullptr,1.0,RESAMPLER_QUALITY_NORMAL,0);
        auto* other=sinc_resampler.init(nullptr,1.0,RESAMPLER_QUALITY_LOWEST,0);
        require(older&&reference&&other,"mixed-quality instances initialized");
        std::vector<float> input(64,0.5f),a(256,0),b(256,0);resampler_data first{},second{};
        first.data_in=second.data_in=input.data();first.data_out=a.data();second.data_out=b.data();
        first.input_frames=second.input_frames=32;first.ratio=second.ratio=1;
        selected(reference,&second);
        require(om_n64_sinc_process_checked(older,&sinc_resampler,&first,128)==1
            && first.output_frames==second.output_frames&&std::equal(a.begin(),a.begin()+first.output_frames*2,b.begin()),"older instance retains its own window processor after global backend changes");
        sinc_resampler.free(older);sinc_resampler.free(reference);sinc_resampler.free(other);
    }
#if defined(__SSE__)
    compare(RESAMPLER_SIMD_SSE,RESAMPLER_QUALITY_NORMAL);
#endif
#if defined(__AVX__)
    compare(RESAMPLER_SIMD_AVX,RESAMPLER_QUALITY_HIGHER);
#endif
#if defined(__ARM_NEON__) || defined(HAVE_NEON)
    compare(RESAMPLER_SIMD_NEON,RESAMPLER_QUALITY_NORMAL);
#endif
    std::cout<<"Actual sinc source cases passed\n";return 0;
}catch(const std::exception& e){std::cerr<<e.what()<<'\n';return 1;}}
