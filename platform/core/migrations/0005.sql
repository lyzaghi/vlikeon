BEGIN IMMEDIATE;
CREATE TABLE firmware_assets (
    content_hash TEXT PRIMARY KEY,
    byte_length INTEGER NOT NULL CHECK(byte_length>0 AND byte_length<=33554432),
    created_at_ms INTEGER NOT NULL
) STRICT;
CREATE TABLE firmware_selections (
    system TEXT NOT NULL,
    role TEXT NOT NULL,
    content_hash TEXT NOT NULL REFERENCES firmware_assets(content_hash),
    PRIMARY KEY(system,role)
) STRICT;
PRAGMA user_version=5;
COMMIT;
