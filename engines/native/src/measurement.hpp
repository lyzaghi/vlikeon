#pragma once
#include "protocol.hpp"
#include <chrono>
#include <string>

namespace omni {
// Owned by the frontend thread. It records frontend frame-completion intervals,
// not GPU timestamps, controller latency or an independently attested benchmark.
class Measurement {
  using Clock=std::chrono::steady_clock;
  enum class State{Empty,Warming,Measuring,Complete,Cancelled};
  State state_=State::Empty;std::string id_,reason_;
  Clock::time_point start_{},begin_{},previous_{};
  double nominal_=0;std::uint32_t warmup_=0,duration_=0,presented_=0,dropped_=0,underruns_=0;
  std::uint64_t first_presented_=0,first_dropped_=0,first_underruns_=0;
  std::vector<double> intervals_;
public:
  bool active()const{return state_==State::Warming||state_==State::Measuring;}
  void start(const std::string& id,double nominal);
  void cancel(const char* reason);
  void frame(Clock::time_point now,double nominal,std::uint64_t presented,std::uint64_t dropped,std::uint64_t underruns);
  Json status()const;
  Json result()const;
  std::vector<std::uint8_t> payload()const;
};
}
