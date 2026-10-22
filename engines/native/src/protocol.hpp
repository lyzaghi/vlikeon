#pragma once
#include <cstdint>
#include <condition_variable>
#include <deque>
#include <mutex>
#include <vector>
#include <nlohmann/json.hpp>

namespace omni {
using Json=nlohmann::json;
constexpr std::size_t kMaxSave=256u*1024u*1024u;
constexpr std::size_t kMaxControlHeader=64u*1024u;
constexpr std::uint64_t kMaxControlSequence=0x7fffffffffffffffULL;
bool initialize_transport();
struct Packet {Json header;std::vector<std::uint8_t> payload;};
class Inbox {
  std::mutex mutex_;std::condition_variable changed_;std::deque<Packet> queue_;bool closed_=false,failed_=false,in_flight_=false;
public:
  void read_stdin();
  bool pop(Packet& packet);
  void complete();
  bool closed();
  bool failed();
};
void reply(std::uint64_t id,const Json& result,const std::vector<std::uint8_t>& payload={});
void reply_error(std::uint64_t id,const char* code);
}
