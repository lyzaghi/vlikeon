// Qualification-only private presentation peer. No window, device or real core.
#include "wire.hpp"
#include <atomic>
#include <chrono>
#include <cstdio>
#include <cstring>
#include <thread>

using namespace omni::presentation;
int main(int argc,char** argv){
  if(argc!=7||std::strcmp(argv[1],"--stdio-presentation-v2")||std::strcmp(argv[3],"64")||std::strcmp(argv[4],"64")||std::strcmp(argv[5],"50")||std::strcmp(argv[6],"no-haptics"))return 2;
  try{
    Duplex pipe(session_from_text(std::span<const char>(argv[2],std::strlen(argv[2]))),64,64);
    if(!pipe.initialize())return 2;
    std::atomic<bool> stopped=false,playing=false;
    std::atomic<std::uint64_t> videos=0,audios=0;
    std::thread writer([&]{try{
      Bytes ready(24);put32(ready,0,kProtocol);put32(ready,4,64);put32(ready,8,64);put32(ready,12,192000);put32(ready,16,kMaxAudioFrames);
      pipe.write(Kind::Ready,1,ready,Direction::HostToActor);
      while(!stopped.load()){
        Bytes input(kInputBytes);input[144]=1;input[29/8]=1u<<(29%8); // Shared input wire: KeyZ held.
        pipe.write(Kind::Input,1,input,Direction::HostToActor);
        Bytes metrics(32);put64(metrics,0,videos.load());put32(metrics,28,playing.load()?1:0);
        pipe.write(Kind::Metrics,1,metrics,Direction::HostToActor);
        std::this_thread::sleep_for(std::chrono::milliseconds(25));
      }
    }catch(...){stopped=true;}});
    bool valid=true;
    try{
      Message message;
      while(pipe.read(message,Direction::ActorToHost)){
        if(message.kind==Kind::Stop)break;
        if(message.kind==Kind::State)playing=message.body[0]!=0;
        if(message.kind==Kind::Video){
          check(u32(message.body,0)==16&&u32(message.body,4)==16&&u32(message.body,32)==0xff345678u);++videos;
        }
        if(message.kind==Kind::Audio){
          check(u32(message.body,0)==48000&&u32(message.body,4)==64&&u16(message.body,16)==123&&u16(message.body,18)==65413);++audios;
        }
      }
    }catch(...){valid=false;}
    stopped=true;writer.join();
    // Fixed test-owned path in the helper's separately staged working directory.
    auto* receipt=std::fopen("presentation-receipt.json","wb");if(!receipt)return 3;
    const bool wrote=std::fprintf(receipt,"{\"valid\":%s,\"video\":%llu,\"audio\":%llu}\n",valid?"true":"false",static_cast<unsigned long long>(videos.load()),static_cast<unsigned long long>(audios.load()))>0;
    const bool closed=std::fclose(receipt)==0;
    return valid&&wrote&&closed?0:1;
  }catch(...){return 1;}
}
