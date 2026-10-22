// Exact selected frontend helper with explicit CoreDoCommand observer, not CPU.
#include "n64-core-owner.h"
#include "api/m64p_types.h"
#include <cstdlib>
#include <cstring>
#include <iostream>
#include <stdexcept>
#include <thread>
static uint8_t* game_data=nullptr;
static uint32_t game_size=0;
static m64p_rom_header ROM_HEADER;
static unsigned opened=0,queried=0,closed=0;
static bool fail_open=false,fail_query=false,fail_close=false;
static m64p_error CoreDoCommand(m64p_command command,int size,void* data){
    if(command==M64CMD_ROM_OPEN){++opened;return fail_open?M64ERR_INPUT_INVALID:M64ERR_SUCCESS;}
    if(command==M64CMD_ROM_GET_HEADER){++queried;std::memset(data,0x39,static_cast<size_t>(size));return fail_query?M64ERR_INTERNAL:M64ERR_SUCCESS;}
    if(command==M64CMD_ROM_CLOSE){++closed;return fail_close?M64ERR_INTERNAL:M64ERR_SUCCESS;}
    return M64ERR_INTERNAL;
}
#include "n64-content-load.c.inc"
static void require(bool good,const char* why){if(!good)throw std::runtime_error(why);}
static void input(){game_size=0x1000;game_data=static_cast<uint8_t*>(std::calloc(1,game_size));require(game_data,"fixture allocation");game_data[0]=0x80;game_data[1]=0x37;game_data[2]=0x12;game_data[3]=0x40;}
static void reset(){opened=queried=closed=0;fail_open=fail_query=fail_close=false;std::memset(&ROM_HEADER,0xa5,sizeof(ROM_HEADER));}
int main(){try{
    reset();input();require(emu_step_load_data()&&om_n64_content_rom_open&&!game_data&&!game_size&&opened==1&&queried==1&&!closed,"successful open/query transfers receipt and releases input");
    require(reinterpret_cast<unsigned char*>(&ROM_HEADER)[0]==0x39,"successful staged header published");
    require(!emu_step_load_data()&&opened==1,"second open cannot overwrite retained receipt");require(om_n64_content_close_unstarted()&&!om_n64_content_rom_open&&closed==1,"confirmed close consumes receipt");
    reset();input();fail_open=true;require(!emu_step_load_data()&&!game_data&&!game_size&&!om_n64_content_rom_open&&!queried&&!closed,"failed open releases input without closing unowned ROM");
    reset();input();fail_query=true;auto prior=ROM_HEADER;
    require(!emu_step_load_data()&&!game_data&&!game_size&&!om_n64_content_rom_open&&closed==1&&!std::memcmp(&prior,&ROM_HEADER,sizeof(prior)),"partial failed header is not published and opened ROM closes");
    reset();require(om_n64_owner_begin()==1,"activate owner");input();fail_query=fail_close=true;
    require(!emu_step_load_data()&&om_n64_content_rom_open&&!game_data&&!om_n64_owner_admitted(),"refused close retains exact ROM receipt and poisons admission");
    require(!emu_step_load_data()&&opened==1,"poisoned pending receipt cannot be overwritten");fail_close=false;require(om_n64_content_close_unstarted()&&!om_n64_content_rom_open&&closed==2,"teardown retry consumes receipt only on successful close");
    require(om_n64_owner_end(nullptr)==1,"retire failed unstarted owner");
    reset();input();require(om_n64_owner_begin()==1,"new owner");bool result=true;std::thread foreign([&]{result=emu_step_load_data();});foreign.join();
    require(!result&&!opened&&game_data&&!om_n64_content_rom_open,"foreign helper cannot open or release owner input");std::free(game_data);game_data=nullptr;game_size=0;require(om_n64_owner_end(nullptr)==1,"retire owner");
    std::cout<<"Content load source cases passed\n";return 0;
}catch(const std::exception& e){std::cerr<<e.what()<<'\n';return 1;}}
