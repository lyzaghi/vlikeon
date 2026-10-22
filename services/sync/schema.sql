PRAGMA foreign_keys=ON;
PRAGMA journal_mode=WAL;
PRAGMA synchronous=FULL;
PRAGMA trusted_schema=OFF;
CREATE TABLE IF NOT EXISTS accounts (
    id TEXT PRIMARY KEY,
    root_public_key TEXT NOT NULL UNIQUE,
    quota_bytes INTEGER NOT NULL,
    deleting INTEGER NOT NULL DEFAULT 0 CHECK(deleting IN (0,1)),
    created_at_ms INTEGER NOT NULL
) STRICT;
CREATE TABLE IF NOT EXISTS retired_accounts (id TEXT PRIMARY KEY) STRICT;
CREATE TABLE IF NOT EXISTS devices (
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    device_id TEXT NOT NULL,
    device_public_key TEXT NOT NULL,
    first_seen_at_ms INTEGER NOT NULL,
    last_seen_at_ms INTEGER NOT NULL,
    revoked_at_ms INTEGER,
    PRIMARY KEY(account_id,device_id)
) STRICT;
CREATE TABLE IF NOT EXISTS nonces (
    account_id TEXT NOT NULL,
    nonce TEXT NOT NULL,
    expires_at_ms INTEGER NOT NULL,
    PRIMARY KEY(account_id,nonce)
) STRICT;
CREATE INDEX IF NOT EXISTS nonce_expiry ON nonces(expires_at_ms);
CREATE TABLE IF NOT EXISTS objects (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    object_id TEXT NOT NULL,
    header_json TEXT NOT NULL,
    ciphertext_sha256 TEXT NOT NULL,
    byte_length INTEGER NOT NULL,
    uploaded_at_ms INTEGER NOT NULL,
    UNIQUE(account_id,object_id)
) STRICT;
CREATE INDEX IF NOT EXISTS account_object_cursor ON objects(account_id,sequence);
CREATE TABLE IF NOT EXISTS reservations (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    object_id TEXT NOT NULL,
    byte_length INTEGER NOT NULL,
    expires_at_ms INTEGER NOT NULL,
    UNIQUE(account_id,object_id)
) STRICT;
PRAGMA user_version=1;
