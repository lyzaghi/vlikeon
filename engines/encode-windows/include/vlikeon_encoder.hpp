#pragma once
#include <Windows.h>
#include <mfidl.h>
#include <mftransform.h>
#include <strmif.h>
#include <wrl/client.h>
#include <cstdint>
#include <deque>
#include <memory>
#include <mutex>
#include <span>
#include <vector>

namespace vlikeon::video {
struct Configuration {
  uint32_t width, height, fps, bitrate;
  uint32_t remaining_ms;
  bool require_hardware;
};
struct Packet {
  std::vector<uint8_t> bytes;
  std::vector<uint8_t> sequence_header;
  int64_t timestamp_100ns;
  bool keyframe;
};
// Private installed-host library; not an RPC/permission or driver API.
// All calls except close_delivery run on one dedicated COM MTA owner thread,
// including destruction. Caller bounds OS waits using a separate helper process
// and checks live capture/owner/pair authority before input AND publication.
class Encoder final {
public:
  static HRESULT create(const Configuration&,std::unique_ptr<Encoder>&);
  ~Encoder();
  Encoder(const Encoder&)=delete;
  Encoder& operator=(const Encoder&)=delete;
  // Tight NV12; S_FALSE means input not accepted (raw frame can be discarded).
  // Never reuse/mutate an input buffer during this synchronous copy.
  HRESULT submit(std::span<const uint8_t> nv12,int64_t timestamp_100ns,bool keyframe);
  // Poll at least every 8ms while active, even with no arriving remote packets.
  // Uses nonblocking event reads; the underlying encoder methods can still stall.
  HRESULT poll();
  HRESULT take(Packet&); // S_FALSE: empty; failed HRESULT: terminal/expired.
  HRESULT output_type(IMFMediaType**); // +1 actual type for negotiation adapter.
  void close_delivery(); // any thread, immediate output fence; not OS stop proof.
  HRESULT stop(); // retains transform on failed teardown; caller retains Encoder.
  // Registered hardware-MFT classification, not runtime GPU/energy evidence.
  bool hardware_mft()const{return hardware_;}
private:
  using Clock=ULONGLONG;
  struct Pending{int64_t pts;Clock received;bool keyframe;};
  struct Ready{Packet packet;Clock received;};
  explicit Encoder(Configuration config):config_(config),thread_(GetCurrentThreadId()){}
  HRESULT configure(IMFActivate*);
  HRESULT read_output();
  HRESULT live();
  HRESULT terminal(HRESULT);
  bool owner()const{return thread_==GetCurrentThreadId();}
  Configuration config_;
  DWORD thread_,input_id_=0,output_id_=0;
  Clock deadline_=0;
  bool mf_started_=false,async_=false,hardware_=false,first_=true,streaming_=false;
  unsigned input_credits_=0;
  int64_t last_input_=-1;
  Microsoft::WRL::ComPtr<IMFActivate> activation_;
  Microsoft::WRL::ComPtr<IMFTransform> transform_;
  Microsoft::WRL::ComPtr<IMFMediaEventGenerator> events_;
  Microsoft::WRL::ComPtr<ICodecAPI> codec_;
  std::deque<Pending> pending_;
  std::mutex output_mutex_;
  bool closed_=false;
  size_t output_bytes_=0;
  std::deque<Ready> output_;
};
}
