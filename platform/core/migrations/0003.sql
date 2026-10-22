BEGIN IMMEDIATE;
CREATE TABLE sync_receipts (
    account_id TEXT NOT NULL,
    save_id TEXT NOT NULL REFERENCES saves(id),
    remote_object_id TEXT NOT NULL,
    direction TEXT NOT NULL CHECK(direction IN ('uploaded','downloaded')),
    acknowledged_at_ms INTEGER NOT NULL,
    PRIMARY KEY(account_id,save_id),
    UNIQUE(account_id,remote_object_id)
) STRICT;
CREATE TABLE sync_inbox (
    account_id TEXT NOT NULL,
    object_id TEXT NOT NULL,
    remote_sequence INTEGER NOT NULL,
    header_json TEXT NOT NULL,
    received_at_ms INTEGER NOT NULL,
    PRIMARY KEY(account_id,object_id)
) STRICT;
PRAGMA user_version=3;
COMMIT;
