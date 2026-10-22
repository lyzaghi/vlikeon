// Actual pool/scheduler/receipt fragment, not CPU or interrupt callbacks.
#include "n64-core-owner.h"
extern "C" {
#include "device/r4300/cp0.h"
#include "device/r4300/interrupt.h"
void om_n64_case_event_insert_reset(cp0*);
}
#include <cstring>
#include <iostream>
#include <memory>
#include <stdexcept>
#include <thread>
static void require(bool ok,const char* why){if(!ok)throw std::runtime_error(why);}
int main(){try{
    std::unique_ptr<cp0> state(new cp0{});om_n64_case_event_insert_reset(state.get());
    r4300_cp0_regs(state.get())[CP0_COUNT_REG]=0xfffffff0;
    require(om_n64_add_interrupt_checked(state.get(),AI_INT,32)==1&&state->q.pool.index==1&&state->q.first->data.type==AI_INT&&state->q.first->data.count==16,"actual node allocated and wrapped absolute count published");
    for(unsigned i=1;i<INTERRUPT_NODES_POOL_CAPACITY;++i)require(om_n64_add_interrupt_checked(state.get(),PI_INT,64+i)==1,"each remaining actual pool slot admitted");
    std::unique_ptr<cp0> before(new cp0(*state));require(om_n64_owner_begin()==1,"activate owner for pool failure");
    require(!om_n64_add_interrupt_checked(state.get(),AI_INT,100)&&!om_n64_owner_admitted()&&!std::memcmp(before.get(),state.get(),sizeof(*state)),"full pool refuses without queue mutation and records failure");
    require(om_n64_owner_end(nullptr)==1,"retire failed owner");
    om_n64_case_event_insert_reset(state.get());require(om_n64_owner_begin()==1,"new owner");*before=*state;int inserted=1;
    std::thread foreign([&]{inserted=om_n64_add_interrupt_checked(state.get(),AI_INT,1);});foreign.join();
    require(!inserted&&!std::memcmp(before.get(),state.get(),sizeof(*state)),"foreign insertion cannot allocate or modify scheduler");
    require(om_n64_add_interrupt_checked(state.get(),AI_INT,0)==1,"zero delay retains scheduler contract");
    require(om_n64_owner_end(nullptr)==1,"retire owner");
    std::cout<<"Event insertion source cases passed\n";return 0;
}catch(const std::exception& e){std::cerr<<e.what()<<'\n';return 1;}}
