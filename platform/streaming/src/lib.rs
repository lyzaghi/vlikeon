#![forbid(unsafe_code)]
mod protocol;
pub mod native_video;
pub mod native_video_colour;
pub mod native_h264_rtp;
pub mod native_h264_receive;
pub mod native_h264_bits;
pub mod native_h264_sps;
pub mod native_h264_pps;
pub mod native_h264_parameters;
pub mod native_h264_slice;
pub mod native_h264_slice_header;
pub mod native_h264_picture;
pub mod native_h264_auxiliary;
pub mod native_rtp_reorder;
pub mod native_credentials;
pub mod native_pair_exchange;
pub mod native_signal;
pub mod native_relay;
pub mod native_ice;
pub use protocol::*;

use omniplay_contracts::{ErrorCode,PlatformError,Result};
pub(crate) fn invalid()->PlatformError{PlatformError::new(ErrorCode::InvalidRequest,"Streaming control data is invalid or outside its bound")}
pub(crate) fn unauthorized()->PlatformError{PlatformError::new(ErrorCode::PermissionDenied,"Streaming authorization is invalid, expired or revoked")}

pub fn validate_audience(value:&str)->Result<()>{
    let url=url::Url::parse(value).map_err(|_|invalid())?;
    if value.len()>256||url.scheme()!="https"||url.host_str().is_none()||!url.username().is_empty()||url.password().is_some()||url.query().is_some()||url.fragment().is_some()||url.path()!="/"||url.origin().ascii_serialization()!=value{return Err(invalid());}Ok(())
}
