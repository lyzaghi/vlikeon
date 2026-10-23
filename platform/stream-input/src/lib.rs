#![deny(unsafe_op_in_unsafe_fn)]
pub mod protocol;
pub mod gamepad;
#[cfg(all(target_os="linux",any(target_arch="x86_64",target_arch="aarch64")))]
pub mod gamepad_uinput;
mod client;
pub use client::InputWorker;
#[cfg(windows)]mod windows;
#[cfg(windows)]mod activity;
#[cfg(windows)]pub use windows::{target_identity,system_directory};
#[cfg(windows)]pub(crate) use windows::TargetInput;

use omniplay_contracts::{ErrorCode,PlatformError};
pub fn supported()->bool{cfg!(windows)}
pub(crate) fn failure()->PlatformError{PlatformError::new(ErrorCode::RuntimeUnavailable,"The native remote-input helper is unavailable or stopped").recover("Keep viewing or reauthorize control from the host")}
mod worker;
pub use worker::run_worker;
