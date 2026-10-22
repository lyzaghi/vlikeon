BEGIN IMMEDIATE;
CREATE TABLE performance_runs (
    id TEXT PRIMARY KEY,
    game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    created_at_ms INTEGER NOT NULL,
    input_hash TEXT NOT NULL,
    input_json TEXT NOT NULL,
    summary_json TEXT NOT NULL
) STRICT;
CREATE INDEX performance_game_time ON performance_runs(game_id,created_at_ms DESC);
PRAGMA user_version=4;
COMMIT;
