#pragma once
#include <optional>
#include <string>
namespace omniplay::macos {
// Kernel boot-session identity; never a renderer/environment/disk-supplied UUID.
// Failure does not authorize launch or reclamation.
std::optional<std::string> boot_session();
}
