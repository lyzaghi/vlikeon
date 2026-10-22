// Authored only. Actual prepared TLB map/unmap and independent wire fixtures.
#include "n64-snapshot-bounds.h"
extern "C" {
#include "device/r4300/tlb.h"
}
#include <algorithm>
#include <array>
#include <cstdint>
#include <cstring>
#include <iostream>
#include <iterator>
#include <memory>
#include <stdexcept>
#include <vector>
static void require(bool good,const char* why){if(!good)throw std::runtime_error(why);}
static void le32(std::uint8_t* p,std::uint32_t value){for(unsigned i=0;i<4;++i)p[i]=static_cast<std::uint8_t>(value>>(8*i));}
static std::array<std::uint8_t,52> wire(unsigned mask,unsigned vpn,unsigned even,unsigned odd){
    std::array<std::uint8_t,52> value{};
    value[0]=static_cast<std::uint8_t>(mask);value[1]=static_cast<std::uint8_t>(mask>>8);
    value[8]=1;value[9]=0xff;value[16]=7;value[17]=1;value[18]=1;value[24]=7;value[25]=1;value[26]=1;value[27]=3;
    le32(value.data()+4,vpn);le32(value.data()+12,even);le32(value.data()+20,odd);
    // Independent 64-bit arithmetic, then truncate to the actual 32-bit wire.
    const std::uint64_t begin=static_cast<std::uint64_t>(vpn)*8192;
    const std::uint64_t length=(static_cast<std::uint64_t>(mask)+1)*4096;
    le32(value.data()+28,static_cast<std::uint32_t>(begin));le32(value.data()+32,static_cast<std::uint32_t>(begin+length-1));le32(value.data()+36,static_cast<std::uint32_t>(static_cast<std::uint64_t>(even)*4096));
    le32(value.data()+40,static_cast<std::uint32_t>(begin+length));le32(value.data()+44,static_cast<std::uint32_t>(begin+2*length-1));le32(value.data()+48,static_cast<std::uint32_t>(static_cast<std::uint64_t>(odd)*4096));return value;
}
static void wire_fields(){
    std::array<std::uint8_t,52> zero{};
    require(om_n64_snapshot_tlb_entry_valid(zero.data()),"never-written power-on entry admitted");
    zero[2]=0xff;zero[3]=0x37;zero[10]=1;zero[11]=2;zero[19]=0x88;
    require(om_n64_snapshot_tlb_entry_valid(zero.data()),"padding is not guest state");
    for(unsigned mask=0;mask<4096;++mask)for(unsigned vpn:{0u,0x7ffffu,0x60000u}){
        const auto value=wire(mask,vpn,0xffffff,0x1ffff);
        require(om_n64_snapshot_tlb_entry_valid(value.data()),"all raw guest mask patterns and modular derived ranges admitted");
    }
    const auto good=wire(3,0x60000,0x1234,0x1ffff);
    for(unsigned at:{8u,17u,18u,25u,26u}){auto bad=good;bad[at]=2;require(!om_n64_snapshot_tlb_entry_valid(bad.data()),"nonboolean TLB flag refused");}
    for(unsigned at:{16u,24u,27u}){auto bad=good;bad[at]=8;require(!om_n64_snapshot_tlb_entry_valid(bad.data()),"oversized cache/region selector refused");}
    {auto bad=good;bad[0]=0;bad[1]=0x10;require(!om_n64_snapshot_tlb_entry_valid(bad.data()),"negative/oversized mask refused");}
    for(unsigned at:{4u,12u,20u}){auto bad=good;le32(bad.data()+at,at==4?0x80000:0x1000000);require(!om_n64_snapshot_tlb_entry_valid(bad.data()),"hardware address field width enforced");}
    for(unsigned at:{28u,32u,36u,40u,44u,48u}){auto bad=good;bad[at]^=0x80;require(!om_n64_snapshot_tlb_entry_valid(bad.data()),"forged derived field refused");}
    std::vector<std::uint8_t> image(OM_N64_SNAPSHOT_BYTES,0);
    std::memcpy(image.data(),"M64+SAVE",8);image[9]=1;image[10]=9;le32(image.data()+OM_N64_SNAPSHOT_QUEUE_AT,UINT32_MAX);
    image[OM_N64_SNAPSHOT_PIF_AT]=1;
    std::memcpy(image.data()+OM_N64_SNAPSHOT_TLB_AT+31*52,good.data(),good.size());
    // The tables may preserve different overlapping read/write history.
    le32(image.data()+OM_N64_SNAPSHOT_LUT_AT,0x80000fff);
    le32(image.data()+OM_N64_SNAPSHOT_LUT_AT+4*(2*0x100000-1),0xa0ffefff);
    require(om_n64_snapshot_pointer_preflight(image.data(),image.size()),"extreme legitimate LUT mappings and final TLB slot admitted");
    const auto admitted=image;
    for(auto bad:{0x80000000u,0x7fffffffu,0xa0ffffffu,0xffffffffu})for(size_t slot:{size_t{0},size_t{0xfffff},size_t{0x100000},size_t{0x1fffff}}){
        image=admitted;le32(image.data()+OM_N64_SNAPSHOT_LUT_AT+4*slot,bad);
        const auto before=image;require(!om_n64_snapshot_pointer_preflight(image.data(),image.size())&&image==before,"forged lookup refused without modifying caller image");
    }
    image=admitted;image[OM_N64_SNAPSHOT_TLB_AT+31*52+18]=2;
    require(!om_n64_snapshot_pointer_preflight(image.data(),image.size()),"late malformed TLB entry rejected during preflight");
}
static tlb_entry entry(unsigned mask,unsigned vpn,unsigned even,unsigned odd){
    tlb_entry value{};value.mask=static_cast<short>(mask);value.vpn2=vpn;value.pfn_even=even;value.pfn_odd=odd;
    value.v_even=1;value.v_odd=1;value.d_even=1;value.d_odd=1;
    const std::uint64_t start=static_cast<std::uint64_t>(vpn)*8192,length=(static_cast<std::uint64_t>(mask)+1)*4096;
    value.start_even=static_cast<std::uint32_t>(start);value.end_even=static_cast<std::uint32_t>(start+length-1);value.phys_even=static_cast<std::uint32_t>(static_cast<std::uint64_t>(even)*4096);
    value.start_odd=static_cast<std::uint32_t>(start+length);value.end_odd=static_cast<std::uint32_t>(start+2*length-1);value.phys_odd=static_cast<std::uint32_t>(static_cast<std::uint64_t>(odd)*4096);return value;
}
static void actual_page_walk(){
    auto state=std::make_unique<tlb>();poweron_tlb(state.get());
    state->entries[0]=entry(0,0x7ffff,0,0x1ffff);
    state->LUT_r[0]=0x12345678;state->LUT_w[0]=0x23456789;state->LUT_r[0xffffd]=0x3456789a;
    tlb_map(state.get(),0);
    require(state->LUT_r[0xffffe]==0x80000fff&&state->LUT_w[0xffffe]==0x80000fff&&state->LUT_r[0xfffff]==0x9fffffff&&state->LUT_w[0xfffff]==0x9fffffff,"actual final two virtual pages map finitely with independent expected values");
    tlb_unmap(state.get(),0);
    require(!state->LUT_r[0xffffe]&&!state->LUT_w[0xffffe]&&!state->LUT_r[0xfffff]&&!state->LUT_w[0xfffff]&&state->LUT_r[0]==0x12345678&&state->LUT_w[0]==0x23456789&&state->LUT_r[0xffffd]==0x3456789a,"actual final page unmap terminates without wrap or adjacent writes");
    poweron_tlb(state.get());state->entries[0]=entry(0xfff,0x60000,0x1ffff,0);
    tlb_map(state.get(),0);
    require(state->LUT_r[0xc0000]==0x9fffffff&&state->LUT_r[0xc0fff]==0xa0ffefff&&state->LUT_w[0xc0fff]==0xa0ffefff&&state->LUT_r[0xc1000]==0x80000fff&&state->LUT_r[0xc1fff]==0x80ffffff&&!state->LUT_r[0xc2000],"maximum hardware span and physical addition stay bounded");
    tlb_unmap(state.get(),0);require(std::all_of(std::begin(state->LUT_r),std::end(state->LUT_r),[](auto value){return value==0;}),"maximum mapped read range retired");
    poweron_tlb(state.get());state->entries[0]=entry(3,0x7ffff,0,0);
    tlb_map(state.get(),0);require(!state->LUT_r[0xffffe]&&state->LUT_r[2]==0x80000fff&&state->LUT_r[5]==0x80003fff,"wrapped even half stays ignored while bounded odd half retains pinned behavior");
    poweron_tlb(state.get());state->entries[0]=entry(0,0x40000,0,0);tlb_map(state.get(),0);
    require(!state->LUT_r[0x80000]&&!state->LUT_r[0x80001],"KSEG exclusion retained");
    state->entries[0]=entry(0,0x60000,0x20000,0x20000);tlb_map(state.get(),0);
    require(!state->LUT_r[0xc0000]&&!state->LUT_w[0xc0000],"physical exclusion retained");
    state->entries[0]=entry(0,0x60000,1,2);tlb_map(state.get(),0);
    state->entries[1]=entry(0,0x60000,3,4);state->entries[1].d_even=0;state->entries[1].d_odd=0;tlb_map(state.get(),1);
    require(state->LUT_r[0xc0000]==0x80003fff&&state->LUT_w[0xc0000]==0x80001fff,"readonly overlap preserves prior write mapping");
    tlb_unmap(state.get(),1);require(!state->LUT_r[0xc0000]&&state->LUT_w[0xc0000]==0x80001fff,"readonly retirement preserves independent write history");
    const auto first_read=state->LUT_r[0xc0000],first_write=state->LUT_w[0xc0000];
    tlb_map(state.get(),32);tlb_unmap(state.get(),SIZE_MAX);tlb_map(nullptr,0);
    require(state->LUT_r[0xc0000]==first_read&&state->LUT_w[0xc0000]==first_write,"nonexistent entry refused before dereference");
    std::uint32_t first=7,limit=9;
    require(!om_n64_tlb_page_span(0,UINT32_MAX,&first,&limit)&&first==7&&limit==9,"nonhardware span leaves range outputs unchanged");
    require(om_n64_tlb_page_span(0xfffff000,UINT32_MAX,&first,&limit)&&first==0xfffff&&limit==0x100000,"last-page exclusive index limit fits without wrapping");
}
int main(){try{wire_fields();actual_page_walk();std::cout<<"N64 TLB snapshot cases passed\n";return 0;}catch(const std::exception& error){std::cerr<<error.what()<<'\n';return 1;}}
