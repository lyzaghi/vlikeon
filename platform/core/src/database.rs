use crate::util::storage_error;
use omniplay_contracts::{ErrorCode, PlatformError, Result, UserSettings};
use rusqlite::{Connection, OptionalExtension};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub struct Database { pub connection: Connection, pub root: PathBuf, pub device_id: Uuid }
pub const CURRENT_SCHEMA_VERSION:u32=15;

impl Database {
    pub fn open(root: &Path) -> Result<Self> {
        std::fs::create_dir_all(root).map_err(storage_error)?;
        #[cfg(unix)]
        {use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(root,std::fs::Permissions::from_mode(0o700)).map_err(storage_error)?;}
        let connection = Connection::open(root.join("omniplay.sqlite3")).map_err(storage_error)?;
        connection.busy_timeout(std::time::Duration::from_secs(5)).map_err(storage_error)?;
        connection.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA trusted_schema=OFF;").map_err(storage_error)?;
        let version: u32 = connection.query_row("PRAGMA user_version", [], |row| row.get(0)).map_err(storage_error)?;
        if version > CURRENT_SCHEMA_VERSION { return Err(PlatformError::new(ErrorCode::InvalidState, "Database was created by a newer application version")); }
        if version == 0 {
            connection.execute_batch(include_str!("../migrations/0001.sql")).map_err(storage_error)?;
        }
        if version < 2 {
            connection.execute_batch(include_str!("../migrations/0002.sql")).map_err(storage_error)?;
        }
        if version < 3 {
            connection.execute_batch(include_str!("../migrations/0003.sql")).map_err(storage_error)?;
        }
        if version < 4 {connection.execute_batch(include_str!("../migrations/0004.sql")).map_err(storage_error)?;}
        if version < 5 {connection.execute_batch(include_str!("../migrations/0005.sql")).map_err(storage_error)?;}
        if version < 6 {connection.execute_batch(include_str!("../migrations/0006.sql")).map_err(storage_error)?;}
        if version < 7 {connection.execute_batch(include_str!("../migrations/0007.sql")).map_err(storage_error)?;}
        if version < 8 {connection.execute_batch(include_str!("../migrations/0008.sql")).map_err(storage_error)?;}
        if version < 9 {connection.execute_batch(include_str!("../migrations/0009.sql")).map_err(storage_error)?;}
        if version < 10 {connection.execute_batch(include_str!("../migrations/0010.sql")).map_err(storage_error)?;}
        if version < 11 {connection.execute_batch(include_str!("../migrations/0011.sql")).map_err(storage_error)?;}
        if version < 12 {connection.execute_batch(include_str!("../migrations/0012.sql")).map_err(storage_error)?;}
        if version < 13 {connection.execute_batch(include_str!("../migrations/0013.sql")).map_err(storage_error)?;}
        if version < 14 {connection.execute_batch(include_str!("../migrations/0014.sql")).map_err(storage_error)?;}
        if version < 15 {connection.execute_batch(include_str!("../migrations/0015.sql")).map_err(storage_error)?;}
        let existing: Option<String> = connection.query_row("SELECT value FROM preferences WHERE key='device_id'", [], |row| row.get(0)).optional().map_err(storage_error)?;
        let device_id = match existing {
            Some(value) => Uuid::parse_str(&value).map_err(|_| PlatformError::new(ErrorCode::CorruptData, "Device identity is invalid"))?,
            None => {
                let id = Uuid::new_v4();
                connection.execute("INSERT INTO preferences(key,value) VALUES('device_id',?1)", [id.to_string()]).map_err(storage_error)?;
                id
            }
        };
        let database=Self { connection, root: root.to_owned(), device_id };
        // Cleanup is bounded and debt is durable. A busy writer/locked file or
        // cleanup failure does not make a readable profile fail to open.
        let _=crate::save_maintenance::cleanup(&database,32);
        let _=crate::native_work_storage::collect_removed(&database,32);
        Ok(database)
    }

    pub fn settings(&self) -> Result<UserSettings> {
        let json: Option<String> = self.connection.query_row("SELECT value FROM preferences WHERE key='settings'", [], |row| row.get(0)).optional().map_err(storage_error)?;
        match json {
            Some(value) => {
                let settings: UserSettings = serde_json::from_str(&value).map_err(|_| PlatformError::new(ErrorCode::CorruptData, "Settings need recovery"))?;
                settings.validate()?;
                Ok(settings)
            }
            None => Ok(UserSettings::default()),
        }
    }

    pub fn set_settings(&self, settings: &UserSettings) -> Result<()> {
        settings.validate()?;
        let json = serde_json::to_string(settings).map_err(storage_error)?;
        self.connection.execute("INSERT INTO preferences(key,value) VALUES('settings',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [json]).map_err(storage_error)?;
        Ok(())
    }

    pub fn record_audit(&self, event: &str, entity: Option<Uuid>) -> Result<()> {
        if event.len() > 128 || !event.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'.') {
            return Err(PlatformError::new(ErrorCode::InvalidRequest, "Invalid audit event"));
        }
        self.connection.execute("INSERT INTO audit(event,entity_id,created_at_ms) VALUES(?1,?2,?3)", rusqlite::params![event, entity.map(|v| v.to_string()), crate::now_ms()]).map_err(storage_error)?;
        Ok(())
    }

    /// Device-local scheduling consent. Missing rows remain disabled; this is
    /// deliberately separate from account connection and foreground sync consent.
    pub fn mobile_background_sync(&self)->Result<bool>{
        let value:Option<String>=self.connection.query_row("SELECT value FROM preferences WHERE key='mobile_background_sync_v1'",[],|row|row.get(0)).optional().map_err(storage_error)?;
        match value.as_deref(){None|Some("false")=>Ok(false),Some("true")=>Ok(true),_=>Err(PlatformError::new(ErrorCode::CorruptData,"Background synchronization preference needs recovery"))}
    }
    pub fn set_mobile_background_sync(&self,enabled:bool)->Result<()>{
        self.connection.execute("INSERT INTO preferences(key,value) VALUES('mobile_background_sync_v1',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[if enabled{"true"}else{"false"}]).map_err(storage_error)?;Ok(())
    }
}
