// Authored for item 16; compile the actual C card bridge, not a reimplementation.
#include "memory_files.hpp"
#include "beetle-memory-files.h"
#include <algorithm>
#include <atomic>
#include <iostream>
#include <stdexcept>
#include <string>
#include <thread>
#include <vector>

static constexpr std::size_t card_bytes=1u<<17;
static void require(bool okay,const char* message){if(!okay)throw std::runtime_error(message);}
static OmniWritableFilesV1 underlying;
static int partial_read(void*,std::uint64_t,std::uint64_t,void* data,std::uint64_t bytes,std::uint64_t* actual){if(bytes)static_cast<std::uint8_t*>(data)[0]=0;*actual=bytes?1:0;return OMNI_FILES_INTERNAL;}
static int partial_write(void*,std::uint64_t handle,std::uint64_t offset,const void* data,std::uint64_t bytes,std::uint64_t* actual){return underlying.write(underlying.context,handle,offset,data,std::min<std::uint64_t>(bytes,17),actual);}
static int fail_flush(void*,std::uint64_t){return OMNI_FILES_INTERNAL;}
static int fail_close(void*,std::uint64_t handle){underlying.close(underlying.context,handle);return OMNI_FILES_INTERNAL;}
static int fail_rename(void*,const char*,const char*,std::uint32_t){return OMNI_FILES_NO_SPACE;}

struct Case {
  omni::MemoryFiles store;OmniWritableFilesV1 api;
  Case():api(store.interface()){
    require(api.mkdir(api.context,"save/PSX",1)==0,"prepare PSX root");
    require(om_ps1_files_install(&api)==1&&om_ps1_files_installed(),"install real store");
  }
  ~Case(){om_ps1_files_install(nullptr);}
  std::vector<std::uint8_t> read(const char* name){std::vector<std::uint8_t> value(card_bytes);require(om_ps1_read_card(name,value.data(),static_cast<std::uint32_t>(value.size()))==1,"complete card read");return value;}
};

static void normalized_paths_missing_and_install_admission(){
  Case value;std::vector<std::uint8_t> input(card_bytes,0x91),output(card_bytes,0x42);
  require(om_ps1_read_card("save/PSX/game.1.mcr",output.data(),card_bytes)==0&&!om_ps1_card_read_failed(),"missing card is distinct from I/O failure");
  require(std::all_of(output.begin(),output.end(),[](auto byte){return byte==0x42;}),"absent card leaves caller bytes unchanged");
  require(om_ps1_write_card("save\\PSX\\game.1.mcr",input.data(),card_bytes)==1,"Windows separator normalization");
  require(value.read("save/PSX/game.1.mcr")==input,"canonical byte round trip");
  for(const char* invalid:{"save/PSX/../a.mcr","other/a.mcr","save/PSX/a:stream.mcr","save/PSX/a/b.mcr","save/PSX/a.bin","save/PSX/a..b.mcr"})require(om_ps1_write_card(invalid,input.data(),card_bytes)==0,"closed card path");
  auto bad=value.api;bad.version=2;require(om_ps1_files_install(&bad)==0&&om_ps1_files_installed(),"reject new protocol without losing current table");
  require(value.read("save/PSX/game.1.mcr")==input,"rejected install preserves table");
  require(om_ps1_files_install(nullptr)==1&&!om_ps1_files_installed(),"uninstall");
  require(om_ps1_write_card("save/PSX/game.1.mcr",input.data(),card_bytes)==0,"no uninstalled OS fallback");
}

static void each_publication_failure_preserves_old_card(){
  Case value;const std::vector<std::uint8_t> old(card_bytes,0x13),next(card_bytes,0x67);
  require(om_ps1_write_card("save/PSX/game.1.mcr",old.data(),card_bytes)==1,"old card");
  underlying=value.api;
  for(unsigned point=0;point<4;++point){auto fault=value.api;
    if(point==0)fault.write=partial_write;else if(point==1)fault.flush=fail_flush;else if(point==2)fault.close=fail_close;else fault.rename=fail_rename;
    require(om_ps1_files_install(&fault)==1,"fault table");
    require(om_ps1_write_card("save/PSX/game.1.mcr",next.data(),card_bytes)==0&&om_ps1_card_write_failed(),"publication failure reported");
    require(om_ps1_files_install(&value.api)==1,"restore actual table");
    require(value.read("save/PSX/game.1.mcr")==old,"old card retained");
    OmniFilesStatV1 temporary{};require(value.api.stat(value.api.context,"save/PSX/game.1.mcr.omtmp",&temporary)==OMNI_FILES_NOT_FOUND,"failed temporary retired");
  }
  require(om_ps1_write_card("save/PSX/game.1.mcr",next.data(),card_bytes)==1&&value.read("save/PSX/game.1.mcr")==next,"retry commits new complete card");
}

static void failed_read_never_replaces_guest_bytes_or_allows_cleanup_overwrite(){
  Case value;const std::vector<std::uint8_t> old(card_bytes,0x19),replacement(card_bytes,0x81);
  require(om_ps1_write_card("save/PSX/game.1.mcr",old.data(),card_bytes)==1,"seed card");
  auto fault=value.api;fault.read=partial_read;require(om_ps1_files_install(&fault)==1,"read-fault table");
  std::vector<std::uint8_t> output(card_bytes,0x55);
  require(om_ps1_read_card("save/PSX/game.1.mcr",output.data(),card_bytes)==-1&&om_ps1_card_read_failed(),"read error latched");
  require(std::all_of(output.begin(),output.end(),[](auto byte){return byte==0x55;}),"partial read cannot corrupt guest NV");
  require(om_ps1_write_card("save/PSX/game.1.mcr",replacement.data(),card_bytes)==0,"cleanup writes blocked after failed read");
  require(om_ps1_files_install(&value.api)==1,"quiescent next installation clears latch");
  require(value.read("save/PSX/game.1.mcr")==old,"source card survives read failure");
  std::uint64_t handle=0;require(value.api.open(value.api.context,"save/PSX/wrong.2.mcr",OMNI_FILES_WRITE|OMNI_FILES_CREATE,&handle)==0&&value.api.resize(value.api.context,handle,17)==0&&value.api.close(value.api.context,handle)==0,"malformed size fixture");
  require(om_ps1_read_card("save/PSX/wrong.2.mcr",output.data(),card_bytes)==-1&&om_ps1_card_read_failed(),"wrong card size is not missing data");
}

static void actual_handle_exhaustion_and_independent_card_writers(){
  Case value;const std::vector<std::uint8_t> old(card_bytes,0x34);
  require(om_ps1_write_card("save/PSX/game.1.mcr",old.data(),card_bytes)==1,"seed quota fixture");
  std::vector<std::uint64_t> handles;
  for(unsigned n=0;n<256;++n){std::uint64_t handle=0;require(value.api.open(value.api.context,"save/PSX/game.1.mcr",OMNI_FILES_READ,&handle)==0,"occupy actual handle quota");handles.push_back(handle);}
  require(om_ps1_write_card("save/PSX/game.1.mcr",old.data(),card_bytes)==0,"quota failure reaches card publisher");
  for(auto handle:handles)require(value.api.close(value.api.context,handle)==0,"release quota handles");
  std::atomic<bool> okay{true};std::vector<std::jthread> threads;
  for(unsigned slot=2;slot<6;++slot)threads.emplace_back([&,slot]{const auto path="save/PSX/game."+std::to_string(slot)+".mcr";const std::vector<std::uint8_t> bytes(card_bytes,static_cast<std::uint8_t>(slot));for(unsigned n=0;n<8;++n)if(om_ps1_write_card(path.c_str(),bytes.data(),card_bytes)!=1)okay=false;});
  for(auto& thread:threads)thread.join();require(okay,"independent concurrent cards");
  for(unsigned slot=2;slot<6;++slot){const auto path="save/PSX/game."+std::to_string(slot)+".mcr";require(value.read(path.c_str())==std::vector<std::uint8_t>(card_bytes,static_cast<std::uint8_t>(slot)),"per-card bytes");}
}

int main(){try{normalized_paths_missing_and_install_admission();each_publication_failure_preserves_old_card();failed_read_never_replaces_guest_bytes_or_allows_cleanup_overwrite();actual_handle_exhaustion_and_independent_card_writers();std::cout<<"PS1 memory bridge cases passed\n";return 0;}catch(const std::exception& error){std::cerr<<error.what()<<'\n';return 1;}}
