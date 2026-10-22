// Authored only: actual RTC/battery/aggregate retirement and actual save store.
#include "memory_files.hpp"
#include "n64-memory-io.h"
#include "n64-save-files.h"
#include "n64-core-owner.h"
#include "n64-rtc-storage.h"
extern "C" {
#include "backends/file_storage.h"
#include "backends/api/clock_backend.h"
#include "device/gb/mbc3_rtc.h"
#include <libretro.h>
#include <zlib.h>
retro_log_printf_t log_cb=nullptr;
int om_n64_case_bind_rtc(unsigned int,struct mbc3_rtc*,const char*);
struct mbc3_rtc* om_n64_case_clock(unsigned int);
struct file_storage* om_n64_case_ram(unsigned int);
int om_n64_case_retire_gb(unsigned int);
}
#include <algorithm>
#include <array>
#include <cstdlib>
#include <cstring>
#include <iostream>
#include <limits>
#include <stdexcept>
#include <string>
#include <thread>
#include <vector>
static void require(bool okay,const char* why){if(!okay)throw std::runtime_error(why);}
static OmniWritableFilesV1 underlying{};static unsigned fault=0;
static int rename_gate(void*,const char* from,const char* to,std::uint32_t replace){
    const std::string name(to);if((fault==1&&name.ends_with(".rtc"))||(fault==2&&name.ends_with(".ram")))return OMNI_FILES_NO_SPACE;
    return underlying.rename(underlying.context,from,to,replace);
}
static int read_gate(void*,std::uint64_t handle,std::uint64_t offset,void* output,std::uint64_t bytes,std::uint64_t* count){if(fault==3){*count=0;return OMNI_FILES_INTERNAL;}return underlying.read(underlying.context,handle,offset,output,bytes,count);}
static std::string path(unsigned port=1,char digest='0'){return "save/N64/tpak/"+std::string(32,digest)+"-p"+std::to_string(port)+".rtc";}
static char* owned(const char* text){auto* p=static_cast<char*>(std::malloc(std::strlen(text)+1));require(p!=nullptr,"owned fixture filename");std::strcpy(p,text);return p;}
struct Clock {
    time_t now=0;unsigned calls=0;mbc3_rtc rtc{};
    static time_t get(void* p){auto& c=*static_cast<Clock*>(p);++c.calls;return c.now;}
    Clock(time_t value=0):now(value){static const clock_backend_interface api{get};init_mbc3_rtc(&rtc,this,&api);}
};
struct Case {
    omni::MemoryFiles store;OmniWritableFilesV1 api=store.interface();
    Case(){require(om_n64_native_peripherals_quiet()&&om_n64_rtc_storage_quiescent(),"no prior peripheral debt");underlying=api;fault=0;auto proxy=api;proxy.rename=rename_gate;proxy.read=read_gate;require(om_n64_files_install(&proxy)==1,"install actual store with bounded fault gates");}
    ~Case(){om_n64_files_install(nullptr);}
    void seed(const std::string& name,const std::vector<std::uint8_t>& bytes){require(om_n64_save_write(name.c_str(),bytes.data(),bytes.size())==OM_N64_SAVE_OK,"seed independent wire bytes");}
    std::vector<std::uint8_t> bytes(const std::string& name){
        OmniFilesStatV1 stat{};require(api.stat(api.context,name.c_str(),&stat)==0&&stat.bytes<=1024,"bounded expected fixture");std::vector<std::uint8_t> result(static_cast<size_t>(stat.bytes));std::uint64_t handle=0,count=0;
        require(api.open(api.context,name.c_str(),OMNI_FILES_READ,&handle)==0&&api.read(api.context,handle,0,result.data(),result.size(),&count)==0&&count==result.size()&&api.close(api.context,handle)==0,"observe original store independently of read fault proxy");return result;
    }
    void absent(const std::string& name){OmniFilesStatV1 stat{};require(api.stat(api.context,name.c_str(),&stat)==OMNI_FILES_NOT_FOUND,"no default battery publication");}
    void clean(){fault=0;require(om_n64_native_retire_peripherals()&&om_n64_native_peripherals_quiet()&&om_n64_rtc_storage_quiescent()&&om_n64_files_idle(),"all actual producers and stream ownership retired");}
};
static void le32(std::uint8_t* p,std::uint32_t value){for(unsigned i=0;i<4;++i)p[i]=static_cast<std::uint8_t>(value>>(i*8));}
static std::vector<std::uint8_t> image(std::array<std::uint8_t,5> regs,time_t stamp){
    std::vector<std::uint8_t> p(32,0);std::memcpy(p.data(),"OMGBRTC1",8);std::copy(regs.begin(),regs.end(),p.begin()+8);
    const auto bits=static_cast<std::uint64_t>(stamp);for(unsigned i=0;i<8;++i)p[16+i]=static_cast<std::uint8_t>(bits>>(i*8));
    // Independent pinned zlib CRC implementation is the wire oracle.
    le32(p.data()+28,static_cast<std::uint32_t>(crc32(0,p.data(),28)));return p;
}
static void rtc_only_offline_time_halt_and_volatile_latch(){
    Case value;const auto name=path();Clock first(1000);
    require(om_n64_case_bind_rtc(0,&first.rtc,name.c_str())==1&&!om_n64_native_peripherals_quiet()&&om_n64_rtc_storage_flush_all()==-1,"staged RTC-only owner cannot be captured before power-on");value.absent(name);
    poweron_mbc3_rtc(&first.rtc);write_mbc3_rtc_regs(&first.rtc,0,10);first.now=1005;latch_mbc3_rtc_regs(&first.rtc,1);
    require(first.rtc.latched_valid&&first.rtc.latched_regs[0]==15&&om_n64_case_retire_gb(0)==1,"first live RTC-only retirement");
    require(value.bytes(name)==image({15,0,0,0,0},1005),"independent complete 32-byte battery image");
    Clock second(1010);require(om_n64_case_bind_rtc(0,&second.rtc,name.c_str())==1&&second.rtc.last_time==0&&second.rtc.regs[0]==0,"bind stages without mutating new RTC");
    poweron_mbc3_rtc(&second.rtc);require(second.rtc.regs[0]==20&&!second.rtc.latched_valid&&!second.rtc.latch,"offline interval advances battery but volatile latch resets");
    write_mbc3_rtc_regs(&second.rtc,4,0x40);require(om_n64_case_retire_gb(0)==1,"halted record retirement");
    Clock third(2000);require(om_n64_case_bind_rtc(0,&third.rtc,name.c_str())==1,"reopen halted RTC");poweron_mbc3_rtc(&third.rtc);
    require(third.rtc.regs[0]==20&&third.rtc.regs[4]==0x40&&third.rtc.last_time==2000,"halted offline interval excluded");
    third.now=2002;poweron_mbc3_rtc(&third.rtc);require(third.rtc.regs[0]==20,"repeated live power-on retains battery state");value.clean();
}
static void publication_debt_and_ram_retirement(){
    Case value;const auto name=path();Clock clock(100);require(om_n64_case_bind_rtc(0,&clock.rtc,name.c_str()),"bind live clock");poweron_mbc3_rtc(&clock.rtc);
    auto* ram=om_n64_case_ram(0);require(open_file_storage(ram,8,owned("save/N64/tpak/paired.ram"))==0,"actual paired RAM");ram->data[0]=11;
    require(om_n64_native_flush_peripherals()==1,"aggregate flush requires RAM and battery");const auto old_clock=value.bytes(name),old_ram=value.bytes("save/N64/tpak/paired.ram");
    write_mbc3_rtc_regs(&clock.rtc,0,33);ram->data[0]=77;auto* retained=ram->data;fault=1;
    require(!om_n64_case_retire_gb(0)&&ram->data==retained&&om_n64_case_clock(0)==&clock.rtc&&!om_n64_native_peripherals_quiet(),"RTC publication failure retains both live producer owners");
    require(value.bytes(name)==old_clock&&value.bytes("save/N64/tpak/paired.ram")==old_ram,"RTC failure cannot replace old image or release RAM");
    fault=2;require(!om_n64_case_retire_gb(0)&&ram->data==retained&&ram->data[0]==77&&om_n64_case_clock(0)==&clock.rtc,"RAM publication failure keeps clock registered after RTC flush");
    write_mbc3_rtc_regs(&clock.rtc,0,44);require(!om_n64_rtc_storage_detach(&clock.rtc),"a later clock mutation invalidates the old retirement receipt");
    require(om_n64_native_flush_peripherals()==0,"partial RAM/RTC publication cannot acknowledge aggregate capture");
    fault=0;require(om_n64_case_retire_gb(0)&&!ram->data&&!om_n64_case_clock(0),"successful retry retires both owners");
    require(value.bytes(name)==image({44,0,0,0,0},100)&&value.bytes("save/N64/tpak/paired.ram")[0]==77,"retry preserves retained live values");value.clean();
}
static void corrupt_inputs_staging_and_unstarted_retirement(){
    Case value;const auto name=path();const auto good=image({7,8,9,10,0x81},123);
    for(unsigned kind=0;kind<5;++kind){auto bad=good;switch(kind){case 0:bad[28]^=1;break;case 1:bad[0]='X';break;case 2:bad[13]=1;break;case 3:bad.pop_back();break;case 4:bad.push_back(0);break;}if(kind==1||kind==2)le32(bad.data()+28,static_cast<std::uint32_t>(crc32(0,bad.data(),28)));value.seed(name,bad);Clock clock(1000);
        require(!om_n64_case_bind_rtc(0,&clock.rtc,name.c_str())&&!om_n64_case_clock(0)&&clock.rtc.regs[0]==0&&clock.rtc.last_time==0&&value.bytes(name)==bad,"malformed battery cannot become cold defaults or a published owner");}
    value.seed(name,good);Clock rejected(1000);fault=3;
    require(!om_n64_case_bind_rtc(0,&rejected.rtc,name.c_str())&&value.bytes(name)==good,"read failure cannot fall back to cold start");fault=0;
    Clock staged;require(om_n64_case_bind_rtc(0,&staged.rtc,name.c_str())&&om_n64_case_retire_gb(0)&&staged.calls==0&&value.bytes(name)==good,"unstarted saved owner retires without clock calls or rewriting");
    Clock missing;const auto absent=path(2);require(om_n64_case_bind_rtc(1,&missing.rtc,absent.c_str())&&om_n64_case_retire_gb(1),"unstarted missing owner retired");value.absent(absent);value.clean();
    for(const time_t stamp:{std::numeric_limits<time_t>::min(),std::numeric_limits<time_t>::max()}){
        const auto signed_image=image({255,255,255,255,255},stamp);value.seed(name,signed_image);Clock exact(stamp);
        require(om_n64_case_bind_rtc(0,&exact.rtc,name.c_str()),"signed timestamp record bind");poweron_mbc3_rtc(&exact.rtc);
        require(exact.rtc.last_time==stamp&&exact.rtc.regs[0]==255&&exact.rtc.regs[4]==255,"signed extreme timestamp and halted raw fields decoded exactly");
        require(om_n64_case_retire_gb(0)&&value.bytes(name)==signed_image,"signed time record re-encodes independently");
    }
    auto bad=good;bad[28]^=1;value.seed(name,bad);Clock rejected_owner;
    require(om_n64_owner_begin()==1&&!om_n64_case_bind_rtc(0,&rejected_owner.rtc,name.c_str())&&!om_n64_owner_admitted(),"actual owner records malformed startup as admission failure");
    require(om_n64_native_flush_peripherals()==-1&&value.bytes(name)==bad,"failed admission cannot acknowledge default capture");value.clean();require(om_n64_owner_end(nullptr)==1,"unstarted rejected owner retirement");
}
static void bounded_ports_names_and_foreign_access(){
    Case value;std::array<Clock,5> clocks{};
    for(unsigned port=0;port<4;++port)require(om_n64_case_bind_rtc(port,&clocks[port].rtc,path(port+1).c_str()),"four distinct physical ports");
    require(!om_n64_rtc_storage_bind(&clocks[4].rtc,path(1,'1').c_str())&&!om_n64_rtc_storage_bind(&clocks[0].rtc,path(2,'1').c_str()),"port and RTC identities cannot be rebound while live");
    value.clean();
    for(const std::string invalid:{path(0),path(5),path(1,'A'),std::string("save/N64/tpak/x.rtc"),std::string("C:/outside.rtc")})require(!om_n64_rtc_storage_bind(&clocks[0].rtc,invalid.c_str()),"noncanonical/foreign battery names refused");
    require(om_n64_owner_begin()==1&&om_n64_case_bind_rtc(0,&clocks[0].rtc,path().c_str()),"current thread owns staged RTC");
    bool denied=false;std::thread foreign([&]{denied=!om_n64_rtc_storage_detach(&clocks[0].rtc)&&!om_n64_rtc_storage_prepare_retire(&clocks[0].rtc)&&om_n64_rtc_storage_flush_all()==-1;});foreign.join();
    require(denied&&om_n64_case_clock(0)==&clocks[0].rtc,"foreign thread cannot release or flush RTC");value.clean();require(om_n64_owner_end(nullptr)==1,"retire real owner manager");
}
int main(){try{rtc_only_offline_time_halt_and_volatile_latch();publication_debt_and_ram_retirement();corrupt_inputs_staging_and_unstarted_retirement();bounded_ports_names_and_foreign_access();std::cout<<"N64 RTC storage cases passed\n";return 0;}catch(const std::exception& error){std::cerr<<error.what()<<'\n';return 1;}}
