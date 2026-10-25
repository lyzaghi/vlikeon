# Save Sync Reconciliation & Fault Recovery Design — Items 81, 82

Status: Authored reconciliation design and fault recovery specifications; live server and database fault injection deferred per project owner policy.

## 1. Asynchronous Transfer Abort & Lost Response Reconciliation (Item 81)

During cloud save synchronization, network disconnections, power loss, or application crashes can interrupt active transfers. The protocol guarantees exact reconciliation without data corruption or redundant re-transmissions:

### 1.1 Resumable Upload State Machine
```text
[Idle] ---> [Staging Locally] ---> [upload_begin] ---> [upload_chunk(0..N)] ---> [upload_commit] ---> [Finished]
                 |                       |                       |                      |
                 v                       v                       v                      v
           (Crash/Abort)            (Lost Begin)            (Lost Chunk)           (Lost Commit)
                 |                       |                       |                      |
                 +-----------------------+-----------------------+----------------------+
                                         |
                                         v
                            [Reconnect & upload_begin]
                                         |
                     +-------------------+-------------------+
                     |                                       |
           Server: Already Stored                  Server: Received Offset = K
                     |                                       |
            [Adopt Remote Digest]               [Resume upload_chunk(K..N)]
```

### 1.2 Reconciliation Rules
1. **Lost `upload_commit` Reply:** When the client disconnects before receiving confirmation of `upload_commit`, it does not re-upload the file. Upon reconnecting, `upload_begin` checks the transfer UUID and hash; if the object was already committed server-side, the server returns `already_stored: true` and the existing object digest, allowing the client to transition directly to `Finished`.
2. **Partial Chunk Progress:** If a multi-chunk upload is interrupted, `upload_begin` reports `received_offset: K`. The client resumes transmitting from chunk `K`, preserving bandwidth.
3. **Explicit Abort (`upload_abort`):** If the user cancels synchronization or a local save is invalidated, the client issues `upload_abort(transfer_id)`. The server immediately purges all staged partial chunks.

## 2. Storage Quota, Orphan Cleanup & Save Provenance (Item 82)

### 2.1 Quota Bounds
- Each account is allocated a hard quota (default: 500 MiB).
- `upload_begin` calculates `current_usage + object_size`; if the total exceeds account quota, the server refuses the upload with error `quota_exceeded`.
- Clients evaluate local storage reserve before downloading incoming saves, requiring at least 100 MiB of available local storage.

### 2.2 Orphan Staging Cleanup
- Incomplete uploads stored in `sync-transfers-v1` are assigned a 24-hour time-to-live (`staging_ttl_seconds = 86400`).
- An hourly background sweeper inspects staging folders:
  - Preserves active transfers referenced by live in-memory sessions.
  - Purges abandoned chunks older than 24 hours.
  - Logs reclamation statistics without leaking account IDs or filenames.

### 2.3 Save Provenance Ledger
- Every save file carries an immutable provenance envelope:
  - Canonical origin device ID.
  - Generation sequence number.
  - Parent state hash.
- Conflicting saves produced concurrently on different devices are preserved as branch snapshots rather than silently overwritten, enabling user-guided or timestamp-verified conflict resolution.
