// Authored only; reserved for the single consolidated final verification.
#include "n64-snapshot-bounds.h"
extern "C" {
#include "device/r4300/cp0.h"
#include "device/r4300/interrupt.h"
void om_n64_case_clear_queue(struct cp0*);
}
#include <algorithm>
#include <array>
#include <cstring>
#include <cstdint>
#include <iostream>
#include <memory>
#include <stdexcept>
#include <vector>
static void require(bool good,const char* why){if(!good)throw std::runtime_error(why);}
static void le32(std::uint8_t* p,std::uint32_t value){for(unsigned i=0;i<4;++i)p[i]=static_cast<std::uint8_t>(value>>(8*i));}
static void host32(char* p,std::uint32_t value){std::memcpy(p,&value,4);}
static void queues(){
    auto cpu=std::make_unique<cp0>();om_n64_case_clear_queue(cpu.get());
    cpu->regs[CP0_COUNT_REG]=0x7ffffff0;add_interrupt_event_count(cpu.get(),VI_INT,0x80000010);
    add_interrupt_event_count(cpu.get(),SPECIAL_INT,0x80000000);
    std::array<char,1024> before{},after{},wire{};
    require(save_eventqueue_infos(cpu.get(),before.data(),before.size())==20,"real scheduler snapshot");
    require(std::all_of(before.begin()+20,before.end(),[](char c){return c==0;}),"serialized padding initialized");
    auto rejected=[&](const std::array<char,1024>& bad,size_t bytes){
        const auto next=cpu->next_interrupt;const auto cycles=cpu->cycle_count;
        require(!load_eventqueue_infos(cpu.get(),bad.data(),bytes),"bad queue refused");
        require(cpu->next_interrupt==next&&cpu->cycle_count==cycles&&save_eventqueue_infos(cpu.get(),after.data(),after.size())==20&&before==after,"refused load leaves real old queue intact");
    };
    wire.fill(0);rejected(wire,wire.size());
    for(size_t n=0;n<20;++n)rejected(before,n);
    wire=before;host32(wire.data(),3);rejected(wire,wire.size());
    wire=before;host32(wire.data(),0x8000);rejected(wire,wire.size());
    wire.fill(0);for(size_t i=0;i<16;++i){host32(wire.data()+i*8,VI_INT);host32(wire.data()+i*8+4,static_cast<unsigned>(i));}host32(wire.data()+128,UINT32_MAX);rejected(wire,wire.size());
    wire.fill(0);host32(wire.data(),SPECIAL_INT);host32(wire.data()+8,SPECIAL_INT);host32(wire.data()+16,UINT32_MAX);rejected(wire,wire.size());
    // Preserve valid duplicate events and reserve one real pool slot for SPECIAL.
    wire.fill(0);for(size_t i=0;i<15;++i){host32(wire.data()+i*8,VI_INT);host32(wire.data()+i*8+4,static_cast<unsigned>(i+17));}host32(wire.data()+120,UINT32_MAX);
    require(load_eventqueue_infos(cpu.get(),wire.data(),124)==1&&cpu->q.pool.index==16,"15 events plus normalized SPECIAL fit real fixed pool");
    after.fill(0x55);const auto untouched=after;
    require(!save_eventqueue_infos(cpu.get(),after.data(),131)&&after==untouched,"short serialization changes no caller bytes");
    auto* first=cpu->q.first;auto* old=first->next;first->next=first;
    require(!save_eventqueue_infos(cpu.get(),after.data(),after.size())&&after==untouched,"cyclic queue refused without publication");
    auto foreign=std::make_unique<cp0>();first->next=&foreign->q.pool.nodes[0];
    require(!save_eventqueue_infos(cpu.get(),after.data(),after.size())&&after==untouched,"foreign link refused before dereference");first->next=old;
}
static void images(){
    std::vector<std::uint8_t> image(OM_N64_SNAPSHOT_BYTES,0);
    std::memcpy(image.data(),"M64+SAVE",8);image[9]=1;
    image[OM_N64_SNAPSHOT_PIF_AT]=1;
    le32(image.data()+OM_N64_SNAPSHOT_QUEUE_AT,UINT32_MAX);
    for(unsigned minor=0;minor<=10;++minor){
        image[10]=static_cast<std::uint8_t>(minor);
        // Exercise every present/absent Transfer Pak combination in both layouts.
        for(unsigned mask=0;mask<16;++mask){
            std::fill(image.begin()+OM_N64_SNAPSHOT_EXTRA_AT,image.end(),0);
            size_t at=OM_N64_SNAPSHOT_EXTRA_AT+(minor==2?46:48);
            for(unsigned port=0;port<4;++port){const bool present=(mask&(1u<<port))!=0;image[at+16]=present?0x41:0;at+=44+(present?92:0);}
            require(om_n64_snapshot_pointer_preflight(image.data(),image.size())==1,"known fixed wire layout admitted");
            if(minor>=2){
                image[at]=127;require(!om_n64_snapshot_pointer_preflight(image.data(),image.size()),"positive PIF offset outside RAM refused before state load");
                image[at]=63;require(!om_n64_snapshot_pointer_preflight(image.data(),image.size()),"one-byte PIF header refused");
                image[at]=128;require(om_n64_snapshot_pointer_preflight(image.data(),image.size()),"legacy negative offset disables channel");
                image[at]=0;image[OM_N64_SNAPSHOT_PIF_AT]=63;image[OM_N64_SNAPSHOT_PIF_AT+1]=63;
                require(!om_n64_snapshot_pointer_preflight(image.data(),image.size()),"PIF payload overflow refused");image[OM_N64_SNAPSHOT_PIF_AT]=1;image[OM_N64_SNAPSHOT_PIF_AT+1]=0;
                image[OM_N64_SNAPSHOT_PIF_AT]=0;require(!om_n64_snapshot_pointer_preflight(image.data(),image.size()),"active PIF command must contain its opcode byte");image[OM_N64_SNAPSHOT_PIF_AT]=1;
            }
        }
    }
    image[10]=11;require(!om_n64_snapshot_pointer_preflight(image.data(),image.size()),"future format requires explicit adaptation");
    image[10]=10;image[11]=1;require(!om_n64_snapshot_pointer_preflight(image.data(),image.size()),"unknown patch layout refused");image[11]=0;
    require(!om_n64_snapshot_pointer_preflight(image.data(),image.size()-1)&&!om_n64_snapshot_pointer_preflight(nullptr,image.size()),"fixed native image extent enforced");
    std::array<std::uint8_t,64> pif{};pif[0]=31;pif[1]=31;
    require(om_n64_pif_command_extent(pif.data(),64)==64&&!om_n64_pif_command_extent(pif.data(),63)&&!om_n64_pif_command_extent(pif.data(),1),"exact-fit and short command extents");
    std::array<std::uint8_t,7> id{0,0x41,0x42,0x43,0x44,0,0};std::uint32_t value=7,expected=0;std::memcpy(&expected,id.data()+1,4);
    require(om_n64_snapshot_disk_word(id.data(),id.size(),1,&value)&&value==expected,"unaligned disk word retains wire semantics");
    value=7;require(!om_n64_snapshot_disk_word(id.data(),id.size(),SIZE_MAX,&value)&&value==7&&!om_n64_snapshot_disk_word(id.data(),id.size(),4,&value)&&value==7,"overflow and short disk IDs leave output unchanged");
}
int main(){try{queues();images();std::cout<<"N64 snapshot bounds cases passed\n";return 0;}catch(const std::exception& error){std::cerr<<error.what()<<'\n';return 1;}}
