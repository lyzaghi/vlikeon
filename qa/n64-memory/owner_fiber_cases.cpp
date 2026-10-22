// Actual Windows fiber adapter + actual owner/memory file code; item 16 only.
#include "memory_files.hpp"
#include "n64-core-owner.h"
#include "n64-memory-io.h"
#include "n64-save-files.h"
extern "C" {
#include <libco.h>
int om_n64_co_retire_thread(void);
}
#ifdef thread_local
#undef thread_local
#endif
#include <array>
#include <cstdlib>
#include <cstring>
#include <iostream>
#include <stdexcept>
static cothread_t root_fiber,guest_fiber;static unsigned steps;
static void must(bool value){if(!value)std::_Exit(2);}
static void guest_entry(){
    must(co_active()==guest_fiber&&om_n64_owner_boundary(guest_fiber,1));co_switch(root_fiber);
    void* data=nullptr;size_t bytes=0;
    must(om_n64_owner_action(guest_fiber,&data,&bytes)==OM_N64_SAVE&&data&&bytes==4);
    must(om_n64_save_write("save/N64/fiber.ram",data,bytes)==OM_N64_SAVE_OK);
    must(om_n64_owner_snapshot_done(guest_fiber,1));co_switch(root_fiber);
    must(om_n64_owner_action(guest_fiber,&data,&bytes)==OM_N64_RUN);++steps;
    must(om_n64_owner_boundary(guest_fiber,0));co_switch(root_fiber);
    must(om_n64_owner_action(guest_fiber,&data,&bytes)==OM_N64_STOP&&om_n64_owner_ended(guest_fiber,1));co_switch(root_fiber);
    std::_Exit(3);
}
static void require(bool okay,const char* why){if(!okay)throw std::runtime_error(why);}
int main(){try{
    omni::MemoryFiles store;const auto api=store.interface();require(om_n64_files_install(&api)&&om_n64_owner_begin(),"actual store/owner");
    root_fiber=co_active();guest_fiber=co_create(1024*1024,guest_entry);require(root_fiber&&guest_fiber&&om_n64_owner_bind(root_fiber,guest_fiber)&&om_n64_owner_loaded(root_fiber),"actual owned Windows root and guest");
    require(om_n64_owner_enter(root_fiber,OM_N64_PREPARE,nullptr,0),"begin preparation");co_switch(guest_fiber);
    require(om_n64_owner_at_boundary(root_fiber)&&om_n64_owner_finish(root_fiber)&&om_n64_owner_ready(root_fiber)&&steps==0,"real first boundary precedes guest run");
    std::array<char,4> payload{{'s','a','v','e'}};require(om_n64_owner_enter(root_fiber,OM_N64_SAVE,payload.data(),payload.size()),"borrow snapshot payload");co_switch(guest_fiber);
    require(om_n64_owner_finish(root_fiber)&&om_n64_files_writes_settled()&&steps==0,"actual snapshot I/O completes without running guest step");
    std::array<char,4> observed{};require(om_n64_save_read("save/N64/fiber.ram",observed.data(),observed.size())==0&&observed==payload,"actual memory save bytes");
    require(om_n64_owner_enter(root_fiber,OM_N64_RUN,nullptr,0),"run owned guest");co_switch(guest_fiber);require(om_n64_owner_finish(root_fiber)&&steps==1,"one actual guest step then checked boundary");
    require(om_n64_owner_enter(root_fiber,OM_N64_STOP,nullptr,0),"request stop");co_switch(guest_fiber);require(om_n64_owner_finish(root_fiber)&&om_n64_owner_stopped(root_fiber),"guest stop before fiber deletion");
    co_delete(guest_fiber);guest_fiber=nullptr;require(om_n64_co_retire_thread()&&om_n64_owner_end(root_fiber)&&om_n64_owner_quiescent()&&om_n64_files_install(nullptr),"actual guest/root conversion and callback ownership retire");
    std::cout<<"N64 owner with actual Windows fibers passed\n";return 0;
}catch(const std::exception& error){std::cerr<<error.what()<<'\n';return 1;}}
