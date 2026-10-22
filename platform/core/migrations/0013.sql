BEGIN IMMEDIATE;
-- Reserve the original UUID path separately from journaled payload files.
-- This conservatively retains its reservation after a legacy payload repair.
CREATE TABLE save_legacy_payloads (
    save_id TEXT PRIMARY KEY REFERENCES saves(id) ON DELETE CASCADE,
    game_id TEXT NOT NULL,
    payload_bytes INTEGER NOT NULL CHECK(payload_bytes BETWEEN 1 AND 268435456)
) STRICT;
INSERT INTO save_legacy_payloads(save_id,game_id,payload_bytes)
    SELECT id,game_id,payload_bytes FROM saves;
CREATE TRIGGER save_legacy_payload_reservation AFTER INSERT ON saves BEGIN
    INSERT INTO save_legacy_payloads(save_id,game_id,payload_bytes)
        VALUES(NEW.id,NEW.game_id,NEW.payload_bytes);
END;
PRAGMA user_version=13;
COMMIT;
