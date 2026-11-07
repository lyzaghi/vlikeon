//! Native SRTP packet processing, demultiplexing, and replay protection.
//! Strictly forbids plaintext fallback. Enforces RFC 3711 SRTP verification,
//! RFC 7983 multiplexing (STUN/DTLS/SRTP/SRTCP), and replay window checks.

use aes::{
    cipher::{BlockEncrypt, KeyInit},
    Aes128,
};
use omniplay_contracts::{ErrorCode, PlatformError, Result};
use ring::hmac;
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

/// Session keys for SRTP_AES128_CM_HMAC_SHA1_80, with key derivation rate zero.
pub struct SrtpKeys {
    encryption: Aes128,
    authentication: Zeroizing<[u8; 20]>,
    salt: Zeroizing<[u8; 14]>,
    ssrc: u32,
}

impl SrtpKeys {
    pub fn new(key: &[u8], salt: &[u8], ssrc: u32) -> Result<Self> {
        let master = Aes128::new_from_slice(key).map_err(|_| denied())?;
        let salt: &[u8; 14] = salt.try_into().map_err(|_| denied())?;
        let encryption: Zeroizing<[u8; 16]> = derive_key(&master, salt, 0x00);
        Ok(Self {
            encryption: Aes128::new_from_slice(encryption.as_ref()).map_err(|_| denied())?,
            authentication: derive_key(&master, salt, 0x01),
            salt: derive_key(&master, salt, 0x02),
            ssrc,
        })
    }
}

fn derive_key<const N: usize>(master: &Aes128, salt: &[u8; 14], label: u8) -> Zeroizing<[u8; N]> {
    let mut iv = *salt;
    iv[7] ^= label;
    let mut key = Zeroizing::new([0; N]);
    aes_counter_xor(master, &iv, key.as_mut());
    iv.zeroize();
    key
}

fn aes_counter_xor(cipher: &Aes128, iv: &[u8; 14], payload: &mut [u8]) {
    for (index, chunk) in payload.chunks_mut(16).enumerate() {
        let mut block = aes::Block::default();
        block[..14].copy_from_slice(iv);
        block[14..].copy_from_slice(&(index as u16).to_be_bytes());
        cipher.encrypt_block(&mut block);
        for (byte, keystream) in chunk.iter_mut().zip(&block) {
            *byte ^= keystream;
        }
        block.as_mut_slice().zeroize();
    }
}

fn packet_iv(salt: &[u8; 14], ssrc: u32, packet_index: u64) -> [u8; 14] {
    let mut iv = *salt;
    for (byte, source) in iv[4..8].iter_mut().zip(ssrc.to_be_bytes()) {
        *byte ^= source;
    }
    for (byte, index) in iv[8..].iter_mut().zip(&packet_index.to_be_bytes()[2..]) {
        *byte ^= index;
    }
    iv
}

fn rtp_header_length(packet: &[u8]) -> Result<usize> {
    let mut length = 12 + usize::from(packet[0] & 0x0f) * 4;
    if packet[0] & 0x10 != 0 {
        let extension = packet.get(length..length + 4).ok_or_else(denied)?;
        let words = u16::from_be_bytes([extension[2], extension[3]]);
        length += 4 + usize::from(words) * 4;
    }
    if length >= packet.len() {
        return Err(denied());
    }
    Ok(length)
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
        if keys.ssrc != ssrc || full_seq >= 1 << 48 {
            return Err(denied());
        }

        let (encrypted, tag) = datagram.split_at(datagram.len() - 10);
        let header_length = rtp_header_length(encrypted)?;
        let roc = (full_seq >> 16) as u32;
        let key = hmac::Key::new(
            hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY,
            keys.authentication.as_ref(),
        );
        let mut auth = hmac::Context::with_key(&key);
        auth.update(encrypted);
        auth.update(&roc.to_be_bytes());
        let expected = auth.sign();
        ring::constant_time::verify_slices_are_equal(tag, &expected.as_ref()[..10])
            .map_err(|_| denied())?;
        self.replay.check_and_update(full_seq)?;

        let mut decrypted = encrypted.to_vec();
        let mut iv = packet_iv(&keys.salt, ssrc, full_seq);
        aes_counter_xor(&keys.encryption, &iv, &mut decrypted[header_length..]);
        iv.zeroize();
        self.packets_processed += 1;
        Ok(decrypted)
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
        self.keys = None;
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

#[cfg(test)]
mod tests {
    use super::*;
    use ring::hmac;

    fn protect_test_srtp_packet(
        master_key: &[u8; 16],
        master_salt: &[u8; 14],
        ssrc: u32,
        full_seq: u64,
        pt: u8,
        payload: &[u8],
    ) -> Vec<u8> {
        let mut header = [0u8; 12];
        header[0] = 0x80; // RTP v2
        header[1] = pt;
        header[2..4].copy_from_slice(&(full_seq as u16).to_be_bytes());
        header[4..8].copy_from_slice(&90_000u32.to_be_bytes());
        header[8..12].copy_from_slice(&ssrc.to_be_bytes());

        let keys = SrtpKeys::new(master_key, master_salt, ssrc).unwrap();
        let mut ciphertext = payload.to_vec();
        let iv = packet_iv(&keys.salt, ssrc, full_seq);
        aes_counter_xor(&keys.encryption, &iv, &mut ciphertext);

        let mut datagram = Vec::with_capacity(12 + ciphertext.len() + 10);
        datagram.extend_from_slice(&header);
        datagram.extend_from_slice(&ciphertext);

        let roc = (full_seq >> 16) as u32;
        let s_key = hmac::Key::new(
            hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY,
            keys.authentication.as_ref(),
        );
        let mut auth_ctx = hmac::Context::with_key(&s_key);
        auth_ctx.update(&datagram);
        auth_ctx.update(&roc.to_be_bytes());
        let tag = auth_ctx.sign();
        datagram.extend_from_slice(&tag.as_ref()[..10]);
        datagram
    }

    #[test]
    fn rfc3711_aes_cm_key_derivation_matches_published_vector() {
        let master = Aes128::new_from_slice(&[
            0xe1, 0xf9, 0x7a, 0x0d, 0x3e, 0x01, 0x8b, 0xe0, 0xd6, 0x4f, 0xa3, 0x2c, 0x06, 0xde,
            0x41, 0x39,
        ])
        .unwrap();
        let salt = [
            0x0e, 0xc6, 0x75, 0xad, 0x49, 0x8a, 0xfe, 0xeb, 0xb6, 0x96, 0x0b, 0x3a, 0xab, 0xe6,
        ];
        assert_eq!(
            *derive_key::<16>(&master, &salt, 0),
            [
                0xc6, 0x1e, 0x7a, 0x93, 0x74, 0x4f, 0x39, 0xee, 0x10, 0x73, 0x4a, 0xfe, 0x3f, 0xf7,
                0xa0, 0x87
            ]
        );
        assert_eq!(
            *derive_key::<20>(&master, &salt, 1),
            [
                0xce, 0xbe, 0x32, 0x1f, 0x6f, 0xf7, 0x71, 0x6b, 0x6f, 0xd4, 0xab, 0x49, 0xaf, 0x25,
                0x6a, 0x15, 0x6d, 0x38, 0xba, 0xa4
            ]
        );
        assert_eq!(
            *derive_key::<14>(&master, &salt, 2),
            [0x30, 0xcb, 0xbc, 0x08, 0x86, 0x3d, 0x8c, 0x85, 0xd4, 0x9d, 0xb3, 0x4a, 0x9a, 0xe1]
        );
    }

    #[test]
    fn two_udp_peers_use_production_srtp_and_retire_old_sessions() {
        use std::net::UdpSocket;
        use std::time::Duration;

        let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
        let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
        sender.connect(receiver.local_addr().unwrap()).unwrap();
        receiver.connect(sender.local_addr().unwrap()).unwrap();
        receiver
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut datagram = [0; 1500];
        let ssrc = 42;
        let salt = [0x31; 14];
        let first_key = [0x40; 16];
        let fresh_key = [0x50; 16];
        let mut retired =
            NativeSrtpSession::new(SrtpKeys::new(&first_key, &salt, ssrc).unwrap(), ssrc);
        let old_packet = protect_test_srtp_packet(&first_key, &salt, ssrc, 1, 96, b"old");
        sender.send(&old_packet).unwrap();
        let size = receiver.recv(&mut datagram).unwrap();
        assert_eq!(&retired.unprotect(&datagram[..size]).unwrap()[12..], b"old");
        assert!(retired.unprotect(&datagram[..size]).is_err());

        retired.retire();
        assert!(retired.unprotect(&datagram[..size]).is_err());
        let mut reconnect =
            NativeSrtpSession::new(SrtpKeys::new(&fresh_key, &salt, ssrc).unwrap(), ssrc);
        assert!(reconnect.unprotect(&datagram[..size]).is_err());
        let fresh_packet = protect_test_srtp_packet(&fresh_key, &salt, ssrc, 1, 96, b"fresh");
        sender.send(&fresh_packet).unwrap();
        let size = receiver.recv(&mut datagram).unwrap();
        assert_eq!(
            &reconnect.unprotect(&datagram[..size]).unwrap()[12..],
            b"fresh"
        );
    }

    #[test]
    fn rfc7983_demux_replay_window_and_srtp_unprotect_roundtrip() {
        assert_eq!(classify_datagram(&[]), DemuxPacketType::Unknown);
        assert_eq!(classify_datagram(&[0x00, 0x01]), DemuxPacketType::Stun);
        assert_eq!(classify_datagram(&[0x16, 0xFE]), DemuxPacketType::Dtls);
        assert_eq!(classify_datagram(&[0x80, 96]), DemuxPacketType::Srtp);
        assert_eq!(classify_datagram(&[0x80, 200]), DemuxPacketType::Srtcp);
        assert_eq!(classify_datagram(&[0xFF, 0x00]), DemuxPacketType::Unknown);

        let key = [0x42u8; 16];
        let salt = [0x24u8; 14];
        let ssrc = 0x1122_3344;
        let keys = SrtpKeys::new(&key, &salt, ssrc).unwrap();
        let mut session = NativeSrtpSession::new(keys, ssrc);

        let pkt100 = protect_test_srtp_packet(&key, &salt, ssrc, 100, 96, b"hello-srtp-frame-100");
        let dec100 = session.unprotect(&pkt100).unwrap();
        assert_eq!(&dec100[12..], b"hello-srtp-frame-100");

        // Exact replay must be rejected
        assert!(session.unprotect(&pkt100).is_err());

        // Tampered ciphertext must be rejected BEFORE updating the replay window
        let pkt101 = protect_test_srtp_packet(&key, &salt, ssrc, 101, 96, b"hello-srtp-frame-101");
        let mut tampered101 = pkt101.clone();
        tampered101[14] ^= 0xFF;
        assert!(session.unprotect(&tampered101).is_err());

        // Because authentication failed first, valid packet 101 is still accepted!
        let dec101 = session.unprotect(&pkt101).unwrap();
        assert_eq!(&dec101[12..], b"hello-srtp-frame-101");

        // Out-of-order late arrival within 128-packet window (seq 99) succeeds once
        let pkt99 = protect_test_srtp_packet(&key, &salt, ssrc, 99, 96, b"late-srtp-frame-99");
        assert!(session.unprotect(&pkt99).is_ok());
        assert!(session.unprotect(&pkt99).is_err());

        // Wrong SSRC rejected
        let wrong_ssrc = protect_test_srtp_packet(&key, &salt, 0x9999_9999, 102, 96, b"wrong-ssrc");
        assert!(session.unprotect(&wrong_ssrc).is_err());

        // Retire wipes keys and refuses subsequent packets
        session.retire();
        assert!(session.is_retired());
        let pkt103 = protect_test_srtp_packet(&key, &salt, ssrc, 103, 96, b"after-retire");
        assert!(session.unprotect(&pkt103).is_err());
    }
}
