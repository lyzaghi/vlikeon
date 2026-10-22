// Authored only: complete source/destination extents and unchanged valid bytes.
#include "n64-core-owner.h"
#include "n64-dma-memory.h"
extern "C" {
#include "cartridge_dma_fixture.h"
#include "backends/api/storage_backend.h"
#include "device/cart/cart.h"
#include "device/cart/sram.h"
#include "device/cart/cart_rom.h"
#include "device/dd/dd_controller.h"
#include "device/device.h"
#include "device/rcp/pi/pi_controller.h"
}
#include <algorithm>
#include <array>
#include <bit>
#include <cstdint>
#include <cstring>
#include <iostream>
#include <limits>
#include <stdexcept>
#include <thread>
#include <vector>
static void require(bool good,const char* why){if(!good)throw std::runtime_error(why);}
static std::size_t wire_byte(std::size_t at){return std::endian::native==std::endian::little?(at&~std::size_t(3))+(3-at%4):at;}
struct Ram {
    std::vector<std::uint32_t> words=std::vector<std::uint32_t>(0x800000/4+16,0x51515151);rdram ram{};
    explicit Ram(std::size_t bytes=0x800000){ram.dram=words.data()+8;ram.dram_size=bytes;}
    std::uint8_t* data(){return reinterpret_cast<std::uint8_t*>(ram.dram);}
    bool guards()const{return std::all_of(words.begin(),words.begin()+8,[](auto v){return v==0x51515151;})&&std::all_of(words.end()-8,words.end(),[](auto v){return v==0x51515151;});}
};
struct Storage {
    std::vector<std::uint32_t> words=std::vector<std::uint32_t>(SRAM_SIZE/4+16,0xa5a5a5a5);
    std::vector<std::uint8_t> committed=std::vector<std::uint8_t>(SRAM_SIZE,0xa5);
    std::size_t capacity=SRAM_SIZE,saves=0,last=0,length=0;mutable unsigned views=0;
    std::uint8_t* data(){return reinterpret_cast<std::uint8_t*>(words.data()+8);}
    static uint8_t* view(const void* p){auto& s=*static_cast<const Storage*>(p);++s.views;return const_cast<Storage&>(s).data();}
    static size_t size(const void* p){return static_cast<const Storage*>(p)->capacity;}
    static void save(void* p,size_t at,size_t count){auto& s=*static_cast<Storage*>(p);require(at<=SRAM_SIZE&&count<=SRAM_SIZE-at,"backend receives a complete SRAM dirty range");std::copy_n(s.data()+at,count,s.committed.data()+at);++s.saves;s.last=at;s.length=count;}
    static const storage_backend_interface& api(){static const storage_backend_interface value{view,size,save};return value;}
    bool guards()const{return std::all_of(words.begin(),words.begin()+8,[](auto v){return v==0xa5a5a5a5;})&&std::all_of(words.end()-8,words.end(),[](auto v){return v==0xa5a5a5a5;});}
};
static void sram_full_transaction_and_words(){
    Ram ram;Storage storage;sram s{};init_sram(&s,&storage,&Storage::api(),&ram.ram);
    for(unsigned i=0;i<16;++i)ram.data()[wire_byte(0x7ffff0+i)]=static_cast<std::uint8_t>(i+0x60);
    require(sram_dma_read(&s,ram.data(),0x7ffff0,0x08007ff0,16)==0x1000,"SRAM pinned DMA cycle count");
    for(unsigned i=0;i<16;++i)require(storage.data()[wire_byte(0x7ff0+i)]==i+0x60,"SRAM exact final bytes preserve byte order");
    require(storage.saves==1&&storage.last==0x7ff0&&storage.length==16&&storage.guards()&&ram.guards(),"exact full SRAM publication range and guards");
    const auto before=storage.words;const auto saved=storage.saves;const auto views=storage.views;
    sram_dma_read(&s,ram.data(),0x7ffff0,0x7ff0,17);sram_dma_read(&s,ram.data(),0x7ffff8,0,16);
    sram_dma_read(&s,ram.data()+1,0,0,8);sram_dma_read(&s,ram.data(),UINT32_MAX,0,8);sram_dma_read(&s,ram.data(),0,0x8000,1);
    require(storage.words==before&&storage.saves==saved&&storage.views==views,"invalid complete source/destination rejected before storage view or partial write");
    const auto old_ram=ram.words;sram_dma_write(&s,ram.data(),0,0x7ff0,17);sram_dma_write(&s,ram.data(),0x7ffffe,0,4);sram_dma_write(&s,nullptr,0,0,4);
    require(ram.words==old_ram,"SRAM read refuses both source and destination overruns without partial RAM");
    sram_dma_write(&s,ram.data(),0x7ffff0,0x7ff0,16);for(unsigned i=0;i<16;++i)require(ram.data()[wire_byte(0x7ffff0+i)]==storage.data()[wire_byte(0x7ff0+i)],"SRAM exact-end readback");
    for(unsigned i=0;i<3;++i)ram.data()[wire_byte(1+i)]=static_cast<std::uint8_t>(0xe0+i);
    sram_dma_read(&s,ram.data(),1,SRAM_SIZE-3,3);
    for(unsigned i=0;i<3;++i)require(storage.committed[wire_byte(SRAM_SIZE-3+i)]==0xe0+i,"short swapped SRAM write publishes every physically changed byte");
    require(storage.last==(std::endian::native==std::endian::little?SRAM_SIZE-4:SRAM_SIZE-3)&&storage.length==(std::endian::native==std::endian::little?4:3),"physical word cover is bounded at SRAM end");
    std::uint32_t expected=0x12345678,observed=0;std::memcpy(storage.data()+SRAM_SIZE-4,&expected,4);read_sram(&s,0x7ffc,&observed);require(observed==expected,"last complete MMIO word");
    write_sram(&s,0x7ffc,0xaabbccdd,0xffff0000);read_sram(&s,0x7ffc,&observed);require(observed==0xaabb5678&&storage.length==4,"MMIO masked write retains other bytes");
    auto word_before=storage.words;const auto last_saved=storage.saves;write_sram(&s,0x7ffd,0,UINT32_MAX);read_sram(&s,0x7ffd,&observed);
    require(observed==0&&storage.words==word_before&&storage.saves==last_saved,"short final MMIO word never escapes backend");
    storage.capacity=SRAM_SIZE-1;const auto short_views=storage.views;write_sram(&s,0,0,UINT32_MAX);sram_dma_read(&s,ram.data(),0,0,4);read_sram(&s,0,&observed);
    require(!observed&&storage.words==word_before&&storage.views==short_views,"undersized backend rejected before obtaining memory");
    storage.capacity=SRAM_SIZE;sram_dma_read(&s,ram.data(),0,0,0);require(storage.saves==last_saved,"zero DMA emits no persistence notification");
    auto missing=Storage::api();missing.save=nullptr;s.istorage=&missing;sram_dma_read(&s,ram.data(),0,0,4);require(storage.words==word_before,"missing save callback rejects SRAM write");
}
static void rom_eof_words_and_invalidation(){
    Ram ram(0x400000);std::array<std::uint32_t,16> source{};auto* bytes=reinterpret_cast<std::uint8_t*>(source.data());for(unsigned i=0;i<sizeof(source);++i)bytes[wire_byte(i)]=static_cast<std::uint8_t>(i+1);
    pi_controller pi{};cart_rom rom{};init_cart_rom(&rom,bytes,sizeof(source),om_n64_case_dma_cpu(&ram.ram),&pi);poweron_cart_rom(&rom);
    auto cycles=cart_rom_dma_write(&rom,ram.data(),0x3ffff0,0x10000038,16);auto stats=om_n64_case_dma_observed();
    for(unsigned i=0;i<16;++i)require(ram.data()[wire_byte(0x3ffff0+i)]==(i<8?i+57:0),"ROM EOF keeps prefix then zero padding");
    require(cycles==9&&stats.invalidations==2&&stats.first_address==0x803ffff0&&stats.last_address==0xa03ffff0&&stats.first_length==16&&stats.last_length==16&&!stats.bad,"only actual complete RAM output invalidated at both aliases");
    om_n64_case_dma_clear();cart_rom_dma_write(&rom,ram.data(),0,0xeffffff0,32);for(unsigned i=0;i<32;++i)require(!ram.data()[wire_byte(i)],"large source offset never wraps into ROM prefix");
    const auto before=ram.words;om_n64_case_dma_clear();cart_rom_dma_write(&rom,ram.data(),0x3ffffc,0,8);cart_rom_dma_write(&rom,ram.data()+4,0,0,8);cart_rom_dma_write(&rom,ram.data(),UINT32_MAX,0,UINT32_MAX);stats=om_n64_case_dma_observed();
    require(ram.words==before&&!stats.invalidations&&!stats.random_calls,"invalid ROM DMA cannot partially copy/invalidate/advance random timing");
    om_n64_case_dma_clear();cycles=cart_rom_dma_write(&rom,ram.data(),ram.ram.dram_size,0,0);stats=om_n64_case_dma_observed();require(cycles==7&&!stats.invalidations&&stats.random_calls==1,"zero ROM DMA retains timing without accidental global invalidation");
    std::uint32_t observed=0;read_cart_rom(&rom,0x1000003c,&observed);require(observed==source.back(),"last complete ROM MMIO word");
    rom.rom_size=sizeof(source)-1;read_cart_rom(&rom,0x1000003c,&observed);require(!observed,"partial final ROM MMIO word refused");
    cart_rom_dma_write(&rom,ram.data(),0,0x1000003c,8);for(unsigned i=0;i<8;++i){auto logical=60+i;auto physical=wire_byte(logical);auto expected=logical<rom.rom_size&&physical<rom.rom_size?bytes[physical]:0;require(ram.data()[wire_byte(i)]==expected,"short physical ROM tail cannot read backing padding");}
    pi.regs[PI_STATUS_REG]=0;om_n64_case_dma_clear();write_cart_rom(&rom,0,0xaabbccdd,0xffff);read_cart_rom(&rom,0,&observed);stats=om_n64_case_dma_observed();
    require(observed==0xccdd&&(pi.regs[PI_STATUS_REG]&PI_STATUS_IO_BUSY)&&stats.scheduled==1&&stats.validated==1,"ROM busy MMIO latch/timing retained");
    write_cart_rom(&rom,0,0x12345678,UINT32_MAX);require(rom.last_write==0x12345678&&(pi.regs[PI_STATUS_REG]&PI_STATUS_ERROR)&&om_n64_case_dma_observed().scheduled==1,"busy write still changes pinned latch but does not schedule again");
    require(ram.guards(),"ROM output guard bytes");
}
static void dd_exact_buffers_and_short_ipl(){
    Ram ram;dd_controller dd{};dd.r4300=om_n64_case_dma_cpu(&ram.ram);std::array<std::uint32_t,16> ipl{};dd.rom=ipl.data();dd.rom_size=sizeof(ipl);
    for(unsigned i=0;i<256;++i)ram.data()[wire_byte(0x7fff00+i)]=static_cast<std::uint8_t>(i^0x35);
    auto cycles=dd_dom_dma_read(&dd,ram.data(),0x7fff00,MM_DD_DS_BUFFER,256);require(cycles==256*63/25,"DD valid cycle formula unchanged");
    for(unsigned i=0;i<256;++i)require(dd.ds_buf[wire_byte(i)]==static_cast<std::uint8_t>(i^0x35),"full data sector copies exact logical bytes");
    std::array<std::uint8_t,sizeof(dd_controller)> before_dd{};std::memcpy(before_dd.data(),&dd,sizeof(dd));
    dd_dom_dma_read(&dd,ram.data(),0,MM_DD_DS_BUFFER,257);dd_dom_dma_read(&dd,ram.data(),0x7fff04,MM_DD_DS_BUFFER,256);dd_dom_dma_read(&dd,ram.data()+1,0,MM_DD_DS_BUFFER,4);
    require(std::memcmp(&dd,before_dd.data(),sizeof(dd))==0,"DD rejects complete invalid destination/source before any device mutation");
    std::fill(std::begin(dd.c2s_buf),std::end(dd.c2s_buf),0x69);om_n64_case_dma_clear();dd_dom_dma_write(&dd,ram.data(),0x7ffc00,MM_DD_C2S_BUFFER,1024);auto stats=om_n64_case_dma_observed();
    require(std::all_of(ram.data()+0x7ffc00,ram.data()+0x800000,[](auto v){return v==0x69;})&&stats.invalidations==2&&!stats.bad,"complete C2 buffer fits exact final RAM span");
    const auto before_ram=ram.words;om_n64_case_dma_clear();dd_dom_dma_write(&dd,ram.data(),0,MM_DD_C2S_BUFFER,1025);dd_dom_dma_write(&dd,ram.data(),0,MM_DD_DS_BUFFER,257);
    dd_dom_dma_write(&dd,ram.data(),0,MM_DD_ROM+60,8);dd_dom_dma_write(&dd,ram.data(),0,MM_DD_ROM+sizeof(ipl),1);dd_dom_dma_write(&dd,ram.data(),UINT32_MAX,MM_DD_DS_BUFFER,8);
    require(ram.words==before_ram&&!om_n64_case_dma_observed().invalidations,"all DD invalid full extents leave RAM and code cache untouched");
    ipl.back()=0x01020304;dd_dom_dma_write(&dd,ram.data(),0,MM_DD_ROM+60,4);std::uint32_t observed=0;read_dd_rom(&dd,MM_DD_ROM+60,&observed);require(observed==ipl.back()&&ram.ram.dram[0]==ipl.back(),"last complete IPL word by DMA and MMIO");
    dd.rom_size=sizeof(ipl)-1;read_dd_rom(&dd,MM_DD_ROM+60,&observed);require(!observed,"short IPL final MMIO word refused");
    const auto short_before=ram.words;om_n64_case_dma_clear();dd_dom_dma_write(&dd,ram.data(),0,MM_DD_ROM+60,4);require(ram.words==short_before&&!om_n64_case_dma_observed().invalidations,"short IPL XOR source range rejected before copy");
    om_n64_case_dma_clear();dd_dom_dma_write(&dd,ram.data(),0,MM_DD_DS_BUFFER,0);require(!om_n64_case_dma_observed().invalidations,"zero DD DMA cannot flush all CPU code");
    cycles=dd_dom_dma_write(&dd,ram.data(),0,MM_DD_DS_BUFFER,UINT32_MAX);require(cycles==std::numeric_limits<unsigned int>::max()&&!om_n64_case_dma_observed().invalidations,"oversized direct DD timing arithmetic saturates without wrap or copy");
    std::memcpy(before_dd.data(),&dd,sizeof(dd));dd_dom_dma_read(&dd,ram.data(),0,MM_DD_MS_RAM,16);require(!std::memcmp(&dd,before_dd.data(),sizeof(dd)),"pinned unimplemented MS request remains a no-op");require(ram.guards(),"DD RAM guard bytes");
}
static void swapped_spans_and_foreign_dispatch(){
    // Exhaustive independent byte-enumeration oracle for small tail views.
    for(std::size_t capacity=0;capacity<=32;++capacity)for(std::uint32_t at=0;at<=36;++at)for(std::uint32_t count=0;count<=36;++count){
        bool fits=at<=capacity&&count<=capacity-std::min<std::size_t>(at,capacity);
        if(fits)for(std::uint32_t i=0;i<count;++i)if(wire_byte(at+i)>=capacity)fits=false;
        require(static_cast<bool>(om_n64_dma_swapped_range(capacity,at,count))==fits,"swapped span agrees with every physical byte, including empty and short tails");
    }
    Ram ram;Storage storage;cart cart{};init_sram(&cart.sram,&storage,&Storage::api(),&ram.ram);cart.use_flashram=0;
    dd_controller dd{};dd.r4300=om_n64_case_dma_cpu(&ram.ram);pi_controller pi{};std::array<std::uint32_t,16> source{};init_cart_rom(&cart.cart_rom,reinterpret_cast<uint8_t*>(source.data()),sizeof(source),dd.r4300,&pi);
    const auto before_ram=ram.words;const auto before_storage=storage.words;require(om_n64_owner_begin()==1,"activate real owner");
    std::thread foreign([&]{cart_dom2_dma_read(&cart,ram.data(),0,0,8);cart_dom2_dma_write(&cart,ram.data(),0,0,8);cart_dom3_dma_write(&cart,ram.data(),0,0,8);dd_dom_dma_read(&dd,ram.data(),0,MM_DD_DS_BUFFER,8);dd_dom_dma_write(&dd,ram.data(),0,MM_DD_DS_BUFFER,8);write_cart_rom(&cart.cart_rom,0,1,UINT32_MAX);write_sram(&cart.sram,0,0,UINT32_MAX);});foreign.join();
    auto stats=om_n64_case_dma_observed();require(cart.use_flashram==0&&ram.words==before_ram&&storage.words==before_storage&&!storage.views&&!storage.saves&&!stats.invalidations&&!stats.random_calls&&!stats.scheduled&&!stats.validated,"foreign domain calls do not select save type, copy, notify or schedule");
    require(om_n64_owner_end(nullptr)==1,"retire owner");
}
int main(){try{sram_full_transaction_and_words();rom_eof_words_and_invalidation();dd_exact_buffers_and_short_ipl();swapped_spans_and_foreign_dispatch();std::cout<<"Cartridge DMA source cases passed\n";return 0;}catch(const std::exception& error){std::cerr<<error.what()<<'\n';return 1;}}
