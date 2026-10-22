BEGIN IMMEDIATE;
CREATE TABLE uploads (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    device_id TEXT NOT NULL,
    object_id TEXT NOT NULL,
    header_json TEXT NOT NULL,
    byte_length INTEGER NOT NULL CHECK(byte_length>=16),
    received_bytes INTEGER NOT NULL DEFAULT 0 CHECK(received_bytes>=0 AND received_bytes<=byte_length),
    expires_at_ms INTEGER NOT NULL,
    completed INTEGER NOT NULL DEFAULT 0 CHECK(completed IN (0,1)),
    UNIQUE(account_id,object_id)
) STRICT;
CREATE INDEX upload_expiry ON uploads(expires_at_ms);
PRAGMA user_version=2;
COMMIT;
