#![deny(unsafe_op_in_unsafe_fn)]
pub mod protocol;
pub mod gamepad;
pub mod gamepad_host;
pub mod gamepad_protocol;
#[cfg(windows)]pub mod gamepad_windows_host;
#[cfg(windows)]pub mod gamepad_process;
#[cfg(windows)]pub mod game_process;
#[cfg(windows)]mod game_windows;
#[cfg(windows)]mod gamepad_child;
#[cfg(windows)]pub mod gamepad_client;
pub mod video_client;
#[cfg(windows)]pub mod video_winsock;
#[cfg(all(target_os="linux",any(target_arch="x86_64",target_arch="aarch64")))]
pub mod gamepad_uinput;
#[cfg(all(target_os="linux",any(target_arch="x86_64",target_arch="aarch64")))]
pub mod gamepad_linux_host;
#[cfg(target_os="linux")]
pub mod game_process_linux;
#[cfg(target_os="linux")]
pub mod capture_linux;
#[cfg(target_os="macos")]
pub mod game_process_macos;
#[cfg(target_os="macos")]
pub mod capture_macos;

#[cfg(windows)]
pub use game_process::{GameWindow, OwnedGameProcess};
#[cfg(target_os="linux")]
pub use game_process_linux::{GameWindow, OwnedGameProcess};
#[cfg(target_os="macos")]
pub use game_process_macos::{GameWindow, OwnedGameProcess};
#[cfg(all(feature="winuhid",target_os="windows",target_env="msvc",any(target_arch="x86_64",target_arch="aarch64")))]
pub mod gamepad_winuhid;
#[cfg(all(feature="winuhid",target_os="windows",target_env="msvc",any(target_arch="x86_64",target_arch="aarch64")))]
mod gamepad_worker;
#[cfg(all(feature="winuhid",target_os="windows",target_env="msvc",any(target_arch="x86_64",target_arch="aarch64")))]
pub fn run_gamepad_worker()->i32{gamepad_worker::run()}
#[cfg(not(all(feature="winuhid",target_os="windows",target_env="msvc",any(target_arch="x86_64",target_arch="aarch64"))))]
pub fn run_gamepad_worker()->i32{2}
mod client;
pub use client::InputWorker;
#[cfg(windows)]mod windows;
#[cfg(windows)]mod activity;
#[cfg(windows)]pub use windows::{target_identity,system_directory};
#[cfg(windows)]pub(crate) use windows::TargetInput;

use omniplay_contracts::{ErrorCode,PlatformError};
pub fn supported()->bool{cfg!(any(windows, target_os="linux", target_os="macos"))}
pub(crate) fn failure()->PlatformError{PlatformError::new(ErrorCode::RuntimeUnavailable,"The native remote-input helper is unavailable or stopped").recover("Keep viewing or reauthorize control from the host")}
mod worker;
pub use worker::run_worker;
