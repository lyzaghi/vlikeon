// Private-file bridge for the single-worker WASM adapter. The Pak keeps the
// upstream canonical byte order; a complete capture checks every declared file.
#pragma once
#include <cerrno>
#include <cstdio>
#include <cstring>
#include <cstdint>
#ifdef OMNIPLAY_NATIVE_MEMORY_FILES
#include "writable_files_v1.h"
#endif
#ifdef OMNIPLAY_NATIVE_FILES
extern "C" int om_md_native_publish(const char*, const char*);
#endif

namespace omni_md_files {
#ifdef OMNIPLAY_NATIVE_MEMORY_FILES
inline OmniWritableFilesV1 memory{};
inline bool installed=false;
inline bool install(const OmniWritableFilesV1* value){
  if(!value){memory={};installed=false;return true;}
  if(value->version!=1||value->struct_bytes!=sizeof(OmniWritableFilesV1)||!value->context||!value->stat||!value->mkdir||!value->open||!value->close||!value->read||!value->write||!value->resize||!value->flush||!value->remove||!value->rename||!value->list||!value->usage)return false;
  memory=*value;installed=true;return true;
}
#endif
inline constexpr const char* names[] = {"save.ram", "save.eeprom"};
inline const char* root() {
#ifdef OMNIPLAY_NATIVE_FILES
  return "save/MD";
#else
  return "/saves";
#endif
}
inline const char* path(const char* name) {
#ifdef OMNIPLAY_NATIVE_FILES
  if (!std::strcmp(name, names[0])) return "save/MD/save.ram";
  if (!std::strcmp(name, names[1])) return "save/MD/save.eeprom";
#else
  if (!std::strcmp(name, names[0])) return "/saves/save.ram";
  if (!std::strcmp(name, names[1])) return "/saves/save.eeprom";
#endif
  return nullptr;
}
inline bool restore(const char* name, std::span<uint8_t> bytes) {
  const char* destination = path(name);
  if (!destination || bytes.empty() || bytes.size() > 1024 * 1024) return false;
#ifdef OMNIPLAY_NATIVE_MEMORY_FILES
  if(!installed)return false;OmniFilesStatV1 shape{};
  const auto found=memory.stat(memory.context,destination,&shape);
  if(found==OMNI_FILES_NOT_FOUND)return true;
  if(found||shape.kind!=OMNI_FILES_REGULAR||shape.bytes!=bytes.size())return false;
  std::uint64_t handle=0,actual=0;
  if(memory.open(memory.context,destination,OMNI_FILES_READ,&handle))return false;
  const bool okay=memory.read(memory.context,handle,0,bytes.data(),bytes.size(),&actual)==OMNI_FILES_OK&&actual==bytes.size();
  const bool closed=memory.close(memory.context,handle)==OMNI_FILES_OK;return okay&&closed;
#else
  FILE* file = std::fopen(destination, "rb");
  if (!file) return errno == ENOENT;
  long length = -1;
  if (!std::fseek(file, 0, SEEK_END)) length = std::ftell(file);
  bool okay = length >= 0 && static_cast<size_t>(length) == bytes.size() &&
      !std::fseek(file, 0, SEEK_SET);
  if (okay) okay = std::fread(bytes.data(), 1, bytes.size(), file) == bytes.size() && !std::ferror(file);
  const bool closed = std::fclose(file) == 0;
  return okay && closed;
#endif
}
inline bool publish(const char* name, std::span<const uint8_t> bytes) {
  const char* destination = path(name);
  if (!destination || bytes.empty() || bytes.size() > 1024 * 1024) return false;
  char temporary[64];
  int length = std::snprintf(temporary, sizeof(temporary), "%s.omtmp", destination);
  if (length < 0 || static_cast<size_t>(length) >= sizeof(temporary)) return false;
#ifdef OMNIPLAY_NATIVE_MEMORY_FILES
  if(!installed)return false;
  const auto removed=memory.remove(memory.context,temporary);
  if(removed!=OMNI_FILES_OK&&removed!=OMNI_FILES_NOT_FOUND)return false;
  std::uint64_t handle=0,actual=0;
  if(memory.open(memory.context,temporary,OMNI_FILES_WRITE|OMNI_FILES_CREATE|OMNI_FILES_EXCLUSIVE,&handle))return false;
  const bool written=memory.write(memory.context,handle,0,bytes.data(),bytes.size(),&actual)==OMNI_FILES_OK&&actual==bytes.size();
  const bool flushed=memory.flush(memory.context,handle)==OMNI_FILES_OK;
  const bool closed=memory.close(memory.context,handle)==OMNI_FILES_OK;
  const bool published=written&&flushed&&closed&&memory.rename(memory.context,temporary,destination,1)==OMNI_FILES_OK;
  if(!published)memory.remove(memory.context,temporary);return published;
#else
  FILE* file = std::fopen(temporary, "wb");
  if (!file) return false;
  bool okay = std::fwrite(bytes.data(), 1, bytes.size(), file) == bytes.size();
  okay = !std::fflush(file) && !std::ferror(file) && okay;
  const bool closed = std::fclose(file) == 0;
  bool published = false;
  if (okay && closed) {
#ifdef OMNIPLAY_NATIVE_FILES
    published = om_md_native_publish(temporary, destination) == 1;
#else
    published = std::rename(temporary, destination) == 0;
#endif
  }
  if (!published) {
    std::remove(temporary);
    return false;
  }
  return true;
#endif
}
}
