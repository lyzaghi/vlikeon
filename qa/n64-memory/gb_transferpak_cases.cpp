// Authored only. Actual prepared GB/Transfer Pak code + file storage and store.
#include "memory_files.hpp"
#include "n64-memory-io.h"
#include "n64-save-files.h"
#include "n64-snapshot-bounds.h"
extern "C" {
#include "backends/file_storage.h"
#include "backends/api/storage_backend.h"
#include "backends/api/clock_backend.h"
#include "backends/api/rumble_backend.h"
#include "device/gb/gb_cart.h"
#include "device/controllers/game_controller.h"
#include "device/controllers/paks/transferpak.h"
#include "main/util.h"
#include "api/callbacks.h"
#include <libretro.h>
retro_log_printf_t log_cb=nullptr;
/* Focused storage fixture: actual full-core diagnostics remain acceptance work. */
void DebugMessage(int,const char*,...){}
}
#include <algorithm>
#include <array>
#include <cstdlib>
#include <cstdint>
#include <cstring>
#include <iostream>
#include <stdexcept>
#include <vector>
static void require(bool okay,const char* why){if(!okay)throw std::runtime_error(why);}
static void le32(std::uint8_t* p,std::uint32_t value){for(unsigned i=0;i<4;++i)p[i]=static_cast<std::uint8_t>(value>>(8*i));}
static const char* save_name="save/N64/tpak/gb-case.ram";
struct Cart {
    omni::MemoryFiles store;OmniWritableFilesV1 api=store.interface();
    std::vector<std::uint8_t> rom;file_storage ram{};gb_cart cart{};
    bool ram_opened=false,rom_released=false,wrong_extent=false;unsigned discarded=0,rumble_started=0;
    static uint8_t* rom_data(const void* opaque){return const_cast<uint8_t*>(static_cast<const Cart*>(opaque)->rom.data());}
    static size_t rom_size(const void* opaque){return static_cast<const Cart*>(opaque)->rom.size();}
    static void rom_save(void*,size_t,size_t){std::abort();}
    static const storage_backend_interface& rom_api(){static const storage_backend_interface value{rom_data,rom_size,rom_save};return value;}
    static void init_rom(void* opaque,void** storage,const storage_backend_interface** api){*storage=opaque;*api=&rom_api();}
    static void release_rom(void* opaque){static_cast<Cart*>(opaque)->rom_released=true;}
    static void init_ram(void* opaque,size_t bytes,void** storage,const storage_backend_interface** api){
        auto& value=*static_cast<Cart*>(opaque);value.ram_opened=true;*storage=nullptr;*api=nullptr;
        auto* name=static_cast<char*>(std::malloc(std::strlen(save_name)+1));require(name!=nullptr,"save name allocation");std::strcpy(name,save_name);
        if(open_file_storage(&value.ram,bytes,name)!=file_ok)return;
        if(value.wrong_extent)--value.ram.size;
        *storage=&value.ram;*api=&g_ifile_storage;
    }
    static void discard_ram(void* opaque){auto& value=*static_cast<Cart*>(opaque);++value.discarded;om_n64_file_storage_discard_unstarted(&value.ram);}
    static time_t now(void*){return 0;}
    static void rumble(void* opaque,rumble_action action){if(action==RUMBLE_START)++static_cast<Cart*>(opaque)->rumble_started;}
    Cart(unsigned type,unsigned ram_code,size_t rom_bytes=0x8000,bool bad_extent=false):rom(rom_bytes,0x4b),wrong_extent(bad_extent){
        require(om_n64_files_install(&api)==1,"install actual store");rom[0x147]=static_cast<std::uint8_t>(type);rom[0x149]=static_cast<std::uint8_t>(ram_code);
        static const clock_backend_interface clock{now};static const rumble_backend_interface rumbler{rumble};
        init_gb_cart(&cart,this,init_rom,release_rom,this,init_ram,discard_ram,nullptr,&clock,this,&rumbler,nullptr,nullptr);
        if(cart.read_gb_cart)poweron_gb_cart(&cart);
    }
    ~Cart(){close_file_storage(&ram);om_n64_files_install(nullptr);}
    void command(std::uint16_t address,std::uint8_t value){require(write_gb_cart(&cart,address,&value,1)==0,"actual cart command");}
    std::vector<std::uint8_t> saved(){
        std::vector<std::uint8_t> bytes(ram.size);require(om_n64_save_read(save_name,bytes.data(),bytes.size())==OM_N64_SAVE_OK,"observe actual complete saved RAM");return bytes;
    }
    void absent(){OmniFilesStatV1 shape{};require(api.stat(api.context,save_name,&shape)==OMNI_FILES_NOT_FOUND,"failed startup created no default RAM image");}
};
static void high_banks_and_rumble(){
    {
        Cart value(0x1b,4,512*0x4000);value.command(0x0000,0x0a);value.command(0x4000,15);
        std::array<std::uint8_t,32> block{};block.fill(0x5a);
        require(write_gb_cart(&value.cart,0xa000,block.data(),block.size())==0,"write actual nonrumble MBC5 high bank");
        require(value.ram.data[15*0x2000]==0x5a&&!value.ram.data[7*0x2000]&&!value.ram.data[0],"bank 15 cannot alias bank 7 or truncate to a low bank");
        const auto retained=value.saved();require(retained[15*0x2000]==0x5a&&!retained[7*0x2000],"real file-storage publication retains high offset");
        block.fill(0);require(read_gb_cart(&value.cart,0xa000,block.data(),block.size())==0&&block[0]==0x5a,"high bank read matches write");
        value.rom[511*0x4000]=0x9b;value.command(0x2000,0xff);value.command(0x3000,1);
        require(read_gb_cart(&value.cart,0x4000,block.data(),block.size())==0&&block[0]==0x9b,"nine-bit ROM bank 511 remains supported");
    }
    {
        Cart value(0x1e,4);value.command(0x0000,0x0a);value.command(0x4000,15);
        const std::uint8_t data=0x71;require(write_gb_cart(&value.cart,0xa000,&data,1)==0,"actual rumble MBC5 write");
        require(value.ram.data[7*0x2000]==0x71&&!value.ram.data[15*0x2000]&&value.rumble_started==1,"rumble bit retains three-bit RAM bank and action");
    }
}
static void camera_bounds_and_failed_admission(){
    {
        Cart value(0xfc,4);value.command(0x0000,0x0a);value.command(0x4000,15);
        const std::uint8_t data=0x69;require(write_gb_cart(&value.cart,0xa000,&data,1)==0&&value.ram.data[15*0x2000]==0x69&&!value.ram.data[7*0x2000],"camera high RAM offset does not truncate");
        value.command(0x4000,0x10);value.command(0xa000,1);
        require(std::all_of(value.ram.data,value.ram.data+4096,[](auto byte){return byte==0xff;})&&!value.ram.data[4096],"missing capture backend fills only the bounded camera tile image");
        std::fill(value.ram.data,value.ram.data+4097,0x22);value.cart.cam.ram_bytes=4095;value.command(0xa000,1);
        require(std::all_of(value.ram.data,value.ram.data+4097,[](auto byte){return byte==0x22;}),"short retained camera extent permits no partial output write");value.cart.cam.ram_bytes=value.ram.size;
        require(om_n64_file_storage_flush(&value.ram)==1&&value.saved()[15*0x2000]==0x69,"checked aggregate-style flush includes camera and high bank data");
    }
    {
        Cart value(0xfc,1);require(!value.cart.read_gb_cart&&!value.ram_opened&&value.rom_released,"undersized camera header rejected before RAM open");value.absent();
    }
    {
        Cart value(0xfc,4,0x8000,true);require(!value.cart.read_gb_cart&&value.ram_opened&&value.discarded==1&&!value.ram.data&&!value.ram.filename,"bad supplied RAM descriptor is discarded during startup");value.absent();
    }
}
static void packet_and_storage_bounds(){
    Cart value(0x1b,4);std::array<std::uint8_t,33> output{};output.fill(0x55);
    require(read_gb_cart(&value.cart,0,output.data(),0)==-1&&read_gb_cart(&value.cart,0,output.data(),33)==-1&&output[0]==0x55,"invalid packet extents leave caller bytes untouched");
    require(write_gb_cart(&value.cart,0,nullptr,0)==-1,"empty packet cannot read data[-1]");
    require(read_gb_cart(nullptr,0,output.data(),32)==-1&&std::all_of(output.begin(),output.begin()+32,[](auto byte){return byte==0xff;}),"absent cart returns deterministic open bus");
    value.cart.rom_bank=511;require(read_gb_cart(&value.cart,0x4000,output.data(),32)==0&&output[0]==0xff,"out-of-ROM bank has deterministic data");
    value.cart.rom_bank=0x200;require(read_gb_cart(&value.cart,0x4000,output.data(),32)==-1&&output[0]==0xff,"forged large bank refused before address multiply");value.cart.rom_bank=1;
    value.command(0,0x0a);value.command(0x4000,15);
    require(write_gb_cart(&value.cart,0xbfff,output.data(),2)==-1,"packet cannot straddle the dispatched bank window");
    transferpak pak{};init_transferpak(&pak,&value.cart);poweron_transferpak(&pak);
    g_itransferpak.read(&pak,0xc000,output.data(),32);require(output[0]==0xff,"disabled Transfer Pak output initialized");
    const std::uint8_t enable=0x84;g_itransferpak.write(&pak,0x8000,&enable,1);require(pak.enabled==1&&pak.bank==3,"actual Transfer Pak enable behavior retained");
    const std::uint8_t bank=0;g_itransferpak.write(&pak,0xa000,&bank,1);g_itransferpak.read(&pak,0xc000,output.data(),32);
    require(output[0]==0x4b,"actual translated cart address reaches ROM");
    g_itransferpak.write(&pak,0xa000,nullptr,0);require(pak.bank==0,"empty Transfer Pak command cannot mutate bank");
    pak.bank=4;output.fill(0x55);g_itransferpak.read(&pak,0xc000,output.data(),32);require(output[0]==0xff,"bad Transfer Pak bank refused before translation");
}
static std::array<std::uint8_t,136> record(unsigned type,unsigned rom,unsigned ram,unsigned version){
    std::array<std::uint8_t,136> p{};le32(p.data(),1);le32(p.data()+4,3);le32(p.data()+8,1);le32(p.data()+12,3);
    p[16]='A';p[35]=static_cast<std::uint8_t>(type);le32(p.data()+44,rom);le32(p.data()+48,ram);le32(p.data()+52,1);le32(p.data()+56,1);
    le32(p.data()+44+(version==0x10200?21:16),1);return p;
}
static void wire_selectors(){
    const std::array<std::array<unsigned,3>,6> limits{{{0,1,0},{1,127,3},{5,15,0},{0x10,127,255},{0x1b,511,15},{0xfc,63,255}}};
    for(unsigned version:{0x10200u,0x10300u,0x10900u,0x10a00u})for(const auto& limit:limits){
        auto p=record(limit[0],limit[1],limit[2],version);size_t used=7;
        require(om_n64_snapshot_transferpak_valid(p.data(),p.size(),version,&used)&&used==136,"legal cartridge selector limits in either wire layout");
        for(size_t short_size:{size_t{0},size_t{43},size_t{135}}){used=7;require(!om_n64_snapshot_transferpak_valid(p.data(),short_size,version,&used)&&used==7,"short record changes no cursor output");}
        for(unsigned at:{0u,4u,8u,12u,44u,48u,52u,56u}){auto bad=p;le32(bad.data()+at,0xffffffff);used=7;require(!om_n64_snapshot_transferpak_valid(bad.data(),bad.size(),version,&used)&&used==7,"forged pak/cart selector refused");}
        auto bad=p;le32(bad.data()+44+(version==0x10200?21:16),version>=0x10a00?256:2);require(!om_n64_snapshot_transferpak_valid(bad.data(),bad.size(),version,&used),"legacy Boolean and modern full-byte RTC latch bounds");
        if(limit[0]==0xfc){p[82]=6;require(om_n64_snapshot_transferpak_valid(p.data(),p.size(),version,&used),"camera register zero hardware bits retained");p[82]=8;require(!om_n64_snapshot_transferpak_valid(p.data(),p.size(),version,&used),"forged camera selector refused");}
    }
    auto p=record(0x10,127,255,0x10300);std::fill(p.begin()+64,p.begin()+136,0xff);le32(p.data()+60,1);
    size_t used=0;require(om_n64_snapshot_transferpak_valid(p.data(),p.size(),0x10300,&used),"raw RTC/time and unused camera bytes are not narrowed to preferred values");
    p[16]=0;require(om_n64_snapshot_transferpak_valid(p.data(),44,0x10300,&used)&&used==44,"absent-cart record has no GB payload");
}
int main(){try{wire_selectors();high_banks_and_rumble();camera_bounds_and_failed_admission();packet_and_storage_bounds();std::cout<<"N64 GB Transfer Pak cases passed\n";return 0;}catch(const std::exception& error){std::cerr<<error.what()<<'\n';return 1;}}
