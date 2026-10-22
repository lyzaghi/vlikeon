/* Included by the adapted program.cpp after nall's VFS types. All guest save
 * requests use bounded, closed-before-return buffers and checked publication.
 * The outer /saves filesystem supplies its separate materialized-tree quota.
 */
#include <cstdio>
#include <cstring>
#include <vector>
#include <climits>
#include <cerrno>

static bool om_bsnes_io_failed = false;
static bool om_bsnes_required_failed = false;
static unsigned om_bsnes_save_handles = 0;
static size_t om_bsnes_save_bytes = 0;
static bool om_bsnes_barrier = false;
static char om_bsnes_save_names[32][97];
static unsigned om_bsnes_save_name_count = 0;

static bool om_bsnes_save_name(const nall::string& name) {
  const char *text = name;
  const size_t length = std::strlen(text);
  if (!length || length > 96 || text[0] == '.' ||
      (std::strcmp(text + (length >= 4 ? length - 4 : 0), ".ram") &&
       std::strcmp(text + (length >= 4 ? length - 4 : 0), ".rtc"))) return false;
  for (size_t index = 0; index < length; ++index)
    if (!((text[index] >= 'a' && text[index] <= 'z') ||
          (text[index] >= '0' && text[index] <= '9') ||
          text[index] == '.' || text[index] == '-' || text[index] == '_')) return false;
  return !std::strstr(text, "..");
}

struct OmBsnesSaveFile final : nall::vfs::file {
  static constexpr size_t limit = 16 * 1024 * 1024;
  std::vector<uint8_t> bytes;
  size_t position = 0;
  char path[128] = {};
  bool writing = false;
  bool bad = false;

  static auto open(const nall::string& name, mode access)
      -> nall::shared_pointer<nall::vfs::file> {
    if (!om_bsnes_save_name(name) ||
        (access != mode::read && access != mode::write) ||
        om_bsnes_save_handles >= 32) {
      om_bsnes_io_failed = true;
      return {};
    }
    auto item = nall::shared_pointer<OmBsnesSaveFile>{new OmBsnesSaveFile};
    std::snprintf(item->path, sizeof(item->path), "/saves/%s", (const char *)name);
    item->writing = access == mode::write;
    if (item->writing && om_bsnes_barrier) {
      for (unsigned index = 0; index < om_bsnes_save_name_count; ++index)
        if (!std::strcmp(om_bsnes_save_names[index], (const char *)name)) {
          om_bsnes_io_failed = true; return {};
        }
      if (om_bsnes_save_name_count == 32) { om_bsnes_io_failed = true; return {}; }
      std::strcpy(om_bsnes_save_names[om_bsnes_save_name_count++], (const char *)name);
    }
    if (!item->writing) {
      FILE *incoming = std::fopen(item->path, "rb");
      if (!incoming) {
        if (errno != ENOENT) om_bsnes_io_failed = true;
        return {}; // Only an absent optional in-game save is normal.
      }
      long length = -1;
      bool okay = std::fseek(incoming, 0, SEEK_END) == 0;
      if (okay) length = std::ftell(incoming);
      okay = okay && length >= 0 && (unsigned long)length <= limit &&
             std::fseek(incoming, 0, SEEK_SET) == 0;
      if (okay) {
        okay = item->resize((size_t)length);
        okay = okay && std::fread(item->bytes.data(), 1, item->bytes.size(), incoming) == item->bytes.size() &&
               !std::ferror(incoming);
      }
      const bool closed = std::fclose(incoming) == 0;
      if (!okay || !closed) { om_bsnes_io_failed = true; return {}; }
    }
    ++om_bsnes_save_handles;
    item->counted = true;
    return item;
  }

  ~OmBsnesSaveFile() override {
    if (writing && counted) publish();
    if (counted) --om_bsnes_save_handles;
    om_bsnes_save_bytes -= bytes.size();
  }

  auto size() const -> uintmax override { return bytes.size(); }
  auto offset() const -> uintmax override { return position; }
  auto seek(intmax value, index origin) -> void override {
    intmax next = value;
    if (origin == index::relative) {
      if (value > INTMAX_MAX - (intmax)position) { fail(); return; }
      next += (intmax)position;
    }
    if (next < 0 || (uintmax)next > limit) { fail(); return; }
    position = (size_t)next;
  }
  auto read() -> uint8_t override {
    if (position >= bytes.size()) { fail(); return 0; }
    return bytes[position++];
  }
  auto write(uint8_t value) -> void override {
    if (!writing || bad || position >= limit) { fail(); return; }
    if (position >= bytes.size() && !resize(position + 1)) return;
    bytes[position++] = value;
  }
  auto flush() -> void override { /* The close barrier publishes once. */ }

private:
  bool counted = false;
  auto fail() -> void { bad = true; om_bsnes_io_failed = true; }
  auto resize(size_t size) -> bool {
    if (size > limit || size < bytes.size() ||
        size - bytes.size() > 32 * 1024 * 1024 - om_bsnes_save_bytes) {
      fail(); return false;
    }
    const size_t previous = bytes.size();
    bytes.resize(size, 0);
    om_bsnes_save_bytes += size - previous;
    return true;
  }
  auto publish() -> void {
    if (bad) { om_bsnes_io_failed = true; return; }
    char temporary[144];
    const int length = std::snprintf(temporary, sizeof(temporary), "%s.omtmp", path);
    if (length < 0 || (size_t)length >= sizeof(temporary)) { fail(); return; }
    FILE *outgoing = std::fopen(temporary, "wb");
    if (!outgoing) { fail(); return; }
    bool okay = std::fwrite(bytes.data(), 1, bytes.size(), outgoing) == bytes.size();
    okay = std::fflush(outgoing) == 0 && !std::ferror(outgoing) && okay;
    const bool closed = std::fclose(outgoing) == 0;
    if (!okay || !closed || std::rename(temporary, path)) {
      std::remove(temporary);
      fail();
    }
  }
};
