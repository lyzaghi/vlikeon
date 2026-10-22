#![forbid(unsafe_code)]

// The workspace may inspect host crates together. This adapter exports its ABI
// only for the explicit browser target; a host library is never a WASM artifact.
#[cfg(all(target_arch="wasm32",target_os="wasi",target_env="p1"))]
mod wasm;
