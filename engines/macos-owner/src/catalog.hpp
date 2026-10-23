#pragma once

#include <optional>
#include <cstdint>
#include <string>
#include <sys/types.h>
#include "virtual_machine.hpp"

namespace omniplay::macos {
struct RuntimeSpec {
  std::string id;
  std::string team_identifier;
  std::string presentation_path;
  std::string presentation_identifier;
  GuestImage guest;
  std::string kernel_sha256;
  std::string initrd_sha256;
  uint32_t presentation_memory_mb;
};

// Reads the schema-2 catalog sealed into the owner bundle and validates guest
// images and the trusted presentation code. Call again before VM creation; callers must
// not treat an earlier result as a durable code-signing admission.
std::optional<RuntimeSpec> admit_runtime(const char* runtime_id);
// Additional admission for a direct child created suspended by this owner and
// not yet reaped. Never authenticate/adopt a PID received over XPC or on disk.
bool admit_owned_child(const RuntimeSpec& runtime, pid_t pid);
}
