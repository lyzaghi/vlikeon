#pragma once

#include "catalog.hpp"
#include "session.hpp"
#include <cstdint>
#include <filesystem>
#include <optional>
#include <string>

namespace omniplay::macos {
struct LaunchParameters {
  std::string session_id;
  uint32_t maximum_width;
  uint32_t maximum_height;
  uint32_t volume;
  bool haptics;
};

struct LaunchEndpoints {
  int input = -1;
  int output = -1;
  LaunchEndpoints(int input_descriptor, int output_descriptor) noexcept;
  ~LaunchEndpoints();
  LaunchEndpoints(const LaunchEndpoints&) = delete;
  LaunchEndpoints& operator=(const LaunchEndpoints&) = delete;
  LaunchEndpoints(LaunchEndpoints&& other) noexcept;
  LaunchEndpoints& operator=(LaunchEndpoints&& other) noexcept;
  int take_input() noexcept;
  int take_output() noexcept;
};

// Endpoints carry the bounded actor control/save protocol. Process authority
// stays with the exact authenticated XPC connection which created this pair.
std::optional<LaunchEndpoints> launch_pair(const RuntimeSpec& runtime,
    SessionLease session, const LaunchParameters& parameters,
    uint64_t connection_owner);
bool session_active(const std::string& session_id);
bool stop_session(const std::string& session_id, uint64_t connection_owner);
bool activate_session(const std::string& session_id, uint64_t connection_owner);
void stop_connection(uint64_t connection_owner);
}
