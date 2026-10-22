BEGIN IMMEDIATE;
-- Media manifests extend persisted game metadata. Keep older readers from treating
-- a bundle identity as a single blob or dropping the format and referenced tracks.
PRAGMA user_version=6;
COMMIT;
