BEGIN IMMEDIATE;
CREATE TABLE installed_games (
    game_id TEXT PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,
    private_launch_json TEXT NOT NULL,
    consented_at_ms INTEGER NOT NULL
) STRICT;
PRAGMA user_version=2;
COMMIT;
