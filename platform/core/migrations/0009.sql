BEGIN IMMEDIATE;
CREATE TABLE locally_removed_saves (
    save_id TEXT PRIMARY KEY,
    game_content_hash TEXT NOT NULL CHECK(length(game_content_hash)=64),
    removed_at_ms INTEGER NOT NULL
) STRICT;
CREATE TABLE save_payload_gc (
    save_id TEXT PRIMARY KEY,
    game_id TEXT NOT NULL,
    payload_bytes INTEGER NOT NULL CHECK(payload_bytes>0),
    retired_at_ms INTEGER NOT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK(attempt_count>=0)
) STRICT;
PRAGMA user_version=9;
COMMIT;
