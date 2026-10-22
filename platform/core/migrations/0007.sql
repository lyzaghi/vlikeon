BEGIN IMMEDIATE;
CREATE INDEX games_search_order ON games(search_title,id);
CREATE INDEX games_system_search_order ON games(system,search_title,id);
PRAGMA user_version=7;
COMMIT;
