// Actual C++11 owner, actual prepared peripheral helpers and real store.
// Opaque tokens exercise protocol admission; they do not simulate OS fibers.
#include "memory_files.hpp"
#include "n64-core-owner.h"
#include "n64-memory-io.h"
#include "n64-save-files.h"
extern "C" {
#include "backends/file_storage.h"
#include "backends/api/storage_backend.h"
#include <libretro.h>
struct file_storage* om_n64_case_ram(unsigned int);
}
#include <array>
#include <cstdlib>
#include <cstring>
#include <iostream>
#include <stdexcept>
#include <string>
#include <thread>
extern "C" {retro_log_printf_t log_cb=nullptr;}
static void require(bool value,const char* why){if(!value)throw std::runtime_error(why);}
static OmniWritableFilesV1 underlying{};static bool reject_rename=false;
static int rename_checked(void*,const char* from,const char* to,std::uint32_t replace){return reject_rename?OMNI_FILES_NO_SPACE:underlying.rename(underlying.context,from,to,replace);}
static char* owned(const char* text){const auto bytes=std::strlen(text)+1;auto* copy=static_cast<char*>(std::malloc(bytes));require(copy,"fixture name");std::memcpy(copy,text,bytes);return copy;}
struct Case {
    omni::MemoryFiles store;OmniWritableFilesV1 api=store.interface();int root_token=0,guest_token=0;
    void* root=&root_token;void* guest=&guest_token;
    Case(){
        require(om_n64_owner_quiescent()&&om_n64_native_peripherals_quiet(),"no prior owner or producer debt");underlying=api;reject_rename=false;
        auto proxy=api;proxy.rename=rename_checked;require(om_n64_files_install(&proxy)==1&&om_n64_owner_begin()==1,"real store and actual owner");
        require(om_n64_owner_bind(root,guest)==1&&om_n64_owner_loaded(root)==1,"exact root/guest bind");
    }
    ~Case(){om_n64_files_install(nullptr);}
    void prepared(){require(om_n64_owner_enter(root,OM_N64_PREPARE,nullptr,0)==1,"enter real preparation transaction");require(!om_n64_owner_ready(root)&&!om_n64_owner_at_boundary(root),"guest in flight cannot be ready");require(om_n64_owner_boundary(guest,1)==1&&om_n64_owner_at_boundary(root)&&om_n64_owner_finish(root)==1&&om_n64_owner_ready(root),"first checked device/CPU boundary becomes ready only after finish");}
    void ended(){require(om_n64_owner_enter(root,OM_N64_STOP,nullptr,0)==1&&om_n64_owner_ended(guest,1)==1&&om_n64_owner_finish(root)==1&&om_n64_owner_stopped(root),"guest exit acknowledged separately from producer retirement");}
    void clean(){require(om_n64_native_retire_peripherals()&&om_n64_native_peripherals_quiet()&&om_n64_files_idle()&&om_n64_owner_end(root)&&om_n64_owner_quiescent(),"actual producer retirement before ending owner");}
    std::string bytes(const char* path){
        OmniFilesStatV1 shape{};require(api.stat(api.context,path,&shape)==0&&shape.bytes<=1024,"small fixture image");std::string value(static_cast<size_t>(shape.bytes),'\0');std::uint64_t handle=0,count=0;
        require(api.open(api.context,path,OMNI_FILES_READ,&handle)==0&&api.read(api.context,handle,0,value.data(),value.size(),&count)==0&&count==value.size()&&api.close(api.context,handle)==0,"observe actual committed bytes");return value;
    }
};
static void exact_owner_and_snapshot_borrow_lifetime(){
    Case value;std::array<char,8> payload{};void* observed=reinterpret_cast<void*>(1);size_t bytes=99;
    require(!om_n64_owner_enter(value.root,OM_N64_RUN,nullptr,0)&&!om_n64_owner_ready(value.root),"no early guest run or fake readiness");
    require(!om_n64_owner_begin()&&!om_n64_owner_bind(value.root,value.root),"live owner cannot be replaced");
    value.prepared();
    bool foreign_ok=false;std::thread foreign([&]{foreign_ok=!om_n64_owner_thread()&&!om_n64_owner_access()&&!om_n64_owner_ready(value.root)&&!om_n64_owner_enter(value.root,OM_N64_RUN,nullptr,0)&&!om_n64_owner_end(value.root);});foreign.join();require(foreign_ok&&om_n64_owner_ready(value.root),"foreign OS thread cannot adopt known tokens or alter healthy state");
    require(!om_n64_owner_enter(value.root,OM_N64_SAVE,nullptr,8)&&!om_n64_owner_enter(value.root,OM_N64_SAVE,payload.data(),32u*1024u*1024u+1),"missing/oversized snapshot buffer rejected before admission");
    require(om_n64_owner_enter(value.root,OM_N64_SAVE,payload.data(),payload.size())==1&&!om_n64_owner_end(value.root),"in-flight snapshot prevents owner retirement");
    require(!om_n64_owner_action(value.root,&observed,&bytes)&&observed==reinterpret_cast<void*>(1)&&bytes==99,"wrong fiber cannot borrow snapshot data");
    require(om_n64_owner_action(value.guest,&observed,&bytes)==OM_N64_SAVE&&observed==payload.data()&&bytes==payload.size(),"only exact guest receives bounded borrowed buffer");
    require(!om_n64_owner_finish(value.root)&&!om_n64_owner_at_boundary(value.root)&&!om_n64_owner_flush_begin(value.root),"early root return cannot clear an outstanding buffer or flush");
    require(om_n64_owner_snapshot_done(value.guest,0)&&om_n64_owner_at_boundary(value.root)&&om_n64_owner_finish(value.root)==0&&om_n64_owner_ready(value.root),"rejected snapshot ends borrow but permits an explicit retry");
    require(om_n64_owner_enter(value.root,OM_N64_LOAD,payload.data(),payload.size())&&om_n64_owner_snapshot_done(value.guest,1)&&om_n64_owner_finish(value.root)==1,"subsequent checked load completes");
    require(om_n64_owner_enter(value.root,OM_N64_RUN,nullptr,0)&&!om_n64_owner_boundary(value.guest,1)&&om_n64_owner_boundary(value.guest,0)&&om_n64_owner_finish(value.root)==1,"later frame boundary cannot masquerade as first initialization");
    value.ended();value.clean();
}
static void real_peripheral_flush_retry_and_readonly_snapshot(){
    Case value;auto* storage=om_n64_case_ram(0);require(storage&&open_file_storage(storage,8,owned("save/N64/tpak/owner.ram"))==0,"actual RAM backend");
    auto* writer=om_n64_fopen("scratch/owner.cache","wb");require(writer&&om_n64_fwrite("cache",1,5,writer)==5&&om_n64_fclose(writer)==0,"actual scratch cache");
    auto* reader=om_n64_fopen("scratch/owner.cache","rb");require(reader&&om_n64_files_writes_settled()&&!om_n64_files_idle(),"readonly held cache is not pending writable I/O");
    value.prepared();storage->data[1]=11;
    require(om_n64_owner_flush_begin(value.root)&&!om_n64_owner_enter(value.root,OM_N64_RUN,nullptr,0)&&!om_n64_owner_flush_begin(value.root),"flush excludes reentrant guest/snapshot/flush work");
    require(om_n64_native_flush_peripherals()==1&&om_n64_files_writes_settled()&&om_n64_owner_flush_end(value.root,1)==1,"actual prepared helper publishes RAM with readonly scratch snapshot retained");
    const auto prior=value.bytes("save/N64/tpak/owner.ram");require(prior[1]==11,"first complete RAM image");
    storage->data[2]=22;reject_rename=true;
    require(om_n64_owner_flush_begin(value.root)&&om_n64_native_flush_peripherals()==0&&om_n64_owner_flush_end(value.root,0)==0&&om_n64_owner_ready(value.root)&&value.bytes("save/N64/tpak/owner.ram")==prior,"failed memory publication cannot acknowledge a frontend capture; owner remains retryable");
    reject_rename=false;require(om_n64_owner_flush_begin(value.root)&&om_n64_native_flush_peripherals()==1&&om_n64_owner_flush_end(value.root,1)==1&&value.bytes("save/N64/tpak/owner.ram")[2]==22,"checked full retry retains earlier/current dirty ranges");
    require(om_n64_fclose(reader)==0,"release actual cache reader");value.ended();
    storage->data[3]=33;auto* retained=storage->data;reject_rename=true;
    require(!om_n64_native_retire_peripherals()&&!om_n64_native_peripherals_quiet()&&storage->data==retained&&storage->data[3]==33,"failed close retains live RAM and cannot satisfy peripheral retirement");
    reject_rename=false;value.clean();
}
static void bad_initial_read_cannot_become_ready_or_write_default(){
    Case value;const char* path="save/N64/tpak/bad.ram";require(om_n64_save_write(path,"bad",3)==0,"independent bad-size retained save");
    auto* storage=om_n64_case_ram(1);require(open_file_storage(storage,8,owned(path))!=0&&!om_n64_owner_admitted(),"actual read admission fault reaches owner");
    require(om_n64_owner_enter(value.root,OM_N64_PREPARE,nullptr,0)==0,"known bad save cannot begin preparation");
    require(value.bytes(path)=="bad"&&close_file_storage(storage)==1&&om_n64_native_peripherals_quiet(),"rejected input is never closed through a default-data writer");
    require(om_n64_owner_cancel_unstarted(value.root)&&om_n64_owner_end(value.root)&&om_n64_owner_quiescent(),"unstarted failed admission can retire without inventing guest completion");
}
static void failed_guest_stopped_save_recovery(){
    Case value;const char* path="save/N64/tpak/failed-guest.ram";
    auto* storage=om_n64_case_ram(0);require(open_file_storage(storage,8,owned(path))==0,"actual stopped-recovery RAM");
    value.prepared();storage->data[0]=41;
    require(om_n64_owner_enter(value.root,OM_N64_RUN,nullptr,0)==1,"start guest before fatal fault");
    om_n64_owner_admission_failed();
    require(!om_n64_owner_boundary(value.guest,0)&&!om_n64_owner_flush_begin(value.root),"fault cannot fake paused boundary or flush before actual guest end");
    require(om_n64_owner_ended(value.guest,0)&&om_n64_owner_at_boundary(value.root)&&!om_n64_owner_finish(value.root)&&om_n64_owner_stopped(value.root),"failed guest end releases operation but preserves failure result");
    require(!om_n64_owner_ready(value.root)&&!om_n64_owner_admitted()&&!om_n64_owner_enter(value.root,OM_N64_RUN,nullptr,0),"stopped failure cannot run again");
    require(!om_n64_owner_persistence_allowed()&&om_n64_native_flush_peripherals()==-1,"failed stopped session still needs explicit flush transaction");
    reject_rename=true;require(om_n64_owner_flush_begin(value.root)&&om_n64_owner_persistence_allowed(),"confirmed failed stop admits persistent-save recovery");
    require(!om_n64_owner_flush_begin(value.root)&&!om_n64_owner_end(value.root),"recovery flush excludes reentry and retirement");
    require(om_n64_native_flush_peripherals()==0&&om_n64_owner_flush_end(value.root,0)==0&&storage->data[0]==41,"failed post-stop publication returns retryable receipt and retains last save bytes");
    reject_rename=false;require(om_n64_owner_flush_begin(value.root)&&om_n64_native_flush_peripherals()==1&&om_n64_owner_flush_end(value.root,1)==1,"successful post-stop publication acknowledged independently from CPU failure");
    require(value.bytes(path)[0]==41&&!om_n64_owner_ready(value.root)&&!om_n64_owner_admitted(),"save receipt cannot erase session failure or restore readiness");
    storage->data[1]=42;reject_rename=true;auto* retained=storage->data;
    require(!om_n64_native_retire_peripherals()&&storage->data==retained&&!om_n64_native_peripherals_quiet(),"failed stopped retirement retains current dirty storage");
    reject_rename=false;value.clean();
}
int main(){try{exact_owner_and_snapshot_borrow_lifetime();real_peripheral_flush_retry_and_readonly_snapshot();bad_initial_read_cannot_become_ready_or_write_default();failed_guest_stopped_save_recovery();std::cout<<"N64 owner and actual peripheral cases passed\n";return 0;}catch(const std::exception& error){std::cerr<<error.what()<<'\n';return 1;}}
