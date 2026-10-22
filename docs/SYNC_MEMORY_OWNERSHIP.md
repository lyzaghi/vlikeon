# Native sync buffer ownership: authored source

The native reconciliation and explicit cloud-recovery paths now hand completed
downloads to their consumers as owned private ciphertext files. Upload preparation
reads a pinned verified save descriptor directly into one fallibly reserved AEAD
buffer. This removes several authored whole-save copies without introducing a new
cipher or wire format. No compiler, cipher, file, network, device or test has run;
no peak-memory or cancellation result has been measured.

## Upload path

`SaveStore::open_for_sync` resolves and verifies the immutable local payload,
retains its file descriptor and returns the current canonical-origin projection.
The upload caller compares that projection to its outgoing fingerprint before
preparing a transport record. It does not first materialize a separate plaintext
payload vector through `read_for_sync`.

`SaveCipher::encrypt_blob_reader_checked` validates metadata, reserves space for
the exact metadata-prefix/payload/tag length with `try_reserve_exact`, then reads
the payload into that allocation in at most 64-KiB steps. A trailing-byte check
and the declared payload SHA-256 must both pass before encryption. The existing
XChaCha20-Poly1305 implementation encrypts in place and appends its existing
16-byte tag. The buffer's ownership is transferred as ciphertext only after
successful encryption, checked ciphertext hashing and header validation.

Input, allocation, guard, hash or AEAD failure drops the zeroizing allocation.
Encrypted output is written to the existing private pending transport file in
64-KiB steps, synced/published and recorded under the existing profile transport
lease. No plaintext temporary file or additional credential storage is added.

## Download and durable inbox path

A completed resumable download retains the same owned private file descriptor
after the existing contiguous-prefix, full ciphertext digest, length and sync
checks. It no longer reads that file into a full-size `Downloaded.bytes` vector
just to return from transport.

Durable inbox staging copies from that descriptor through a 64-KiB scratch buffer
into a private pending file while checking exact length, end of input and the
declared ciphertext digest. Publication follows successful verification. An
existing inbox target is verified through an independently opened ordinary file
and an I/O sink using the same bounded scratch path. The inbox row is inserted
before the caller may advance the global remote cursor, as before.

`finish_download` consumes the descriptor-owning value and closes it before
unlinking the slot. This also preserves the Windows exclusive-file lifetime:
cleanup does not attempt to remove the download while its write handle is open.
Early return drops the descriptor and leaves the persisted resumable slot for
the existing retry/cleanup rules. The descriptor adds no SQLite/profile borrow.

## Decode and recovery path

Inbox import, explicit recovery and the duplicate-upload conflict path pass the
owned ciphertext file to `decrypt_blob_reader_checked`. It allocates one bounded
zeroizing vector, reads at most 64 KiB at a time, verifies the full transport hash
and decrypts/authenticates in that same allocation with the existing AEAD.

Authenticated metadata, object identity, declared payload length and actual
payload digest are checked before returning bytes to save admission. Removing
the metadata prefix uses `copy_within`, then explicitly wipes the vacated tail
before truncation so the eventual zeroizing logical-length wipe does not leave
discarded plaintext in the spare portion of that allocation. Failed validation
does not return plaintext. Save compatibility, provenance admission, staged
publication and repair/cancellation transactions retain their existing rules.

Consent, cancellation, native credential and, for explicit recovery, current
request/lifecycle guards run between reader, hash, copy and staging steps and
before/after AEAD. Existing OS file-open/verification and individual blocking
OS/network/AEAD or memory-copy calls remain non-preemptible.

## Compatibility and remaining limits

The version-one binding order, AAD domain, random 24-byte nonce, encrypted
metadata-prefix layout and authentication tag remain unchanged. No schema,
certificate, transfer protocol, service or PWA account capability is advanced.
The legacy borrowed `encrypt_blob`/`decrypt_blob` APIs remain available through
reader wrappers; `decrypt_save` transfers its decoded ciphertext allocation to
the owned decode helper. Legacy JSON/base64 APIs still need their expanded
representation and are not described as the native reconciliation memory path.

There is still one whole-object AEAD working allocation, up to the save limit
plus bounded metadata/tag space. OS/allocator overhead, core save capture, SQL,
HTTP chunks and other concurrent product work are additional costs. Fallible
reservation is not a physical-RAM guarantee, especially on overcommitting OSes.
This is not segmented encryption, fixed RSS, immediate cancellation or proof that
a 256-MiB save is suitable for every mobile background opportunity. A versioned,
reviewed segmented-crypto design and device memory/energy budgets remain due.

The final consolidated pass must establish old/new ciphertext interoperability;
all metadata/nonce/tag/hash/size and short/long/read-failure cases; allocation and
guard failures; zeroizing/error paths; descriptor position/ownership/Windows
cleanup; all inbox/publication/cursor and cancellation races; consent/credential
expiry; resource-pressure behavior and actual peak RSS, latency and mobile energy.
The complete original runtime/platform/streaming/security/release scope remains
open. No final qualification or completion evidence is implied by this source.
