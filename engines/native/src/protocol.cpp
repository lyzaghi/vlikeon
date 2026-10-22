#include "protocol.hpp"
#include <array>
#include <cstdio>
#include <limits>
#include <stdexcept>
#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#include <windows.h>
#else
#include <unistd.h>
#endif

namespace omni {
static FILE* control_input=nullptr;static FILE* control_output=nullptr;
bool initialize_transport(){
  // Core libraries can use their own CRT or print directly to OS standard handles.
  // Keep private duplicates for protocol traffic, then redirect ordinary stdio before
  // loading any core. Legitimate upstream logging cannot corrupt control frames.
#ifdef _WIN32
  const int input=_dup(_fileno(stdin)),output=_dup(_fileno(stdout));
  if(input<0||output<0)return false;control_input=_fdopen(input,"rb");control_output=_fdopen(output,"wb");
  if(!control_input||!control_output||!std::freopen("NUL","rb",stdin)||!std::freopen("NUL","wb",stdout))return false;
  SetStdHandle(STD_INPUT_HANDLE,reinterpret_cast<HANDLE>(_get_osfhandle(_fileno(stdin))));
  SetStdHandle(STD_OUTPUT_HANDLE,reinterpret_cast<HANDLE>(_get_osfhandle(_fileno(stdout))));
#else
  const int input=dup(fileno(stdin)),output=dup(fileno(stdout));
  if(input<0||output<0)return false;control_input=fdopen(input,"rb");control_output=fdopen(output,"wb");
  if(!control_input||!control_output||!std::freopen("/dev/null","rb",stdin)||!std::freopen("/dev/null","wb",stdout))return false;
#endif
  return true;
}
static bool read_exact(void* target,std::size_t bytes){return bytes==0||std::fread(target,1,bytes,control_input)==bytes;}
static bool length(std::uint32_t& value){std::array<std::uint8_t,4> raw{};if(!read_exact(raw.data(),4))return false;value=std::uint32_t(raw[0])|(std::uint32_t(raw[1])<<8)|(std::uint32_t(raw[2])<<16)|(std::uint32_t(raw[3])<<24);return true;}
static bool read_part(std::vector<std::uint8_t>& out,std::size_t maximum){std::uint32_t size=0;if(!length(size)||size>maximum)return false;out.resize(size);return read_exact(out.data(),size);}
static void write_part(const void* data,std::size_t size){
  if(size>std::numeric_limits<std::uint32_t>::max())throw std::runtime_error("frame_bound");
  const auto value=static_cast<std::uint32_t>(size);const std::array<std::uint8_t,4> raw{std::uint8_t(value),std::uint8_t(value>>8),std::uint8_t(value>>16),std::uint8_t(value>>24)};
  if(std::fwrite(raw.data(),1,4,control_output)!=4||(size&&std::fwrite(data,1,size,control_output)!=size))throw std::runtime_error("channel_closed");
}
void Inbox::read_stdin(){
  try {for(;;){
    {std::unique_lock lock(mutex_);changed_.wait(lock,[this]{return queue_.empty()||closed_;});if(closed_)break;}
    std::vector<std::uint8_t> raw;Packet packet;
    if(!read_part(raw,64u*1024u)||raw.empty()||!read_part(packet.payload,kMaxSave))break;
    packet.header=Json::parse(raw.begin(),raw.end());
    if(!packet.header.is_object()||packet.header.value("protocol",0)!=1||!packet.header.contains("id")||!packet.header["id"].is_number_unsigned()||!packet.header.contains("method")||!packet.header["method"].is_string()||!packet.header.contains("params")||!packet.header["params"].is_object())break;
    {std::lock_guard lock(mutex_);queue_.push_back(std::move(packet));}changed_.notify_all();
  }}catch(...){/* Malformed input terminates the transport, never logs payload data. */}
  {std::lock_guard lock(mutex_);closed_=true;}changed_.notify_all();
}
bool Inbox::pop(Packet& packet){std::lock_guard lock(mutex_);if(queue_.empty())return false;packet=std::move(queue_.front());queue_.pop_front();changed_.notify_all();return true;}
bool Inbox::closed(){std::lock_guard lock(mutex_);return closed_&&queue_.empty();}
void reply(std::uint64_t id,const Json& result,const std::vector<std::uint8_t>& payload){const auto header=Json{{"protocol",1},{"id",id},{"status","ok"},{"result",result}}.dump();if(header.size()>64u*1024u||payload.size()>kMaxSave)throw std::runtime_error("frame_bound");write_part(header.data(),header.size());write_part(payload.data(),payload.size());if(std::fflush(control_output)!=0)throw std::runtime_error("channel_closed");}
void reply_error(std::uint64_t id,const char* code){const auto header=Json{{"protocol",1},{"id",id},{"status","error"},{"error_code",code}}.dump();write_part(header.data(),header.size());write_part(nullptr,0);if(std::fflush(control_output)!=0)throw std::runtime_error("channel_closed");}
}
