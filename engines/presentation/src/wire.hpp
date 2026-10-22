#pragma once
#include <array>
#include <cstdint>
#include <cstdio>
#include <span>
#include <stdexcept>
#include <vector>

namespace omni::presentation {
using Bytes=std::vector<std::uint8_t>;
using Session=std::array<std::uint8_t,16>;
constexpr std::uint32_t kMaxDimension=4096,kMaxFrame=4096u*4096u*4u,kMaxAudioFrames=8192;
constexpr std::size_t kHeaderBytes=56,kInputBytes=160;
constexpr std::uint16_t kProtocol=2;
constexpr std::uint64_t kMaxSequence=0x7fffffffffffffffULL;
enum class Kind:std::uint16_t{Video=1,Audio=2,State=3,Rumble=4,Heartbeat=5,Stop=6,Ready=128,Input=129,Metrics=130};
enum class Direction{ActorToHost,HostToActor};
struct Message{Kind kind;std::uint64_t sequence=0,epoch=0;Bytes body;};
struct Failure:std::runtime_error{Failure():std::runtime_error("presentation_boundary") {}};
inline void check(bool value){if(!value)throw Failure();}
std::uint16_t u16(std::span<const std::uint8_t>,std::size_t);
std::uint32_t u32(std::span<const std::uint8_t>,std::size_t);
std::uint64_t u64(std::span<const std::uint8_t>,std::size_t);
void put16(std::span<std::uint8_t>,std::size_t,std::uint16_t);
void put32(std::span<std::uint8_t>,std::size_t,std::uint32_t);
void put64(std::span<std::uint8_t>,std::size_t,std::uint64_t);
Session session_from_text(std::span<const char>);
std::size_t body_limit(Kind,Direction);
void validate(const Message&,Direction);

// Binary handles are privately duplicated before SDL initialization. No path,
// native handle, environment value, JSON or executable command exists on wire.
class Duplex{
  FILE* input_=nullptr;FILE* output_=nullptr;Session session_{};std::uint64_t received_=0,sent_=0;std::size_t video_bytes_=kMaxFrame;
public:
  explicit Duplex(Session session,std::uint32_t width=kMaxDimension,std::uint32_t height=kMaxDimension):session_(session){bool nonzero=false;for(const auto byte:session)nonzero|=byte!=0;check(nonzero&&width>0&&width<=kMaxDimension&&height>0&&height<=kMaxDimension);video_bytes_=std::size_t(width)*height*4;}
  ~Duplex(){if(input_)std::fclose(input_);if(output_)std::fclose(output_);}
  Duplex(const Duplex&)=delete;Duplex& operator=(const Duplex&)=delete;
  bool initialize();
  bool initialize_inherited(std::uintptr_t input,std::uintptr_t output);
  bool read(Message&,Direction);
  void write(Kind,std::uint64_t,std::span<const std::uint8_t>,Direction);
};
}
