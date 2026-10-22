// Actual AI publication/backend copy bodies with an explicit sample processor.
#include "n64-core-owner.h"
extern "C" {
#include "backends/api/audio_out_backend.h"
#include "device/rcp/ai/ai_controller.h"
#include "device/rcp/ri/ri_controller.h"
#include "device/rdram/rdram.h"
}
#include <array>
#include <cstdlib>
#include <cstring>
#include <iostream>
#include <stdexcept>
#include <thread>
#include <vector>
static int om_n64_audio_ready=1;
static int idle=1,fail_allocate=0;
static size_t processed=0,allocations=0,frees=0;
static std::vector<uint8_t> received;
static int om_n64_audio_callback_idle(){return om_n64_owner_access()&&idle;}
static void* allocate(size_t size){++allocations;return fail_allocate?nullptr:std::malloc(size);}
static void release(void* ptr){if(ptr){++frees;std::free(ptr);}}
#define OM_N64_AUDIO_ALLOC allocate
#define OM_N64_AUDIO_FREE release
static void aiLenChanged(void*,const void* buffer,size_t bytes){
    ++processed;auto* writable=static_cast<uint8_t*>(const_cast<void*>(buffer));
    received.assign(writable,writable+bytes);
    for(size_t i=0;i<bytes;i+=4){auto a=writable[i],b=writable[i+1];writable[i]=writable[i+2];writable[i+1]=writable[i+3];writable[i+2]=a;writable[i+3]=b;}
}
#include "n64-audio-source.c.inc"
#include "n64-ai-memory.c.inc"
static void push(void* ai,const void* buffer,size_t bytes){push_audio_samples_via_libretro(ai,buffer,bytes);}
static void frequency(void*,unsigned){}
static void require(bool ok,const char* why){if(!ok)throw std::runtime_error(why);}
int main(){try{
    std::array<uint32_t,64> words{};for(size_t i=0;i<words.size();++i)words[i]=static_cast<uint32_t>(i+0x12340000);const auto original=words;
    rdram ram{};ram.dram=words.data();ram.dram_size=sizeof(words);ri_controller ri{};ri.rdram=&ram;ai_controller ai{};ai.ri=&ri;
    audio_out_backend_interface api{frequency,push};ai.iaout=&api;ai.aout=&ai;
    ai.fifo[0].address=sizeof(words)-16;ai.fifo[0].length=16;ai.last_read=16;
    require(om_n64_ai_publish(&ai,8)&&ai.last_read==8&&received.size()==8&&!std::memcmp(received.data(),reinterpret_cast<uint8_t*>(words.data())+sizeof(words)-16,8),"first partial publication matches admitted RAM slice");
    require(om_n64_ai_publish(&ai,0)&&ai.last_read==0&&words==original&&allocations==2&&frees==2,"final publication leaves game RAM unchanged and private copies retire");
    ai.fifo[0].address=sizeof(words)-12;ai.last_read=16;auto before=processed;
    require(!om_n64_ai_publish(&ai,0)&&processed==before&&ai.last_read==16,"invalid complete FIFO span cannot publish even a fitting suffix");
    ai.fifo[0].address=0;ai.last_read=17;require(!om_n64_ai_publish(&ai,0)&&processed==before,"last_read cannot underflow source offset");
    ai.last_read=8;require(!om_n64_ai_publish(&ai,12)&&processed==before,"remaining count cannot exceed last_read");
    auto* bytes=reinterpret_cast<uint8_t*>(words.data());push_audio_samples_via_libretro(&ai,bytes+sizeof(words)-4,8);push_audio_samples_via_libretro(&ai,bytes+1,4);push_audio_samples_via_libretro(&ai,nullptr,4);
    require(processed==before&&words==original,"backend rejects end overrun, misalignment and null before copying");
    fail_allocate=1;push_audio_samples_via_libretro(&ai,bytes,4);require(processed==before&&words==original,"allocation failure does not modify RAM or invoke processor");fail_allocate=0;
    require(om_n64_owner_begin()==1,"activate owner");idle=0;push_audio_samples_via_libretro(&ai,bytes,4);require(processed==before&&!om_n64_owner_admitted(),"callback borrow rejects recursive source publication");idle=1;require(om_n64_owner_end(nullptr)==1,"retire failed owner");
    require(om_n64_owner_begin()==1,"new owner");ai.last_read=8;before=allocations;std::thread foreign([&]{om_n64_ai_publish(&ai,0);push_audio_samples_via_libretro(&ai,bytes,4);});foreign.join();
    require(allocations==before&&ai.last_read==8&&words==original,"foreign thread cannot publish/copy/advance audio RAM");require(om_n64_owner_end(nullptr)==1,"retire owner");
    std::cout<<"Audio source cases passed\n";return 0;
}catch(const std::exception& e){std::cerr<<e.what()<<'\n';return 1;}}
