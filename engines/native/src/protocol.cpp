#include "protocol.hpp"
#include <algorithm>
#include <array>
#include <cstdio>
#include <limits>
#include <stdexcept>
#include <set>
#include <string_view>
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
  if(!SetStdHandle(STD_INPUT_HANDLE,reinterpret_cast<HANDLE>(_get_osfhandle(_fileno(stdin))))||
     !SetStdHandle(STD_OUTPUT_HANDLE,reinterpret_cast<HANDLE>(_get_osfhandle(_fileno(stdout)))))return false;
#else
  const int input=dup(fileno(stdin)),output=dup(fileno(stdout));
  if(input<0||output<0)return false;control_input=fdopen(input,"rb");control_output=fdopen(output,"wb");
  if(!control_input||!control_output||!std::freopen("/dev/null","rb",stdin)||!std::freopen("/dev/null","wb",stdout))return false;
#endif
  return true;
}
static bool read_exact(void* target,std::size_t bytes){return bytes==0||std::fread(target,1,bytes,control_input)==bytes;}
static bool length(std::uint32_t& value,bool boundary_eof){
  std::array<std::uint8_t,4> raw{};const auto got=std::fread(raw.data(),1,raw.size(),control_input);
  if(!got&&boundary_eof&&std::feof(control_input))return false;
  if(got!=raw.size())throw std::runtime_error("incomplete_control_frame");
  value=std::uint32_t(raw[0])|(std::uint32_t(raw[1])<<8)|(std::uint32_t(raw[2])<<16)|(std::uint32_t(raw[3])<<24);return true;
}
static bool read_part(std::vector<std::uint8_t>& out,std::size_t maximum,bool boundary_eof=false){
  std::uint32_t size=0;if(!length(size,boundary_eof))return false;
  if(size>maximum)throw std::runtime_error("control_frame_bound");
  out.resize(size);if(!read_exact(out.data(),size))throw std::runtime_error("incomplete_control_frame");return true;
}
static Json request_header(const std::vector<std::uint8_t>& raw,std::uint64_t expected){
  std::vector<std::set<std::string>> objects;
  auto callback=[&objects](int depth,Json::parse_event_t event,Json& value){
    if(depth<0||depth>32)throw std::runtime_error("control_json_depth");
    if(event==Json::parse_event_t::object_start)objects.emplace_back();
    else if(event==Json::parse_event_t::key){
      if(objects.empty()||!objects.back().insert(value.get<std::string>()).second)throw std::runtime_error("control_duplicate_key");
    }else if(event==Json::parse_event_t::object_end){if(objects.empty())throw std::runtime_error("control_json_shape");objects.pop_back();}
    return true;
  };
  auto header=Json::parse(raw.begin(),raw.end(),callback);
  if(!header.is_object()||header.size()!=4||!header.contains("protocol")||!header["protocol"].is_number_unsigned()||header["protocol"]!=1||
     !header.contains("id")||!header["id"].is_number_unsigned()||header["id"]!=expected||
     !header.contains("method")||!header["method"].is_string()||!header.contains("params")||!header["params"].is_object())throw std::runtime_error("control_header_shape");
  constexpr std::array<std::string_view,17> methods{"hello","prepare","start","resume","pause","cancel_close","status","metrics","measurement_start","measurement_status","measurement_cancel","measurement_read","disc_status","disc_select","save","load","stop"};
  const auto& method=header["method"].get_ref<const std::string&>();
  if(std::find(methods.begin(),methods.end(),std::string_view(method))==methods.end())throw std::runtime_error("control_method");
  return header;
}
static void write_part(const void* data,std::size_t size){
  if(size>std::numeric_limits<std::uint32_t>::max())throw std::runtime_error("frame_bound");
  const auto value=static_cast<std::uint32_t>(size);const std::array<std::uint8_t,4> raw{std::uint8_t(value),std::uint8_t(value>>8),std::uint8_t(value>>16),std::uint8_t(value>>24)};
  if(std::fwrite(raw.data(),1,4,control_output)!=4||(size&&std::fwrite(data,1,size,control_output)!=size))throw std::runtime_error("channel_closed");
}
void Inbox::read_stdin(){
  bool bad=false;std::uint64_t sequence=0;
  try {for(;;){
    {std::unique_lock lock(mutex_);changed_.wait(lock,[this]{return !in_flight_||closed_;});if(closed_)break;}
    std::vector<std::uint8_t> raw;Packet packet;
    if(!read_part(raw,kMaxControlHeader,true))break;
    if(sequence==kMaxControlSequence)throw std::runtime_error("control_sequence_exhausted");
    packet.header=request_header(raw,sequence+1);
    // Validate method/identity/JSON before reading a payload length or allocating
    // its body. Only load accepts binary input; no speculative next save buffer.
    read_part(packet.payload,packet.header["method"]=="load"?kMaxSave:0);
    {std::lock_guard lock(mutex_);queue_.push_back(std::move(packet));in_flight_=true;++sequence;}changed_.notify_all();
  }}catch(...){bad=true;/* Never log control payloads or child-controlled text. */}
  {std::lock_guard lock(mutex_);closed_=true;failed_=bad;}changed_.notify_all();
}
bool Inbox::pop(Packet& packet){std::lock_guard lock(mutex_);if(queue_.empty())return false;packet=std::move(queue_.front());queue_.pop_front();return true;}
void Inbox::complete(){std::lock_guard lock(mutex_);in_flight_=false;changed_.notify_all();}
bool Inbox::closed(){std::lock_guard lock(mutex_);return closed_&&queue_.empty();}
bool Inbox::failed(){std::lock_guard lock(mutex_);return failed_;}
void reply(std::uint64_t id,const Json& result,const std::vector<std::uint8_t>& payload){const auto header=Json{{"protocol",1},{"id",id},{"status","ok"},{"result",result}}.dump();if(header.size()>64u*1024u||payload.size()>kMaxSave)throw std::runtime_error("frame_bound");write_part(header.data(),header.size());write_part(payload.data(),payload.size());if(std::fflush(control_output)!=0)throw std::runtime_error("channel_closed");}
void reply_error(std::uint64_t id,const char* code){const auto header=Json{{"protocol",1},{"id",id},{"status","error"},{"error_code",code}}.dump();write_part(header.data(),header.size());write_part(nullptr,0);if(std::fflush(control_output)!=0)throw std::runtime_error("channel_closed");}
}
