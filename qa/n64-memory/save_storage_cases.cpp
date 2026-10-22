// Authored only; actual prepared file_storage/util, bridge and store. Item 16.
#include "memory_files.hpp"
#include "n64-memory-io.h"
#include "n64-save-files.h"
extern "C" {
#include "backends/file_storage.h"
#include "backends/api/storage_backend.h"
#include "main/util.h"
#include "libretro_private.h"
}
#include <array>
#include <cerrno>
#include <cstdlib>
#include <cstring>
#include <iostream>
#include <limits>
#include <stdexcept>
#include <string>
#include <vector>

retro_log_printf_t log_cb=nullptr;
static void require(bool value,const char* message){if(!value)throw std::runtime_error(message);}
static OmniWritableFilesV1 original{};
static int deny_read(void*,std::uint64_t,std::uint64_t,void*,std::uint64_t,std::uint64_t* count){*count=0;return OMNI_FILES_INTERNAL;}
static int deny_flush(void*,std::uint64_t){return OMNI_FILES_INTERNAL;}
static int deny_rename(void*,const char*,const char*,std::uint32_t){return OMNI_FILES_NO_SPACE;}
static int unknown_close(void*,std::uint64_t){return OMNI_FILES_INTERNAL;}
static int short_write(void*,std::uint64_t handle,std::uint64_t offset,const void* data,std::uint64_t bytes,std::uint64_t* count){return original.write(original.context,handle,offset,data,bytes?bytes-1:0,count);}
static char* owned_name(const char* name){const auto size=std::strlen(name)+1;auto* value=static_cast<char*>(std::malloc(size));require(value!=nullptr,"owned name allocation");std::memcpy(value,name,size);return value;}
struct Case {
    omni::MemoryFiles store;OmniWritableFilesV1 api=store.interface();
    Case(){require(om_n64_files_install(&api)==1,"install actual save table");}
    ~Case(){om_n64_files_install(nullptr);}
    void raw(const char* path,const std::string& bytes){
        const std::string name(path);require(api.mkdir(api.context,name.substr(0,name.rfind('/')).c_str(),1)==0,"fixture parents");
        std::uint64_t handle=0,actual=0;
        require(api.open(api.context,path,OMNI_FILES_WRITE|OMNI_FILES_CREATE|OMNI_FILES_TRUNCATE,&handle)==0,"fixture open");
        require(api.write(api.context,handle,0,bytes.data(),bytes.size(),&actual)==0&&actual==bytes.size()&&api.close(api.context,handle)==0,"fixture complete image");
    }
    std::string bytes(const char* path){
        OmniFilesStatV1 shape{};require(api.stat(api.context,path,&shape)==0&&shape.bytes<1024*1024,"small expected fixture");
        std::string value(static_cast<std::size_t>(shape.bytes),'\0');std::uint64_t handle=0,count=0;
        require(api.open(api.context,path,OMNI_FILES_READ,&handle)==0&&api.read(api.context,handle,0,value.data(),value.size(),&count)==0&&count==value.size()&&api.close(api.context,handle)==0,"observe original store bytes");return value;
    }
    void clean(){OmniFilesUsageV1 usage{};require(api.usage(api.context,&usage)==0&&usage.open_handles==0,"every resolved handle retired");}
};
static void namespace_exact_read_and_outputs(){
    Case value;const char* path="save/N64/tpak/fixture.ram";value.raw(path,"01234567");
    require(!om_n64_fopen(path,"wb")&&!om_n64_save_fopen("scratch/cache","wb"),"separate save and scratch authority");
    for(const char* invalid:{"../escape","save/../escape","save/N64/CON","C:/outside","save/N64/name:stream"})require(!om_n64_save_fopen(invalid,"wb"),"invalid save destinations refused");
    std::array<char,8> output{};output.fill('x');const auto prior=output;
    require(om_n64_save_read(path,output.data(),7)==OM_N64_SAVE_SIZE&&output==prior,"wrong size cannot mutate output");
    auto faults=value.api;faults.read=deny_read;require(om_n64_files_install(&faults)==1,"read fault table");
    require(om_n64_save_read(path,output.data(),output.size())==OM_N64_SAVE_READ&&output==prior,"read fault preserves caller bytes");
    void* loaded=output.data();size_t length=123;
    require(om_n64_save_load(path,&loaded,&length)==OM_N64_SAVE_READ&&loaded==output.data()&&length==123,"load outputs commit together only after complete read/close");
    require(om_n64_files_install(&value.api)==1,"restore healthy reader");
    require(om_n64_save_read(path,output.data(),output.size())==0&&std::string(output.data(),output.size())=="01234567","exact persistent bytes");
    require(om_n64_save_load(path,&loaded,&length)==0&&length==8&&std::memcmp(loaded,"01234567",8)==0,"owned full image");std::free(loaded);
    value.clean();
}
static void publication_faults_and_checked_utility(){
    Case value;const char* path="save/N64/tpak/fixture.ram";value.raw(path,"abcdefgh");original=value.api;
    auto faults=value.api;faults.write=short_write;require(om_n64_files_install(&faults)==1,"short-write table");
    require(write_to_file(path,"12345678",8)==file_write_error&&value.bytes(path)=="abcdefgh","actual utility propagates short write and retains old name");
    faults=value.api;faults.flush=deny_flush;require(om_n64_files_install(&faults)==1,"flush fault table");
    require(om_n64_save_write(path,"12345678",8)==OM_N64_SAVE_WRITE&&value.bytes(path)=="abcdefgh","flush is part of publication acknowledgement");
    faults=value.api;faults.rename=deny_rename;require(om_n64_files_install(&faults)==1,"rename fault table");
    require(write_chunk_to_file(path,"ZZ",2,3)==file_write_error&&value.bytes(path)=="abcdefgh","failed patch publication preserves entire previous image");
    faults=value.api;faults.read=deny_read;require(om_n64_files_install(&faults)==1,"update-copy read fault");
    require(om_n64_save_patch(path,"ZZ",2,3)!=0&&value.bytes(path)=="abcdefgh","failed update read cannot fall back to destructive wb");
    require(om_n64_files_install(&value.api)==1,"healthy publication");
    require(write_chunk_to_file(path,"ZZ",2,3)==file_ok&&value.bytes(path)=="abcZZfgh","actual checked utility patch");
    require(om_n64_save_patch(path,"x",1,OM_N64_SAVE_MAX_BYTES)!=0&&value.bytes(path)=="abcZZfgh","overflow end rejected before mutation");
    require(write_to_file("C:/must-never-open","x",1)==file_open_error,"utility has no OS write fallback");
    require(write_chunk_to_file("save/N64/new.ram","ab",2,3)==file_ok&&value.bytes("save/N64/new.ram")==std::string("\0\0\0ab",5),"literal absence permits sparse new save");
    value.clean();
}
static void actual_storage_read_admission_and_dirty_retry(){
    Case value;const char* path="save/N64/tpak/cart.ram";file_storage storage{};
    require(open_file_storage(&storage,8,owned_name(path))==file_ok&&storage.data&&storage.size==8&&!storage.loaded_existing&&!storage.write_blocked,"literal absence admits fully initialized default RAM");
    for(unsigned i=0;i<8;++i)require(storage.data[i]==0,"default RAM bytes initialized");
    storage.data[1]=11;auto faults=value.api;faults.rename=deny_rename;
    require(om_n64_files_install(&faults)==1,"failed first publication");g_ifile_storage.save(&storage,1,1);
    require(storage.first_access&&storage.write_failed,"first-access dirty state retained after failure");
    require(om_n64_files_install(&value.api)==1,"retry healthy");storage.data[6]=22;g_ifile_storage.save(&storage,6,1);
    require(!storage.first_access&&!storage.write_failed&&value.bytes(path)==std::string("\0\x0b\0\0\0\0\x16\0",8),"full retry retains both old and new dirty ranges");
    faults=value.api;faults.rename=deny_rename;require(om_n64_files_install(&faults)==1,"failed later chunk");storage.data[2]=33;g_ifile_storage.save(&storage,2,1);require(storage.write_failed,"later dirty chunk retained");
    require(om_n64_files_install(&value.api)==1,"later retry healthy");storage.data[7]=44;g_ifile_storage.save(&storage,7,1);
    const auto committed=value.bytes(path);require(static_cast<unsigned char>(committed[2])==33&&static_cast<unsigned char>(committed[7])==44&&!storage.write_failed,"later successful save retries all guest dirty bytes");
    const auto retained=value.bytes(path);g_ifile_storage.save(&storage,std::numeric_limits<size_t>::max(),1);require(storage.write_failed&&value.bytes(path)==retained,"storage range overflow refused");
    file_storage child{};child.data=storage.data+4;child.size=4;child.offset=4;child.filename=reinterpret_cast<const char*>(&storage);
    g_isubfile_storage.save(&child,4,1);require(value.bytes(path)==retained,"subfile range cannot cross parent bounds");
    close_file_storage(&storage);
    faults=value.api;faults.read=deny_read;require(om_n64_files_install(&faults)==1,"initial read failure");
    require(open_file_storage(&storage,8,owned_name(path))==file_read_error&&!storage.data&&storage.size==0&&storage.write_blocked,"bad read exposes no partial/default RAM for guest overwrite");
    g_ifile_storage.save(&storage,0,0);require(value.bytes(path)==retained,"rejected storage cannot publish");close_file_storage(&storage);
    require(om_n64_files_install(&value.api)==1,"restore initial reader");
    require(open_file_storage(&storage,7,owned_name(path))==file_size_error&&!storage.data&&storage.write_blocked,"truncated or oversized existing save refuses guest admission");close_file_storage(&storage);
    value.clean();
}
static void actual_save_quota_is_separate_from_scratch(){
    Case value;const char* path="save/N64/tpak/quota.ram";value.raw(path,"old");
    std::uint64_t handle=0;require(value.api.open(value.api.context,"save/N64/filler",OMNI_FILES_WRITE|OMNI_FILES_CREATE,&handle)==0,"save quota filler");
    require(value.api.resize(value.api.context,handle,256ull*1024*1024)==0&&value.api.close(value.api.context,handle)==0,"one full logical save file");
    require(value.api.open(value.api.context,"save/N64/filler2",OMNI_FILES_WRITE|OMNI_FILES_CREATE,&handle)==0,"remaining save quota filler");
    require(value.api.resize(value.api.context,handle,16ull*1024*1024-4096)==0&&value.api.close(value.api.context,handle)==0,"actual page budget exhausted");
    require(om_n64_save_write(path,"new",3)!=0&&value.bytes(path)=="old","charged old/new save images enforce real 272-MiB quota");
    auto* scratch=om_n64_fopen("scratch/available","wb");require(scratch&&om_n64_fwrite("ok",1,2,scratch)==2&&om_n64_fclose(scratch)==0,"save exhaustion does not consume scratch budget");
    require(value.api.remove(value.api.context,"save/N64/filler2")==0&&om_n64_save_write(path,"new",3)==0&&value.bytes(path)=="new","release then retry preserves publication semantics");value.clean();
}
static void uncertain_read_close_retains_outputs_and_context(){
    // Last: there is intentionally no in-process reset for unknown close.
    Case value;const char* path="save/N64/tpak/uncertain.ram";value.raw(path,"retained");
    auto faults=value.api;faults.close=unknown_close;require(om_n64_files_install(&faults)==1,"uncertain-close callback");
    std::array<char,8> output{};output.fill('x');const auto prior=output;
    require(om_n64_save_read(path,output.data(),output.size())==OM_N64_SAVE_READ&&output==prior,"successful read with unknown close cannot commit caller outputs");
    require(!om_n64_files_install(nullptr)&&!om_n64_save_fopen(path,"wb"),"unconfirmed callback context retained until actor exit");
    OmniFilesUsageV1 usage{};require(value.api.usage(value.api.context,&usage)==0&&usage.open_handles==1&&value.bytes(path)=="retained","real unknown handle debt and prior data retained");
}
int main(){try{
    namespace_exact_read_and_outputs();publication_faults_and_checked_utility();actual_storage_read_admission_and_dirty_retry();
    actual_save_quota_is_separate_from_scratch();uncertain_read_close_retains_outputs_and_context();
    std::cout<<"N64 persistent file-storage cases passed\n";return 0;
}catch(const std::exception& error){std::cerr<<error.what()<<'\n';return 1;}}
