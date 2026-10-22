#pragma once
#include <algorithm>
#include <array>
#include <atomic>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <span>

namespace omni {
// One guest producer and one SDL-serialized consumer. Reset requires the guest
// pause barrier AND the SDL stream lock; it is not a concurrent ring operation.
class AudioRing {
public:
  static constexpr std::uint64_t capacity=65536;
  static_assert((capacity&(capacity-1))==0); // Unsigned counter wrap preserves slot indices.
  static constexpr std::size_t batch_limit=16384,output_limit=4096;
  struct WriteReceipt {std::size_t frames=0;bool valid=false;};
  enum class Delivery {Accepted,Rejected,Invalid};
  struct ReadReceipt {Delivery delivery;std::size_t frames=0,trimmed=0;bool underrun=false;};
private:
  std::array<std::int16_t,capacity*2> samples_{};
  std::atomic<std::uint64_t> read_{0},write_{0};
  bool primed_=false;
public:
  WriteReceipt push(std::span<const std::int16_t> stereo) noexcept {
    if(stereo.size()%2||stereo.size()/2>batch_limit)return {};
    const auto write=write_.load(std::memory_order_relaxed);
    const auto read=read_.load(std::memory_order_acquire);
    const auto queued=write-read;
    if(queued>capacity)return {};
    const auto count=std::min<std::uint64_t>(stereo.size()/2,capacity-queued);
    for(std::uint64_t n=0;n<count;++n){
      const auto index=((write+n)%capacity)*2;
      samples_[index]=stereo[n*2];samples_[index+1]=stereo[n*2+1];
    }
    write_.store(write+count,std::memory_order_release);
    return {static_cast<std::size_t>(count),true};
  }

  // The sink receives a private copy. Do not release ring slots, change priming
  // or count underruns until it accepts the complete offered output. A rejected
  // sink may itself have accepted an unknown prefix: the caller must retire or
  // clear that sink before retrying, never assume its failure is transactional.
  template<class Sink>
  ReadReceipt deliver(std::span<std::int16_t> output,std::uint64_t target,
                      std::uint64_t slack,float volume,Sink&& sink){
    if(output.empty()||output.size()%2||output.size()/2>output_limit||
       !target||target>capacity/2||slack>capacity||
       !std::isfinite(volume)||volume<0||volume>1)return {Delivery::Invalid};
    const auto original_read=read_.load(std::memory_order_relaxed);
    const auto write=write_.load(std::memory_order_acquire);
    auto queued=write-original_read;
    if(queued>capacity)return {Delivery::Invalid};
    auto read=original_read;
    std::size_t trimmed=0;
    if(queued>std::min<std::uint64_t>(capacity-1,target+slack)){
      trimmed=static_cast<std::size_t>(queued-target);read=write-target;queued=target;
    }
    const bool primed=primed_||queued>=target;
    const auto requested=output.size()/2;
    const auto count=primed?std::min<std::uint64_t>(requested,queued):0;
    std::fill(output.begin(),output.end(),0);
    for(std::uint64_t n=0;n<count;++n){
      const auto index=((read+n)%capacity)*2;
      output[n*2]=static_cast<std::int16_t>(std::lround(samples_[index]*volume));
      output[n*2+1]=static_cast<std::int16_t>(std::lround(samples_[index+1]*volume));
    }
    if(!sink(std::span<const std::int16_t>(output)))return {Delivery::Rejected};
    const bool underrun=primed&&count<requested;
    primed_=primed&&!underrun;
    read_.store(read+count,std::memory_order_release);
    return {Delivery::Accepted,static_cast<std::size_t>(count),trimmed,underrun};
  }

  void reset_quiescent() noexcept {
    read_.store(0,std::memory_order_relaxed);write_.store(0,std::memory_order_relaxed);primed_=false;
  }
};
}
