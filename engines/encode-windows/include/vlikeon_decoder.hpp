#pragma once
#include <Windows.h>
#include <mfidl.h>
#include <mftransform.h>
#include <wrl/client.h>
#include <cstdint>
#include <deque>
#include <memory>
#include <mutex>
#include <span>
#include <vector>

namespace vlikeon::video {
struct DecoderConfiguration{uint32_t width,height,fps,remaining_ms;};
struct DecodedFrame{
  uint32_t width,height,stride;
  int64_t timestamp_100ns;
  uint64_t received_tick_ms; // Original authenticated receiver receipt age.
  std::vector<uint8_t> nv12;
};
// Private decoder backend, not receiver/codec/consent admission. The native host
// first validates authenticated negotiated progressive SDR BT.709 H.264 access
// units, SPS/PPS/profile/level/VUI and complete transport assembly. No raw socket.
// All methods/destruction run on one COM MTA thread except close_delivery.
// Blocking OS/codec calls require an externally supervised helper process.
class Decoder final{
public:
  static HRESULT create(DecoderConfiguration,std::unique_ptr<Decoder>&);
  ~Decoder();
  Decoder(const Decoder&)=delete;
  Decoder& operator=(const Decoder&)=delete;
  // Complete admitted Annex B access unit; original local receipt time must
  // survive reorder/IPC. S_FALSE means not consumed: retain/retry in order or
  // retire the generation, never skip a dependent encoded picture.
  HRESULT submit(std::span<const uint8_t>,int64_t timestamp_100ns,uint64_t received_tick_ms);
  HRESULT poll();
  HRESULT take(DecodedFrame&);
  void close_delivery();
  HRESULT stop(); // Failed teardown retains COM references in the live object.
private:
  explicit Decoder(DecoderConfiguration config):config_(config),thread_(GetCurrentThreadId()){}
  HRESULT live();HRESULT terminal(HRESULT);HRESULT select_output();HRESULT read_output();
  HRESULT copy_frame(IMFSample*,DecodedFrame&);
  struct Pending{int64_t pts;ULONGLONG received;};
  DecoderConfiguration config_;DWORD thread_;ULONGLONG deadline_=0;
  bool mf_started_=false,streaming_=false;int64_t last_input_=-1;
  uint32_t coded_height_=0,stride_=0;
  Microsoft::WRL::ComPtr<IMFTransform> transform_;
  std::deque<Pending> pending_;std::mutex output_mutex_;bool closed_=false;
  std::deque<DecodedFrame> frames_;
};
}
