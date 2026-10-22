#![forbid(unsafe_code)]

pub mod errors;
pub mod game;
pub mod ipc;
pub mod metrics;
pub mod saves;
pub mod session;
pub mod routing;
pub mod performance;
pub mod media;
pub mod streaming;
pub mod input;
pub mod core_options;
pub mod sync_actions;
pub mod native_work;

pub use errors::{ErrorCode, PlatformError, Result};
pub use game::*;
pub use ipc::*;
pub use metrics::*;
pub use saves::*;
pub use session::*;
pub use routing::*;
pub use performance::*;
pub use media::*;
pub use streaming::*;
pub use input::*;
pub use core_options::*;
pub use sync_actions::*;
pub use native_work::*;

pub const PROTOCOL_MAJOR: u16 = 1;
pub const PROTOCOL_MINOR: u16 = 0;
pub const MAX_CONTROL_MESSAGE_BYTES: usize = 1024 * 1024;
pub const MAX_SAVE_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_ROM_BYTES: u64 = 4 * 1024 * 1024 * 1024;
