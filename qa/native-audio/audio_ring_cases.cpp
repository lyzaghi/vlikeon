// Authored only: actual production ring, scripted sinks, no SDL/core/device.
#include "audio_ring.hpp"
#include <chrono>
#include <iostream>
#include <limits>
#include <memory>
#include <stdexcept>
#include <thread>
#include <vector>

using omni::AudioRing;
static void require(bool value,const char* why){if(!value)throw std::runtime_error(why);}
static std::vector<std::int16_t> samples(std::size_t first,std::size_t frames){
  std::vector<std::int16_t> value(frames*2);
  for(std::size_t i=0;i<frames;++i){const auto n=static_cast<std::int16_t>((first+i)%30000);value[i*2]=n;value[i*2+1]=static_cast<std::int16_t>(-n);}
  return value;
}
static void fill(AudioRing& ring,std::size_t frames){
  for(std::size_t at=0;at<frames;){const auto count=std::min(AudioRing::batch_limit,frames-at);const auto input=samples(at,count);const auto receipt=ring.push(input);require(receipt.valid&&receipt.frames==count,"fill receipt");at+=count;}
}
static void rejection_and_trim(){
  auto ring=std::make_unique<AudioRing>();fill(*ring,AudioRing::capacity-2);
  const auto extra=samples(AudioRing::capacity-2,5);
  const auto partial=ring->push(extra);require(partial.valid&&partial.frames==2,"only free frames copied");
  require(ring->push(extra).frames==0,"full ring returns zero");
  std::array<std::int16_t,4> output{};
  bool blocked=false;std::vector<std::int16_t> rejected;
  const auto failed=ring->deliver(output,2,0,1,[&](auto value){
    const auto write=ring->push(extra);blocked=write.valid&&!write.frames;
    rejected.assign(value.begin(),value.end());return false;
  });
  require(failed.delivery==AudioRing::Delivery::Rejected&&!failed.frames&&!failed.trimmed&&!failed.underrun&&blocked,"no slots or state released before acceptance");
  require(ring->push(extra).frames==0,"failed trim retains entire old ring");
  // This scripted sink accepted nothing. Actual SDL failures must clear/retire
  // their unknown partial output before a retry could be allowed.
  const auto accepted=ring->deliver(output,2,0,1,[&](auto value){return std::equal(value.begin(),value.end(),rejected.begin(),rejected.end());});
  require(accepted.delivery==AudioRing::Delivery::Accepted&&accepted.frames==2&&accepted.trimmed==AudioRing::capacity-2&&!accepted.underrun,"successful trim commits once");
  require(output[0]==static_cast<std::int16_t>((AudioRing::capacity-2)%30000)&&output[2]==static_cast<std::int16_t>((AudioRing::capacity-1)%30000),"trim keeps newest stereo frames");
  require(ring->push(extra).frames==5,"slots reusable after acceptance");
}
static void wrap_and_order(){
  auto ring=std::make_unique<AudioRing>();std::vector<std::int16_t> output(AudioRing::output_limit*2);
  std::size_t sequence=0;
  for(unsigned cycle=0;cycle<12;++cycle){
    const auto input=samples(sequence,AudioRing::batch_limit);
    require(ring->push(input).frames==AudioRing::batch_limit,"wrapped producer");
    for(std::size_t at=0;at<AudioRing::batch_limit;at+=AudioRing::output_limit){
      const auto receipt=ring->deliver(output,1,AudioRing::capacity,1,[&](auto value){return std::equal(value.begin(),value.end(),input.begin()+static_cast<std::ptrdiff_t>(at*2));});
      require(receipt.delivery==AudioRing::Delivery::Accepted&&receipt.frames==AudioRing::output_limit&&!receipt.trimmed&&!receipt.underrun,"wrapped exact sample order");
    }
    sequence+=AudioRing::batch_limit;
  }
}
static void priming_and_admission(){
  auto ring=std::make_unique<AudioRing>();auto input=samples(1,1);std::array<std::int16_t,8> output{};
  require(ring->push(input).frames==1,"initial input");
  auto receipt=ring->deliver(output,2,AudioRing::capacity,1,[](auto value){return std::all_of(value.begin(),value.end(),[](auto sample){return sample==0;});});
  require(receipt.delivery==AudioRing::Delivery::Accepted&&!receipt.frames&&!receipt.underrun,"unprimed silence retains input");
  require(ring->push(input).frames==1,"reach priming target");
  receipt=ring->deliver(output,2,AudioRing::capacity,1,[](auto){return false;});
  require(receipt.delivery==AudioRing::Delivery::Rejected&&!receipt.underrun,"rejected underrun not published");
  receipt=ring->deliver(output,2,AudioRing::capacity,1,[](auto){return true;});
  require(receipt.frames==2&&receipt.underrun&&output[0]==1&&output[1]==-1&&output[4]==0&&output[7]==0,"accepted data then zero padding and one underrun");
  receipt=ring->deliver(output,2,AudioRing::capacity,1,[](auto){return true;});
  require(!receipt.frames&&!receipt.underrun,"underrun de-primes once");
  unsigned calls=0;auto sink=[&](auto){++calls;return true;};
  require(ring->deliver(output,0,0,1,sink).delivery==AudioRing::Delivery::Invalid,"zero target rejected");
  require(ring->deliver(output,1,0,std::numeric_limits<float>::quiet_NaN(),sink).delivery==AudioRing::Delivery::Invalid,"nonfinite volume rejected");
  require(ring->deliver(std::span<std::int16_t>(output.data(),3),1,0,1,sink).delivery==AudioRing::Delivery::Invalid&&!calls,"invalid outputs never invoke sink");
  require(!ring->push(std::span<const std::int16_t>(output.data(),3)).valid,"odd producer samples rejected");
  const auto large=samples(0,AudioRing::batch_limit+1);require(!ring->push(large).valid,"oversized producer rejected");
  require(ring->push({}).valid,"empty producer admitted");
  ring->reset_quiescent();input={32767,-32768};require(ring->push(input).frames==1,"post-reset producer");
  std::array<std::int16_t,2> stereo{};
  receipt=ring->deliver(stereo,1,0,.5f,sink);
  require(receipt.frames==1&&stereo[0]==16384&&stereo[1]==-16384,"volume rounding stays within signed stereo");
  ring->reset_quiescent();receipt=ring->deliver(stereo,1,0,1,sink);require(!receipt.frames&&!receipt.underrun&&stereo[0]==0&&stereo[1]==0,"reset removes old data and priming");
}
static void concurrent_order(){
  auto ring=std::make_unique<AudioRing>();constexpr std::size_t total=500000,chunk=257;
  std::atomic<std::size_t> delivered{0};std::atomic<bool> stopped{false},failed{false};
  const auto until=std::chrono::steady_clock::now()+std::chrono::seconds(15);
  std::thread producer([&]{std::size_t at=0;while(at<total&&!stopped.load()){
    if(std::chrono::steady_clock::now()>=until){failed=true;break;}
    // Keep below the deliberate latency-trim threshold. This case checks SPSC
    // ordering; overload trimming has a separate explicit expectation above.
    if(at-delivered.load(std::memory_order_acquire)>AudioRing::capacity/4){std::this_thread::yield();continue;}
    const auto count=std::min(chunk,total-at);const auto input=samples(at,count);const auto receipt=ring->push(input);
    if(!receipt.valid||receipt.frames!=count){failed=true;break;}at+=count;
  }});
  std::array<std::int16_t,chunk*2> output{};std::size_t at=0;
  while(at<total&&!failed.load()){
    if(std::chrono::steady_clock::now()>=until){failed=true;break;}
    const auto count=std::min(chunk,total-at);const auto receipt=ring->deliver(std::span<std::int16_t>(output.data(),count*2),1,AudioRing::capacity,1,[](auto){return true;});
    if(receipt.delivery!=AudioRing::Delivery::Accepted||receipt.trimmed){failed=true;break;}
    for(std::size_t i=0;i<receipt.frames;++i){const auto expected=static_cast<std::int16_t>((at+i)%30000);if(output[i*2]!=expected||output[i*2+1]!=-expected){failed=true;break;}}
    at+=receipt.frames;delivered.store(at,std::memory_order_release);
    if(!receipt.frames)std::this_thread::yield();
  }
  stopped=true;producer.join();require(!failed&&at==total,"concurrent exact stereo order and finite completion");
}
int main(){try{rejection_and_trim();wrap_and_order();priming_and_admission();concurrent_order();std::cout<<"Native audio ring cases passed\n";return 0;}catch(const std::exception& e){std::cerr<<e.what()<<'\n';return 1;}}
