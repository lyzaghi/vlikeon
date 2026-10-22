// SPDX-License-Identifier: GPL-3.0-or-later
// Adapted from melonDS DS retro/info.hpp, Copyright 2023 Jesse Talavera-Greenberg.
#ifndef MELONDSDS_RETRO_GAMEINFO_HPP
#define MELONDSDS_RETRO_GAMEINFO_HPP
#include <array>
#include <cstddef>
#include <cstdint>
#include <memory>
#include <string>
#include <string_view>
#include "std/span.hpp"
struct retro_game_info;
namespace retro {
class GameInfo {
public:
    GameInfo(const retro_game_info& info);
    std::string_view GetPath() const noexcept { return _path; }
    std::span<const std::byte> GetData() const noexcept {
        return _data ? std::span(reinterpret_cast<const std::byte*>(_data.get()), _size)
                     : std::span(_header.data(), _size);
    }
    std::string_view GetMeta() const noexcept { return _meta; }
    void PrepareNdsData() const;
    // The fixed DS-only frontend consumes the cartridge once. Later upstream
    // accesses only inspect its NDSHeader; DSi installation/slot-2 are disabled.
    // Reset reconstructs the console and reopens the same verified media lease.
    std::unique_ptr<uint8_t[]> TakeNdsData() const;
private:
    std::string _path;
    mutable std::unique_ptr<uint8_t[]> _data;
    mutable size_t _size = 0;
    std::string _meta;
    alignas(8) mutable std::array<std::byte, 0x1000> _header{};
};
}
#endif
