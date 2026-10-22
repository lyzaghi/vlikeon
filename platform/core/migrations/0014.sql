BEGIN IMMEDIATE;
CREATE TABLE native_work_storage (
    work_id TEXT PRIMARY KEY NOT NULL,
    profile_id TEXT NOT NULL,
    reserved_bytes INTEGER NOT NULL CHECK(reserved_bytes BETWEEN 1 AND 8589934592),
    file_count INTEGER NOT NULL CHECK(file_count BETWEEN 1 AND 144),
    phase TEXT NOT NULL CHECK(phase IN ('reserved','staged','retiring')),
    manifest_sha256 TEXT CHECK(manifest_sha256 IS NULL OR length(manifest_sha256)=64 AND manifest_sha256 NOT GLOB '*[^0-9a-f]*'),
    created_at_ms INTEGER NOT NULL CHECK(created_at_ms >= 0),
    CHECK((phase='reserved' AND manifest_sha256 IS NULL) OR phase='staged' AND manifest_sha256 IS NOT NULL OR phase='retiring')
) STRICT;
CREATE TRIGGER native_work_storage_quota BEFORE INSERT ON native_work_storage
WHEN (SELECT count(*) FROM native_work_storage)>=64
  OR (SELECT COALESCE(sum(reserved_bytes),0) FROM native_work_storage)+NEW.reserved_bytes>34359738368
  OR (SELECT COALESCE(sum(file_count),0) FROM native_work_storage)+NEW.file_count>4096
BEGIN SELECT RAISE(ABORT,'native_work_storage_quota'); END;
CREATE TRIGGER native_work_storage_identity BEFORE UPDATE OF work_id,profile_id,reserved_bytes,file_count,created_at_ms ON native_work_storage
BEGIN SELECT RAISE(ABORT,'native_work_storage_identity'); END;
CREATE INDEX native_work_storage_retirement ON native_work_storage(phase,created_at_ms,work_id);
CREATE TRIGGER native_work_storage_phase BEFORE UPDATE OF phase ON native_work_storage
WHEN NOT (NEW.phase=OLD.phase OR OLD.phase='reserved' AND NEW.phase IN ('staged','retiring') OR OLD.phase='staged' AND NEW.phase='retiring')
BEGIN SELECT RAISE(ABORT,'native_work_storage_phase'); END;
PRAGMA user_version=14;
COMMIT;
