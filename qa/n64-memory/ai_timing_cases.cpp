// Exact prepared request/DMA methods; CP0/scheduler/backend are observers.
#include "n64-core-owner.h"
#include "n64-device-memory.h"
#include "n64-ai-timing.h"
extern "C" {
#include "backends/api/audio_out_backend.h"
#include "device/rcp/ai/ai_controller.h"
#include "device/rcp/ri/ri_controller.h"
#include "device/rcp/mi/mi_controller.h"
#include "device/rcp/vi/vi_controller.h"
#include "device/r4300/r4300_core.h"
#include "device/r4300/interrupt.h"
#include "device/rdram/rdram.h"
}
#include <array>
#include <cstring>
#include <iostream>
#include <limits>
#include <stdexcept>
static r4300_core cpu;
static unsigned scheduled=0,formatted=0,observed_rate=0,observed_duration=0;
static bool refuse_event=false;
static int stopped=0;
extern "C" void cp0_update_count(r4300_core*){}
extern "C" void add_interrupt_event(cp0*,int,unsigned int delay){++scheduled;observed_duration=delay;}
extern "C" int om_n64_add_interrupt_checked(cp0* value,int type,unsigned int delay){if(refuse_event)return 0;add_interrupt_event(value,type,delay);return 1;}
extern "C" int* r4300_stop(r4300_core*){return &stopped;}
static bool refuse_format=false;
static void frequency(void*,unsigned rate){++formatted;observed_rate=rate;if(refuse_format)om_n64_owner_admission_failed();}
static void samples(void*,const void*,size_t){}
#define AI_STATUS_BUSY UINT32_C(0x40000000)
#define AI_STATUS_FULL UINT32_C(0x80000000)
#include "n64-ai-request-actual.c.inc"
static void require(bool good,const char* why){if(!good)throw std::runtime_error(why);}
struct Fixture {
    std::array<uint32_t,4096> words{};rdram ram{};ri_controller ri{};mi_controller mi{};vi_controller vi{};ai_controller ai{};
    audio_out_backend_interface api{frequency,samples};
    Fixture(){ram.dram=words.data();ram.dram_size=sizeof(words);ri.rdram=&ram;mi.r4300=&cpu;vi.clock=48681812;vi.expected_refresh_rate=60;vi.delay=500000;
        ai.ri=&ri;ai.mi=&mi;ai.vi=&vi;ai.iaout=&api;ai.dma_modifier=1;ai.regs[AI_DACRATE_REG]=1103;ai.regs[AI_LEN_REG]=64;scheduled=formatted=0;}
};
int main(){try{
    uint32_t duration=123;
    require(om_n64_ai_duration(48681812,500000,60,1103,64,1,&duration)&&duration==64u*(30000000u/(4u*(48681812u/1104u))),"pinned division and modifier order retained");
    require(om_n64_ai_duration(48681812,0,60,0,64,0,&duration)&&duration==0,"zero multiplier retains zero-duration contract");
    for(float multiplier: {-1.0f,std::numeric_limits<float>::infinity(),std::numeric_limits<float>::quiet_NaN(),std::numeric_limits<float>::max()}){
        duration=123;require(!om_n64_ai_duration(48681812,500000,60,1103,64,multiplier,&duration)&&duration==123,"invalid float never commits or converts to unsigned");}
    require(!om_n64_ai_duration(48681812,500000,60,UINT32_MAX,64,1,&duration),"DAC divisor addition cannot wrap to zero");
    require(!om_n64_ai_duration(1,1,60,2,64,1,&duration),"zero sample rate rejected before duration division");
    require(!om_n64_ai_duration(48681812,UINT32_MAX,60,1103,64,1,&duration),"VI duration product cannot wrap");
    Fixture valid;valid.ai.samples_format_changed=1;fifo_push(&valid.ai);
    require(scheduled==1&&formatted==1&&observed_rate==48681812u/1104u&&valid.ai.last_read==64&&(valid.ai.regs[AI_STATUS_REG]&AI_STATUS_BUSY),"actual request publishes format, FIFO and schedule");
    require(observed_duration==valid.ai.fifo[0].duration,"actual scheduled duration equals admitted FIFO duration");
    Fixture bad;bad.ai.regs[AI_DACRATE_REG]=UINT32_MAX;auto prior=bad.ai;fifo_push(&bad.ai);
    require(!scheduled&&!formatted&&!std::memcmp(&prior,&bad.ai,sizeof(prior)),"invalid timing cannot mutate request FIFO or schedule");
    Fixture short_ram;short_ram.ai.regs[AI_DRAM_ADDR_REG]=sizeof(short_ram.words)-32;prior=short_ram.ai;fifo_push(&short_ram.ai);
    require(!scheduled&&!std::memcmp(&prior,&short_ram.ai,sizeof(prior)),"full RAM request required before FIFO publication");
    Fixture carry;carry.ai.delayed_carry=1;carry.ai.fifo[0]={static_cast<uint32_t>(sizeof(carry.words)-16),16,100};prior=carry.ai;do_dma(&carry.ai,&carry.ai.fifo[0]);
    require(!scheduled&&!formatted&&!std::memcmp(&prior,&carry.ai,sizeof(prior)),"delayed carry rechecked before changing addresses or calling backend");
    Fixture exact;exact.ai.delayed_carry=1;exact.ai.samples_format_changed=1;exact.ai.fifo[0]={0,0x2000,100};do_dma(&exact.ai,&exact.ai.fifo[0]);
    require(scheduled==1&&exact.ai.fifo[0].address==0x2000&&exact.ai.last_read==0x2000&&exact.ai.delayed_carry==1,"valid carry and exact end retain pinned transition");
    Fixture snapshot;snapshot.ai.fifo[0]={0,64,UINT32_MAX};prior=snapshot.ai;do_dma(&snapshot.ai,&snapshot.ai.fifo[0]);require(!scheduled&&!std::memcmp(&prior,&snapshot.ai,sizeof(prior)),"snapshot delay outside signed horizon cannot schedule");
    Fixture first_failure;first_failure.ai.delayed_carry=1;first_failure.ai.regs[AI_DRAM_ADDR_REG]=sizeof(first_failure.words)-64;prior=first_failure.ai;fifo_push(&first_failure.ai);
    require(!scheduled&&!formatted&&!std::memcmp(&prior,&first_failure.ai,sizeof(prior)),"failed first-slot delayed carry leaves FIFO/status unmodified");
    Fixture promotion;promotion.ai.regs[AI_STATUS_REG]=AI_STATUS_BUSY|AI_STATUS_FULL;promotion.ai.fifo[0]={0,64,100};promotion.ai.fifo[1]={static_cast<uint32_t>(sizeof(promotion.words)-32),64,100};prior=promotion.ai;fifo_pop(&promotion.ai);
    require(!scheduled&&!formatted&&!std::memcmp(&prior,&promotion.ai,sizeof(prior)),"failed pending promotion retains both slots and full status");
    Fixture queued;queued.ai.regs[AI_STATUS_REG]=AI_STATUS_BUSY;queued.ai.fifo[0]={0,32,100};fifo_push(&queued.ai);
    require(!scheduled&&(queued.ai.regs[AI_STATUS_REG]&AI_STATUS_FULL)&&queued.ai.fifo[0].length==32&&queued.ai.fifo[1].length==64,"busy request stages second slot without starting it");
    fifo_pop(&queued.ai);require(scheduled==1&&queued.ai.fifo[0].length==64&&!(queued.ai.regs[AI_STATUS_REG]&AI_STATUS_FULL)&&(queued.ai.regs[AI_STATUS_REG]&AI_STATUS_BUSY),"successful promotion publishes selected slot and clears only full status");
    fifo_pop(&queued.ai);require(!(queued.ai.regs[AI_STATUS_REG]&AI_STATUS_BUSY)&&!queued.ai.delayed_carry,"empty pending queue clears busy and carry");
    Fixture backend_failure;backend_failure.ai.samples_format_changed=1;prior=backend_failure.ai;
    require(om_n64_owner_begin()==1,"activate backend-fault owner");refuse_format=true;fifo_push(&backend_failure.ai);refuse_format=false;
    require(formatted==1&&!scheduled&&!std::memcmp(&prior,&backend_failure.ai,sizeof(prior))&&!om_n64_owner_admitted(),"backend format rejection cannot publish FIFO or schedule");
    require(om_n64_owner_end(nullptr)==1,"retire failed unstarted owner");
    Fixture event_failure;event_failure.ai.samples_format_changed=1;prior=event_failure.ai;
    require(om_n64_owner_begin()==1,"activate event-fault owner");refuse_event=true;stopped=0;fifo_push(&event_failure.ai);refuse_event=false;
    require(stopped==1&&!scheduled&&!std::memcmp(&prior,&event_failure.ai,sizeof(prior))&&!om_n64_owner_admitted(),"refused scheduler insertion stops CPU without descriptor/status publication");
    require(om_n64_owner_end(nullptr)==1,"retire event-fault owner");
    std::cout<<"AI timing source cases passed\n";return 0;
}catch(const std::exception& e){std::cerr<<e.what()<<'\n';return 1;}}
