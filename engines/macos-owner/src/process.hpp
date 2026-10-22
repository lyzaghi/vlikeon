#pragma once

#include "catalog.hpp"
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
};

// This process owner stays disconnected from the XPC protocol until checked
// save return and recovery are present alongside launch.
std::optional<LaunchEndpoints> launch_pair(const RuntimeSpec& runtime,
    const std::filesystem::path& session_directory, const LaunchParameters& parameters,
    uint64_t connection_owner);
bool session_active(const std::string& session_id);
void stop_session(const std::string& session_id);
void stop_connection(uint64_t connection_owner);
}
