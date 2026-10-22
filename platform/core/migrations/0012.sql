BEGIN IMMEDIATE;
-- Repair files have a separate UUID namespace. A staged file is never read as
-- saved progress; only a committed location can make it visible to readers.
CREATE TABLE save_repair_files (
    file_id TEXT PRIMARY KEY,
    save_id TEXT NOT NULL,
    game_id TEXT NOT NULL,
    payload_hash TEXT NOT NULL CHECK(length(payload_hash)=64),
    payload_bytes INTEGER NOT NULL CHECK(payload_bytes BETWEEN 1 AND 268435456),
    state TEXT NOT NULL CHECK(state IN ('staged','live','retired')),
    created_at_ms INTEGER NOT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK(attempt_count>=0)
) STRICT;
CREATE TABLE save_payload_locations (
    save_id TEXT PRIMARY KEY REFERENCES saves(id) ON DELETE CASCADE,
    file_id TEXT NOT NULL UNIQUE REFERENCES save_repair_files(file_id)
) STRICT;
CREATE INDEX save_repair_collection ON save_repair_files(state,attempt_count,created_at_ms);
CREATE TRIGGER save_payload_location_retired AFTER DELETE ON save_payload_locations BEGIN
    UPDATE save_repair_files SET state='retired' WHERE file_id=OLD.file_id;
END;
PRAGMA user_version=12;
COMMIT;
