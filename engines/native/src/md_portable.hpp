#pragma once
#include "saved_files.hpp"
#include <array>
#include <cstdint>
#include <span>
#include <vector>
namespace omni {
inline constexpr const char* md_portable_format="ares-md-canonical-files-v1";
struct MdSaveContract {
  std::array<std::uint32_t,6> words{};
  bool operator==(const MdSaveContract&)const=default;
  static MdSaveContract read(void(*describe)(std::uint32_t*));
  std::vector<SaveFileShape> shapes()const;
  void validate(std::span<const std::uint8_t> archive)const;
  static std::span<const std::uint8_t> files(std::span<const std::uint8_t> payload);
};
}
