// Authored only: actual PI/FB code plus explicit observing callback substitutes.
#include "n64-core-owner.h"
#include "n64-device-memory.h"
#include "n64-framebuffer-cache.hpp"
extern "C" {
#include "pi_framebuffer_fixture.h"
}
#include <algorithm>
#include <array>
#include <cstdint>
#include <iostream>
#include <stdexcept>
#include <thread>
#include <vector>
static void require(bool yes,const char* why){if(!yes)throw std::runtime_error(why);}
struct Fixture {
    std::vector<std::uint32_t> words=std::vector<std::uint32_t>(0x800000/4+16,0xa5a5a5a5);
    rdram ram{};ri_controller ri{};rdp_core dp{};pi_controller pi{};
    explicit Fixture(std::size_t bytes=0x800000){ram.dram=words.data()+8;ram.dram_size=bytes;reset();}
    void reset(){om_n64_case_pi_fb_reset(&pi,&ri,&dp,&ram);}
    void frame(std::uint32_t at,std::uint32_t bytes){dp.fb.infos[0]={at,1,bytes,1};}
    bool guards()const{return std::all_of(words.begin(),words.begin()+8,[](auto v){return v==0xa5a5a5a5;})&&std::all_of(words.end()-8,words.end(),[](auto v){return v==0xa5a5a5a5;});}
};
static void pi_adjusted_ranges_and_order(){
    Fixture f;f.frame(0x7ff000,0x1000);f.dp.fb.dirty_page[0x7ff]=1;
    f.pi.regs[PI_DRAM_ADDR_REG]=0x7fff80;f.pi.regs[PI_RD_LEN_REG]=126;
    om_n64_case_pi_start(&f.pi,0);auto s=om_n64_case_pi_fb_observed();
    require(s.transfers==1&&s.transfer_length==128&&s.reads_at_transfer==1&&s.cycles==1234&&s.scheduled==1&&!s.bad,"127-byte PI request rounded before admission, exact-end readback before transfer");
    require(f.pi.regs[PI_DRAM_ADDR_REG]==0x800000&&f.pi.regs[PI_CART_ADDR_REG]==128&&(f.pi.regs[PI_STATUS_REG]&PI_STATUS_DMA_BUSY),"valid PI status/address scheduling retained");
    f.reset();f.frame(0x7ff000,0x1000);f.dp.fb.dirty_page[0x7ff]=1;
    f.pi.regs[PI_DRAM_ADDR_REG]=0x7fff82;f.pi.regs[PI_RD_LEN_REG]=126;
    om_n64_case_pi_start(&f.pi,0);s=om_n64_case_pi_fb_observed();
    require(!s.getter&&!s.transfers&&!s.reads&&!s.scheduled&&(f.pi.regs[PI_STATUS_REG]&PI_STATUS_ERROR)&&f.dp.fb.dirty_page[0x7ff],"rounded overrun refused before dispatch/readback/schedule");
    f.reset();f.frame(0x1000,0x2000);f.dp.fb.dirty_page[1]=f.dp.fb.dirty_page[2]=1;
    f.pi.regs[PI_DRAM_ADDR_REG]=0xff0;f.pi.regs[PI_RD_LEN_REG]=0x201f;
    om_n64_case_pi_start(&f.pi,0);s=om_n64_case_pi_fb_observed();
    require(s.reads==2&&s.first_read==0x1000&&s.last_read==0x2000&&s.reads_at_transfer==2&&!s.bad,"DMA crossing buffers and dirty pages reads each page before cartridge");
    f.reset();f.frame(0x1000,0x1000);f.pi.regs[PI_DRAM_ADDR_REG]=0x1006;f.pi.regs[PI_WR_LEN_REG]=0;
    om_n64_case_pi_start(&f.pi,1);s=om_n64_case_pi_fb_observed();
    require(s.transfers==1&&!s.transfer_length&&!s.writes&&s.scheduled==1&&s.cycles==4321,"zero-effective short write retains timing without FB notifications");
    f.reset();f.frame(0x7ff000,0x1000);f.pi.regs[PI_DRAM_ADDR_REG]=0x7ffffe;f.pi.regs[PI_WR_LEN_REG]=7;
    om_n64_case_pi_start(&f.pi,1);s=om_n64_case_pi_fb_observed();
    require(s.transfer_length==2&&s.writes==1&&s.last_size==2&&s.last_write==0x7ffffe&&s.cycles==4321&&!s.bad&&f.guards(),"short-write address adjustment admitted against final RAM bytes");
    for(unsigned mode=1;mode<=3;++mode){f.reset();om_n64_case_pi_fb_handler(mode);om_n64_case_pi_start(&f.pi,mode==2);s=om_n64_case_pi_fb_observed();require(!s.transfers&&!s.scheduled&&(f.pi.regs[PI_STATUS_REG]&PI_STATUS_ERROR),"missing selected handler refused");}
    f.reset();f.pi.regs[PI_STATUS_REG]=PI_STATUS_IO_BUSY;om_n64_case_pi_start(&f.pi,0);s=om_n64_case_pi_fb_observed();require(!s.getter&&(f.pi.regs[PI_STATUS_REG]&PI_STATUS_ERROR),"busy request denied before dispatch");
    f.reset();f.pi.ri=nullptr;om_n64_case_pi_start(&f.pi,0);require(!om_n64_case_pi_fb_observed().getter&&(f.pi.regs[PI_STATUS_REG]&PI_STATUS_ERROR),"missing RAM graph denied");
}
static void descriptors_mapping_and_notifications(){
    Fixture f;std::array<FrameBufferInfo,6> infos{};
    infos[0]={0x1000,4,UINT32_MAX,UINT32_MAX}; // multiplication would overflow upstream
    infos[1]={0x807ff000,4,0x400,1}; // canonical last page
    infos[2]={0xa0001000,2,8,2};
    infos[3]={0x40001000,2,8,2}; // unrelated segment cannot alias physical RAM
    infos[4]={0x7ffffc,4,2,1};
    om_n64_case_pi_fb_stage(infos.data());protect_framebuffers(&f.dp.fb);auto s=om_n64_case_pi_fb_observed();
    require(s.mappings==2&&s.invalidations==1&&!s.bad&&f.dp.fb.infos[0].addr==0x7ff000&&f.dp.fb.infos[1].addr==0x1000&&!f.dp.fb.infos[2].addr,"overflow/outside descriptors rejected and valid aliases compacted");
    require(f.dp.fb.dirty_page[0x7ff]&&f.dp.fb.dirty_page[1],"last dirty-page slot and lower page admitted");
    unprotect_framebuffers(&f.dp.fb);s=om_n64_case_pi_fb_observed();require(s.mappings==4&&!s.bad&&f.dp.fb.infos[0].addr==0x7ff000,"paired unprotect restores admitted mapping and retains old descriptors");
    pre_framebuffer_read(&f.dp.fb,0xa07fffff);pre_framebuffer_read(&f.dp.fb,0x807fffff);
    s=om_n64_case_pi_fb_observed();require(s.reads==1&&s.last_read==0x7fffff,"canonical aliases consume dirty page once");
    pre_framebuffer_read_range(&f.dp.fb,0x7ffffc,8);post_framebuffer_write(&f.dp.fb,0x7ffffc,8);
    require(om_n64_case_pi_fb_observed().reads==1&&!om_n64_case_pi_fb_observed().writes,"incomplete callback request never partially notifies");
    f.reset();f.frame(0x1003,20);post_framebuffer_write(&f.dp.fb,0x1000,16);s=om_n64_case_pi_fb_observed();
    require(s.writes==3&&s.first_write==0x1004&&s.last_write==0x100c&&s.last_size==4,"intersection preserves original step phase");
    f.reset();f.frame(0x1000,12);post_framebuffer_write(&f.dp.fb,0x1001,3);s=om_n64_case_pi_fb_observed();require(s.writes==3&&s.last_size==1,"three-byte notification preserves byte granularity");
    std::uint32_t value=7;read_rdram_fb(&f.dp.fb,0x00800000,&value);write_rdram_fb(&f.dp.fb,0x00800000,0,UINT32_MAX);
    s=om_n64_case_pi_fb_observed();require(value==0&&!s.raw_reads&&!s.raw_writes,"mapped-region tail refuses raw RAM word access");
    read_rdram_fb(&f.dp.fb,0x807fffff,&value);write_rdram_fb(&f.dp.fb,0xa07ffffc,0x12345678,UINT32_MAX);
    s=om_n64_case_pi_fb_observed();require(s.raw_reads==1&&s.raw_writes==1&&!s.bad&&f.guards(),"last aligned RAM word uses bounded canonical access");
    Fixture small(0x400000);small.frame(0x3ff000,0x1000);small.pi.regs[PI_DRAM_ADDR_REG]=0x3ffff8;small.pi.regs[PI_RD_LEN_REG]=7;om_n64_case_pi_start(&small.pi,0);
    require(om_n64_case_pi_fb_observed().transfers==1,"4-MiB exact endpoint uses actual capacity");
    small.reset();small.pi.regs[PI_DRAM_ADDR_REG]=0x400000;om_n64_case_pi_start(&small.pi,0);require(!om_n64_case_pi_fb_observed().getter,"4-MiB endpoint overrun rejected despite larger fixture backing");
}
static void full_cache_and_foreign_owner(){
    std::array<int,7> values{};std::array<const int*,6> cache{};
    for(unsigned i=0;i<6;++i){auto slot=om_n64_find_framebuffer(cache,&values[i]);require(!slot.first&&slot.second==i,"next finite cache slot");cache[slot.second]=&values[i];}
    const auto full=cache;auto found=om_n64_find_framebuffer(cache,&values[5]);auto missing=om_n64_find_framebuffer(cache,&values[6]);
    require(found.first&&found.second==5&&!missing.first&&missing.second==6&&cache==full,"full six-slot lookup returns bounded sentinel and exact identity");
    Fixture f;f.frame(0x1000,0x1000);f.dp.fb.dirty_page[1]=1;require(om_n64_owner_begin()==1,"activate real owner");
    std::thread foreign([&]{om_n64_case_pi_start(&f.pi,0);pre_framebuffer_read_range(&f.dp.fb,0x1000,64);post_framebuffer_write(&f.dp.fb,0x1000,64);protect_framebuffers(&f.dp.fb);unprotect_framebuffers(&f.dp.fb);});foreign.join();
    auto s=om_n64_case_pi_fb_observed();require(!s.getter&&!s.reads&&!s.writes&&!s.mappings&&f.dp.fb.dirty_page[1],"foreign owner cannot dispatch/mutate/callback/map");require(om_n64_owner_end(nullptr)==1,"retire owner");
}
int main(){try{pi_adjusted_ranges_and_order();descriptors_mapping_and_notifications();full_cache_and_foreign_owner();std::cout<<"PI/framebuffer source cases passed\n";return 0;}catch(const std::exception& error){std::cerr<<error.what()<<'\n';return 1;}}
