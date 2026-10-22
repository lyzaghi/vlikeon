// Actual non-Apple lifecycle body with allocation/resampler failure injection.
#include "n64-core-owner.h"
#include "n64-audio-output.h"
#include <cstdint>
#include <cstdlib>
#include <iostream>
#include <set>
#include <stdexcept>
#include <thread>
static unsigned MAX_AUDIO_FRAMES=2048;
struct FixtureResampler {void(*free)(void*);void(*process)(void*,void*);};
using retro_resampler_t=FixtureResampler;
static const retro_resampler_t* resampler=nullptr;
static void* resampler_audio_data=nullptr;
static float* audio_in_buffer_float=nullptr;
static float* audio_out_buffer_float=nullptr;
static int16_t* audio_out_buffer_s16=nullptr;
static unsigned allocations=0,fail_at=0,releases=0,simd_calls=0;
static bool fail_resampler=false;
static std::set<void*> live;
static void* allocate(std::size_t bytes){++allocations;if(allocations==fail_at)return nullptr;auto p=std::malloc(bytes);if(p)live.insert(p);return p;}
static void release(void* p){if(!p)return;if(live.erase(p)!=1)throw std::runtime_error("foreign or duplicate release");++releases;std::free(p);}
static void process(void*,void*){}
static constexpr int RESAMPLER_QUALITY_DONTCARE=0;
static bool retro_resampler_realloc(void** out,const retro_resampler_t** api,const char*,int,double){
    static const FixtureResampler methods{release,process};
    *out=allocate(64);*api=&methods;return *out && !fail_resampler;
}
static void convert_s16_to_float_init_simd(){++simd_calls;}
static void convert_float_to_s16_init_simd(){++simd_calls;}
static struct om_n64_audio_queue om_n64_audio_pending;
#define OM_N64_AUDIO_CAN_RETIRE (!om_n64_audio_pending.busy)
#define OM_N64_AUDIO_CANCEL() do {om_n64_audio_pending.head=om_n64_audio_pending.count=0;om_n64_audio_pending.fault=0;} while(0)
#define OM_N64_AUDIO_ALLOC allocate
#define OM_N64_AUDIO_FREE release
#ifdef __APPLE__
#error This fixture qualifies the selected Windows/non-Apple source branch only
#endif
#include "n64-audio-lifecycle.c.inc"
#undef OM_N64_AUDIO_FREE
#undef OM_N64_AUDIO_ALLOC
static size_t (*audio_batch_cb)(const int16_t*,size_t)=nullptr;
#define OM_N64_AUDIO_PENDING_DEFINED 1
#include "n64-audio-output.c.inc"
static void require(bool good,const char* why){if(!good)throw std::runtime_error(why);}
static void begin(){allocations=fail_at=releases=simd_calls=0;fail_resampler=false;require(live.empty(),"previous allocations retired");require(om_n64_owner_begin()==1,"activate owner");}
static void end(){deinit_audio_libretro();require(live.empty()&&!resampler&&!resampler_audio_data&&!audio_in_buffer_float&&!audio_out_buffer_float&&!audio_out_buffer_s16&&!om_n64_audio_is_ready()&&om_n64_audio_is_retired(),"complete independent cleanup");require(om_n64_owner_end(nullptr)==1,"retire owner");}
static size_t attempt_retire(const int16_t* samples,size_t frames){
    const auto before=live.size();require(!om_n64_audio_callback_idle()&&!om_n64_audio_is_retired(),"active borrow cannot acknowledge retirement");
    deinit_audio_libretro();require(live.size()==before&&om_n64_audio_ready&&samples[0]==12&&samples[1]==-12,"reentrant direct backend retirement cannot release borrowed storage");return frames;
}
int main(){try{
    for(unsigned failure=1;failure<=4;++failure){begin();fail_at=failure;init_audio_libretro(2048);require(!om_n64_audio_is_ready()&&!om_n64_owner_admitted()&&live.empty()&&!simd_calls,"every partial allocation failure cleans buffers and denies readiness");end();}
    begin();fail_resampler=true;init_audio_libretro(2048);require(!om_n64_audio_is_ready()&&live.empty()&&releases==1,"failed resampler with partial handle still retired");end();
    begin();init_audio_libretro(2048);require(om_n64_audio_is_ready()&&live.size()==4&&simd_calls==2,"readiness published only after complete allocation");
    const auto prior=allocations;std::thread foreign([]{deinit_audio_libretro();init_audio_libretro(2048);});foreign.join();require(om_n64_audio_is_ready()&&allocations==prior&&live.size()==4,"foreign thread cannot replace or release audio ownership");
    init_audio_libretro(2048);require(!om_n64_owner_admitted()&&allocations==prior&&live.size()==4,"live reinitialization rejected without replacing buffers");end();
    for(unsigned frames: {0u,65537u,UINT32_MAX}){begin();init_audio_libretro(frames);require(!allocations&&!om_n64_audio_is_ready()&&!om_n64_owner_admitted(),"invalid capacity rejected before allocation");end();}
    begin();init_audio_libretro(65536);require(om_n64_audio_is_ready()&&MAX_AUDIO_FRAMES==65536,"maximum admitted frames");end();deinit_audio_libretro();require(live.empty(),"idempotent retirement");
    begin();init_audio_libretro(2048);audio_batch_cb=attempt_retire;const int16_t samples[2]={12,-12};
    require(om_n64_audio_queue_append(&om_n64_audio_pending,samples,1)&&om_n64_audio_drain_pending()==1&&om_n64_audio_callback_idle(),"real queue callback releases borrow before later cleanup");
    require(!om_n64_owner_admitted()&&!om_n64_audio_is_retired(),"forbidden direct retirement records failure without false retired receipt");end();
    std::cout<<"Audio lifecycle source cases passed\n";return 0;
}catch(const std::exception& e){std::cerr<<e.what()<<'\n';return 1;}}
