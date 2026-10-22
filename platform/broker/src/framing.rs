use omniplay_contracts::{ErrorCode, PlatformError, Result, MAX_CONTROL_MESSAGE_BYTES};
use std::io::{Read,Write};

pub fn read_message(source: &mut impl Read) -> Result<Option<Vec<u8>>> {
    let mut header = [0u8;4];
    loop {
        match source.read(&mut header[..1]) {
            Ok(0) => return Ok(None),
            Ok(_) => break,
            Err(error) if error.kind()==std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(PlatformError::new(ErrorCode::InvalidRequest,"Native message stream failed")),
        }
    }
    source.read_exact(&mut header[1..]).map_err(|_| PlatformError::new(ErrorCode::InvalidRequest,"Native message header is truncated"))?;
    // Chromium native messaging uses native-endian unsigned 32-bit framing.
    let length = u32::from_ne_bytes(header) as usize;
    if length == 0 || length > MAX_CONTROL_MESSAGE_BYTES { return Err(PlatformError::new(ErrorCode::ResourceLimit,"Native message length exceeds policy")); }
    let mut payload = vec![0u8;length];
    source.read_exact(&mut payload).map_err(|_| PlatformError::new(ErrorCode::InvalidRequest,"Native message body is truncated"))?;
    Ok(Some(payload))
}

pub fn write_message(destination: &mut impl Write, bytes: &[u8]) -> Result<()> {
    if bytes.is_empty() || bytes.len() > MAX_CONTROL_MESSAGE_BYTES { return Err(PlatformError::new(ErrorCode::ResourceLimit,"Native response length exceeds policy")); }
    destination.write_all(&(bytes.len() as u32).to_ne_bytes()).and_then(|_|destination.write_all(bytes)).and_then(|_|destination.flush()).map_err(|_|PlatformError::new(ErrorCode::RuntimeUnavailable,"Native message output disconnected"))
}
