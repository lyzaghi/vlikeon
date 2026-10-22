#pragma once
#include <cstdint>
#include <filesystem>
#include <span>
#include <string>
#include <vector>

namespace omni {
class MemoryFiles;
struct SaveFileShape{std::string name;std::uint64_t bytes;};
// The directory is confined to private session work. Its archive is a save blob,
// never a destination chosen by renderer input or an ordinary archive extractor.
class SavedFiles {
  std::filesystem::path root_;
  MemoryFiles* memory_=nullptr;
public:
  void prepare(const std::string& relative,MemoryFiles* memory=nullptr);
  bool enabled()const{return !root_.empty();}
  std::vector<std::uint8_t> read(const std::vector<SaveFileShape>* expected=nullptr)const;
  void replace(std::span<const std::uint8_t> archive)const;
  static void validate(std::span<const std::uint8_t> archive);
};
}
