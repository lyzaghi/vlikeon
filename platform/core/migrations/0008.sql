BEGIN IMMEDIATE;
ALTER TABLE games ADD COLUMN favorite INTEGER NOT NULL DEFAULT 0 CHECK(favorite IN (0,1));
CREATE INDEX games_favorite_search_order ON games(favorite,search_title,id);
CREATE INDEX games_system_favorite_search_order ON games(system,favorite,search_title,id);
PRAGMA user_version=8;
COMMIT;
