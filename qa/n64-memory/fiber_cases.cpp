// Authored real-Windows fixture only. Never execute before final item 16.
#include <windows.h>
#include <libco.h>
#include <atomic>
#include <barrier>
#include <cfenv>
#include <iostream>
#include <stdexcept>
#include <thread>

extern "C" int om_n64_co_retire_thread(void);
static void require(bool okay,const char* why){if(!okay)throw std::runtime_error(why);}
struct Guest {cothread_t root=nullptr,child=nullptr;unsigned calls=0;bool identity=true,rounding=true;};
static thread_local Guest* current=nullptr;

static void guest_entry(){
  for(;;){
    auto& guest=*current;guest.identity=guest.identity&&co_active()==guest.child;
    if(guest.calls)guest.rounding=guest.rounding&&std::fegetround()==FE_TOWARDZERO;
    guest.rounding=guest.rounding&&std::fesetround(FE_TOWARDZERO)==0;
    ++guest.calls;co_switch(guest.root);
  }
}

static void roundtrip(std::barrier<>* concurrent=nullptr){
  require(!IsThreadAFiber(),"fixture starts as an ordinary Windows thread");
  const auto initial=std::fegetround();Guest guest;current=&guest;
  guest.root=co_active();require(guest.root&&IsThreadAFiber()&&GetCurrentFiber()==guest.root,"own checked root conversion");
  if(concurrent)concurrent->arrive_and_wait();
  require(co_active()==guest.root,"root identity is thread local");
  guest.child=co_create(1024u*1024u,guest_entry);require(guest.child,"own guest creation");
  require(!co_create(1024u*1024u,guest_entry),"no untracked second guest");
  require(!om_n64_co_retire_thread(),"live guest blocks root retirement");
  co_delete(guest.root);require(co_active()==guest.root,"delete cannot adopt the root as a guest");
  require(std::fesetround(FE_DOWNWARD)==0,"set root floating state");
  co_switch(guest.child);require(guest.calls==1&&guest.identity&&std::fegetround()==FE_DOWNWARD,"guest return preserves root identity and floating state");
  co_switch(guest.child);require(guest.calls==2&&guest.identity&&guest.rounding&&std::fegetround()==FE_DOWNWARD,"repeated switches retain independent floating state");
  co_delete(guest.child);guest.child=nullptr;
  require(om_n64_co_retire_thread()==1&&!IsThreadAFiber(),"joined guest permits return to ordinary thread");
  require(om_n64_co_retire_thread()==1,"never recreate a root while confirming its retirement");
  require(std::fesetround(initial)==0,"restore fixture floating state");current=nullptr;
}

static void reject_foreign_root(){
  require(!IsThreadAFiber(),"foreign-root fixture starts as a thread");
  const auto root=ConvertThreadToFiberEx(nullptr,FIBER_FLAG_FLOAT_SWITCH);require(root,"fixture-owned foreign root");
  require(!co_active()&&!co_create(1024u*1024u,guest_entry),"adapter cannot adopt a foreign root");
  require(om_n64_co_retire_thread()==1&&IsThreadAFiber()&&GetCurrentFiber()==root,"adapter retirement cannot convert another owner's root");
  require(ConvertFiberToThread()!=0&&!IsThreadAFiber(),"fixture retires its own foreign root");
}

int main(){try{
  reject_foreign_root();for(unsigned n=0;n<3;++n)roundtrip();
  std::barrier concurrent(2);std::atomic<bool> okay{true};
  const auto run=[&]{try{roundtrip(&concurrent);}catch(...){okay=false;concurrent.arrive_and_drop();}};
  std::jthread first(run),second;
  try{second=std::jthread(run);}catch(...){concurrent.arrive_and_drop();first.join();throw;}
  first.join();second.join();require(okay,"simultaneous roots/guests have independent thread-local ownership");
  std::cout<<"N64 owned Windows fiber cases passed\n";return 0;
}catch(const std::exception& error){std::cerr<<error.what()<<'\n';return 1;}}
