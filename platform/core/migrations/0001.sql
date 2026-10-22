BEGIN IMMEDIATE;
CREATE TABLE preferences (key TEXT PRIMARY KEY, value TEXT NOT NULL) STRICT;
CREATE TABLE games (
    id TEXT PRIMARY KEY,
    content_hash TEXT NOT NULL UNIQUE CHECK(length(content_hash)=64),
    title TEXT NOT NULL,
    search_title TEXT NOT NULL,
    system TEXT NOT NULL,
    manifest_json TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    last_played_at_ms INTEGER,
    playtime_ms INTEGER NOT NULL DEFAULT 0 CHECK(playtime_ms>=0)
) STRICT;
CREATE INDEX games_title ON games(title COLLATE NOCASE);
CREATE TABLE collections (id TEXT PRIMARY KEY, title TEXT NOT NULL) STRICT;
CREATE TABLE collection_games (
    collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    PRIMARY KEY(collection_id,game_id)
) STRICT;
CREATE TABLE saves (
    id TEXT PRIMARY KEY,
    game_id TEXT NOT NULL REFERENCES games(id),
    kind TEXT NOT NULL CHECK(kind IN ('in_game','emulator_state')),
    slot INTEGER NOT NULL CHECK(slot>=0 AND slot<=65535),
    generation INTEGER NOT NULL CHECK(generation>0),
    parent_id TEXT REFERENCES saves(id),
    metadata_json TEXT NOT NULL,
    payload_hash TEXT NOT NULL CHECK(length(payload_hash)=64),
    payload_bytes INTEGER NOT NULL CHECK(payload_bytes>0),
    created_at_ms INTEGER NOT NULL,
    UNIQUE(game_id,kind,slot,generation)
) STRICT;
CREATE TABLE save_heads (
    game_id TEXT NOT NULL REFERENCES games(id),
    kind TEXT NOT NULL,
    slot INTEGER NOT NULL,
    save_id TEXT NOT NULL REFERENCES saves(id),
    PRIMARY KEY(game_id,kind,slot)
) STRICT;
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    runtime_id TEXT NOT NULL,
    state TEXT NOT NULL,
    info_json TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
) STRICT;
CREATE TABLE audit (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    event TEXT NOT NULL,
    entity_id TEXT,
    created_at_ms INTEGER NOT NULL
) STRICT;
CREATE TABLE trusted_release_state (
    channel TEXT PRIMARY KEY,
    metadata_version INTEGER NOT NULL CHECK(metadata_version>=0),
    security_epoch INTEGER NOT NULL CHECK(security_epoch>=0),
    trusted_root_version INTEGER NOT NULL CHECK(trusted_root_version>=0),
    updated_at_ms INTEGER NOT NULL
) STRICT;
CREATE TABLE sync_outbox (
    id TEXT PRIMARY KEY,
    save_id TEXT NOT NULL REFERENCES saves(id),
    created_at_ms INTEGER NOT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    state TEXT NOT NULL CHECK(state IN ('pending','uploading','uploaded','conflict','failed'))
) STRICT;
PRAGMA user_version=1;
COMMIT;
