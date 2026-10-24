//! Safe broker facade. Win32 calls live in the native adapter crate; broker
//! retains its crate-wide prohibition on unsafe code.
pub use omniplay_stream_input::video_winsock::SelectedWinsockDatagrams;
