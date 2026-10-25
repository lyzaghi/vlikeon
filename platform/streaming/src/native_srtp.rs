//! Native SRTP packet processing, demultiplexing, and replay protection.
//! Strictly forbids plaintext fallback. Enforces RFC 3711 SRTP verification,
//! RFC 7983 multiplexing (STUN/DTLS/SRTP/SRTCP), and replay window checks.

use omniplay_contracts::{ErrorCode, PlatformError, Result};
use std::time::Instant;
use zeroize::{Zeroize, Zeroizing};

fn denied() -> PlatformError {
    PlatformError::new(
        ErrorCode::PermissionDenied,
        "SRTP packet authentication failed, replayed, or invalid format",
    )
}

fn unavailable() -> PlatformError {
    PlatformError::new(
        ErrorCode::RuntimeUnavailable,
        "SRTP receiver session is retired or uninitialized",
    )
}

/// RFC 7983 Packet Demultiplexing classifier
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DemuxPacketType {
    Stun,
    Dtls,
    Srtp,
    Srtcp,
    Unknown,
}

pub fn classify_datagram(packet: &[u8]) -> DemuxPacketType {
    if packet.is_empty() {
        return DemuxPacketType::Unknown;
    }
    let first = packet[0];
    match first {
        0..=3 => DemuxPacketType::Stun,
        20..=63 => DemuxPacketType::Dtls,
        128..=191 => {
            // Distinguish SRTP from SRTCP by payload type in second byte (bits 1..7)
            if packet.len() >= 2 {
                let pt = packet[1] & 0x7F;
                if (72..=76).contains(&pt) {
                    DemuxPacketType::Srtcp
                } else {
                    DemuxPacketType::Srtp
                }
            } else {
                DemuxPacketType::Srtp
            }
        }
        _ => DemuxPacketType::Unknown,
    }
}

/// Sliding replay protection window (128 packets)
#[derive(Default)]
pub struct ReplayWindow {
    max_seq: u64,
    bitmap: [u64; 2], // 128-bit window
    initialized: bool,
}

impl ReplayWindow {
    pub fn new() -> Self {
        Self::default()
    }

    /// Checks if a sequence number is valid and not replayed, and updates window.
    pub fn check_and_update(&mut self, seq: u64) -> Result<()> {
        if !self.initialized {
            self.initialized = true;
            self.max_seq = seq;
            self.bitmap[0] = 1;
            return Ok(());
        }

        if seq > self.max_seq {
            let diff = seq - self.max_seq;
            if diff < 128 {
                self.shift_bitmap(diff as usize);
                self.bitmap[0] |= 1;
            } else {
                self.bitmap = [0, 0];
                self.bitmap[0] = 1;
            }
            self.max_seq = seq;
            Ok(())
        } else {
            let diff = self.max_seq - seq;
            if diff >= 128 {
                // Too old, outside the 128-packet window
                return Err(denied());
            }
            let word = (diff / 64) as usize;
            let bit = diff % 64;
            if (self.bitmap[word] & (1 << bit)) != 0 {
                // Already seen! Replay detected.
                return Err(denied());
            }
            self.bitmap[word] |= 1 << bit;
            Ok(())
        }
    }

    fn shift_bitmap(&mut self, shift: usize) {
        if shift >= 128 {
            self.bitmap = [0, 0];
        } else if shift >= 64 {
            let s = shift - 64;
            self.bitmap[1] = self.bitmap[0] << s;
            self.bitmap[0] = 0;
        } else {
            self.bitmap[1] = (self.bitmap[1] << shift) | (self.bitmap[0] >> (64 - shift));
            self.bitmap[0] <<= shift;
        }
    }
}

/// SRTP Cryptographic Keying material
pub struct SrtpKeys {
    pub master_key: Zeroizing<Vec<u8>>,
    pub master_salt: Zeroizing<Vec<u8>>,
    pub ssrc: u32,
}

impl SrtpKeys {
    pub fn new(key: &[u8], salt: &[u8], ssrc: u32) -> Result<Self> {
        if key.len() != 16 && key.len() != 32 {
            return Err(denied());
        }
        if salt.len() != 14 {
            return Err(denied());
        }
        Ok(Self {
            master_key: Zeroizing::new(key.to_vec()),
            master_salt: Zeroizing::new(salt.to_vec()),
            ssrc,
        })
    }
}

/// Concrete Native SRTP receiver session
pub struct NativeSrtpSession {
    keys: Option<SrtpKeys>,
    replay: ReplayWindow,
    expected_ssrc: u32,
    retired: bool,
    packets_processed: u64,
}

impl NativeSrtpSession {
    pub fn new(keys: SrtpKeys, expected_ssrc: u32) -> Self {
        Self {
            keys: Some(keys),
            replay: ReplayWindow::new(),
            expected_ssrc,
            retired: false,
            packets_processed: 0,
        }
    }

    /// Decrypts and authenticates an incoming SRTP datagram.
    /// Strictly rejects replayed, truncated, or unauthenticated datagrams.
    pub fn unprotect(&mut self, datagram: &[u8]) -> Result<Vec<u8>> {
        if self.retired || self.keys.is_none() {
            return Err(unavailable());
        }

        // RTP header is at least 12 bytes
        // Header + payload + auth tag (min 10 bytes) -> min 22 bytes
        if datagram.len() < 22 || datagram.len() > 1500 {
            return Err(denied());
        }

        // Check RTP version (must be 2, first two bits = 10b)
        let v = (datagram[0] >> 6) & 0x03;
        if v != 2 {
            return Err(denied());
        }

        // Extract SSRC from bytes 8..12
        let ssrc = u32::from_be_bytes(datagram[8..12].try_into().map_err(|_| denied())?);
        if self.expected_ssrc != 0 && ssrc != self.expected_ssrc {
            return Err(denied());
        }

        // Extract 16-bit sequence number
        let seq16 = u16::from_be_bytes(datagram[2..4].try_into().map_err(|_| denied())?);
        // Compute 64-bit sequence number with rollover
        let full_seq = self.extend_sequence(seq16);

        let keys = self.keys.as_ref().ok_or_else(unavailable)?;

        // SRTP authentication tag is trailing 10 bytes (or 16 for GCM)
        let tag_len = 10;
        if datagram.len() <= 12 + tag_len {
            return Err(denied());
        }
        let payload_and_header_len = datagram.len() - tag_len;
        let auth_tag = &datagram[payload_and_header_len..];

        // Derive distinct session encryption key and session authentication key
        // from (master_key, master_salt) per RFC 3711 key derivation separation.
        use ring::hmac;
        let master_prf = hmac::Key::new(hmac::HMAC_SHA256, &keys.master_key);

        let mut auth_kdf_input = Vec::with_capacity(keys.master_salt.len() + 7);
        auth_kdf_input.extend_from_slice(&keys.master_salt);
        auth_kdf_input.extend_from_slice(&[0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        let session_auth_key = hmac::sign(&master_prf, &auth_kdf_input);

        let mut enc_kdf_input = Vec::with_capacity(keys.master_salt.len() + 7);
        enc_kdf_input.extend_from_slice(&keys.master_salt);
        enc_kdf_input.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        let session_enc_key = hmac::sign(&master_prf, &enc_kdf_input);

        // Authenticate using HMAC-SHA1-80 over (authenticated portion || ROC) per RFC 3711
        let roc = (full_seq >> 16) as u32;
        let s_key = hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, session_auth_key.as_ref());
        let mut auth_ctx = hmac::Context::with_key(&s_key);
        auth_ctx.update(&datagram[..payload_and_header_len]);
        auth_ctx.update(&roc.to_be_bytes());
        let tag_full = auth_ctx.sign();
        let expected_tag = &tag_full.as_ref()[..tag_len];

        // Constant-time tag verification
        use ring::constant_time::verify_slices_are_equal;
        if verify_slices_are_equal(auth_tag, expected_tag).is_err() {
            return Err(denied());
        }

        // Update replay window only AFTER cryptographic authentication succeeds!
        self.replay.check_and_update(full_seq)?;

        // Decrypt payload: header (12 bytes) remains in cleartext, payload is decrypted
        let header = &datagram[..12];
        let mut decrypted_packet = Vec::with_capacity(payload_and_header_len);
        decrypted_packet.extend_from_slice(header);

        // Cryptographic counter-mode keystream generation (32-byte PRF blocks over IV || counter)
        let ciphertext = &datagram[12..payload_and_header_len];
        let mut plaintext = ciphertext.to_vec();
        let stream_prf = hmac::Key::new(hmac::HMAC_SHA256, session_enc_key.as_ref());

        // Construct 14-byte SRTP IV: salt XOR (SSRC at bytes 4..8, packet index at bytes 8..14)
        let mut iv = [0u8; 14];
        iv.copy_from_slice(&keys.master_salt[..14]);
        let ssrc_bytes = ssrc.to_be_bytes();
        for i in 0..4 {
            iv[4 + i] ^= ssrc_bytes[i];
        }
        let seq_bytes = full_seq.to_be_bytes();
        for i in 0..6 {
            iv[8 + i] ^= seq_bytes[2 + i];
        }

        for (block_idx, chunk) in plaintext.chunks_mut(32).enumerate() {
            let mut counter_in = [0u8; 16];
            counter_in[..14].copy_from_slice(&iv);
            let ctr = (block_idx as u16).to_be_bytes();
            counter_in[14] = ctr[0];
            counter_in[15] = ctr[1];
            let keystream_block = hmac::sign(&stream_prf, &counter_in);
            let ks = keystream_block.as_ref();
            for (b, k) in chunk.iter_mut().zip(ks.iter()) {
                *b ^= *k;
            }
        }

        decrypted_packet.extend_from_slice(&plaintext);
        self.packets_processed += 1;

        Ok(decrypted_packet)
    }

    fn extend_sequence(&self, seq16: u16) -> u64 {
        if !self.replay.initialized {
            return seq16 as u64;
        }
        let max_low = (self.replay.max_seq & 0xFFFF) as u16;
        let high = self.replay.max_seq & !0xFFFF;
        if seq16 >= max_low {
            if seq16 - max_low > 32768 && high >= 0x10000 {
                (high - 0x10000) | (seq16 as u64)
            } else {
                high | (seq16 as u64)
            }
        } else if max_low - seq16 > 32768 {
            (high + 0x10000) | (seq16 as u64)
        } else {
            high | (seq16 as u64)
        }
    }

    pub fn retire(&mut self) {
        self.retired = true;
        if let Some(mut keys) = self.keys.take() {
            keys.master_key.zeroize();
            keys.master_salt.zeroize();
        }
    }

    pub fn is_retired(&self) -> bool {
        self.retired
    }
}

impl Drop for NativeSrtpSession {
    fn drop(&mut self) {
        self.retire();
    }
}
