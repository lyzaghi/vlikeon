// Authored only. Actual flash/SI/C2 code, with explicit SI callback stubs.
#include "n64-core-owner.h"
#include "n64-device-memory.h"
#include "n64-snapshot-bounds.h"
extern "C" {
#include "backends/api/storage_backend.h"
#include "device/cart/flashram.h"
#include "device/rdram/rdram.h"
#include "device/pif/pif.h"
#include "device/rcp/si/si_controller.h"
#include "device/rcp/ri/ri_controller.h"
#include "device/dd/dd_controller.h"
void om_n64_case_si_init(struct si_controller*,struct pif*,struct ri_controller*);
int om_n64_case_si_copy(struct si_controller*);
void om_n64_case_si_start(struct si_controller*,int);
unsigned int om_n64_case_si_scheduled(void);
unsigned int om_n64_case_si_pif_calls(void);
unsigned int om_n64_case_si_raised(void);
void om_n64_case_dd_c2(struct dd_controller*);
}
#include <algorithm>
#include <array>
#include <cstdint>
#include <cstring>
#include <iostream>
#include <iterator>
#include <stdexcept>
#include <thread>
#include <vector>
static void require(bool good,const char* why){if(!good)throw std::runtime_error(why);}
struct Storage {
    std::vector<std::uint8_t> bytes=std::vector<std::uint8_t>(FLASHRAM_SIZE+64,0xa5);
    size_t capacity=FLASHRAM_SIZE,saves=0,last=0,length=0;
    static uint8_t* data(const void* p){return const_cast<uint8_t*>(static_cast<const Storage*>(p)->bytes.data()+32);}
    static size_t size(const void* p){return static_cast<const Storage*>(p)->capacity;}
    static void save(void* p,size_t at,size_t count){auto& s=*static_cast<Storage*>(p);require(at<=FLASHRAM_SIZE&&count<=FLASHRAM_SIZE-at,"save callback range");++s.saves;s.last=at;s.length=count;}
    static const storage_backend_interface& api(){static const storage_backend_interface value{data,size,save};return value;}
    bool guards()const{return std::all_of(bytes.begin(),bytes.begin()+32,[](auto v){return v==0xa5;})&&std::all_of(bytes.end()-32,bytes.end(),[](auto v){return v==0xa5;});}
};
static void command(flashram& f,std::uint32_t value){write_flashram(&f,0x08010000,value,UINT32_MAX);}
static void flash_pages_commands_and_dma(){
    Storage store;std::array<std::uint32_t,64> words{};rdram ram{};ram.dram=words.data();ram.dram_size=sizeof(words);flashram f{};
    init_flashram(&f,MX29L0000_ID,&store,&Storage::api(),&ram);poweron_flashram(&f);format_flashram(Storage::data(&store));
    auto* dram=reinterpret_cast<std::uint8_t*>(words.data());for(unsigned i=0;i<128;++i)dram[i]=static_cast<std::uint8_t>(i^0x5b);
    command(f,0xb4000000);require(flashram_dma_read(&f,dram,0,0,128)==0x1000,"page-buffer DMA keeps pinned cycle result");
    command(f,0xa50003ff);require(std::memcmp(Storage::data(&store)+FLASHRAM_SIZE-128,dram,128)==0&&store.last==FLASHRAM_SIZE-128&&store.length==128&&f.status==4&&f.mode==FLASHRAM_MODE_STATUS&&store.guards(),"last flash page exact data/callback/status and guards");
    auto before=store.bytes;const auto saved=store.saves;const auto status=f.status;
    command(f,0xa5000400);command(f,0xa500ffff);require(store.bytes==before&&store.saves==saved&&f.status==status,"past-end program commands cannot mutate or report new success");
    command(f,0x4b00ffff);const auto erase_status=f.status;command(f,0x78000000);
    require(store.bytes==before&&store.saves==saved&&f.status==erase_status&&f.mode==FLASHRAM_MODE_SECTOR_ERASE,"sector arithmetic cannot escape flash extent");
    command(f,0x4b0003ff);command(f,0x78000000);
    require(store.last==FLASHRAM_SIZE-16384&&store.length==16384&&std::all_of(Storage::data(&store)+FLASHRAM_SIZE-16384,Storage::data(&store)+FLASHRAM_SIZE,[](auto v){return v==0xff;})&&store.guards(),"last complete sector erased");
    command(f,0x3c000000);command(f,0x78000000);require(store.last==0&&store.length==FLASHRAM_SIZE&&store.guards(),"full-chip erase bounded to actual flash");
    write_flashram(&f,0,0x12345678,0xffff);std::uint32_t observed=7;read_flashram(&f,0,&observed);require(observed==0x78,"status mask retained");read_flashram(&f,0x20,&observed);require(observed==0,"unknown MMIO read initialized");
    command(f,0xb4000000);std::array<std::uint8_t,128> retained{};std::copy(std::begin(f.page_buf),std::end(f.page_buf),retained.begin());
    flashram_dma_read(&f,dram,sizeof(words)-127,0,128);flashram_dma_read(&f,nullptr,0,0,128);
    require(std::equal(retained.begin(),retained.end(),f.page_buf),"bad full-page DMA never partially overwrites page buffer");
    for(unsigned i=0;i<FLASHRAM_SIZE;++i)Storage::data(&store)[i]=static_cast<std::uint8_t>(i^0x63);
    command(f,0xf0000000);std::fill(words.begin(),words.end(),0x55555555);
    flashram_dma_write(&f,dram,0,0xfffc,8);require(std::memcmp(dram,Storage::data(&store)+FLASHRAM_SIZE-8,8)==0,"old-silicon doubled address at exact end");
    const auto old_dram=words;flashram_dma_write(&f,dram,0,0xffff,4);flashram_dma_write(&f,dram,sizeof(words)-4,0,8);flashram_dma_write(&f,dram,UINT32_MAX,0,8);
    require(words==old_dram&&store.guards(),"flash source and RDRAM destination must both fit before DMA");
    command(f,0xe1000000);flashram_dma_write(&f,dram,2,0,8);require(words[0]==FLASHRAM_TYPE_ID&&words[1]==MX29L0000_ID,"silicon ID retains pinned word alignment");
    const auto id_dram=words;flashram_dma_write(&f,dram,sizeof(words)-4,0,8);require(words==id_dram,"silicon ID checks all eight output bytes");
    store.capacity=FLASHRAM_SIZE-1;before=store.bytes;const auto saved_before=store.saves;command(f,0xa5000000);require(store.bytes==before&&store.saves==saved_before,"undersized backing storage refused");
}
struct Si {
    std::array<std::uint32_t,64> words{};std::array<std::uint32_t,16> packet{};rdram ram{};pif p{};ri_controller ri{};si_controller si{};
    Si(){ram.dram=words.data();ram.dram_size=sizeof(words);p.ram=reinterpret_cast<uint8_t*>(packet.data());ri.rdram=&ram;om_n64_case_si_init(&si,&p,&ri);si.regs[SI_PIF_ADDR_RD64B_REG]=0x1fc007c0;si.regs[SI_PIF_ADDR_WR64B_REG]=0x1fc007c0;}
};
static void si_whole_blocks_and_pending_register_changes(){
    Si out;for(unsigned i=0;i<16;++i)out.words[i]=0x01020300+i;om_n64_case_si_start(&out.si,0);
    auto* wire=reinterpret_cast<const std::uint8_t*>(out.packet.data());require(wire[0]==1&&wire[1]==2&&wire[2]==3&&wire[3]==0&&wire[63]==15&&om_n64_case_si_scheduled()==1,"actual SI write copies full big-endian packet before scheduling");
    si_end_of_dma_event(&out.si);require(om_n64_case_si_pif_calls()==1&&om_n64_case_si_raised()==1&&out.si.dma_dir==SI_NO_DMA,"actual completion keeps processing/interrupt control flow");
    Si bad;bad.packet.fill(0x55555555);const auto old_packet=bad.packet;bad.si.regs[SI_DRAM_ADDR_REG]=sizeof(bad.words)-60;
    om_n64_case_si_start(&bad.si,0);require(bad.packet==old_packet&&(bad.si.regs[SI_STATUS_REG]&SI_STATUS_DMA_ERROR)&&!om_n64_case_si_scheduled()&&!om_n64_case_si_pif_calls(),"short SI write rejected before any copy/callback/schedule");
    om_n64_case_si_start(&bad.si,1);require(!om_n64_case_si_pif_calls()&&!om_n64_case_si_scheduled(),"short SI read rejected before PIF callback");
    Si in;auto* bytes=reinterpret_cast<std::uint8_t*>(in.packet.data());for(unsigned i=0;i<64;++i)bytes[i]=static_cast<std::uint8_t>(i);in.si.regs[SI_DRAM_ADDR_REG]=sizeof(in.words)-64;
    om_n64_case_si_start(&in.si,1);si_end_of_dma_event(&in.si);require(in.words[48]==0x00010203&&in.words[63]==0x3c3d3e3f&&om_n64_case_si_raised()==1,"exact final SI block preserves conversion");
    Si pending;pending.si.regs[SI_DRAM_ADDR_REG]=0;om_n64_case_si_start(&pending.si,1);const auto preserved=pending.words;pending.si.regs[SI_DRAM_ADDR_REG]=sizeof(pending.words)-4;si_end_of_dma_event(&pending.si);
    require(pending.words==preserved&&(pending.si.regs[SI_STATUS_REG]&SI_STATUS_DMA_ERROR)&&(pending.si.regs[SI_STATUS_REG]&SI_STATUS_INTERRUPT)&&pending.si.dma_dir==SI_NO_DMA,"changed pending address rechecked at use, completion still retires transfer");
    require(om_n64_si_dram_extent(0x800000,0x7fffc0)&&!om_n64_si_dram_extent(0x800000,0x7fffc4)&&!om_n64_si_dram_extent(0x800004,0)&&om_n64_si_dram_extent(256,0xff000000),"declared memory capacity and pinned low-address aliasing");
}
static void dd_c2_sectors_and_foreign_access(){
    dd_controller dd{};std::fill(std::begin(dd.c2s_buf),std::end(dd.c2s_buf),0x5a);const auto initial=std::vector<std::uint8_t>(std::begin(dd.c2s_buf),std::end(dd.c2s_buf));
    dd.bm_zone=0;dd.regs[DD_ASIC_CUR_SECTOR]=84<<16;om_n64_case_dd_c2(&dd);require(std::equal(initial.begin(),initial.end(),dd.c2s_buf)&&(dd.regs[DD_ASIC_BM_STATUS_CTL]&DD_BM_STATUS_MICRO),"C2 sector subtraction cannot underflow");
    dd.regs[DD_ASIC_CUR_SECTOR]=89<<16;om_n64_case_dd_c2(&dd);require(std::all_of(dd.c2s_buf,dd.c2s_buf+256,[](auto v){return v==0x5a;})&&std::all_of(dd.c2s_buf+256,dd.c2s_buf+488,[](auto v){return v==0;})&&dd.c2s_buf[488]==0x5a,"last C2 sector has exact bounded range");
    std::fill(std::begin(dd.c2s_buf),std::end(dd.c2s_buf),0x5a);dd.bm_zone=16;om_n64_case_dd_c2(&dd);require(std::equal(initial.begin(),initial.end(),dd.c2s_buf),"bad C2 zone preserves bytes");
    dd.bm_zone=0;dd.regs[DD_ASIC_CUR_SECTOR]=85<<16;dd.regs[DD_ASIC_BM_STATUS_CTL]=0;
    Si si;si.si.dma_dir=SI_DMA_WRITE;require(om_n64_owner_begin()==1,"activate owner for denial");int copied=1;std::thread foreign([&]{copied=om_n64_case_si_copy(&si.si);om_n64_case_dd_c2(&dd);});foreign.join();require(!copied&&std::equal(initial.begin(),initial.end(),dd.c2s_buf),"foreign thread cannot execute device copies");require(om_n64_owner_end(nullptr)==1,"owner retirement");
}
static void le32(std::uint8_t* p,std::uint32_t value){for(unsigned i=0;i<4;++i)p[i]=static_cast<std::uint8_t>(value>>(i*8));}
static void snapshot_device_selectors(){
    for(unsigned minor=0;minor<=10;++minor){std::vector<std::uint8_t> image(OM_N64_SNAPSHOT_BYTES,0);std::memcpy(image.data(),"M64+SAVE",8);image[9]=1;image[10]=static_cast<std::uint8_t>(minor);image[OM_N64_SNAPSHOT_PIF_AT]=1;le32(image.data()+OM_N64_SNAPSHOT_QUEUE_AT,UINT32_MAX);
        require(om_n64_snapshot_pointer_preflight(image.data(),image.size()),"baseline device state for every supported version");if(minor<2)continue;
        size_t pif=OM_N64_SNAPSHOT_EXTRA_AT+(minor==2?46:48)+4*44;auto bad=image;bad[pif+(minor==2?9:5)]=3;require(!om_n64_snapshot_pointer_preflight(bad.data(),bad.size()),"unknown SI direction refused");bad=image;bad[pif+(minor==2?10:6)]=4;require(!om_n64_snapshot_pointer_preflight(bad.data(),bad.size()),"unknown deferred DP flags refused");
        image[pif+(minor==2?9:5)]=1;le32(image.data()+44+188,UINT32_MAX);require(om_n64_snapshot_pointer_preflight(image.data(),image.size()),"raw pending address retained for bounded runtime handling");
        size_t at=pif+291;
        if(minor>=4){bad=image;le32(bad.data()+at+428,16);require(om_n64_snapshot_pointer_preflight(bad.data(),bad.size())&&!om_n64_snapshot_devices_valid(bad.data(),bad.size(),1,0)&&om_n64_snapshot_devices_valid(bad.data(),bad.size(),1,7),"zero-ID DD disambiguated with actual disk context; absent padding ignored");
            for(size_t field:{size_t{44},size_t{416},size_t{420},size_t{428}}){bad=image;le32(bad.data()+at,0x12345678);le32(bad.data()+at+field,field==44?256:field==428?16:2);require(!om_n64_snapshot_pointer_preflight(bad.data(),bad.size()),"DD selector cannot wrap into safe-looking loaded value");}at+=436;}
        if(minor>=5)at+=4;
        if(minor>=7){for(size_t offset:{size_t{0},size_t{16}}){bad=image;le32(bad.data()+at+offset,2);require(!om_n64_snapshot_pointer_preflight(bad.data(),bad.size()),"bad SP FIFO direction refused");}at+=32;}
        if(minor>=8){bad=image;bad[at+142]=6;require(!om_n64_snapshot_pointer_preflight(bad.data(),bad.size()),"bad flash mode refused");bad=image;le32(bad.data()+at+136,256);require(!om_n64_snapshot_pointer_preflight(bad.data(),bad.size()),"oversized flash status refused");}
    }
}
int main(){try{flash_pages_commands_and_dma();si_whole_blocks_and_pending_register_changes();dd_c2_sectors_and_foreign_access();snapshot_device_selectors();std::cout<<"N64 peripheral memory cases passed\n";return 0;}catch(const std::exception& error){std::cerr<<error.what()<<'\n';return 1;}}
