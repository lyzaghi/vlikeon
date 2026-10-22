#include "engine.hpp"
#include "measurement.hpp"
#include <algorithm>
#include <bit>
#include <cmath>
#include <limits>

namespace omni {
void Measurement::start(const std::string& id,double nominal){
  if(active())throw Failure("invalid_state");
  if(id.size()!=36||id.find_first_not_of("0123456789abcdef-")!=std::string::npos||!std::isfinite(nominal)||nominal<1||nominal>240)throw Failure("invalid_state");
  id_=id;nominal_=nominal;reason_.clear();warmup_=duration_=presented_=dropped_=underruns_=0;
  intervals_.clear();intervals_.reserve(30000);start_=Clock::now();begin_=previous_={};state_=State::Warming;
}
void Measurement::cancel(const char* reason){
  if(!active())return;
  const auto origin=state_==State::Warming?start_:begin_;
  duration_=static_cast<std::uint32_t>(std::clamp<std::int64_t>(std::chrono::duration_cast<std::chrono::milliseconds>(Clock::now()-origin).count(),0,240000));
  reason_=reason;state_=State::Cancelled;intervals_.clear();
}
void Measurement::frame(Clock::time_point now,double nominal,std::uint64_t presented,std::uint64_t dropped,std::uint64_t underruns){
  if(!active())return;
  if(!std::isfinite(nominal)||std::abs(nominal-nominal_)>.0001){cancel("game_clock_changed");return;}
  if(state_==State::Warming){
    const auto elapsed=std::chrono::duration_cast<std::chrono::milliseconds>(now-start_).count();
    if(elapsed>60000){cancel("warmup_timeout");return;}if(elapsed<10000)return;
    warmup_=static_cast<std::uint32_t>(elapsed);begin_=previous_=now;
    first_presented_=presented;first_dropped_=dropped;first_underruns_=underruns;state_=State::Measuring;return;
  }
  const auto elapsed=std::chrono::duration_cast<std::chrono::milliseconds>(now-begin_).count();
  const auto interval=std::chrono::duration<double,std::milli>(now-previous_).count();
  if(elapsed>180000||intervals_.size()>=36000||!std::isfinite(interval)||interval<=0||interval>5000||presented<first_presented_||dropped<first_dropped_||underruns<first_underruns_){cancel("sample_bounds_exceeded");return;}
  const auto frames=presented-first_presented_,missed=dropped-first_dropped_,starved=underruns-first_underruns_;
  if(frames>std::numeric_limits<std::uint32_t>::max()||missed>std::numeric_limits<std::uint32_t>::max()||starved>std::numeric_limits<std::uint32_t>::max()){cancel("sample_bounds_exceeded");return;}
  intervals_.push_back(interval);previous_=now;duration_=static_cast<std::uint32_t>(elapsed);
  presented_=static_cast<std::uint32_t>(frames);dropped_=static_cast<std::uint32_t>(missed);underruns_=static_cast<std::uint32_t>(starved);
  if(elapsed>=120000){if(intervals_.size()<100){cancel("too_few_frames");return;}state_=State::Complete;}
}
Json Measurement::status()const{
  if(state_==State::Empty)throw Failure("invalid_state");
  const char* state=state_==State::Warming?"warming":state_==State::Measuring?"measuring":state_==State::Complete?"complete":"cancelled";
  auto elapsed=duration_;
  if(active())elapsed=static_cast<std::uint32_t>(std::clamp<std::int64_t>(std::chrono::duration_cast<std::chrono::milliseconds>(Clock::now()-(state_==State::Warming?start_:begin_)).count(),0,240000));
  return {{"id",id_},{"state",state},{"elapsed_ms",elapsed},{"warmup_ms",warmup_},{"target_ms",120000},{"cancel_reason",reason_.empty()?Json(nullptr):Json(reason_)}};
}
Json Measurement::result()const{
  if(state_!=State::Complete)throw Failure("invalid_state");
  return {{"id",id_},{"nominal_refresh_hz",nominal_},{"observed_duration_ms",duration_},{"warmup_ms",warmup_},{"frames_presented",presented_},{"frames_emulated",intervals_.size()},{"dropped_frames",dropped_},{"audio_underruns",underruns_},{"interval_count",intervals_.size()},{"interval_encoding","float64_le"}};
}
std::vector<std::uint8_t> Measurement::payload()const{
  if(state_!=State::Complete)throw Failure("invalid_state");
  static_assert(sizeof(double)==8&&std::numeric_limits<double>::is_iec559);
  std::vector<std::uint8_t> bytes;bytes.reserve(intervals_.size()*8);
  for(const auto interval:intervals_){const auto bits=std::bit_cast<std::uint64_t>(interval);for(unsigned shift=0;shift<64;shift+=8)bytes.push_back(static_cast<std::uint8_t>(bits>>shift));}return bytes;
}
}
