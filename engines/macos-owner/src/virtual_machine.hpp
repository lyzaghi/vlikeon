#pragma once
#include <atomic>
#include <filesystem>
#include <memory>
#include <string>
#include <cstdint>

namespace omniplay::macos {
struct GuestImage {
  std::string kernel;
  std::string initrd;
  uint32_t memory_mb = 0;
  uint32_t cpu_count = 0;
  uint32_t scratch_mb = 0;
};
// All VZ operations run on a private serial queue. No guest process is a host
// child, and no writable host directory, network or physical device is exposed.
class VirtualMachine {
 public:
  struct State;
  static std::shared_ptr<VirtualMachine> launch(const GuestImage& image,
      const std::filesystem::path& inputs, const std::string& session,
      int control_read, int control_write, int presentation_read, int presentation_write);
  ~VirtualMachine();
  void stop();
  bool ended() const;
  bool failed() const;
 private:
  explicit VirtualMachine(std::shared_ptr<State> state);
  std::shared_ptr<State> state_;
};
}
