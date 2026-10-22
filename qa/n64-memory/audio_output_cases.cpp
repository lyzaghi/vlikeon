// Production queue with scripted consumers; no resampler or device executes.
#include "n64-audio-output.h"
#include <algorithm>
#include <iostream>
#include <iterator>
#include <memory>
#include <stdexcept>
#include <vector>
static std::vector<int16_t> received;
static size_t limit=0,calls=0;
static bool overreport=false;
static om_n64_audio_queue* reentrant=nullptr;
static bool denied=false;
static size_t consume(const int16_t* samples,size_t frames){
    if(frames>OM_N64_AUDIO_BATCH_FRAMES)throw std::runtime_error("batch exceeds native callback frame contract");
    ++calls;
    if(reentrant){const int16_t one[2]={1,2};denied=!om_n64_audio_queue_append(reentrant,one,1)&&om_n64_audio_queue_drain(reentrant,consume)==-1;}
    if(overreport)return frames+1;
    auto count=std::min(frames,limit);received.insert(received.end(),samples,samples+count*2);return count;
}
static void require(bool good,const char* why){if(!good)throw std::runtime_error(why);}
int main(){try{
    std::unique_ptr<om_n64_audio_queue> q(new om_n64_audio_queue{});
    const int16_t input[]={1,-1,2,-2,3,-3,4,-4,5,-5};
    require(om_n64_audio_queue_append(q.get(),input,5)&&q->count==5,"append exact stereo frames");
    limit=0;calls=0;require(om_n64_audio_queue_drain(q.get(),consume)==0&&calls==1&&q->count==5&&!q->head&&received.empty(),"zero receipt returns promptly without loss");
    limit=1;calls=0;require(om_n64_audio_queue_drain(q.get(),consume)==1&&calls==5&&std::equal(received.begin(),received.end(),std::begin(input))&&!q->count,"partial consumption preserves sample order");
    std::vector<int16_t> bulk(OM_N64_AUDIO_QUEUE_FRAMES*2);for(size_t i=0;i<bulk.size();++i)bulk[i]=static_cast<int16_t>(i%30000);
    received.clear();require(om_n64_audio_queue_append(q.get(),bulk.data(),OM_N64_AUDIO_QUEUE_FRAMES),"exact full ring with nonzero head wraps");
    require(!om_n64_audio_queue_append(q.get(),input,1)&&q->count==OM_N64_AUDIO_QUEUE_FRAMES,"full ring refuses complete new input without discarding existing frames");
    limit=1;calls=0;require(om_n64_audio_queue_drain(q.get(),consume)==0&&calls==8&&q->count==OM_N64_AUDIO_QUEUE_FRAMES-8,"slow consumer bounded to eight calls");
    limit=OM_N64_AUDIO_QUEUE_FRAMES;calls=0;
    for(unsigned pass=0;pass<4&&q->count;++pass){const auto prior_calls=calls;require(om_n64_audio_queue_drain(q.get(),consume)>=0&&calls-prior_calls<=8,"each native-sized drain remains bounded");}
    require(!q->count&&received==bulk,"wrapped ring drains every byte in original order using admitted native batch sizes");
    require(om_n64_audio_queue_append(q.get(),input,5),"append fault fixture");auto head=q->head,count=q->count;overreport=true;calls=0;
    require(om_n64_audio_queue_drain(q.get(),consume)==-1&&calls==1&&q->head==head&&q->count==count&&q->fault,"overreported receipt cannot underflow or advance ownership");
    require(!om_n64_audio_queue_append(q.get(),input,1)&&om_n64_audio_queue_drain(q.get(),consume)==-1,"fault retains queue until explicit session retirement");
    q.reset(new om_n64_audio_queue{});overreport=false;limit=5;require(om_n64_audio_queue_append(q.get(),input,5),"new session");reentrant=q.get();
    require(om_n64_audio_queue_drain(q.get(),consume)==1&&denied&&!q->busy,"reentrant append/drain denied while callback borrows samples");reentrant=nullptr;
    require(!om_n64_audio_queue_append(q.get(),nullptr,1)&&om_n64_audio_queue_append(q.get(),nullptr,0),"empty input and missing input distinguished");
    std::cout<<"Audio output source cases passed\n";return 0;
}catch(const std::exception& e){std::cerr<<e.what()<<'\n';return 1;}}
