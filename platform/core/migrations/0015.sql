BEGIN IMMEDIATE;
-- Existing debts have no proof that their creator observed the Windows
-- creation lease. They keep protocol zero and cannot use pre-launch recovery.
ALTER TABLE native_work_storage ADD COLUMN windows_creation_protocol INTEGER
    NOT NULL DEFAULT 0 CHECK(windows_creation_protocol IN (0,1));
CREATE TRIGGER native_work_storage_creation_protocol
BEFORE UPDATE OF windows_creation_protocol ON native_work_storage
BEGIN SELECT RAISE(ABORT,'native_work_storage_creation_protocol'); END;
PRAGMA user_version=15;
COMMIT;
