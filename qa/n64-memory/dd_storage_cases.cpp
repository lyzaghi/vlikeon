// Authored only. Actual prepared DD format/geometry/loader + actual store.
#include "memory_files.hpp"
#include "n64-memory-io.h"
#include "n64-dd-storage.h"
extern "C" {
#include "backends/file_storage.h"
#include "backends/api/storage_backend.h"
#include "device/dd/disk.h"
#include "api/m64p_types.h"
#include "main/util.h"
#include <libretro.h>
void om_n64_dd_case_reject_allocation(int);
}
#include <algorithm>
#include <array>
#include <cstdlib>
#include <cstring>
#include <iostream>
#include <limits>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>
extern "C" {retro_log_printf_t log_cb=nullptr;}
static void require(bool okay,const char* why){if(!okay)throw std::runtime_error(why);}
static void be16(std::vector<std::uint8_t>& bytes,size_t at,unsigned value){bytes.at(at)=static_cast<std::uint8_t>(value>>8);bytes.at(at+1)=static_cast<std::uint8_t>(value);}
static void be32(std::vector<std::uint8_t>& bytes,size_t at,std::uint32_t value){for(unsigned i=0;i<4;++i)bytes.at(at+i)=static_cast<std::uint8_t>(value>>(24-i*8));}
static std::vector<std::uint8_t> d64(unsigned type=5){
    // Independent wire fixture: one 232*85-byte ROM LBA, no stored RAM.
    std::vector<std::uint8_t> bytes(0x200+19720,0);
    bytes[5]=static_cast<std::uint8_t>(type);be16(bytes,6,1);be32(bytes,28,0x80000400);
    be16(bytes,224,0);be16(bytes,226,0xffff);be16(bytes,228,0xffff);
    std::memcpy(bytes.data()+0x100,"OMNIPLAY SYNTHETIC DISK ID",25);bytes[0x200]=0x57;return bytes;
}
struct Layout {size_t length,system=19,id=23,ram=29,ram_bytes=31;unsigned format=7,development=9;};
static std::uint8_t* scan(std::uint8_t* data,Layout& value){return scan_and_expand_disk_format(data,&value.length,&value.format,&value.development,&value.system,&value.id,&value.ram,&value.ram_bytes);}
static std::uint8_t* allocated(const std::vector<std::uint8_t>& bytes){auto* data=static_cast<std::uint8_t*>(std::malloc(bytes.empty()?1:bytes.size()));require(data!=nullptr,"fixture allocation");if(!bytes.empty())std::memcpy(data,bytes.data(),bytes.size());return data;}
static void rejected_unchanged(const std::vector<std::uint8_t>& bytes){
    auto* data=allocated(bytes);Layout value{bytes.size()},before=value;
    require(!scan(data,value),"malformed disk refused");
    require(value.length==before.length&&value.system==before.system&&value.id==before.id&&value.ram==before.ram&&value.ram_bytes==before.ram_bytes&&value.format==before.format&&value.development==before.development,"rejected layout outputs unchanged");
    require(bytes.empty()||std::memcmp(data,bytes.data(),bytes.size())==0,"rejected original image remains owned and unchanged");std::free(data);
}
static void bounded_formats_expansion_and_oom(){
    for(size_t size=0;size<0x200;++size)rejected_unchanged(std::vector<std::uint8_t>(size,0));
    auto bytes=d64();auto* data=allocated(bytes);Layout value{bytes.size()};
    om_n64_dd_case_reject_allocation(1);require(!scan(data,value)&&value.length==bytes.size()&&std::memcmp(data,bytes.data(),bytes.size())==0,"actual expansion allocation failure retains input and length");om_n64_dd_case_reject_allocation(0);
    auto* expanded=scan(data,value);require(expanded&&value.format==DISK_FORMAT_D64&&value.development==1&&value.system==0&&value.id==0x100&&value.ram==0x200+19720&&value.ram_bytes==0x1da240&&value.length==0x200+19720+0x1da240,"independent type-5 D64 expansion layout");
    require(expanded[4]==0x10&&expanded[5]==0x15&&expanded[0x200]==0x57&&std::all_of(expanded+value.ram,expanded+value.length,[](std::uint8_t byte){return byte==0;}),"actual expanded buffer initialized and data retained");
    const auto* sys=reinterpret_cast<const dd_sys_data*>(expanded);
    require(big16(sys->ram_lba_start)==0x1010-24&&big16(sys->ram_lba_end)==0x10db-24,"normalized complete RAM descriptor");std::free(expanded);
    for(unsigned kind=0;kind<8;++kind){auto bad=d64();switch(kind){case 0:bad[5]=7;break;case 1:be16(bad,224,0xffff);break;case 2:be16(bad,228,0);break;case 3:be16(bad,226,1);be16(bad,228,0);break;case 4:bad[8]=193;break;case 5:bad[8]=1;bad[9]=0;break;case 6:be32(bad,28,0x7fffffff);break;case 7:bad.push_back(0);break;}rejected_unchanged(bad);}
    auto no_ram=d64(6);data=allocated(no_ram);value=Layout{no_ram.size()};expanded=scan(data,value);require(expanded&&value.length==no_ram.size()&&value.ram_bytes==0,"type-6 no-RAM D64 retains exact extent");std::free(expanded);
}
static std::vector<std::uint8_t> repeated_image(bool sdk,unsigned system_block,bool development){
    std::vector<std::uint8_t> bytes(sdk?SDK_FORMAT_DUMP_SIZE:MAME_FORMAT_DUMP_SIZE,0);
    auto header=d64(5);be32(header,0,DD_REGION_JP);
    const size_t stride=development?192:232,offset=system_block*19720;
    for(unsigned sector=0;sector<85;++sector)std::memcpy(bytes.data()+offset+sector*stride,header.data(),stride);
    for(unsigned sector=0;sector<85;++sector)std::memcpy(bytes.data()+14*19720+sector*232,header.data()+0x100,32);
    return bytes;
}
static void physical_backup_blocks_and_sdk_layout(){
    for(const auto choice:{std::array<unsigned,3>{0,8,0},std::array<unsigned,3>{0,10,1},std::array<unsigned,3>{1,8,0}}){
        auto bytes=repeated_image(choice[0]!=0,choice[1],choice[2]!=0);auto* data=allocated(bytes);Layout layout{bytes.size()};auto* parsed=scan(data,layout);
        require(parsed==data&&layout.length==bytes.size()&&layout.system==choice[1]*19720&&layout.id==14*19720&&layout.development==choice[2],"backup system block is physical block number, including development block 10");
        if(choice[0])require(layout.format==DISK_FORMAT_SDK&&layout.ram_bytes==0x1da240&&layout.ram+layout.ram_bytes==SDK_FORMAT_DUMP_SIZE,"independent SDK RAM tail extent");
        else require(layout.format==DISK_FORMAT_MAME&&layout.ram==0&&layout.ram_bytes==0,"physical MAME representation remains full image");
        std::free(parsed);
    }
}
static void geometry_golden_bounds_and_staged_table(){
    auto bytes=d64();auto* data=allocated(bytes);Layout shape{bytes.size()};data=scan(data,shape);require(data,"geometry fixture");
    file_storage owner{};owner.data=data;owner.size=shape.length;dd_disk disk{};disk.storage=&owner;disk.istorage=&g_ifile_storage_ro;disk.format=static_cast<std::uint8_t>(shape.format);disk.development=1;disk.offset_sys=shape.system;disk.offset_id=shape.id;disk.offset_ram=shape.ram;
    require(GenerateLBAToPhysTable(&disk)==1,"complete actual LBA table");
    require(disk.lba_phys_table[0]==0&&disk.lba_phys_table[1]==0x2000&&disk.lba_phys_table[2]==0x2001&&disk.lba_phys_table[3]==1&&disk.lba_phys_table[0x86e]==0x3418,"independent first-block and reverse-head row goldens");
    for(unsigned lba=0;lba<SIZE_LBA;++lba){const auto code=disk.lba_phys_table[lba];require(PhysToLBA(&disk,(code>>12)&1,code&0xfff,(code>>13)&1)==lba,"both heads roundtrip every admitted LBA");}
    auto code=disk.lba_phys_table[24];require(get_sector_base(&disk,(code>>12)&1,code&0xfff,(code>>13)&1,0,232)==data+0x200,"exact first D64 ROM sector");
    require(!get_sector_base(&disk,2,0,0,0,1)&&!get_sector_base(&disk,0,4095,0,0,1)&&!get_sector_base(&disk,0,0,2,0,1)&&!get_sector_base(&disk,0,0,0,85,1)&&!get_sector_base(&disk,0,0,0,0,257),"physical coordinates and byte requests bounded");
    require(LBAToVZoneA(7,0)==UINT32_MAX&&LBAToByteA(5,SIZE_LBA,1)==UINT32_MAX&&LBAToByteA(5,0,UINT32_MAX)==UINT32_MAX,"type/LBA/count overflow rejected");
    std::array<std::uint16_t,SIZE_LBA> before{};std::copy_n(disk.lba_phys_table,SIZE_LBA,before.begin());data[8]=193;
    require(!GenerateLBAToPhysTable(&disk)&&std::equal(before.begin(),before.end(),disk.lba_phys_table),"invalid system cannot partially replace an old table");std::free(data);
}
static int deny_rename(void*,const char*,const char*,std::uint32_t){return OMNI_FILES_NO_SPACE;}
struct Case {
    omni::MemoryFiles store;OmniWritableFilesV1 api=store.interface();
    Case(){require(om_n64_dd_quiescent()&&om_n64_files_install(&api)==1,"fresh actual DD/store ownership");}
    ~Case(){om_n64_files_install(nullptr);}
    void raw(const std::string& path,const std::vector<std::uint8_t>& bytes){
        require(api.mkdir(api.context,path.substr(0,path.rfind('/')).c_str(),1)==0,"fixture ancestors");std::uint64_t handle=0;
        require(api.open(api.context,path.c_str(),OMNI_FILES_WRITE|OMNI_FILES_CREATE|OMNI_FILES_TRUNCATE,&handle)==0,"raw fixture writer");
        for(size_t at=0;at<bytes.size();){auto count=std::min<size_t>(OMNI_FILES_MAX_TRANSFER,bytes.size()-at);std::uint64_t actual=0;require(api.write(api.context,handle,at,bytes.data()+at,count,&actual)==0&&actual==count,"raw fixture bytes");at+=count;}
        require(api.close(api.context,handle)==0,"raw fixture close");
    }
    std::vector<std::uint8_t> bytes(const char* path){
        OmniFilesStatV1 shape{};require(api.stat(api.context,path,&shape)==0&&shape.bytes<=80ull*1024*1024,"bounded observation");std::vector<std::uint8_t> value(static_cast<size_t>(shape.bytes));std::uint64_t handle=0;
        require(api.open(api.context,path,OMNI_FILES_READ,&handle)==0,"observation open");
        for(size_t at=0;at<value.size();){auto count=std::min<size_t>(OMNI_FILES_MAX_TRANSFER,value.size()-at);std::uint64_t actual=0;require(api.read(api.context,handle,at,value.data()+at,count,&actual)==0&&actual==count,"observation complete");at+=count;}
        require(api.close(api.context,handle)==0,"observation close");return value;
    }
    void clean(){OmniFilesUsageV1 usage{};require(api.usage(api.context,&usage)==0&&usage.open_handles==0&&om_n64_dd_quiescent(),"no resolved handle or live descriptor retained");}
};
static void retained_save_legacy_precedence_and_no_bad_fallback(){
    Case value;const std::string source="save/fixtures/disk.d64";const auto original=d64();value.raw(source,original);
    std::vector<std::uint8_t> legacy(0x1da240,0x5a);value.raw(source+".ram",legacy);
    dd_disk disk{};const storage_backend_interface* interface=nullptr;
    require(om_n64_dd_open(&disk,&interface,source.c_str(),1)==1&&interface==&g_istorage_disk_ram_only,"actual readonly legacy RAM import");
    auto* writable=static_cast<file_storage*>(disk.save_storage);const std::string destination=writable->filename;
    require(destination.starts_with("save/N64/dd/")&&writable->size==legacy.size()&&value.bytes(destination.c_str())==legacy&&value.bytes(source.c_str())==original,"import publishes only memory save and retains original source bytes");
    auto* full=static_cast<file_storage*>(disk.storage);full->data[disk.offset_ram-1]=0x99;full->data[disk.offset_ram]=0x12;full->data[disk.offset_ram+1]=0x34;
    interface->save(&disk,disk.offset_ram-1,3);legacy[0]=0x12;legacy[1]=0x34;
    require(value.bytes(destination.c_str())==legacy,"overlapping full-image write persists precisely its RAM intersection");
    require(om_n64_dd_close(&disk)==1,"retire imported disk");
    value.raw(source+".ram",std::vector<std::uint8_t>(legacy.size(),0x6b));
    require(om_n64_dd_open(&disk,&interface,source.c_str(),1)==1&&static_cast<file_storage*>(disk.save_storage)->data[0]==0x12,"retained memory save wins over changed old import");require(om_n64_dd_close(&disk)==1,"retire retained disk");
    value.raw(destination,std::vector<std::uint8_t>{'b','a','d'});const auto bad=value.bytes(destination.c_str());interface=&g_istorage_disk_read_only;
    require(om_n64_dd_open(&disk,&interface,source.c_str(),1)==0&&!disk.storage&&interface==&g_istorage_disk_read_only&&value.bytes(destination.c_str())==bad,"bad retained save cannot fall back to base/legacy and overwrite progress");
    value.clean();
}
static void full_dirty_close_retry_and_unstarted_discard(){
    Case value;const std::string source="save/fixtures/full.d64";value.raw(source,d64());dd_disk disk{};const storage_backend_interface* interface=nullptr;
    require(om_n64_dd_open(&disk,&interface,source.c_str(),0)==1&&om_n64_dd_flush_all()==1,"checked full image publication");
    auto* storage=static_cast<file_storage*>(disk.save_storage);const std::string destination=storage->filename;const auto retained=value.bytes(destination.c_str());storage->data[disk.offset_ram]=0x77;
    auto fault=value.api;fault.rename=deny_rename;require(om_n64_files_install(&fault)==1,"DD publication fault");
    require(om_n64_dd_flush_all()==0&&om_n64_dd_close(&disk)==0&&disk.storage&&!om_n64_dd_quiescent()&&value.bytes(destination.c_str())==retained,"failed close retains descriptor, live RAM and old published image");
    require(om_n64_files_install(&value.api)==1&&om_n64_dd_close(&disk)==1&&om_n64_dd_quiescent(),"same owned descriptor can retry and retire");
    require(value.bytes(destination.c_str()).at(0x200+19720)==0x77,"retry publishes retained dirty full image");
    auto malformed=std::vector<std::uint8_t>{'b','a','d'};value.raw(destination,malformed);interface=nullptr;
    require(!om_n64_dd_open(&disk,&interface,source.c_str(),0)&&!disk.storage&&!interface&&value.bytes(destination.c_str())==malformed,"bad full save cannot silently start from base");
    const std::string other="save/fixtures/unstarted.d64";auto other_bytes=d64();other_bytes[0x201]=1;value.raw(other,other_bytes);
    require(om_n64_dd_open(&disk,&interface,other.c_str(),0)==1,"unstarted descriptor");const std::string unused=static_cast<file_storage*>(disk.save_storage)->filename;
    om_n64_dd_discard_unstarted(&disk);OmniFilesStatV1 shape{};require(value.api.stat(value.api.context,unused.c_str(),&shape)==OMNI_FILES_NOT_FOUND,"rejected IPL cleanup cannot create a default save");value.clean();
}
static void bounded_descriptor_ownership_and_readonly(){
    Case value;auto source=d64(6);value.raw("save/fixtures/one",source);source[0x201]=2;value.raw("save/fixtures/two",source);source[0x201]=3;value.raw("save/fixtures/three",source);
    dd_disk first{},second{},third{};const storage_backend_interface* interface=nullptr;
    require(om_n64_dd_open(&first,&interface,"save/fixtures/one",0)==1,"first owned DD");
    require(!om_n64_dd_open(&second,&interface,"save/fixtures/one",0),"two mutable owners cannot publish to the same save identity");
    require(om_n64_dd_open(&second,&interface,"save/fixtures/two",0)==1&&!om_n64_dd_open(&third,&interface,"save/fixtures/three",0),"two-descriptor old/new bound");
    require(om_n64_dd_close(&first)==1&&om_n64_dd_close(&second)==1,"retire both owned disks");
    require(om_n64_dd_open(&third,&interface,"save/fixtures/three",-1)==1&&!third.save_storage&&interface==&g_istorage_disk_read_only&&om_n64_dd_close(&third)==1,"explicit readonly mode has no persistent writer");value.clean();
}
static void actual_snapshot_disk_id_bounds(){
    Case value;const auto bytes=d64(6);value.raw("save/fixtures/small-id.d64",bytes);
    dd_disk disk{};const storage_backend_interface* interface=nullptr;std::uint32_t id=7,expected=0;
    std::memcpy(&expected,bytes.data()+0x100,4);
    require(bytes.size()<0x43670&&om_n64_dd_open(&disk,&interface,"save/fixtures/small-id.d64",-1)==1,"small no-RAM D64 admitted");
    require(om_n64_dd_snapshot_id(&disk,&id)==1&&id==expected,"actual descriptor uses D64 ID instead of outside physical-block offset");
    const auto offset=disk.offset_id;disk.offset_id=SIZE_MAX;id=7;
    require(!om_n64_dd_snapshot_id(&disk,&id)&&id==7,"actual ID overflow refuses without changing output");
    disk.offset_id=bytes.size()-3;require(!om_n64_dd_snapshot_id(&disk,&id)&&id==7,"actual short ID refuses");disk.offset_id=offset;
    auto copied=std::make_unique<dd_disk>(disk);require(!om_n64_dd_snapshot_id(copied.get(),&id),"unregistered descriptor cannot borrow source ownership");
    require(om_n64_dd_close(&disk)==1&&!om_n64_dd_snapshot_id(&disk,&id),"retired descriptor cannot supply snapshot ID");value.clean();
}
int main(){try{
    bounded_formats_expansion_and_oom();physical_backup_blocks_and_sdk_layout();geometry_golden_bounds_and_staged_table();
    retained_save_legacy_precedence_and_no_bad_fallback();full_dirty_close_retry_and_unstarted_discard();bounded_descriptor_ownership_and_readonly();
    actual_snapshot_disk_id_bounds();
    std::cout<<"N64 DD storage cases passed\n";return 0;
}catch(const std::exception& error){std::cerr<<error.what()<<'\n';return 1;}}
