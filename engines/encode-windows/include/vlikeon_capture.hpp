#pragma once
#include <Windows.h>
#include <cstdint>
#include <functional>
#include <memory>
#include <vector>

namespace vlikeon::video {
struct CaptureConfiguration{uint32_t width,height,fps,remaining_ms;};
struct BgraFrame{
  uint32_t width,height,stride;
  uint64_t timestamp_us,received_tick_ms; // Preserve original native callback age.
  std::vector<uint8_t> bytes;
};
// Private source candidate for a trusted native host retaining the original game
// process handle. Creation presents a local, default-No confirmation naming only
// that process's exact window. No renderer PID/title is capture authority.
// Factory/take/stop/destruction use one dedicated COM MTA owner thread; only
// close_delivery may run elsewhere. OS/driver waits need external supervision.
// The host-alive callback runs on owner and WGC callback threads; it must be
// thread-safe/nonblocking and must not infer authority from arriving media.
class WindowCapture final{
public:
  static HRESULT confirm_and_create(HANDLE original_game,HWND window,
      CaptureConfiguration,std::unique_ptr<WindowCapture>&,std::function<bool()> host_alive);
  ~WindowCapture();
  WindowCapture(const WindowCapture&)=delete;
  WindowCapture& operator=(const WindowCapture&)=delete;
  // Latest raw frame, aspect-preserving GPU scale with black letterboxing and
  // opaque BGRA output. S_FALSE means no fresh/within-FPS-budget frame yet.
  // SDR-monitor admission is conservative; actual colour output is unqualified.
  HRESULT take(BgraFrame&);
  void close_delivery();
  HRESULT stop(); // API Close acknowledgement only; not a process-exit receipt.
private:
  struct Impl;std::unique_ptr<Impl> impl_;
  WindowCapture();
};
}
