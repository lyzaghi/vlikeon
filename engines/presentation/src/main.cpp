#include "wire.hpp"
#include "owner.hpp"
#include <SDL3/SDL.h>
#include <SDL3/SDL_main.h>
#include <algorithm>
#include <atomic>
#include <charconv>
#include <chrono>
#include <cmath>
#include <condition_variable>
#include <cstdlib>
#include <cstring>
#include <deque>
#include <mutex>
#include <optional>
#include <thread>
#ifdef __APPLE__
#include "../../common/macos_child.hpp"
#endif

namespace omni::presentation {
using Clock=std::chrono::steady_clock;
struct Pending{Kind kind;std::uint64_t epoch;Bytes body;};
struct Batch{std::deque<Message> controls,audio;std::optional<Message> video;};
class Bus{
  std::mutex mutex_;std::condition_variable changed_;bool closed_=false;
  std::deque<Message> controls_,audio_;std::optional<Message> video_;std::size_t audio_bytes_=0;std::uint32_t audio_rate_=0;
  std::optional<Pending> input_,metrics_;std::uint64_t dropped_video_=0,dropped_audio_=0;
public:
  bool closed(){std::lock_guard lock(mutex_);return closed_;}
  void close(){std::lock_guard lock(mutex_);closed_=true;changed_.notify_all();}
  void incoming(Message message){std::lock_guard lock(mutex_);if(closed_)return;
    if(message.kind==Kind::Video){if(video_)dropped_video_++;video_=std::move(message);}
    else if(message.kind==Kind::Audio){const auto rate=u32(message.body,0);const auto bytes=message.body.size()-16,limit=std::min<std::size_t>(128u*1024u,rate);if(bytes>limit){dropped_audio_++;return;}if(rate!=audio_rate_){dropped_audio_+=audio_.size();audio_.clear();audio_bytes_=0;audio_rate_=rate;}while(!audio_.empty()&&(audio_.size()>=32||audio_bytes_+bytes>limit)){audio_bytes_-=audio_.front().body.size()-16;audio_.pop_front();dropped_audio_++;}audio_bytes_+=bytes;audio_.push_back(std::move(message));}
    else{
      // Commands and media are consumed in separate batches. Retire earlier
      // queued media on every playback transition before publishing the State,
      // so a pause/resume pair cannot replay pre-pause samples in the same batch.
      if(message.kind==Kind::State){dropped_audio_+=audio_.size();audio_.clear();audio_bytes_=0;if(video_){dropped_video_++;video_.reset();}}
      check(controls_.size()<8);controls_.push_back(std::move(message));
    }
  }
  Batch take(bool video){std::lock_guard lock(mutex_);Batch result;result.controls.swap(controls_);result.audio.swap(audio_);audio_bytes_=0;if(video){result.video=std::move(video_);video_.reset();}return result;}
  void outgoing(Pending message){std::lock_guard lock(mutex_);if(closed_)return;if(message.kind==Kind::Input)input_=std::move(message);else{check(message.kind==Kind::Metrics);metrics_=std::move(message);}changed_.notify_one();}
  bool next(Pending& message){std::unique_lock lock(mutex_);changed_.wait(lock,[&]{return closed_||input_||metrics_;});if(closed_)return false;if(input_){message=std::move(*input_);input_.reset();}else{message=std::move(*metrics_);metrics_.reset();}return true;}
  std::array<std::uint64_t,2> dropped(){std::lock_guard lock(mutex_);return{dropped_video_,dropped_audio_};}
};
class Host{
  Bus& bus_;Owner& owner_;std::uint32_t max_width_,max_height_,volume_ceiling_;bool haptics_,focused_=false,playing_=false,quit_=false;
  SDL_Window* window_=nullptr;SDL_Renderer* renderer_=nullptr;SDL_Texture* texture_=nullptr;SDL_AudioStream* audio_=nullptr;
  std::array<SDL_Gamepad*,4> pads_{};std::uint32_t width_=640,height_=480,logical_width_=640,audio_rate_=48000,reason_=0;std::uint64_t epoch_=1,presented_=0;
  Clock::time_point last_activity_=Clock::now(),last_input_=Clock::now(),last_metrics_=Clock::now(),last_video_=Clock::now(),last_pads_=Clock::now();
  bool finger_down_=false;SDL_FingerID finger_=0;float finger_x_=0,finger_y_=0,wheel_x_=0,wheel_y_=0;
  static std::int16_t number(float value){if(!std::isfinite(value))return 0;return static_cast<std::int16_t>(std::lround(std::clamp(value,-32767.0f,32767.0f)));}
  void stop_haptics(){for(auto* pad:pads_)if(pad)SDL_RumbleGamepad(pad,0,0,0);}
  void fence(std::uint32_t reason){check(epoch_<kMaxSequence);epoch_++;playing_=false;finger_down_=false;wheel_x_=wheel_y_=0;reason_=reason;stop_haptics();if(audio_)check(SDL_ClearAudioStream(audio_));send_input();}
  bool admit(const Message& message){check(message.epoch<=epoch_);if(message.epoch!=epoch_)return false;last_activity_=Clock::now();return true;}
  void update_pads(){
    for(auto*& pad:pads_)if(pad&&!SDL_GamepadConnected(pad)){SDL_RumbleGamepad(pad,0,0,0);SDL_CloseGamepad(pad);pad=nullptr;}
    int count=0;auto* ids=SDL_GetGamepads(&count);if(!ids)return;const auto bound=std::clamp(count,0,32);for(int i=0;i<bound;i++){if(std::any_of(pads_.begin(),pads_.end(),[&](auto* pad){return pad&&SDL_GetGamepadID(pad)==ids[i];}))continue;const auto free=std::find(pads_.begin(),pads_.end(),nullptr);if(free==pads_.end())break;*free=SDL_OpenGamepad(ids[i]);}SDL_free(ids);
  }
  void control(const Message& message){
    if(!admit(message))return;const auto b=std::span<const std::uint8_t>(message.body);
    if(message.kind==Kind::Stop){quit_=true;return;}
    if(message.kind==Kind::State){playing_=b[0]!=0&&focused_;if(b[1]&&!SDL_SetWindowFullscreen(window_,b[1]==2))reason_=4;check(SDL_SetAudioStreamGain(audio_,static_cast<float>(std::min<std::uint32_t>(u16(b,2),volume_ceiling_))/100.0f));if(!playing_){stop_haptics();check(SDL_ClearAudioStream(audio_));}return;}
    if(message.kind==Kind::Rumble){if(playing_&&focused_&&haptics_&&pads_[b[0]]&&SDL_GetBooleanProperty(SDL_GetGamepadProperties(pads_[b[0]]),SDL_PROP_GAMEPAD_CAP_RUMBLE_BOOLEAN,false))SDL_RumbleGamepad(pads_[b[0]],u16(b,2),u16(b,4),u16(b,6));return;}
    check(message.kind==Kind::Heartbeat);
  }
  void video(const Message& message){
    if(!admit(message))return;const auto b=std::span<const std::uint8_t>(message.body);const auto w=u32(b,0),h=u32(b,4);check(w<=max_width_&&h<=max_height_);
    if(!texture_||w!=width_||h!=height_){if(texture_)SDL_DestroyTexture(texture_);texture_=nullptr;width_=w;height_=h;texture_=SDL_CreateTexture(renderer_,SDL_PIXELFORMAT_RGBA32,SDL_TEXTUREACCESS_STREAMING,static_cast<int>(w),static_cast<int>(h));check(texture_&&SDL_SetTextureBlendMode(texture_,SDL_BLENDMODE_NONE));}logical_width_=u32(b,16);check(SDL_SetRenderLogicalPresentation(renderer_,static_cast<int>(logical_width_),static_cast<int>(h),SDL_LOGICAL_PRESENTATION_LETTERBOX));
    check(SDL_UpdateTexture(texture_,nullptr,b.data()+32,static_cast<int>(u32(b,8)))&&SDL_SetRenderDrawColor(renderer_,0,0,0,255)&&SDL_RenderClear(renderer_)&&SDL_RenderTexture(renderer_,texture_,nullptr,nullptr)&&SDL_RenderPresent(renderer_));presented_++;
  }
  void audio(const Message& message){
    if(!admit(message)||!playing_||!focused_)return;const auto b=std::span<const std::uint8_t>(message.body);const auto rate=u32(b,0);if(rate!=audio_rate_){check(SDL_ClearAudioStream(audio_));SDL_AudioSpec spec{SDL_AUDIO_S16LE,2,static_cast<int>(rate)};check(SDL_SetAudioStreamFormat(audio_,&spec,nullptr));audio_rate_=rate;}
    const auto queued=SDL_GetAudioStreamQueued(audio_);check(queued>=0);
    const auto bytes=b.size()-16,limit=std::min<std::size_t>(128u*1024u,audio_rate_);
    if(bytes>limit)return;
    // Bound the queue after admission, including the packet being appended.
    // SDL reports source-format bytes; stereo s16 uses four bytes per frame.
    if(static_cast<std::size_t>(queued)>limit-bytes)check(SDL_ClearAudioStream(audio_));
    check(SDL_PutAudioStreamData(audio_,b.data()+16,static_cast<int>(bytes)));
  }
  void send_input(){
    Bytes b(kInputBytes,0);if(focused_){b[144]=1;int count=0;const bool* keys=SDL_GetKeyboardState(&count);if(keys)for(int i=0;i<std::clamp(count,0,512);i++)if(keys[i])b[static_cast<unsigned>(i)/8]|=std::uint8_t(1u<<(static_cast<unsigned>(i)%8));
      for(unsigned port=0;port<4;port++){auto* pad=pads_[port];if(!pad||!SDL_GamepadConnected(pad))continue;const auto at=64u+port*20u;b[at]=1;b[at+1]=haptics_&&SDL_GetBooleanProperty(SDL_GetGamepadProperties(pad),SDL_PROP_GAMEPAD_CAP_RUMBLE_BOOLEAN,false)?1:0;std::uint32_t mask=0;for(unsigned button=0;button<27;button++)if(button<SDL_GAMEPAD_BUTTON_COUNT&&SDL_GetGamepadButton(pad,SDL_GamepadButton(button)))mask|=1u<<button;put32(b,at+4,mask);for(unsigned axis=0;axis<6;axis++)put16(b,at+8+axis*2,static_cast<std::uint16_t>(SDL_GetGamepadAxis(pad,SDL_GamepadAxis(axis))));}
      float x=0,y=0;bool down=false,available=false;if(finger_down_){int w=0,h=0;if(SDL_GetWindowSize(window_,&w,&h)&&w>0&&h>0){x=finger_x_*static_cast<float>(w);y=finger_y_*static_cast<float>(h);down=true;available=true;}}
      else if(SDL_GetMouseFocus()==window_){down=(SDL_GetMouseState(&x,&y)&SDL_BUTTON_LMASK)!=0;available=true;}
      float lx=0,ly=0;if(available&&std::isfinite(x)&&std::isfinite(y)&&SDL_RenderCoordinatesFromWindow(renderer_,x,y,&lx,&ly)&&std::isfinite(lx)&&std::isfinite(ly)&&lx>=0&&ly>=0&&lx<static_cast<float>(logical_width_)&&ly<static_cast<float>(height_)){b[145]=1;b[146]=down?1:0;put16(b,148,static_cast<std::uint16_t>(number((lx/static_cast<float>(logical_width_)*2-1)*32767)));put16(b,150,static_cast<std::uint16_t>(number((ly/static_cast<float>(height_)*2-1)*32767)));}
      put16(b,156,static_cast<std::uint16_t>(number(wheel_x_)));put16(b,158,static_cast<std::uint16_t>(number(wheel_y_)));
    }wheel_x_=wheel_y_=0;bus_.outgoing({Kind::Input,epoch_,std::move(b)});last_input_=Clock::now();
  }
  void send_metrics(){const auto dropped=bus_.dropped();Bytes b(32,0);put64(b,0,presented_);put64(b,8,dropped[0]);put64(b,16,dropped[1]);put32(b,24,reason_);put32(b,28,playing_?1:0);bus_.outgoing({Kind::Metrics,epoch_,std::move(b)});last_metrics_=Clock::now();}
public:
  Host(Bus& bus,Owner& owner,std::uint32_t w,std::uint32_t h,std::uint32_t volume,bool haptics):bus_(bus),owner_(owner),max_width_(w),max_height_(h),volume_ceiling_(volume),haptics_(haptics){
    check(SDL_Init(SDL_INIT_VIDEO|SDL_INIT_AUDIO));SDL_InitSubSystem(SDL_INIT_GAMEPAD);window_=SDL_CreateWindow("OmniPlay",std::min(960,static_cast<int>(w)),std::min(720,static_cast<int>(h)),SDL_WINDOW_RESIZABLE);check(window_);renderer_=SDL_CreateRenderer(window_,nullptr);check(renderer_);width_=std::min(w,640u);height_=std::min(h,480u);logical_width_=width_;check(SDL_SetRenderLogicalPresentation(renderer_,static_cast<int>(logical_width_),static_cast<int>(height_),SDL_LOGICAL_PRESENTATION_LETTERBOX));SDL_AudioSpec spec{SDL_AUDIO_S16LE,2,48000};audio_=SDL_OpenAudioDeviceStream(SDL_AUDIO_DEVICE_DEFAULT_PLAYBACK,&spec,nullptr,nullptr);check(audio_&&SDL_SetAudioStreamGain(audio_,static_cast<float>(volume)/100.0f)&&SDL_ResumeAudioStreamDevice(audio_));focused_=SDL_GetKeyboardFocus()==window_;update_pads();
  }
  Pending ready(){Bytes b(24,0);put32(b,0,kProtocol);put32(b,4,max_width_);put32(b,8,max_height_);put32(b,12,192000);put32(b,16,kMaxAudioFrames);put32(b,20,haptics_?1:0);return{Kind::Ready,epoch_,std::move(b)};}
  void run(){
    while(!quit_&&!bus_.closed()){
      if(owner_.stop_requested())break;
      if(owner_.take_focus()){
        const auto flags=SDL_GetWindowFlags(window_);
        if((flags&SDL_WINDOW_MINIMIZED)!=0)SDL_RestoreWindow(window_);
        if(!SDL_RaiseWindow(window_))reason_=4;
        SDL_SyncWindow(window_);
      }
      SDL_Event event;while(SDL_PollEvent(&event)){
        if(event.type==SDL_EVENT_QUIT||(event.type==SDL_EVENT_WINDOW_CLOSE_REQUESTED&&event.window.windowID==SDL_GetWindowID(window_))){reason_=1;quit_=true;}
        else if((event.type==SDL_EVENT_WINDOW_FOCUS_LOST||event.type==SDL_EVENT_WINDOW_MINIMIZED||event.type==SDL_EVENT_WINDOW_HIDDEN)&&event.window.windowID==SDL_GetWindowID(window_)){focused_=false;fence(2);}
        else if((event.type==SDL_EVENT_WINDOW_FOCUS_GAINED||event.type==SDL_EVENT_WINDOW_RESTORED||event.type==SDL_EVENT_WINDOW_SHOWN)&&event.window.windowID==SDL_GetWindowID(window_)){
          const auto flags=SDL_GetWindowFlags(window_);
          const bool focused=SDL_GetKeyboardFocus()==window_&&(flags&(SDL_WINDOW_HIDDEN|SDL_WINDOW_MINIMIZED))==0;
          if(focused&&!focused_){focused_=true;fence(2);}
        }
        else if((event.type==SDL_EVENT_RENDER_DEVICE_LOST||event.type==SDL_EVENT_RENDER_DEVICE_RESET||event.type==SDL_EVENT_RENDER_TARGETS_RESET)&&event.render.windowID==SDL_GetWindowID(window_)){fence(4);throw Failure();}
        else if(event.type==SDL_EVENT_GAMEPAD_REMOVED){const bool owned=std::any_of(pads_.begin(),pads_.end(),[&](auto* pad){return pad&&SDL_GetGamepadID(pad)==event.gdevice.which;});update_pads();if(owned)fence(2);}
        else if(event.type==SDL_EVENT_GAMEPAD_ADDED)update_pads();
        else if(event.type==SDL_EVENT_KEY_DOWN&&!event.key.repeat&&event.key.windowID==SDL_GetWindowID(window_)&&event.key.scancode==SDL_SCANCODE_F11){if(!SDL_SetWindowFullscreen(window_,!(SDL_GetWindowFlags(window_)&SDL_WINDOW_FULLSCREEN)))reason_=4;}
        else if(event.type==SDL_EVENT_KEY_DOWN&&!event.key.repeat&&event.key.windowID==SDL_GetWindowID(window_)&&event.key.scancode==SDL_SCANCODE_ESCAPE){if(playing_)fence(2);}
        else if(event.type==SDL_EVENT_MOUSE_WHEEL&&event.wheel.windowID==SDL_GetWindowID(window_)){wheel_x_=std::clamp(wheel_x_+event.wheel.x,-32767.0f,32767.0f);wheel_y_=std::clamp(wheel_y_+event.wheel.y,-32767.0f,32767.0f);}
        else if((event.type==SDL_EVENT_FINGER_DOWN||event.type==SDL_EVENT_FINGER_MOTION||event.type==SDL_EVENT_FINGER_UP||event.type==SDL_EVENT_FINGER_CANCELED)&&event.tfinger.windowID==SDL_GetWindowID(window_)){if(event.type==SDL_EVENT_FINGER_DOWN&&!finger_down_){finger_=event.tfinger.fingerID;finger_down_=true;}if(finger_down_&&finger_==event.tfinger.fingerID){finger_x_=event.tfinger.x;finger_y_=event.tfinger.y;if(event.type==SDL_EVENT_FINGER_UP||event.type==SDL_EVENT_FINGER_CANCELED)finger_down_=false;}}
      }
      if(quit_||bus_.closed())break;
      const auto flags=SDL_GetWindowFlags(window_);
      const bool focused=SDL_GetKeyboardFocus()==window_&&(flags&(SDL_WINDOW_HIDDEN|SDL_WINDOW_MINIMIZED))==0;
      if(focused!=focused_){focused_=focused;fence(2);}const auto now=Clock::now();if(playing_&&now-last_activity_>std::chrono::milliseconds(250))fence(3);
      const bool present=now-last_video_>=std::chrono::microseconds(8333);auto batch=bus_.take(present);for(const auto& message:batch.controls){control(message);if(quit_||bus_.closed())break;}if(quit_||bus_.closed())break;for(const auto& message:batch.audio){if(bus_.closed())break;audio(message);}if(bus_.closed())break;if(batch.video){video(*batch.video);last_video_=Clock::now();}
      if(now-last_pads_>=std::chrono::seconds(1)){
        update_pads();last_pads_=now;SDL_AudioSpec spec{};
        const auto device=SDL_GetAudioStreamDevice(audio_);
        if(!device||!SDL_GetAudioDeviceFormat(device,&spec,nullptr)){fence(4);throw Failure();}
      }
      if(now-last_input_>=std::chrono::microseconds(8333))send_input();if(now-last_metrics_>=std::chrono::seconds(1))send_metrics();SDL_Delay(1);
    }
  }
  void shutdown(){playing_=false;focused_=false;stop_haptics();for(auto*& pad:pads_)if(pad){SDL_CloseGamepad(pad);pad=nullptr;}if(audio_){SDL_DestroyAudioStream(audio_);audio_=nullptr;}if(texture_){SDL_DestroyTexture(texture_);texture_=nullptr;}if(renderer_){SDL_DestroyRenderer(renderer_);renderer_=nullptr;}if(window_){SDL_DestroyWindow(window_);window_=nullptr;}SDL_Quit();}
};
static unsigned argument(const char* raw,unsigned maximum,bool zero=false){check(raw&&std::strlen(raw)>0&&std::strlen(raw)<=4&&(raw[0]!='0'||raw[1]=='\0'));unsigned value=0;const auto end=raw+std::strlen(raw);const auto result=std::from_chars(raw,end,value);check(result.ec==std::errc{}&&result.ptr==end&&value<=maximum&&(zero||value>0));return value;}
}
int main(int argc,char** argv){
  using namespace omni::presentation;
#ifdef __APPLE__
  if(!omni::macos::establish_child())std::_Exit(2);
#endif
  try{
    const bool gated=argc==8&&std::strcmp(argv[1],"--stdio-presentation-v2-gated")==0;check(gated||(argc==7&&std::strcmp(argv[1],"--stdio-presentation-v2")==0));const auto session=session_from_text(std::span<const char>(argv[2],std::strlen(argv[2])));const auto width=argument(argv[3],kMaxDimension),height=argument(argv[4],kMaxDimension),volume=argument(argv[5],100,true);const bool haptics=std::strcmp(argv[6],"haptics")==0;check(haptics||std::strcmp(argv[6],"no-haptics")==0);Owner owner;
    if(gated){const auto length=std::strlen(argv[7]);check(length>0&&length<=10&&argv[7][0]!='0');std::uint32_t parent=0;const auto parsed=std::from_chars(argv[7],argv[7]+length,parent);check(parsed.ec==std::errc{}&&parsed.ptr==argv[7]+length&&parent>1&&parent<=0x7fffffffu);owner=Owner::gated(session,parent);}
    Duplex transport(session,width,height);check(transport.initialize());SDL_SetLogOutputFunction([](void*,int,SDL_LogPriority,const char*){},nullptr);SDL_SetMainReady();Bus bus;Host host(bus,owner,width,height,volume,haptics);const auto ready=host.ready();
    // Both blocking pipe threads refer to process-lifetime state. Catch inside
    // its scope, close SDL resources, then exit without unwinding that state
    // beneath a reader/writer still blocked in the operating system.
    try{
      std::thread([&]{try{transport.write(ready.kind,ready.epoch,ready.body,Direction::HostToActor);Pending message;while(bus.next(message))transport.write(message.kind,message.epoch,message.body,Direction::HostToActor);}catch(...){bus.close();}}).detach();
      std::thread([&]{try{Message message;while(transport.read(message,Direction::ActorToHost)){bus.incoming(std::move(message));message=Message{};}}catch(...){}bus.close();}).detach();
      host.run();host.shutdown();bus.close();std::_Exit(0);
    }catch(...){host.shutdown();bus.close();std::_Exit(1);}
  }catch(...){SDL_Quit();std::_Exit(1);}
}
