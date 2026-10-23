#pragma once

#include <optional>
#include <string>
#include <sys/types.h>

namespace omniplay::macos {
struct RuntimeSpec {
  std::string id;
  std::string team_identifier;
  std::string actor_path;
  std::string actor_identifier;
  std::string presentation_path;
  std::string presentation_identifier;
  std::string core_path;
  std::string core_identifier;
};

// Reads the catalog sealed into the owner bundle and validates the selected
// code objects. Call again immediately before process creation; callers must
// not treat an earlier result as a durable code-signing admission.
std::optional<RuntimeSpec> admit_runtime(const char* runtime_id);
// Additional admission for a direct child created suspended by this owner and
// not yet reaped. Never authenticate/adopt a PID received over XPC or on disk.
bool admit_owned_child(const RuntimeSpec& runtime, bool presentation, pid_t pid);
}
