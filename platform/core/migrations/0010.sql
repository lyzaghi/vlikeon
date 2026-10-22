BEGIN IMMEDIATE;
CREATE INDEX saves_game_history_page ON saves(game_id,generation DESC,created_at_ms DESC,id DESC);
PRAGMA user_version=10;
COMMIT;
