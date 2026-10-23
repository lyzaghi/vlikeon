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
