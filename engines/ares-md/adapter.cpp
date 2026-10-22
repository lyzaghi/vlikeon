// Mega Drive libretro boundary over the pinned ares software core. Source only;
// exported core calls are never permitted to unwind into the host ABI.
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <fstream>
#include <iterator>
#include <limits>
#include <memory>
#include <span>
#include <stdexcept>
#include <string>
#include <vector>
#include <libretro.h>
#include <md/md.hpp>
#include "cartridge_analysis.hpp"
#ifdef OMNIPLAY_FILE_SAVES
#include "web-files.hpp"
#endif

namespace {
using namespace ares;
constexpr size_t kMaximumRom=64*1024*1024,kMaximumState=64*1024*1024;
constexpr double kSampleRate=48000.0;
constexpr char kVersion[]="0.3.0+ares.9408cb43d494";
struct Host final:Platform {
  retro_environment_t environment=nullptr;retro_video_refresh_t video_callback=nullptr;
  retro_audio_sample_t sample_callback=nullptr;retro_audio_sample_batch_t batch_callback=nullptr;
  retro_input_poll_t poll_callback=nullptr;retro_input_state_t input_callback=nullptr;
  Node::System root;VFS::Pak system_pak,cartridge_pak;
  std::vector<Node::Audio::Stream> streams;
  std::vector<u32> pixels;std::vector<int16_t> samples;
  std::vector<uint8_t> rom,svp;unsigned width=0,height=0;double refresh=60,published_refresh=60;
  bool loaded=false,fault=false,pal=false;
#ifdef OMNIPLAY_FILE_SAVES
  std::array<size_t,2> persistent_sizes{};
  bool describe_save(std::uint32_t* output){
    if(!output||!loaded||fault||!cartridge_pak)return false;
    std::uint32_t ram_mode=0,eeprom_type=0;
    for(size_t index=0;index<persistent_sizes.size();++index){
      auto file=cartridge_pak->read(omni_md_files::names[index]);
      if(!persistent_sizes[index]){if(file)return false;continue;}
      if(!file||!file->data()||file->size()!=persistent_sizes[index]||persistent_sizes[index]>1024*1024)return false;
      auto mode=file->attribute("mode");
      if(index==0){if(mode=="word")ram_mode=1;else if(mode=="upper")ram_mode=2;else if(mode=="lower")ram_mode=3;else return false;}
      else{
        static const char* modes[]={"X24C01","M24C01","M24C02","M24C04","M24C08","M24C16","M24C32","M24C64","M24C65","M24C128","M24C256","M24C512"};
        for(size_t slot=0;slot<std::size(modes);++slot)if(mode==modes[slot]){eeprom_type=static_cast<std::uint32_t>(slot+1);break;}
        if(!eeprom_type)return false;
      }
    }
    output[1]=1;output[2]=static_cast<std::uint32_t>(persistent_sizes[0]);output[3]=static_cast<std::uint32_t>(persistent_sizes[1]);output[4]=ram_mode;output[5]=eeprom_type;
    output[0]=0x3143444du; // MDC1, written last as the completion tag.
    return true;
  }
  bool persist_files(){
    if(!loaded||fault||!root||!cartridge_pak)return false;
    const char* directory=nullptr;
    if(!environment||!environment(RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY,&directory)||!directory||std::strcmp(directory,omni_md_files::root()))return false;
    MegaDrive::Board::omni_save_failed=false;
    root->save();
    if(fault||MegaDrive::Board::omni_save_failed)return false;
    bool okay=true;
    for(size_t index=0;index<persistent_sizes.size();++index){
      auto file=cartridge_pak->read(omni_md_files::names[index]);
      if(!persistent_sizes[index]){if(file)okay=false;continue;}
      if(!file||!file->data()||file->size()!=persistent_sizes[index]){okay=false;continue;}
      if(!omni_md_files::publish(omni_md_files::names[index],{file->data(),persistent_sizes[index]}))okay=false;
    }
    return okay;
  }
#endif

  std::string option(const char* key,const char* fallback) {
    retro_variable value{key,nullptr};
    if(environment&&environment(RETRO_ENVIRONMENT_GET_VARIABLE,&value)&&value.value)return value.value;
    return fallback;
  }
  auto attach(Node::Object node) -> void override {
    try {if(auto stream=node->cast<Node::Audio::Stream>()){if(streams.size()>=8){fault=true;return;}stream->setResamplerFrequency(kSampleRate);streams.push_back(stream);}}
    catch(...){fault=true;}
  }
  auto detach(Node::Object node) -> void override {
    try{if(auto stream=node->cast<Node::Audio::Stream>())std::erase(streams,stream);}catch(...){fault=true;}
  }
  auto pak(Node::Object node) -> VFS::Pak override {
    if(node->name()=="Mega Drive")return system_pak;
    if(node->name()=="Mega Drive Cartridge")return cartridge_pak;
    return {};
  }
  auto video(Node::Video::Screen,const u32* data,u32 pitch,u32 w,u32 h) -> void override {
    try {
      if(!data||w==0||h==0||w>2048||h>1024||pitch<size_t(w)*4||pitch>32*1024*1024u/h){fault=true;return;}
      pixels.resize(size_t(w)*h);width=w;height=h;
      for(unsigned y=0;y<h;++y)std::memcpy(pixels.data()+size_t(y)*w,reinterpret_cast<const uint8_t*>(data)+size_t(y)*pitch,size_t(w)*4);
    }catch(...){fault=true;}
  }
  auto refreshRateHint(double value) -> void override {if(std::isfinite(value)&&value>=45&&value<=65)refresh=value;}
  auto audio(Node::Audio::Stream) -> void override {
    try {
      if(streams.empty())return;
      while(true){
        for(const auto& stream:streams)if(!stream->pending())return;
        double mix[2]={0,0};
        for(auto& stream:streams){double frame[2]={0,0};auto channels=stream->read(frame);if(channels!=1&&channels!=2){fault=true;return;}mix[0]+=frame[0];mix[1]+=frame[channels==1?0:1];}
        if(samples.size()>=32768){fault=true;return;}
        for(double value:mix)samples.push_back(std::isfinite(value)?static_cast<int16_t>(std::lround(std::clamp(value,-1.0,1.0)*32767.0)):0);
      }
    }catch(...){fault=true;}
  }
  auto input(Node::Input::Input node) -> void override {
    try {
      auto button=node->cast<Node::Input::Button>();if(!button)return;
      unsigned port=4;
      for(auto parent=node->parent().lock();parent;parent=parent->parent().lock()){
        auto name=parent->name();
        if(name=="Controller Port 1"){port=0;break;}if(name=="Controller Port 2"){port=1;break;}
      }
      const std::array<const char*,12> names{"Up","Down","Left","Right","A","B","C","X","Y","Z","Mode","Start"};
      const std::array<unsigned,12> keys{RETRO_DEVICE_ID_JOYPAD_UP,RETRO_DEVICE_ID_JOYPAD_DOWN,RETRO_DEVICE_ID_JOYPAD_LEFT,RETRO_DEVICE_ID_JOYPAD_RIGHT,RETRO_DEVICE_ID_JOYPAD_Y,RETRO_DEVICE_ID_JOYPAD_B,RETRO_DEVICE_ID_JOYPAD_A,RETRO_DEVICE_ID_JOYPAD_L,RETRO_DEVICE_ID_JOYPAD_X,RETRO_DEVICE_ID_JOYPAD_R,RETRO_DEVICE_ID_JOYPAD_SELECT,RETRO_DEVICE_ID_JOYPAD_START};
      bool pressed=false;
      if(port<4&&input_callback)for(size_t i=0;i<names.size();++i)if(button->name()==names[i]){pressed=input_callback(port,RETRO_DEVICE_JOYPAD,0,keys[i])!=0;break;}
      button->setValue(pressed);
    }catch(...){fault=true;}
  }
  void flush_audio(){
    if(batch_callback&&!samples.empty())batch_callback(samples.data(),samples.size()/2);
    else if(sample_callback)for(size_t i=0;i+1<samples.size();i+=2)sample_callback(samples[i],samples[i+1]);
    samples.clear();
  }
  void unload() noexcept {
    loaded=false;try{if(root)root->unload();}catch(...){}root.reset();
    streams.clear();system_pak.reset();cartridge_pak.reset();rom.clear();svp.clear();pixels.clear();samples.clear();width=height=0;fault=false;
#ifdef OMNIPLAY_FILE_SAVES
    persistent_sizes={};
#endif
  }
  bool create(const retro_game_info* game){
    unload();platform=this;
    retro_pixel_format pixel_format=RETRO_PIXEL_FORMAT_XRGB8888;if(!environment||!environment(RETRO_ENVIRONMENT_SET_PIXEL_FORMAT,&pixel_format))return false;
    if(!game||!game->data||game->size<0x800||game->size>kMaximumRom)return false;
    const auto* bytes=static_cast<const uint8_t*>(game->data);
    if(!std::memcmp(bytes+0x100,"SEGA 32X",8)||!std::memcmp(bytes+0x100,"SEGA32X",7))return false;
    rom.assign(bytes,bytes+game->size);
    omni_md::MegaDrive analysis;auto report=analysis.analyze(rom);auto document=BML::unserialize(report);if(!document)return false;
    cartridge_pak=std::make_shared<vfs::directory>();system_pak=std::make_shared<vfs::directory>();
    cartridge_pak->setAttribute("title",document["game/title"].string());cartridge_pak->setAttribute("serial",document["game/serial"].string());
    cartridge_pak->setAttribute("region",document["game/region"].string());cartridge_pak->setAttribute("board",document["game/board"].string());
    cartridge_pak->setAttribute("label",document["game/label"].string());cartridge_pak->setAttribute("bootable",true);
    cartridge_pak->setAttribute("jcart",(bool)document["game/board(peripheral=J-Cart)"]);
    cartridge_pak->append("program.rom",std::span<const u8>(rom.data(),rom.size()));
    if(document["game/board/memory(type=ROM,content=SVP)"]){
      const char* directory=nullptr;if(!environment||!environment(RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY,&directory)||!directory)return false;
      std::ifstream source(std::string(directory)+"/svp.rom",std::ios::binary);svp.resize(2048);
      if(!source.read(reinterpret_cast<char*>(svp.data()),svp.size())||source.peek()!=std::char_traits<char>::eof())return false;
      cartridge_pak->append("svp.rom",std::span<const u8>(svp.data(),svp.size()));
    }
#ifdef OMNIPLAY_FILE_SAVES
    const char* save_directory=nullptr;
    if(!environment||!environment(RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY,&save_directory)||!save_directory||std::strcmp(save_directory,omni_md_files::root()))return false;
#endif
    size_t persistent_index=0;
    for(const auto& spec:std::array<std::pair<const char*,const char*>,2>{{{"RAM","ram"},{"EEPROM","eeprom"}}}){
      const size_t current_index=persistent_index++;
      auto memory=document[string{"game/board/memory(type=",spec.first,",content=Save)"}];if(!memory)continue;
      auto size=memory["size"].natural();if(size==0||size>1024*1024)return false;
      string filename={"save.",spec.second};if(!cartridge_pak->append(filename,size))return false;
      auto file=cartridge_pak->write(filename);if(!file)return false;
      file->setAttribute("mode",memory["mode"].string());file->setAttribute("address",memory["address"].natural());file->setAttribute("enable",memory["enable"].boolean());
      file->setAttribute("rsda",memory["rsda"].natural());file->setAttribute("wsda",memory["wsda"].natural());file->setAttribute("wscl",memory["wscl"].natural());
      for(u64 offset=0;offset<size;++offset)file->write(0xff);
#ifdef OMNIPLAY_FILE_SAVES
      if(!file->data()||file->size()!=size||!omni_md_files::restore(omni_md_files::names[current_index],{file->data(),static_cast<size_t>(size)}))return false;
      persistent_sizes[current_index]=static_cast<size_t>(size);
#endif
    }
    auto region=option("omniplay_md_region","auto");
    if(region=="auto"){auto regions=document["game/region"].string();region=regions.find("NTSC-U")?"ntsc-u":regions.find("NTSC-J")?"ntsc-j":regions.find("PAL")?"pal":"ntsc-u";}
    pal=region=="pal";refresh=pal?50:60;
    MegaDrive::option("TMSS","false");MegaDrive::option("Recompiler","false");
    if(!MegaDrive::load(root,pal?"[Sega] Mega Drive (PAL)":region=="ntsc-j"?"[Sega] Mega Drive (NTSC-J)":"[Sega] Mega Drive (NTSC-U)"))return false;
    auto slot=root->find<Node::Port>("Cartridge Slot");if(!slot)return false;slot->allocate();slot->connect();
    const auto pad=option("omniplay_md_pad","auto");
    auto device=pad=="6-button"||(pad=="auto"&&omni_md::six_button_safe(document["game/serial"].string()))?"Fighting Pad":"Control Pad";
    for(const char* name:{"Controller Port 1","Controller Port 2"})if(auto port=root->find<Node::Port>(name)){port->allocate(device);port->connect();}
    root->power();if(fault)return false;loaded=true;return true;
  }
} host;
void stop_on_fault() noexcept {host.fault=true;if(host.environment)host.environment(RETRO_ENVIRONMENT_SHUTDOWN,nullptr);}
}

extern "C" {
RETRO_API unsigned retro_api_version(){return RETRO_API_VERSION;}
RETRO_API void retro_set_environment(retro_environment_t value){host.environment=value;}
RETRO_API void retro_set_video_refresh(retro_video_refresh_t value){host.video_callback=value;}
RETRO_API void retro_set_audio_sample(retro_audio_sample_t value){host.sample_callback=value;}
RETRO_API void retro_set_audio_sample_batch(retro_audio_sample_batch_t value){host.batch_callback=value;}
RETRO_API void retro_set_input_poll(retro_input_poll_t value){host.poll_callback=value;}
RETRO_API void retro_set_input_state(retro_input_state_t value){host.input_callback=value;}
RETRO_API void retro_init(){
  platform=&host;
  if(host.environment){retro_pixel_format format=RETRO_PIXEL_FORMAT_XRGB8888;if(!host.environment(RETRO_ENVIRONMENT_SET_PIXEL_FORMAT,&format))host.fault=true;
    static const retro_variable variables[]={{"omniplay_md_region","Region; auto|ntsc-u|ntsc-j|pal"},{"omniplay_md_pad","Controller; auto|3-button|6-button"},{nullptr,nullptr}};host.environment(RETRO_ENVIRONMENT_SET_VARIABLES,const_cast<retro_variable*>(variables));}
}
RETRO_API void retro_deinit(){host.unload();platform=nullptr;}
RETRO_API void retro_get_system_info(retro_system_info* info){if(info)*info={"OmniPlay ares MD",kVersion,"md|gen|bin",false,true};}
RETRO_API void retro_get_system_av_info(retro_system_av_info* info){if(info){*info={{320,host.pal?240u:224u,2048,1024,4.0f/3.0f},{host.refresh,kSampleRate}};host.published_refresh=host.refresh;}}
RETRO_API void retro_set_controller_port_device(unsigned,unsigned){}
RETRO_API bool retro_load_game(const retro_game_info* game){try{if(host.create(game))return true;}catch(...){}host.unload();return false;}
RETRO_API bool retro_load_game_special(unsigned,const retro_game_info*,size_t){return false;}
RETRO_API void retro_unload_game(){host.unload();}
RETRO_API void retro_reset(){try{if(host.loaded){host.samples.clear();host.root->power(true);}}catch(...){stop_on_fault();}}
RETRO_API void retro_run(){
  try{if(!host.loaded||host.fault){stop_on_fault();return;}if(host.poll_callback)host.poll_callback();host.samples.clear();host.root->run();if(host.fault){stop_on_fault();return;}if(std::abs(host.refresh-host.published_refresh)>.0001&&host.environment){retro_system_av_info av{};retro_get_system_av_info(&av);host.environment(RETRO_ENVIRONMENT_SET_SYSTEM_AV_INFO,&av);}host.flush_audio();if(host.video_callback&&host.width&&host.height)host.video_callback(host.pixels.data(),host.width,host.height,size_t(host.width)*4);}
  catch(...){stop_on_fault();}
}
RETRO_API size_t retro_serialize_size(){try{if(host.loaded){auto state=host.root->serialize();return state.valid()&&state.size()<=kMaximumState?state.size():0;}}catch(...){}return 0;}
RETRO_API bool retro_serialize(void* data,size_t size){try{if(!host.loaded||!data||size>kMaximumState)return false;auto state=host.root->serialize();if(!state.valid()||state.size()!=size)return false;std::memcpy(data,state.data(),size);return true;}catch(...){return false;}}
RETRO_API bool retro_unserialize(const void* data,size_t size){try{if(!host.loaded||!data||size<533||size>kMaximumState||static_cast<const uint8_t*>(data)[4]!=1||size!=retro_serialize_size())return false;serializer state(static_cast<const u8*>(data),static_cast<u32>(size));bool restored=host.root->unserialize(state)&&state.valid();host.samples.clear();host.pixels.clear();host.width=host.height=0;return restored;}catch(...){return false;}}
RETRO_API void* retro_get_memory_data(unsigned type){if(!host.loaded)return nullptr;
#ifndef OMNIPLAY_FILE_SAVES
  if(type==RETRO_MEMORY_SAVE_RAM)return MegaDrive::omni_save_memory().data();
#endif
  if(type==RETRO_MEMORY_SYSTEM_RAM)return MegaDrive::cpu.ram.data();return nullptr;}
RETRO_API size_t retro_get_memory_size(unsigned type){if(!host.loaded)return 0;
#ifndef OMNIPLAY_FILE_SAVES
  if(type==RETRO_MEMORY_SAVE_RAM)return MegaDrive::omni_save_memory().size();
#endif
  if(type==RETRO_MEMORY_SYSTEM_RAM)return MegaDrive::cpu.ram.size()*sizeof(n16);return 0;}
#ifdef OMNIPLAY_FILE_SAVES
RETRO_API int om_web_flush_save_io(){try{return host.persist_files()?1:0;}catch(...){return 0;}}
RETRO_API void om_md_describe_save(std::uint32_t* output){
  if(!output)return;std::fill_n(output,6,0u);
  try{if(!host.describe_save(output))std::fill_n(output,6,0u);}catch(...){std::fill_n(output,6,0u);}
}
#endif
#ifdef OMNIPLAY_NATIVE_FILES
#ifdef OMNIPLAY_NATIVE_MEMORY_FILES
RETRO_API int omniplay_install_writable_files_v1(const OmniWritableFilesV1* files){
  try{if(host.loaded)return 0;return omni_md_files::install(files)?1:0;}catch(...){return 0;}
}
#endif
RETRO_API int omniplay_pause_guest_checked_v1(){
  // The worker calls this between exported core calls. All guest contexts are
  // cooperative on this owner thread, and Video::Threaded is disabled by the
  // recipe. There is no background guest I/O to join; publication runs only in
  // the checked barrier. co_active additionally enforces the owner/fiber check.
  if(!host.loaded)return 0;
  (void)co_active();return 1;
}
RETRO_API void omniplay_pause_guest(){(void)omniplay_pause_guest_checked_v1();}
RETRO_API int omniplay_flush_io_checked(){return om_web_flush_save_io();}
#endif
RETRO_API unsigned retro_get_region(){return host.pal?RETRO_REGION_PAL:RETRO_REGION_NTSC;}
RETRO_API void retro_cheat_reset(){}
RETRO_API void retro_cheat_set(unsigned,bool,const char*){}
}
