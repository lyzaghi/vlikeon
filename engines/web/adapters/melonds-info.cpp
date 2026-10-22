// SPDX-License-Identifier: GPL-3.0-or-later
// Full-path DS input uses the verified read-only media filesystem. A cartridge
// owns the allocation after parsing; no second frontend ROM copy is retained.
#include "info.hpp"
#include <algorithm>
#include <cstdio>
#include <cstring>
#include <stdexcept>
#include <utility>
#include <libretro.h>
namespace {
constexpr size_t MaxRom = 512u * 1024u * 1024u;
struct Close { void operator()(FILE* file) const noexcept { if(file) std::fclose(file); } };
}
retro::GameInfo::GameInfo(const retro_game_info& info)
    : _path(info.path ? info.path : ""), _meta(info.meta ? info.meta : "") {
    if(_path.empty() || _path.size() > 256 || _path.rfind("/game/", 0) != 0 ||
       _path.find("..") != std::string::npos || info.data || info.size)
        throw std::runtime_error("NDS content requires one verified full-path media file");
    std::unique_ptr<FILE, Close> file(std::fopen(_path.c_str(), "rb"));
    if(!file || std::fseek(file.get(), 0, SEEK_END))
        throw std::runtime_error("NDS media could not be opened");
    const long length = std::ftell(file.get());
    if(length < 0x1000 || static_cast<unsigned long>(length) > MaxRom ||
       std::fseek(file.get(), 0, SEEK_SET))
        throw std::runtime_error("NDS media size is outside the cartridge bound");
    _size = static_cast<size_t>(length);
    _data = std::make_unique<uint8_t[]>(_size);
    for(size_t offset = 0; offset < _size;) {
        const size_t count = std::min<size_t>(65536, _size - offset);
        if(std::fread(_data.get() + offset, 1, count, file.get()) != count)
            throw std::runtime_error("NDS media read was incomplete");
        offset += count;
    }
    if(std::fgetc(file.get()) != EOF || std::ferror(file.get()))
        throw std::runtime_error("NDS media changed while loading");
}
void retro::GameInfo::PrepareNdsData() const {
    if(_data) return;
    const retro_game_info source{_path.c_str(), nullptr, 0, nullptr};
    GameInfo replacement(source);
    _data = std::move(replacement._data);
    _size = replacement._size;
}
std::unique_ptr<uint8_t[]> retro::GameInfo::TakeNdsData() const {
    if(!_data || _size < _header.size())
        throw std::runtime_error("NDS cartridge ownership was already transferred");
    std::memcpy(_header.data(), _data.get(), _header.size());
    _size = _header.size();
    return std::move(_data);
}
