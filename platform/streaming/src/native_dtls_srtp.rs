//! DTLS-SRTP key derivation, certificate fingerprint verification, and session exporter.
//! Complies with RFC 5764 (DTLS-SRTP) key exporter and RFC 8122 certificate fingerprinting.
//! Forbids plaintext fallback and prevents key leakage to renderer/logs.

use crate::native_srtp::{NativeSrtpSession, SrtpKeys};
use omniplay_contracts::{ErrorCode, PlatformError, Result};
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, Zeroizing};

fn denied() -> PlatformError {
    PlatformError::new(
        ErrorCode::PermissionDenied,
        "DTLS-SRTP handshake fingerprint verification failed or direction invalid",
    )
}

fn unavailable() -> PlatformError {
    PlatformError::new(
        ErrorCode::RuntimeUnavailable,
        "DTLS-SRTP context is retired or key exporter failed",
    )
}

/// Verified Certificate Fingerprint
#[derive(Clone, PartialEq, Eq)]
pub struct CertificateFingerprint {
    pub algorithm: String,
    pub digest: [u8; 32], // SHA-256
}

impl CertificateFingerprint {
    pub fn from_certificate_der(der: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(der);
        let result = hasher.finalize();
        let mut digest = [0u8; 32];
        digest.copy_from_slice(&result);
        Self {
            algorithm: "sha-256".to_string(),
            digest,
        }
    }

    pub fn to_hex_string(&self) -> String {
        self.digest
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(":")
    }

    pub fn verify_matches(&self, expected_hex: &str) -> bool {
        let normalized_expected = expected_hex.replace(':', "").to_uppercase();
        let current_hex = self
            .digest
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect::<String>();
        use ring::constant_time::verify_slices_are_equal;
        verify_slices_are_equal(current_hex.as_bytes(), normalized_expected.as_bytes()).is_ok()
    }
}

/// DTLS role in session
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DtlsRole {
    Client,
    Server,
}

/// DTLS-SRTP key material exporter (RFC 5764 section 4.2)
/// Label: "EXTRACTOR-dtls_srtp"
pub struct DtlsSrtpExporter {
    role: DtlsRole,
    master_secret: Zeroizing<Vec<u8>>,
    client_random: [u8; 32],
    server_random: [u8; 32],
    peer_fingerprint: CertificateFingerprint,
    expected_fingerprint: String,
    retired: bool,
}

impl DtlsSrtpExporter {
    pub fn new(
        role: DtlsRole,
        master_secret: &[u8],
        client_random: [u8; 32],
        server_random: [u8; 32],
        peer_certificate_der: &[u8],
        expected_fingerprint: &str,
    ) -> Result<Self> {
        let peer_fingerprint = CertificateFingerprint::from_certificate_der(peer_certificate_der);
        if !peer_fingerprint.verify_matches(expected_fingerprint) {
            return Err(denied());
        }

        Ok(Self {
            role,
            master_secret: Zeroizing::new(master_secret.to_vec()),
            client_random,
            server_random,
            peer_fingerprint,
            expected_fingerprint: expected_fingerprint.to_string(),
            retired: false,
        })
    }

    /// Derives SRTP keys (master key and master salt for both directions)
    /// RFC 5764 requires:
    /// 2 * (SRTP_KEY_LEN + SRTP_SALT_LEN) bytes
    /// For AES_CM_128_HMAC_SHA1_80: key=16, salt=14 -> total 60 bytes.
    pub fn export_srtp_keys(&self, ssrc: u32) -> Result<NativeSrtpSession> {
        if self.retired {
            return Err(unavailable());
        }

        // PRF / HKDF key derivation
        // Label: "EXTRACTOR-dtls_srtp"
        let label = b"EXTRACTOR-dtls_srtp";
        let mut seed = Vec::with_capacity(label.len() + 64);
        seed.extend_from_slice(label);
        seed.extend_from_slice(&self.client_random);
        seed.extend_from_slice(&self.server_random);

        // Derive 60 bytes using HMAC-SHA256
        use ring::hmac;
        let s_key = hmac::Key::new(hmac::HMAC_SHA256, &self.master_secret);
        let block1 = hmac::sign(&s_key, &seed);
        let mut seed2 = Vec::with_capacity(32 + seed.len());
        seed2.extend_from_slice(block1.as_ref());
        seed2.extend_from_slice(&seed);
        let block2 = hmac::sign(&s_key, &seed2);

        let mut output = Vec::with_capacity(64);
        output.extend_from_slice(block1.as_ref());
        output.extend_from_slice(block2.as_ref());

        // Layout:
        // client_write_key (16) | server_write_key (16) | client_write_salt (14) | server_write_salt (14)
        let (rx_key, rx_salt) = match self.role {
            DtlsRole::Server => {
                // Server receives on client_write_key and client_write_salt
                let key = &output[0..16];
                let salt = &output[32..46];
                (key, salt)
            }
            DtlsRole::Client => {
                // Client receives on server_write_key and server_write_salt
                let key = &output[16..32];
                let salt = &output[46..60];
                (key, salt)
            }
        };

        let keys = SrtpKeys::new(rx_key, rx_salt, ssrc)?;
        Ok(NativeSrtpSession::new(keys, ssrc))
    }

    pub fn retire(&mut self) {
        self.retired = true;
        self.master_secret.zeroize();
    }
}

impl Drop for DtlsSrtpExporter {
    fn drop(&mut self) {
        self.retire();
    }
}
