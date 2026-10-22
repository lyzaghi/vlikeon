// SPDX-License-Identifier: GPL-2.0-or-later
// Product bounds for the explicitly pinned FFmpeg 9 decoder integration.
#pragma once
extern "C" {
#include <libavcodec/avcodec.h>
}
inline void OmniPlayCodecLimits(AVCodecContext *context) {
    if (!context) return;
    context->thread_count = 1;
    context->thread_type = 0;
    context->max_pixels = 1920 * 1080;
    context->max_samples = 8192;
}
struct OmniPlayPacket {
    AVPacket *value = av_packet_alloc();
    OmniPlayPacket() = default;
    OmniPlayPacket(const OmniPlayPacket &) = delete;
    OmniPlayPacket &operator=(const OmniPlayPacket &) = delete;
    ~OmniPlayPacket() { av_packet_free(&value); }
};
