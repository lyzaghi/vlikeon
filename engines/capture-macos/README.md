# macOS capture adapter — authored, not integrated

Private ScreenCaptureKit library for a trusted host that still owns its
directly launched child. macOS SDK build, broker admission, private media
transport and real capture/audio qualification remain open. The product's
macOS capture capability remains disabled.

See [`MACOS_OWNED_CAPTURE.md`](../../docs/MACOS_OWNED_CAPTURE.md) for the exact
source behavior, caller obligations, executed portable checks and missing work.
`include/bounded_samples.hpp` is used by the adapter and its portable ownership
fixture. A passing mailbox fixture is not a ScreenCaptureKit build or runtime
result.

Item 56 adds a private VideoToolbox encoder candidate (`VLKMacEncoder.h`,
`src/encoder.mm`) for polled BGRA video samples. It is compiled into the same
static-library recipe; no service executable or caller is integrated yet.
H.264/HEVC sessions use checked realtime properties, no frame reordering and
optional hardware-required creation. At most two input frames remain pending;
encoded output is bounded and closes on overflow rather than discarding
dependent frames. API/callback source review is not an SDK build or runtime pass.
See [`NATIVE_VIDEO_SERVICE.md`](../../docs/NATIVE_VIDEO_SERVICE.md).
