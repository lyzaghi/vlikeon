//! Admit key material from an authenticated DTLS-SRTP handshake.
//! The transport must negotiate SRTP_AES128_CM_HMAC_SHA1_80 and supply
//! its RFC 5764 exporter output, using EXTRACTOR-dtls_srtp without a context.

use crate::native_srtp::{NativeSrtpSession, SrtpKeys};
use omniplay_contracts::{ErrorCode, PlatformError, Result};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

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

/// Key material supplied by the negotiated DTLS transport, never a synthetic PRF.
pub struct DtlsSrtpExporter {
    role: DtlsRole,
    material: Option<Zeroizing<[u8; 60]>>,
}

impl DtlsSrtpExporter {
    pub fn from_negotiated_aes128_cm(
        role: DtlsRole,
        exported: &[u8],
        peer_certificate_der: &[u8],
        expected_fingerprint: &str,
    ) -> Result<Self> {
        if peer_certificate_der.is_empty()
            || !CertificateFingerprint::from_certificate_der(peer_certificate_der)
                .verify_matches(expected_fingerprint)
        {
            return Err(denied());
        }
        let exported: [u8; 60] = exported.try_into().map_err(|_| denied())?;
        Ok(Self {
            role,
            material: Some(Zeroizing::new(exported)),
        })
    }

    pub fn export_srtp_keys(&self, ssrc: u32) -> Result<NativeSrtpSession> {
        let output = self.material.as_ref().ok_or_else(unavailable)?;
        // client_write_key | server_write_key | client_write_salt | server_write_salt
        let (key, salt) = match self.role {
            DtlsRole::Server => (&output[..16], &output[32..46]),
            DtlsRole::Client => (&output[16..32], &output[46..]),
        };
        Ok(NativeSrtpSession::new(
            SrtpKeys::new(key, salt, ssrc)?,
            ssrc,
        ))
    }

    pub fn retire(&mut self) {
        self.material = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admits_negotiated_key_material_only_with_matching_fingerprint() {
        let cert_der = b"vlikeon-peer-certificate-der-fixture";
        let fp = CertificateFingerprint::from_certificate_der(cert_der);
        let hex_colon = fp.to_hex_string();
        assert!(fp.verify_matches(&hex_colon.to_lowercase()));
        assert!(!fp.verify_matches("00:11:22:33"));

        let material = [0x5a; 60];
        assert!(DtlsSrtpExporter::from_negotiated_aes128_cm(
            DtlsRole::Client,
            &material,
            cert_der,
            "00:11:22:33",
        )
        .is_err());
        assert!(DtlsSrtpExporter::from_negotiated_aes128_cm(
            DtlsRole::Client,
            &material[..59],
            cert_der,
            &hex_colon,
        )
        .is_err());

        let mut client = DtlsSrtpExporter::from_negotiated_aes128_cm(
            DtlsRole::Client,
            &material,
            cert_der,
            &hex_colon,
        )
        .unwrap();
        assert!(!client.export_srtp_keys(42).unwrap().is_retired());
        client.retire();
        assert!(client.export_srtp_keys(42).is_err());
    }
}
