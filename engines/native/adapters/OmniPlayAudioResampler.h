// SPDX-License-Identifier: GPL-2.0-or-later
// Private bounded PCM conversion for the pinned FFmpeg 9 source profile.
#pragma once
#include <array>
#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <cstring>
extern "C" {
#include <libavutil/channel_layout.h>
#include <libavutil/frame.h>
#include <libavutil/samplefmt.h>
#include <libswresample/swresample.h>
}

class OmniPlayAudioResampler {
public:
    static constexpr size_t MaximumFrames = 8192;
    OmniPlayAudioResampler() = default;
    OmniPlayAudioResampler(const OmniPlayAudioResampler &) = delete;
    OmniPlayAudioResampler &operator=(const OmniPlayAudioResampler &) = delete;
    ~OmniPlayAudioResampler() { Reset(); }

    void Reset() {
        swr_free(&context_);
        av_channel_layout_uninit(&inputLayout_);
        rate_ = 0; format_ = -1; outputChannels_ = 0;
    }

    // Returns decoded frames for discard, or committed frames for a real output.
    // The destination remains untouched on any rejected conversion outcome.
    int Convert(const AVFrame *frame, int outputChannels, int16_t *output, size_t capacityFrames) {
        if (!frame || frame->nb_samples < 0 || frame->nb_samples > (int)MaximumFrames ||
            frame->sample_rate < 1 || frame->sample_rate > 384000 ||
            frame->ch_layout.nb_channels < 1 || frame->ch_layout.nb_channels > 2 ||
            !av_channel_layout_check(&frame->ch_layout) ||
            (outputChannels != 1 && outputChannels != 2) ||
            av_get_bytes_per_sample((AVSampleFormat)frame->format) < 1 || av_get_bytes_per_sample((AVSampleFormat)frame->format) > 8 ||
            (output && !capacityFrames) || (!output && capacityFrames)) return -1;
        if (!output) { Reset(); return frame->nb_samples; }
        capacityFrames = std::min(capacityFrames, MaximumFrames);
        if ((size_t)frame->nb_samples > capacityFrames) return -1;
        const uint8_t *input[2]{};
        const int planes = av_sample_fmt_is_planar((AVSampleFormat)frame->format) ? frame->ch_layout.nb_channels : 1;
        if (!frame->extended_data) return -1;
        for (int plane = 0; plane < planes; ++plane) {
            input[plane] = frame->extended_data[plane];
            if (frame->nb_samples && !input[plane]) return -1;
        }
        if (!Configure(frame, outputChannels)) return -1;
        const int64_t delay = swr_get_delay(context_, rate_);
        if (delay < 0 || delay > (int64_t)(capacityFrames - (size_t)frame->nb_samples)) return -1;
        uint8_t *destination = (uint8_t *)scratch_.data();
        const int frames = swr_convert(context_, &destination, (int)capacityFrames,
            input, frame->nb_samples);
        if (frames < 0 || (size_t)frames > capacityFrames) { Reset(); return -1; }
        std::memcpy(output, scratch_.data(), (size_t)frames * (size_t)outputChannels * sizeof(int16_t));
        return frames;
    }

private:
    bool Configure(const AVFrame *frame, int outputChannels) {
        if (context_ && rate_ == frame->sample_rate && format_ == frame->format &&
            outputChannels_ == outputChannels && av_channel_layout_compare(&inputLayout_, &frame->ch_layout) == 0) return true;
        AVChannelLayout input{};
        if (av_channel_layout_copy(&input, &frame->ch_layout) < 0) return false;
        AVChannelLayout wanted{};
        av_channel_layout_default(&wanted, outputChannels);
        SwrContext *candidate = nullptr;
        const int configured = swr_alloc_set_opts2(&candidate, &wanted, AV_SAMPLE_FMT_S16, frame->sample_rate,
                &input, (AVSampleFormat)frame->format, frame->sample_rate, 0, nullptr);
        av_channel_layout_uninit(&wanted);
        if (configured < 0 || !candidate || swr_init(candidate) < 0) {
            swr_free(&candidate); av_channel_layout_uninit(&input); return false;
        }
        Reset(); context_ = candidate; inputLayout_ = input;
        rate_ = frame->sample_rate; format_ = frame->format; outputChannels_ = outputChannels;
        return true;
    }
    SwrContext *context_ = nullptr;
    AVChannelLayout inputLayout_{};
    int rate_ = 0, format_ = -1, outputChannels_ = 0;
    std::array<int16_t, MaximumFrames * 2> scratch_{};
};
