# Encrypted resumable save transport source

Desktop and native mobile reconciliation now have authored resumable upload and
download paths. This changes source only. No service, network, save cipher,
database, transport file lease, compiler, device, simulator or test has been run. It is
not evidence of interoperability, durability, security, performance or release
readiness. Core/profile schema is now 13 for `SAVE_PUBLICATION_JOURNAL.md`, following
the origin ledger in `SAVE_ORIGINS.md`; the separate
sync-service schema remains 2.

## Wire extension and authentication

The existing `/v1/sync` endpoint and signed device request/certificate domains
carry these additional operations. They preserve fresh per-request nonces,
account/audience/device binding, certificate expiry, revocation and HTTPS policy.

| Operation | Signed content and result |
|---|---|
| `transfer_capabilities` | Protocol 1, 256 KiB chunk limit, object limit and 24-hour staging lifetime |
| `upload_begin` | Client-generated transfer UUID and complete BlobHeader; returns the durable received prefix or an already-stored result |
| `upload_chunk` | Transfer UUID, aligned offset, exact byte count and SHA-256 of the HTTP body |
| `upload_commit` | Transfer UUID and unchanged BlobHeader; publishes a fully verified object |
| `upload_abort` | Transfer UUID; only its active account/device owner may remove unfinished staging |
| `object_header` | Immutable object UUID; returns its validated BlobHeader |
| `object_chunk` | Object UUID, aligned offset and exact byte count; returns binary bytes with the unchanged header and chunk digest |

Every chunk is exactly 256 KiB except the final remainder. Zero, out-of-range,
unaligned, oversized, wrong-length, wrong-hash or changed-header requests/results
are rejected. The server accepts one contiguous prefix, with idempotent duplicate
chunks only when their exact bytes match. New clients require this authenticated
capability extension. There is no automatic downgrade after authentication or
integrity failures. Old whole-object put/get operations remain available for
older clients; deploy the matching server source before upgrading reconciliation.

The cipher stays the existing XChaCha20-Poly1305 envelope/AAD format. Binary
`encrypt_blob/decrypt_blob` APIs avoid base64-expanded full-object copies in the
new path. The full ciphertext SHA-256 and AEAD authentication are still required
before plaintext progress is imported. Chunk hashes authorize/validate transport;
they do not replace the save's AEAD or metadata/payload integrity checks.

## Client publication and resumption

One upload slot and one download slot use bounded preference records in the
existing profile database and generated UUID files in `sync-transfers-v1`.
Records are limited to 4096 bytes. Each file is bounded by the existing encrypted
object limit. Only encrypted bytes and nonsecret transport identity/hash fields
are persisted. Device/root secrets and plaintext save payloads are not staged.

An outgoing save is encrypted once, published as a fully synced immutable file,
then referenced by its persisted transfer record. A restart reuses those exact
ciphertext bytes, nonce, header and UUID. It asks the server for the received
prefix and sends the remaining chunks. The currently staged save is prioritized
before other outgoing rows so new local captures cannot repeatedly evict it.
Before sending bytes the client rehashes its staged ciphertext; a missing/corrupt
upload is aborted and reset without changing the original local save. Upload
receipts are committed before local encrypted staging is retired. A lost commit
reply is recovered by the next begin's already-stored result.

If a remote object with that save UUID already uses another ciphertext nonce,
the client downloads it through the same resumable path and authenticates/decrypts
it. The subsequent `SAVE_ORIGINS.md` ledger supplies the complete canonical
fingerprint when known. Only legacy unknown projections use the older relation
that excludes mapped game IDs, local order/conflict state and missing-parent
projections. A matching authenticated wire record can establish the retained
origin; it does not prove an older historical capture. The object is acknowledged,
never overwritten or reparented. Different known origins remain a conflict.
Complete versioned provenance envelopes and their evidence remain required.

For downloads the client commits each chunk's data before advancing its SQLite
prefix offset and prefix digest. Restart truncates an uncommitted tail, rehashes
the committed prefix and continues the same immutable object. A bad prefix is
discarded; a remote header change cannot silently adopt old partial bytes. The
complete ciphertext enters the durable existing inbox before the partial slot is
retired or the account list cursor advances. An already-staged inbox object is
reused after a lost cursor write instead of being downloaded again.

Sweep code inspects at most 16 entries, keeps the two currently referenced files,
and removes only regular generated UUID upload/download/temporary files within
the private transport directory. Unexpected names, links, reparse points and
oversized files fail closed. Account disconnect disables sync/consent and removes
these client transport slots before removing the delegated credential. Incomplete
server staging is separately bounded by expiry and explicit root account deletion.

## Process and OS opportunity ownership

A native profile's sync/account mutation paths acquire one nonblocking kernel
file lease, retained until the actual operation exits. It covers reconciliation,
account creation/registration/recovery/renewal/root operations and disconnect;
delete-and-disconnect share one lease. Competing processes receive Busy instead
of sweeping each other's live encrypted files. The lease file is never unlinked,
replaced or treated as an expiring heartbeat. The server also holds one process
lease over its storage root, preventing multiple writers from bypassing the
in-process publication lock. Rust's minimum version is now 1.89 for the stable
standard-library file-lock API; the cached toolchain manifest is newer, but no
compiler or lock API has run.

Mobile OS expiration/cancellation remains cooperative. Every range request uses
the remaining monotonic native opportunity deadline, with lifecycle/native lock
checks between phases. Whole-object encryption/decryption, file sync, hashing and
OS/DNS operations are not forcibly preempted. The native worker and kernel lease
remain alive after OS task completion until actual exit. The UI reports durable
transferred bytes independently from completed save counts and identifies a fully
transferred upload that still needs its final acknowledgement.

## Server durability and resource ownership

The service's transactional schema-2 upload row binds account, device, object,
header, received prefix, expiry and completion. Chunk files are synced and
published before prefix acknowledgement; replay after a crash verifies an already
published chunk before advancing SQLite. Commit assembles and verifies the full
ciphertext, syncs/publishes its object file, and commits the immutable object row
before acknowledging success. A crash between file publication and SQL commit
reuses verified content. An expired or revoked upload cannot publish through a
fresh commit boundary.

Four uploads per account and 32 per service are admitted. Twice each staged
ciphertext size is reserved against account/global quotas for parts plus assembly;
legacy reservations also count these staged bytes. Failed cleanup retains its
row and conservative quota debt. Completion and 24-hour expiry cleanup are bounded
to eight records per admission or sequential 60-second maintenance opportunity.
Busy permits defer maintenance; expiry refuses access but does not promise file
deletion at an exact wall-clock instant. Cleanup enumerates only the expected generated
files and refuses unknown entries. Restart/retry removes stale temporary assembly
files before new allocation; an at-most-32-entry process cache avoids rescanning
the whole directory for every chunk. Publication failures invalidate that cache.

Eight explicit operation permits survive dropped HTTP futures, queued/running
blocking work and response-body byte ownership. Request buffers are at most one
chunk, with a 30-second body deadline; metadata responses are at most 16 KiB.
`spawn_blocking` isolates synchronous file/SQLite work from Tokio worker threads.
Operations currently serialize on the store mutex, including commit assembly;
throughput/latency still need measurement and refinement. Revocation cancels all
unfinished uploads for its account, and root deletion removes upload files before
retiring the account. Both recheck the existing authenticated root/device rules.

## Explicit limits and final evidence still required

No tests or builds have run. The consolidated pass must cover legacy/new wire and
cipher compatibility; all signature/nonce/account/device/offset/hash/size failures;
capability mismatch and downgrade refusal; each file/SQLite/ack crash boundary;
duplicate and lost begin/chunk/commit replies; expiry/revocation/deletion during
receive/assembly/publication; permit retention after disconnect/abandonment and
slow responses; two clients/processes and real OS file locks; quotas/temp recovery;
malformed and corrupt local records/files; prefix recovery and inbox/cursor order;
missing games/parents and preserved conflict branches; cancellation/locking/OS
opportunities; backup/uninstall; real mobile energy, memory, latency and throughput.

The native path now uses a single owned in-place AEAD allocation and private-file
download/inbox handoff; see `SYNC_MEMORY_OWNERSHIP.md`. Full-object AEAD still
requires a whole-object working buffer. This is not a fixed-RSS or streaming-AEAD
claim. Slow devices can exhaust an OS
opportunity during hashing/crypto/preparation. Strict asynchronous transport abort,
segmented-crypto/resource design, complete canonical save provenance, full
filesystem containment/quota/orphan recovery, service maintenance/ingress, privacy/
store/device/security evidence and all other original product requirements remain
outstanding. Unix no-follow leaves and Windows reparse rejection do not establish
kernel containment against hostile ancestor replacement. Directory durability on
Windows is not claimed. The service remains an unlaunched private loopback HTTP
upstream requiring a real authenticated production TLS deployment.

Primary implementation contracts: [Rust file leases](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock),
[Tokio blocking mutex use](https://docs.rs/tokio/1.53.1/tokio/sync/struct.Mutex.html#method.blocking_lock),
[pinned byte-owner lifetime](https://docs.rs/bytes/1.12.1/bytes/struct.Bytes.html#method.from_owner),
[Windows reparse handling](https://learn.microsoft.com/en-us/windows/win32/fileio/symbolic-link-effects-on-file-systems-functions),
[Linux no-follow behavior](https://man7.org/linux/man-pages/man2/open.2.html).
