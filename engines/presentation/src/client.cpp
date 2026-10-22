#include "client.hpp"
#include <algorithm>
#include <condition_variable>
#include <deque>
#include <list>
#include <mutex>
#include <optional>
#include <thread>

namespace omni::presentation {
using Clock=std::chrono::steady_clock;
struct Pending{Kind kind;std::uint64_t epoch;Bytes body;};
struct Client::State{
  std::shared_ptr<Duplex> transport;std::mutex mutex;std::condition_variable changed;
  bool failed=false,stopping=false,writer_finished=false,input_seen=false;Snapshot value;
  Clock::time_point input_at{},progress_at=Clock::now(),sent_at=Clock::now();
  std::deque<Pending> controls;std::list<Pending> audio;std::optional<Pending> video;std::size_t audio_bytes=0;unsigned audio_rate=0;std::uint64_t media_generation=0;
  explicit State(Session session):transport(std::make_shared<Duplex>(session)){}
  bool fresh(Clock::time_point now)const{return value.ready&&input_seen&&!failed&&!stopping&&now-input_at<=std::chrono::milliseconds(250);}
  void clear_media(){
    // A pause or stale-producer clear can happen without a new host epoch.
    // Invalidate copies already staging outside the mutex as well.
    if(media_generation==kMaxSequence)failed=true;else ++media_generation;
    audio.clear();audio_bytes=0;video.reset();
  }
  void fail(){std::lock_guard lock(mutex);failed=true;value.input.fill(0);value.focused=false;controls.clear();clear_media();changed.notify_all();}
  void receive(){
    try{Message message;while(transport->read(message,Direction::HostToActor)){
      std::lock_guard lock(mutex);if(failed)return;
      if(!value.ready){check(message.kind==Kind::Ready);value.ready=true;value.epoch=message.epoch;value.maximum_width=u32(message.body,4);value.maximum_height=u32(message.body,8);value.haptics_admitted=u32(message.body,20)!=0;changed.notify_all();continue;}
      check(message.kind!=Kind::Ready);if(message.epoch<value.epoch)continue;
      if(message.epoch>value.epoch){value.epoch=message.epoch;value.input.fill(0);value.focused=false;value.playing=false;input_seen=false;controls.clear();clear_media();}
      if(message.kind==Kind::Input){std::copy(message.body.begin(),message.body.end(),value.input.begin());value.focused=value.input[144]!=0;input_seen=true;input_at=Clock::now();if(!value.focused){controls.clear();clear_media();}}
      else{check(message.kind==Kind::Metrics);value.presented=u64(message.body,0);value.dropped_video=u64(message.body,8);value.dropped_audio=u64(message.body,16);value.reason=u32(message.body,24);value.playing=u32(message.body,28)!=0;}
      changed.notify_all();message=Message{};
    }}catch(...){}fail();
  }
  void transmit(){
    try{for(;;){Pending message{};bool stopping_now=false;{
      std::unique_lock lock(mutex);changed.wait_for(lock,std::chrono::milliseconds(50),[&]{return failed||stopping||!controls.empty()||!audio.empty()||video.has_value();});if(failed)return;
      if(stopping){if(!value.ready){writer_finished=true;changed.notify_all();return;}message={Kind::Stop,value.epoch,{}};stopping_now=true;}
      else{const auto now=Clock::now();if(!value.ready)continue;const bool producer_live=now-progress_at<=std::chrono::milliseconds(150);if(!producer_live){controls.clear();clear_media();continue;}
        if(!controls.empty()){message=std::move(controls.front());controls.pop_front();}
        else if(!audio.empty()){message=std::move(audio.front());audio_bytes-=message.body.size()-16;audio.pop_front();}
        else if(video){message=std::move(*video);video.reset();}
        else{if(now-sent_at<std::chrono::milliseconds(100))continue;message={Kind::Heartbeat,value.epoch,{}};}
        if(message.epoch!=value.epoch)continue;
        if((message.kind==Kind::Audio||message.kind==Kind::Rumble||(message.kind==Kind::State&&!message.body.empty()&&message.body[0]))&&(!fresh(now)||!value.focused))continue;
      }
    }transport->write(message.kind,message.epoch,message.body,Direction::ActorToHost);{
      std::lock_guard lock(mutex);sent_at=Clock::now();if(stopping_now){writer_finished=true;controls.clear();clear_media();changed.notify_all();return;}
    }}}catch(...){fail();}
  }
};
Client::Client(const Admission& admission):state_(std::make_shared<State>(admission.session)){
  check(state_->transport->initialize_inherited(admission.input,admission.output));const auto state=state_;
  try{std::thread([state]{state->receive();}).detach();std::thread([state]{state->transmit();}).detach();}catch(...){state->fail();throw;}
}
Client::~Client(){close();}
bool Client::wait_ready(std::chrono::milliseconds timeout){check(timeout.count()>0&&timeout<=std::chrono::seconds(5));const auto state=state_;std::unique_lock lock(state->mutex);state->changed.wait_for(lock,timeout,[&]{return state->failed||state->stopping||state->value.ready;});return state->value.ready&&!state->failed&&!state->stopping;}
bool Client::wait_focused(std::chrono::milliseconds timeout){check(timeout.count()>0&&timeout<=std::chrono::seconds(1));const auto state=state_;std::unique_lock lock(state->mutex);state->changed.wait_for(lock,timeout,[&]{return state->failed||state->stopping||(state->fresh(Clock::now())&&state->value.focused);});return state->fresh(Clock::now())&&state->value.focused;}
Snapshot Client::snapshot()const{const auto state=state_;std::lock_guard lock(state->mutex);auto value=state->value;value.fresh=state->fresh(Clock::now());value.unavailable=state->failed||state->stopping||state->writer_finished;if(!value.fresh){value.input.fill(0);value.focused=false;}return value;}
void Client::progress(){const auto state=state_;std::lock_guard lock(state->mutex);if(!state->failed&&!state->stopping){state->progress_at=Clock::now();state->changed.notify_one();}}
bool Client::video(std::span<const std::uint32_t> pixels,unsigned width,unsigned height,unsigned logical_width,std::uint64_t timestamp){
  const auto state=state_;std::uint64_t epoch=0,generation=0;{
    std::lock_guard lock(state->mutex);if(state->failed||state->stopping||!state->value.ready)return false;check(width>0&&width<=state->value.maximum_width&&height>0&&height<=state->value.maximum_height&&logical_width>0&&logical_width<=kMaxDimension*10&&pixels.size()==std::size_t(width)*height&&timestamp<=kMaxSequence);epoch=state->value.epoch;generation=state->media_generation;
  }
  Bytes body(32+pixels.size()*4,0);put32(body,0,width);put32(body,4,height);put32(body,8,width*4);put32(body,12,1);put32(body,16,logical_width);put64(body,24,timestamp);for(std::size_t i=0;i<pixels.size();i++){const auto color=pixels[i];body[32+i*4]=std::uint8_t(color>>16);body[33+i*4]=std::uint8_t(color>>8);body[34+i*4]=std::uint8_t(color);body[35+i*4]=255;}
  std::lock_guard lock(state->mutex);if(state->failed||state->stopping||state->value.epoch!=epoch||state->media_generation!=generation)return false;state->video=Pending{Kind::Video,epoch,std::move(body)};state->changed.notify_one();return true;
}
bool Client::audio(std::span<const std::int16_t> samples,unsigned rate){
  check(rate>=8000&&rate<=192000&&!samples.empty()&&samples.size()%2==0&&samples.size()<=65536u*2u);const auto state=state_;std::uint64_t epoch=0,generation=0;{
    std::lock_guard lock(state->mutex);if(!state->fresh(Clock::now())||!state->value.focused)return false;epoch=state->value.epoch;generation=state->media_generation;
  }
  const auto chunk_frames=std::min<unsigned>(kMaxAudioFrames,std::max(1u,rate/100));const auto frames=samples.size()/2;
  // Stage every allocation before mutating shared state. A failed copy or a
  // superseded focus epoch must not leave a prefix that the guest will replay.
  std::list<Pending> staged;
  for(std::size_t offset=0;offset<frames;){const auto count=std::min<std::size_t>(chunk_frames,frames-offset);Bytes body(16+count*4,0);put32(body,0,rate);put32(body,4,static_cast<std::uint32_t>(count));put16(body,8,2);put16(body,10,1);for(std::size_t i=0;i<count*2;i++)put16(body,16+i*2,static_cast<std::uint16_t>(samples[offset*2+i]));
    staged.push_back({Kind::Audio,epoch,std::move(body)});offset+=count;
  }
  std::lock_guard lock(state->mutex);
  if(!state->fresh(Clock::now())||!state->value.focused||state->value.epoch!=epoch||state->media_generation!=generation)return false;
  if(state->audio_rate!=rate){state->audio.clear();state->audio_bytes=0;state->audio_rate=rate;}
  // Equal std::allocator lists transfer their nodes without allocating. From
  // here through the receipt there is no fallible operation or partial publish.
  state->audio.splice(state->audio.end(),staged);state->audio_bytes+=samples.size_bytes();
  const auto limit=std::min<std::size_t>(128u*1024u,rate);
  while(state->audio.size()>32||state->audio_bytes>limit){state->audio_bytes-=state->audio.front().body.size()-16;state->audio.pop_front();}
  state->changed.notify_one();return true;
}
bool Client::playback(bool running,unsigned volume,unsigned fullscreen){check(volume<=100&&fullscreen<=2);const auto state=state_;std::lock_guard lock(state->mutex);if(state->failed||state->stopping||!state->value.ready||(running&&(!state->fresh(Clock::now())||!state->value.focused)))return false;if(!running){state->controls.clear();state->clear_media();if(state->failed){state->changed.notify_all();return false;}}else{std::erase_if(state->controls,[](const Pending& value){return value.kind==Kind::State;});}if(state->controls.size()>=8)return false;state->progress_at=Clock::now();Bytes body(8,0);body[0]=running?1:0;body[1]=static_cast<std::uint8_t>(fullscreen);put16(body,2,static_cast<std::uint16_t>(volume));state->controls.push_back({Kind::State,state->value.epoch,std::move(body)});state->changed.notify_one();return true;}
bool Client::rumble(unsigned port,std::uint16_t strong,std::uint16_t weak,unsigned milliseconds){check(port<4&&milliseconds<=250);const auto state=state_;std::lock_guard lock(state->mutex);if(!state->fresh(Clock::now())||!state->value.focused||!state->value.haptics_admitted||!state->value.input[64+port*20+1]||state->controls.size()>=8)return false;Bytes body(8,0);body[0]=static_cast<std::uint8_t>(port);put16(body,2,strong);put16(body,4,weak);put16(body,6,static_cast<std::uint16_t>(milliseconds));state->controls.push_back({Kind::Rumble,state->value.epoch,std::move(body)});state->changed.notify_one();return true;}
void Client::close(){const auto state=state_;if(!state)return;std::lock_guard lock(state->mutex);if(state->stopping||state->failed)return;state->stopping=true;state->value.input.fill(0);state->value.focused=false;state->controls.clear();state->clear_media();state->changed.notify_all();}
}
