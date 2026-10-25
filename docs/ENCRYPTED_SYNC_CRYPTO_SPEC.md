# Encrypted Save Synchronization Cryptographic Specification — Items 77, 78, 80

Status: Cryptographic specification and test vectors defined; cryptographic execution and third-party audit deferred per project owner policy.

## 1. Cryptographic Primitive & Envelope Structure

All game saves, states, and backup archives synchronized through OmniPlay are protected with client-side authenticated encryption:
- **Cipher:** XChaCha20-Poly1305 (IETF RFC 8439 extended with 192-bit eXtended Nonce).
- **Key Derivation:** HKDF-SHA256 (RFC 5869) expanding the user's 256-bit root master secret with domain-separated salt and info strings:
  - Salt: `b"org.vlikeon.sync.v1.salt"`
  - Info: `b"org.vlikeon.sync.v1.save_encryption_key"`
- **Nonce Generation:** 24 bytes (192 bits) cryptographically secure pseudorandom bytes (`OsRng`) per encrypted object. Reusing nonces is cryptographically prohibited.

### Envelope Wire Layout (BlobHeader)
```text
+-----------------------+-----------------------+-----------------------+
| Magic: "VLKSYNC1"     | Version: 0x0001       | Cipher: 0x01 (XChCh)  |
| (8 bytes ASCII)       | (2 bytes LE)          | (1 byte)              |
+-----------------------+-----------------------+-----------------------+
| Profile UUID          | Object UUID           | Timestamp (ms)        |
| (16 bytes binary)     | (16 bytes binary)     | (8 bytes LE uint64)   |
+-----------------------+-----------------------+-----------------------+
| Nonce                 | Plaintext Length      | Ciphertext Length     |
| (24 bytes binary)     | (8 bytes LE uint64)   | (8 bytes LE uint64)   |
+-----------------------+-----------------------+-----------------------+
| Payload SHA-256       | Poly1305 MAC Tag      | Ciphertext Bytes      |
| (32 bytes binary)     | (16 bytes binary)     | (Variable length)     |
+-----------------------+-----------------------+-----------------------+
```

## 2. Additional Authenticated Data (AAD)

To prevent cross-profile or cross-object ciphertext splicing, the Poly1305 tag authenticates an immutable AAD block prepended to the payload:
```text
AAD = Profile_UUID (16B) || Object_UUID (16B) || Schema_Version (4B LE) || Timestamp_MS (8B LE)
Total AAD Length: 44 bytes
```

## 3. Large File Chunking & Memory Bounding (Item 80)

To ensure memory safety on resource-constrained mobile and embedded devices:
- Saves larger than 256 KiB are divided into contiguous **256 KiB chunks** (`262,144 bytes`).
- **Memory Ceiling:** At most two 256 KiB chunks (512 KiB) are admitted in memory concurrently during streaming encryption or decryption.
- Base64 encoding/expansion of binary payloads in memory is strictly forbidden.
- Each chunk has an independent SHA-256 checksum for transport verification:
  `Chunk_Digest = SHA256(Chunk_Bytes)`
- Full object decryption and state import is admitted only after the complete ciphertext is verified against the outer Poly1305 MAC tag and full-object SHA-256 digest.

## 4. Test Vectors & Cross-Platform Interoperability (Item 78)

Cross-platform parity between Rust (`ring`/`chacha20poly1305`), Web/JavaScript (`WebCrypto`), Android (`AndroidKeyStore`), and Apple (`CryptoKit`/`Keychain`) is verified using standard deterministic test vectors:
- **Key (hex):** `000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f`
- **Nonce (hex):** `000000000000000000000000000000000000000000000001`
- **Profile ID:** `11111111-1111-1111-1111-111111111111`
- **Object ID:** `22222222-2222-2222-2222-222222222222`
- Test suite verification is implemented in `qa/sync_qualification.py`.
