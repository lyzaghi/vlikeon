#pragma once
#include "wire.hpp"
#include <chrono>
#include <memory>

namespace omni::presentation {
struct Admission{Session session;std::uintptr_t input,output;};
struct Snapshot{
  bool ready=false,fresh=false,focused=false,haptics_admitted=false,unavailable=true;
  std::uint32_t maximum_width=0,maximum_height=0;std::uint64_t epoch=0;
  std::array<std::uint8_t,kInputBytes> input{};
  std::uint64_t presented=0,dropped_video=0,dropped_audio=0;std::uint32_t reason=0;bool playing=false;
};
class Client{
  struct State;std::shared_ptr<State> state_;
public:
  explicit Client(const Admission&);
  Client(const Client&)=delete;Client& operator=(const Client&)=delete;
  ~Client();
  bool wait_ready(std::chrono::milliseconds timeout);
  bool wait_focused(std::chrono::milliseconds timeout);
  Snapshot snapshot()const;
  // The core loop must refresh this after it makes actual progress. A pipe
  // thread alone cannot keep a hung core's host heartbeat alive indefinitely.
  void progress();
  bool video(std::span<const std::uint32_t> argb,unsigned width,unsigned height,unsigned logical_width,std::uint64_t timestamp_us);
  bool audio(std::span<const std::int16_t> stereo,unsigned sample_rate);
  bool playback(bool running,unsigned volume_percent,unsigned fullscreen_change=0);
  bool rumble(unsigned port,std::uint16_t strong,std::uint16_t weak,unsigned milliseconds);
  // This requests an asynchronous Stop. The broker must still own and
  // join/terminate the actual actor/helper processes; this is not a stop ack.
  void close();
};
}
