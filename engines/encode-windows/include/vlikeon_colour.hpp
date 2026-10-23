#pragma once
#include "vlikeon_capture.hpp"
namespace vlikeon::video {
// CPU SDR reference conversion; same explicit contract as native_video_colour.rs.
// No scaler/HDR inference. The caller preserves capture receipt age separately.
HRESULT to_nv12(const BgraFrame&,std::vector<uint8_t>&);
}
