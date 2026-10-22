#![forbid(unsafe_code)]
mod channel;
mod registry;
mod runtime;
mod work;
mod linux_policy;
mod mac_policy;
#[cfg(all(target_os="linux",target_arch="x86_64",target_env="gnu"))]mod linux_work;
#[cfg(all(target_os="linux",target_arch="x86_64",target_env="gnu"))]mod linux_image_cache;
#[cfg(all(target_os="linux",target_arch="x86_64",target_env="gnu"))]mod linux_package;
#[cfg(all(target_os="linux",target_arch="x86_64",target_env="gnu"))]mod linux_staging;
#[cfg(all(target_os="linux",target_arch="x86_64",target_env="gnu"))]mod linux_session;
#[cfg(all(target_os="linux",target_arch="x86_64",target_env="gnu"))]mod linux_adapter;
pub use registry::NativeRegistry;
#[cfg(windows)]pub use work::recover_windows_work;
#[cfg(all(target_os="linux",target_arch="x86_64",target_env="gnu"))]pub use linux_work::{recover_linux_work,LinuxRecoverySummary};
use omniplay_contracts::{ErrorCode,PlatformError};
fn unavailable(message:&str)->PlatformError{PlatformError::new(ErrorCode::RuntimeUnavailable,message).recover("Install an approved native runtime or select an available web core")}
