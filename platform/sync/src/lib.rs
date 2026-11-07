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

#[cfg(test)]
mod tests {
    use super::*;
    use omniplay_contracts::{ContentHash, SaveMetadata};
    use sha2::{Digest, Sha256};
    use std::io::Cursor;
    use uuid::Uuid;

    fn sample_save_metadata(id: Uuid, payload: &[u8]) -> SaveMetadata {
        let payload_hash = ContentHash(format!("{:x}", Sha256::digest(payload)));
        let mut meta: SaveMetadata = serde_json::from_value(serde_json::json!({
            "schema_version": 1,
            "id": id,
            "game_id": "11111111-2222-3333-4444-555555555555",
            "game_content_hash": "a".repeat(64),
            "kind": "in_game",
            "slot": 0,
            "generation": 1,
            "parent_id": null,
            "core_id": "mesen-nes-native",
            "core_version": "0.2.0",
            "state_format_version": "mesen-nes-sram-v1",
            "platform": "windows-x86_64",
            "core_config_hash": "b".repeat(64),
            "payload_hash": payload_hash.0,
            "payload_bytes": payload.len() as u64,
            "created_at_ms": 1729850000000u64,
            "device_id": "66666666-7777-8888-9999-aaaaaaaaaaaa",
            "label": "Stage 1",
            "conflict": false
        }))
        .expect("valid SaveMetadata json");
        meta.payload_bytes = payload.len() as u64;
        meta
    }

    #[test]
    fn account_lifecycle_recovery_and_device_authorization_verify() {
        let account = AccountIdentity::generate().expect("generate account");
        let recovery_key = account.recovery_key();
        assert!(recovery_key.starts_with("OP1-"));
        assert_eq!(recovery_key.len(), 47);

        let recovered = AccountIdentity::recover(&recovery_key).expect("recover account");
        assert_eq!(account.account_id(), recovered.account_id());
        assert_eq!(account.public_key_base64(), recovered.public_key_base64());

        assert!(AccountIdentity::recover("OP2-invalid").is_err());

        let device = DeviceIdentity::generate().expect("generate device");
        let audience = "https://sync.vlikeon.example.org";
        let now = 1_729_850_000_000;
        let cert = account.authorize_device(&device, audience, now).expect("authorize device");

        let req = SyncRequest {
            schema_version: 1,
            account_id: account.account_id().to_string(),
            device_id: device.id,
            audience: audience.to_string(),
            nonce: Uuid::new_v4(),
            issued_at_ms: now,
            operation: SyncOperation::TransferCapabilities,
        };
        let signed = device.sign_request(&req, cert).expect("sign request");
        let (verified_cert, verified_req) =
            verify_device_request(&account.public_key_base64(), &signed, audience, now)
                .expect("verify device request");
        assert_eq!(verified_cert.device_id, device.id);
        assert_eq!(verified_req.nonce, req.nonce);

        // Wrong audience or expired timestamp must fail closed
        assert!(verify_device_request(
            &account.public_key_base64(),
            &signed,
            "https://evil.example.org",
            now
        )
        .is_err());
        assert!(verify_device_request(
            &account.public_key_base64(),
            &signed,
            audience,
            now + 91 * 24 * 60 * 60 * 1000
        )
        .is_err());
    }

    #[test]
    fn xchacha20_poly1305_blob_and_chunked_streaming_integrity() {
        let account = AccountIdentity::from_seed([0x42; 32]).expect("deterministic account");
        let cipher = account.save_cipher().expect("save cipher");

        // Multi-chunk payload: 2 full 256 KiB chunks + 12345 byte remainder
        let total_len = (2 * TRANSFER_CHUNK_BYTES as usize) + 12345;
        let mut payload = vec![0u8; total_len];
        for (i, b) in payload.iter_mut().enumerate() {
            *b = (i % 251) as u8;
        }
        let save_id = Uuid::from_u128(0xabcdef01_2345_6789_abcd_ef0123456789);
        let metadata = sample_save_metadata(save_id, &payload);

        let (header, ciphertext) = cipher
            .encrypt_blob_reader_checked(&metadata, &mut Cursor::new(&payload), &|| Ok(()))
            .expect("encrypt_blob_reader_checked");
        header.validate().expect("valid blob header");
        assert_eq!(header.binding.account_id, account.account_id());
        assert_eq!(header.binding.object_id, save_id);

        // Verify 256 KiB chunking bounds
        assert_eq!(
            chunk_length(header.ciphertext_bytes, 0),
            Some(TRANSFER_CHUNK_BYTES)
        );
        assert_eq!(
            chunk_length(header.ciphertext_bytes, u64::from(TRANSFER_CHUNK_BYTES)),
            Some(TRANSFER_CHUNK_BYTES)
        );
        let third_chunk = chunk_length(header.ciphertext_bytes, 2 * u64::from(TRANSFER_CHUNK_BYTES))
            .expect("third chunk remainder");
        assert!(third_chunk > 12345 && third_chunk < TRANSFER_CHUNK_BYTES);
        assert_eq!(chunk_length(header.ciphertext_bytes, 123), None);

        // Copy checked through streaming 64 KiB scratch buffer
        let mut copied = Vec::new();
        header
            .copy_blob_checked(&mut Cursor::new(&ciphertext), &mut copied, &|| Ok(()))
            .expect("copy_blob_checked");
        assert_eq!(copied, ciphertext);

        // Decrypt and verify exact metadata + payload restoration
        let (decrypted_meta, decrypted_payload) = cipher
            .decrypt_blob_reader_checked(&header, &mut Cursor::new(&copied), &|| Ok(()))
            .expect("decrypt_blob_reader_checked");
        assert_eq!(decrypted_meta.id, metadata.id);
        assert_eq!(decrypted_meta.payload_hash, metadata.payload_hash);
        assert_eq!(decrypted_payload.as_slice(), payload.as_slice());

        // Tampered ciphertext must be rejected by AEAD / SHA-256 verification
        let mut tampered = copied.clone();
        tampered[0] ^= 0xff;
        assert!(cipher
            .decrypt_blob_reader_checked(&header, &mut Cursor::new(&tampered), &|| Ok(()))
            .is_err());
    }
}
