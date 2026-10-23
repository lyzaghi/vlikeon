#pragma once
#include <xpc/xpc.h>
#include <cstdint>
#include <filesystem>
#include <optional>
#include <string>

namespace omniplay::macos {
enum class StageStatus { ok, rejected, storage_failure };
StageStatus stage_session(xpc_object_t request);
StageStatus discard_session(xpc_object_t request);
class SessionLease {
 public:
  ~SessionLease();
  SessionLease(const SessionLease&) = delete;
  SessionLease& operator=(const SessionLease&) = delete;
  SessionLease(SessionLease&& other) noexcept;
  SessionLease& operator=(SessionLease&&) = delete;
  const std::string& id() const { return session_; }
  const std::string& runtime() const { return runtime_; }
  const std::filesystem::path& directory() const { return path_; }
  bool begin_launch();
  // Caller must have confirmed every owned direct child through waitpid.
  // Destruction alone NEVER removes the durable launch fence.
  bool finish_launch();
 private:
  SessionLease() = default;
  bool valid() const;
  int directory_ = -1, lease_ = -1;
  uint64_t device_ = 0, inode_ = 0, lease_device_ = 0, lease_inode_ = 0;
  uint64_t marker_device_ = 0, marker_inode_ = 0;
  bool marked_ = false, marker_unlinked_ = false;
  std::string session_, runtime_;
  std::filesystem::path path_;
  friend std::optional<SessionLease> admit_staged_session(const std::string&, const std::string&);
};
std::optional<SessionLease> admit_staged_session(const std::string& session, const std::string& runtime);
}
