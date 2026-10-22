// Authored only. Actual prepared RTC implementation; synthetic clock, no CPU.
#include "n64-core-owner.h"
#include "n64-snapshot-bounds.h"
extern "C" {
#include "device/gb/mbc3_rtc.h"
#include "backends/api/clock_backend.h"
}
#include <algorithm>
#include <array>
#include <cstdint>
#include <cstring>
#include <iostream>
#include <iterator>
#include <limits>
#include <stdexcept>
#include <thread>
#include <vector>
static void require(bool okay,const char* why){if(!okay)throw std::runtime_error(why);}
struct Clock {time_t current=0;unsigned calls=0;static time_t get(void* p){auto& c=*static_cast<Clock*>(p);++c.calls;return c.current;}};
struct Case {
    Clock clock;mbc3_rtc rtc{};
    explicit Case(time_t value=0){clock.current=value;static const clock_backend_interface api{Clock::get};init_mbc3_rtc(&rtc,&clock,&api);poweron_mbc3_rtc(&rtc);}
    void advance(time_t value){clock.current=value;(void)read_mbc3_rtc_regs(&rtc,0);}
};
static void basic_time_halt_and_carry(){
    Case value(2000000000);require(value.rtc.last_time==2000000000&&read_mbc3_rtc_regs(&value.rtc,0)==0,"power-on anchors now without an epoch-sized jump");
    const time_t start=value.clock.current;
    for(const auto& expectation:std::array<std::array<unsigned,5>,7>{{{59,59,0,0,0},{60,0,1,0,0},{3599,59,59,0,0},{3600,0,0,1,0},{86399,59,59,23,0},{86400,0,0,0,1},{172800,0,0,0,2}}}){
        value.advance(start+expectation[0]);require(value.rtc.regs[0]==expectation[1]&&value.rtc.regs[1]==expectation[2]&&value.rtc.regs[2]==expectation[3]&&value.rtc.regs[3]==expectation[4],"independent minute/hour/day boundary goldens");
    }
    Case days;days.advance(255*86400);require(days.rtc.regs[3]==255&&days.rtc.regs[4]==0,"day 255");
    days.advance(256*86400);require(days.rtc.regs[3]==0&&days.rtc.regs[4]==1,"day high bit is stored in bit zero");
    days.advance(512*86400);require(days.rtc.regs[3]==0&&days.rtc.regs[4]==0x80,"day 512 wraps and raises carry");
    days.advance(513*86400);require(days.rtc.regs[3]==1&&days.rtc.regs[4]==0x80,"carry stays set");
    write_mbc3_rtc_regs(&days.rtc,4,0);require(days.rtc.regs[4]==0,"program can explicitly clear carry");
    Case halted(1000);halted.clock.current=1005;write_mbc3_rtc_regs(&halted.rtc,4,0x40);
    require(halted.rtc.regs[0]==5,"running time charged before halt write");halted.advance(1000005);
    require(halted.rtc.regs[0]==5&&halted.rtc.last_time==1000005,"halted interval only reanchors time");
    write_mbc3_rtc_regs(&halted.rtc,0,59);halted.clock.current=2000005;write_mbc3_rtc_regs(&halted.rtc,4,0);
    require(halted.rtc.regs[0]==59&&!halted.rtc.regs[1],"resume excludes all stopped time");
    halted.advance(2000006);require(!halted.rtc.regs[0]&&halted.rtc.regs[1]==1,"first resumed second carries normally");
}
static void persistent_latch_and_command_edges(){
    Case value;value.advance(7);latch_mbc3_rtc_regs(&value.rtc,0);latch_mbc3_rtc_regs(&value.rtc,1);
    require(value.rtc.latched_valid==1&&read_mbc3_rtc_regs(&value.rtc,0)==7,"zero-one captures timer");
    value.clock.current=8;require(read_mbc3_rtc_regs(&value.rtc,0)==7&&value.rtc.regs[0]==8,"timer ticks behind frozen readable latch");
    latch_mbc3_rtc_regs(&value.rtc,0);require(read_mbc3_rtc_regs(&value.rtc,0)==7,"zero command does not discard a captured latch");
    value.clock.current=9;write_mbc3_rtc_regs(&value.rtc,0,23);require(read_mbc3_rtc_regs(&value.rtc,0)==7,"timer write cannot overwrite captured bytes");
    latch_mbc3_rtc_regs(&value.rtc,1);require(read_mbc3_rtc_regs(&value.rtc,0)==23,"next exact zero-one edge refreshes latch");
    value.clock.current=10;latch_mbc3_rtc_regs(&value.rtc,2);latch_mbc3_rtc_regs(&value.rtc,1);
    require(read_mbc3_rtc_regs(&value.rtc,0)==23&&value.rtc.regs[0]==24,"two-one is not a zero-one edge");
    latch_mbc3_rtc_regs(&value.rtc,0);latch_mbc3_rtc_regs(&value.rtc,1);require(read_mbc3_rtc_regs(&value.rtc,0)==24,"full command byte and separate validity retained");
}
static void extreme_time_and_raw_bytes(){
    Case value;value.rtc.last_time=std::numeric_limits<time_t>::min();value.advance(std::numeric_limits<time_t>::max());
    constexpr auto elapsed=std::numeric_limits<std::uint64_t>::max();
    require(value.rtc.regs[0]==elapsed%60&&value.rtc.regs[1]==(elapsed/60)%60&&value.rtc.regs[2]==(elapsed/3600)%24&&value.rtc.regs[3]==((elapsed/86400)&255)&&value.rtc.regs[4]==(0x80|((elapsed/86400>>8)&1)),"full signed-time distance fits bounded unsigned arithmetic");
    const auto seconds=value.rtc.regs[0];value.advance(std::numeric_limits<time_t>::min());
    require(value.rtc.regs[0]==seconds&&value.rtc.last_time==std::numeric_limits<time_t>::min(),"backward clock reanchors without reverse ticking");
    Case raw;raw.rtc.regs[0]=255;raw.advance(1);require(raw.rtc.regs[0]==0&&!raw.rtc.regs[1],"raw byte wraps before equality carry");
    raw.advance(60);require(raw.rtc.regs[0]==59&&!raw.rtc.regs[1],"raw prefix preserves carry timing");raw.advance(61);require(!raw.rtc.regs[0]&&raw.rtc.regs[1]==1,"raw prefix eventually reaches normal cadence");
    // Independent one-tick reference, deliberately unlike the constant-time
    // aggregate algorithm; finite test time never depends on hostile timestamps.
    for(unsigned initial:{0u,59u,60u,63u,255u})for(unsigned ticks:{0u,1u,59u,60u,256u,1024u}){
        Case actual;std::array<std::uint8_t,5> expected{static_cast<std::uint8_t>(initial),255,255,255,0x31};
        std::copy(expected.begin(),expected.end(),actual.rtc.regs);
        for(unsigned tick=0;tick<ticks;++tick){
            bool carry=true;
            for(unsigned r=0;r<3&&carry;++r){const unsigned next=(unsigned(expected[r])+1)&255;const unsigned radix=r==2?24:60;carry=next==radix;expected[r]=static_cast<std::uint8_t>(carry?0:next);}
            if(carry){unsigned day=((unsigned(expected[4])&1)*256+expected[3])+1;expected[3]=static_cast<std::uint8_t>(day);expected[4]=static_cast<std::uint8_t>((expected[4]&0xfe)|((day>>8)&1)|(day>511?0x80:0));}
        }
        actual.advance(ticks);require(std::equal(expected.begin(),expected.end(),actual.rtc.regs),"aggregate progression matches independent finite tick trace");
    }
}
static void denied_access_and_register_bounds(){
    Case value;const auto previous=value.rtc;const auto calls=value.clock.calls;
    require(read_mbc3_rtc_regs(&value.rtc,5)==0xff&&read_mbc3_rtc_regs(nullptr,0)==0xff,"invalid register and null read bounded");
    write_mbc3_rtc_regs(&value.rtc,UINT32_MAX,255);latch_mbc3_rtc_regs(nullptr,1);
    require(std::equal(std::begin(previous.regs),std::end(previous.regs),value.rtc.regs)&&value.clock.calls==calls,"invalid access has no clock or state side effect");
    require(om_n64_owner_begin()==1,"activate real owner manager");
    bool denied=false;std::thread foreign([&]{denied=read_mbc3_rtc_regs(&value.rtc,0)==0xff;write_mbc3_rtc_regs(&value.rtc,0,33);latch_mbc3_rtc_regs(&value.rtc,1);poweron_mbc3_rtc(&value.rtc);});foreign.join();
    require(denied&&value.rtc.regs[0]==previous.regs[0]&&!value.rtc.latched_valid&&value.clock.calls==calls,"foreign thread cannot use RTC or its clock callback");require(om_n64_owner_end(nullptr)==1,"retire owner manager");
}
static void le32(std::uint8_t* p,std::uint32_t value){for(unsigned i=0;i<4;++i)p[i]=static_cast<std::uint8_t>(value>>(8*i));}
static void wire_flags(){
    for(unsigned mask=0;mask<16;++mask){
        std::vector<std::uint8_t> image(OM_N64_SNAPSHOT_BYTES,0);std::memcpy(image.data(),"M64+SAVE",8);image[9]=1;image[10]=10;le32(image.data()+OM_N64_SNAPSHOT_QUEUE_AT,UINT32_MAX);
        image[OM_N64_SNAPSHOT_PIF_AT]=1;
        size_t at=OM_N64_SNAPSHOT_EXTRA_AT+48;
        for(unsigned port=0;port<4;++port){if(mask&(1u<<port)){image[at+16]='A';image[at+35]=0x10;le32(image.data()+at+60,255);at+=136;}else at+=44;}
        // Independent protocol extent: PIF/SI/DP/VI 11 + RDRAM 280 + DD 436
        // + stop 4 + SP 32 + flash 144 + CP0/CP2 16 = 923 bytes.
        const size_t flags=at+923;
        for(unsigned port=0;port<4;++port)le32(image.data()+flags+port*4,(mask>>port)&1);
        require(om_n64_snapshot_pointer_preflight(image.data(),image.size()),"v1.10 full-byte latch commands and per-port validity flags admitted");
        std::array<std::uint8_t,4> staged{};require(om_n64_snapshot_rtc_flags(image.data(),image.size(),staged.data()),"all modern flags staged before device restore");
        for(unsigned port=0;port<4;++port)require(staged[port]==((mask>>port)&1),"port flag order matches independent wire");
        auto invalid=image;le32(invalid.data()+flags+12,2);staged.fill(0x77);const auto untouched=staged;
        require(!om_n64_snapshot_pointer_preflight(invalid.data(),invalid.size())&&!om_n64_snapshot_rtc_flags(invalid.data(),invalid.size(),staged.data())&&staged==untouched,"late invalid flag refused without partially publishing early ports");
        image[10]=9;
        if(mask)require(!om_n64_snapshot_pointer_preflight(image.data(),image.size()),"full-byte commands cannot masquerade as legacy v1.9");
        at=OM_N64_SNAPSHOT_EXTRA_AT+48;
        for(unsigned port=0;port<4;++port){if(mask&(1u<<port)){le32(image.data()+at+60,1);at+=136;}else at+=44;}
        le32(image.data()+flags+12,UINT32_MAX);require(om_n64_snapshot_pointer_preflight(image.data(),image.size()),"legacy valid latches retain import with old reserved bytes ignored");
    }
}
int main(){try{basic_time_halt_and_carry();persistent_latch_and_command_edges();extreme_time_and_raw_bytes();denied_access_and_register_bounds();wire_flags();std::cout<<"N64 MBC3 RTC cases passed\n";return 0;}catch(const std::exception& error){std::cerr<<error.what()<<'\n';return 1;}}
