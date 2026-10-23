#pragma once
#include "VLKWinGamepad.h"
#include <array>
#include <cstddef>
#include <cstdint>

namespace vlikeon::winpad {
// Original generic HID gamepad descriptor, not an Xbox/DS controller identity.
// ID 1: 12 buttons + 4 padding bits; X/Y/Rx/Ry signed 16-bit; Z/Rz unsigned
// 16-bit analog triggers; 4-bit null-state hat + 4 padding bits. 16 bytes total.
inline constexpr std::uint8_t descriptor[] = {
    0x05,0x01, 0x09,0x05, 0xa1,0x01, 0x85,0x01,
    0x05,0x09, 0x19,0x01, 0x29,0x0c, 0x15,0x00, 0x25,0x01,
    0x75,0x01, 0x95,0x0c, 0x81,0x02,
    0x75,0x04, 0x95,0x01, 0x81,0x03,
    0x05,0x01, 0x09,0x30, 0x09,0x31, 0x09,0x33, 0x09,0x34,
    0x16,0x01,0x80, 0x26,0xff,0x7f, 0x75,0x10, 0x95,0x04, 0x81,0x02,
    0x09,0x32, 0x09,0x35, 0x15,0x00, 0x27,0xff,0xff,0x00,0x00,
    0x75,0x10, 0x95,0x02, 0x81,0x02,
    0x09,0x39, 0x15,0x00, 0x25,0x07, 0x35,0x00, 0x46,0x3b,0x01,
    0x65,0x14, 0x75,0x04, 0x95,0x01, 0x81,0x42,
    0x65,0x00, 0x75,0x04, 0x95,0x01, 0x81,0x03, 0xc0
};
inline bool neutral(const VLKWinPadReport& report) noexcept {
    for (auto axis:report.sticks) if (axis!=0) return false;
    for (auto trigger:report.triggers) if (trigger!=0) return false;
    return report.buttons==0 && report.reserved==0;
}
inline std::array<std::uint8_t,16> encode(const VLKWinPadReport& report) noexcept {
    std::array<std::uint8_t,16> bytes{};
    bytes[0]=1;
    const auto little=[&bytes](std::size_t offset,std::uint16_t value) {
        bytes[offset]=static_cast<std::uint8_t>(value);
        bytes[offset+1]=static_cast<std::uint8_t>(value>>8);
    };
    little(1,report.buttons&0x0fff);
    for (std::size_t index=0;index<4;++index) {
        // -32768 is outside the descriptor's symmetric stick range.
        const auto value=report.sticks[index]==INT16_MIN ? -32767 : report.sticks[index];
        little(3+index*2,static_cast<std::uint16_t>(value));
    }
    little(11,report.triggers[0]);little(13,report.triggers[1]);
    // Standard bits: up=12/down=13/left=14/right=15. Opposites cancel.
    const int x=((report.buttons>>15)&1)-((report.buttons>>14)&1);
    const int y=((report.buttons>>13)&1)-((report.buttons>>12)&1);
    constexpr std::uint8_t hats[3][3]={{7,0,1},{6,8,2},{5,4,3}};
    bytes[15]=hats[y+1][x+1];
    return bytes;
}
static_assert(sizeof(VLKWinPadReport)==16);
}
