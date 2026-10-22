#![forbid(unsafe_code)]

mod account;
mod encryption;
mod protocol;
mod transfers;
pub mod transfer_files;

pub use account::{AccountIdentity, DeviceIdentity};
pub use encryption::{EncryptedSave, SaveBinding,BlobHeader,SaveCipher,MAX_CIPHERTEXT_BYTES};
pub use protocol::*;
pub use transfers::*;

use omniplay_contracts::{ErrorCode,PlatformError,Result};

pub(crate) fn error(message:&str)->PlatformError {
    PlatformError::new(ErrorCode::PermissionDenied,message)
}

pub(crate) fn random<const N:usize>()->Result<[u8;N]> {
    let mut bytes=[0u8;N];
    getrandom::getrandom(&mut bytes).map_err(|_|error("Secure device randomness is unavailable"))?;
    Ok(bytes)
}
