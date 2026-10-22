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
bool initialize_transport();
struct Packet {Json header;std::vector<std::uint8_t> payload;};
class Inbox {
  std::mutex mutex_;std::condition_variable changed_;std::deque<Packet> queue_;bool closed_=false;
public:
  void read_stdin();
  bool pop(Packet& packet);
  bool closed();
};
void reply(std::uint64_t id,const Json& result,const std::vector<std::uint8_t>& payload={});
void reply_error(std::uint64_t id,const char* code);
}
