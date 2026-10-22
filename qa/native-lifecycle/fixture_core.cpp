// Qualification-only core. Never admit this DLL to a product catalog/release.
// Deliberate C++ exceptions exercise the worker's unknown-lifetime handling.
#include <libretro.h>
#include <array>
#include <atomic>
#include <chrono>
#include <cstdlib>
#include <cstring>
#include <thread>
#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#endif

namespace {
retro_environment_t environment=nullptr;
std::thread producer;
std::atomic<bool> stop_producer{false};
bool initialized=false,loaded=false,ran=false,unknown=false,producer_owned=false;
unsigned pauses=0,unloads=0,deinits=0,runs=0;
std::array<unsigned char,16> memory{};
char scenario[32]="normal";
bool mode(const char* value){return std::strcmp(scenario,value)==0;}
void forbid_unknown(){if(unknown)std::_Exit(86);}
struct Witness{~Witness(){if(unknown||producer.joinable()||initialized||loaded)std::_Exit(86);}} witness;
void join_producer(){stop_producer.store(true);if(producer.joinable())producer.join();producer_owned=false;}
void start_producer(){if(!producer.joinable()){stop_producer=false;producer=std::thread([]{while(!stop_producer.load())std::this_thread::sleep_for(std::chrono::milliseconds(1));});producer_owned=true;}}
int checked_pause(){
  forbid_unknown();++pauses;
  if(ran&&mode("pause-refuse")){unknown=true;return 0;}
  if(ran&&mode("pause-throw")){unknown=true;throw 71;}
  if(ran&&mode("pause-invalid")){unknown=true;return 2;}
  join_producer();return 1;
}
void publish_memory(){const std::array<unsigned,4> words{pauses,unloads,deinits,runs};for(std::size_t i=0;i<words.size();++i)for(unsigned byte=0;byte<4;++byte)memory[i*4+byte]=static_cast<unsigned char>(words[i]>>(byte*8));}
}
#ifdef _WIN32
BOOL WINAPI DllMain(HINSTANCE,DWORD reason,LPVOID){
  if(reason==DLL_PROCESS_DETACH&&(unknown||producer_owned||initialized||loaded))TerminateProcess(GetCurrentProcess(),86);
  return TRUE;
}
#endif
extern "C" {
RETRO_API void retro_set_environment(retro_environment_t callback){environment=callback;}
RETRO_API void retro_set_video_refresh(retro_video_refresh_t){}
RETRO_API void retro_set_audio_sample(retro_audio_sample_t){}
RETRO_API void retro_set_audio_sample_batch(retro_audio_sample_batch_t){}
RETRO_API void retro_set_input_poll(retro_input_poll_t){}
RETRO_API void retro_set_input_state(retro_input_state_t){}
RETRO_API unsigned retro_api_version(){return RETRO_API_VERSION;}
RETRO_API void retro_init(){initialized=true;retro_variable option{"fixture_scenario",nullptr};if(environment&&environment(RETRO_ENVIRONMENT_GET_VARIABLE,&option)&&option.value){if(std::strlen(option.value)>=sizeof(scenario))std::_Exit(86);std::strcpy(scenario,option.value);}if(mode("init-throw")){unknown=true;throw 74;}}
RETRO_API void retro_deinit(){forbid_unknown();if(++deinits!=1)std::_Exit(86);if(mode("deinit-throw")){unknown=true;throw 72;}join_producer();initialized=false;}
RETRO_API void retro_get_system_info(retro_system_info* info){*info={};info->library_name="OmniPlay lifecycle qualification";info->library_version="1.0.0";info->valid_extensions="bin";}
RETRO_API void retro_get_system_av_info(retro_system_av_info* info){*info={};info->geometry.base_width=16;info->geometry.base_height=16;info->geometry.max_width=16;info->geometry.max_height=16;info->timing.fps=60;info->timing.sample_rate=48000;}
RETRO_API void retro_set_controller_port_device(unsigned,unsigned){}
RETRO_API bool retro_load_game(const retro_game_info* game){loaded=game&&game->data&&game->size==64;if(mode("load-throw")){start_producer();unknown=true;throw 75;}return loaded;}
RETRO_API void retro_unload_game(){forbid_unknown();if(++unloads!=1)std::_Exit(86);if(mode("unload-throw")){unknown=true;throw 73;}join_producer();loaded=false;}
RETRO_API void retro_run(){forbid_unknown();if(!loaded)std::_Exit(86);ran=true;++runs;start_producer();if(mode("run-throw")){unknown=true;throw 76;}}
RETRO_API void retro_reset(){}
RETRO_API std::size_t retro_serialize_size(){return memory.size();}
RETRO_API bool retro_serialize(void* data,std::size_t size){if(!data||size!=memory.size())return false;if(mode("serialize-throw")){unknown=true;throw 77;}publish_memory();std::memcpy(data,memory.data(),size);return true;}
RETRO_API bool retro_unserialize(const void*,std::size_t){if(mode("unserialize-throw")){unknown=true;throw 78;}return false;}
RETRO_API void* retro_get_memory_data(unsigned id){publish_memory();return id==RETRO_MEMORY_SAVE_RAM?memory.data():nullptr;}
RETRO_API std::size_t retro_get_memory_size(unsigned id){return id==RETRO_MEMORY_SAVE_RAM?memory.size():0;}
RETRO_API void omniplay_pause_guest(){(void)checked_pause();}
#ifndef OMNIPLAY_LIFECYCLE_VOID_ONLY
RETRO_API int omniplay_pause_guest_checked_v1(){return checked_pause();}
#endif
}
