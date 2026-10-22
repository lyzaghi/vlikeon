// Production phase planner against an independent per-tick sinc walk.
#include "n64-sinc-capacity.h"
#include <iostream>
#include <limits>
#include <stdexcept>
static void require(bool good,const char* why){if(!good)throw std::runtime_error(why);}
int main(){try{
    for(uint32_t phases: {16u,256u,4096u})for(uint32_t step: {1u,3u,16u,300u})
    for(uint32_t initial: {0u,1u,15u,1000u})for(size_t frames=0;frames<24;++frames){
        uint64_t time=initial;size_t remaining=frames,count=0;
        while(remaining){while(remaining&&time>=phases){time-=phases;--remaining;}while(time<phases){++count;time+=step;}}
        size_t observed=123;double ratio=static_cast<double>(phases)/step;
        // Match the actual integer step, including floating division rounding.
        auto actual_step=static_cast<uint32_t>(phases/ratio);if(actual_step!=step)continue;
        require(om_n64_sinc_output_count(phases,initial,ratio,frames,OM_N64_SINC_OUTPUT_FRAMES,&observed)==1&&observed==count,"preflight equals pinned independent tick walk");
        if(count){observed=123;require(om_n64_sinc_output_count(phases,initial,ratio,frames,count-1,&observed)==0&&observed==123,"insufficient output rejects without publishing count");}
    }
    size_t output=123;
    for(double ratio: {0.0,-1.0,std::numeric_limits<double>::infinity(),std::numeric_limits<double>::quiet_NaN(),1e100,1e-100})
        require(om_n64_sinc_output_count(1u<<24,0,ratio,1,OM_N64_SINC_OUTPUT_FRAMES,&output)==-1&&output==123,"invalid or unrepresentable fixed-point step rejected");
    require(om_n64_sinc_output_count(1u<<24,0,44100.0,1,OM_N64_SINC_OUTPUT_FRAMES,&output)==1&&output<=OM_N64_SINC_OUTPUT_FRAMES,"one-frame low-frequency expansion fits fixed output bound");
    require(om_n64_sinc_output_count(1u<<24,0,44100.0,2048,OM_N64_SINC_OUTPUT_FRAMES,&output)==0,"large low-frequency block requires subdivision before writes");
    require(om_n64_sinc_output_count(1u<<24,0,1.0,65536,OM_N64_SINC_OUTPUT_FRAMES,&output)==1&&output==65537,"startup phase output included, not only input times ratio");
    require(om_n64_sinc_output_count(3,0,1,1,100,&output)==-1&&om_n64_sinc_output_count(16,0,1,65537,100,&output)==-1,"phase and work budgets admitted");
    std::cout<<"Sinc capacity source cases passed\n";return 0;
}catch(const std::exception& e){std::cerr<<e.what()<<'\n';return 1;}}
