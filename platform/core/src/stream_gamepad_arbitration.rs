//! One durable controller group across cooperating profiles/processes. Every
//! broker must use the same native-selected per-user directory. Not a driver
//! ACL, a machine-wide service, or authority over other applications' devices.
use crate::{Database,stream_gamepad_journal::{self as journal,Record,Snapshot,Phase,RecoveryObserver},util::storage_error};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use rusqlite::{Connection,TransactionBehavior,params};
use std::{fs::{File,OpenOptions},path::Path};
const APPLICATION:u32=0x564c4750;
fn invalid()->PlatformError{PlatformError::new(ErrorCode::CorruptData,"Shared gamepad ownership requires confirmed native recovery")}
fn busy()->PlatformError{PlatformError::new(ErrorCode::Busy,"Another native gamepad group still owns the shared reservation")}
fn conflict()->PlatformError{PlatformError::new(ErrorCode::Conflict,"Shared gamepad ownership changed; reload its authoritative record")}

/// One group of up to four slots, matching the current single-group host model.
/// Persisted ownership is never deleted by object Drop, expiry or process loss.
pub struct Arbitration{connection:Connection,_file:File,_directory:File}
impl Arbitration{
    /// The install service supplies an existing private coordination directory.
    /// Its ACL/ancestor integrity is a prerequisite; no renderer-selected path,
    /// environment override or directory creation is accepted by this API.
    pub fn open(directory:&Path)->Result<Self>{
        if !directory.is_absolute(){return Err(invalid());}
        ordinary(directory,true)?;let path=directory.join("native-gamepad-ownership.sqlite3");
        let guard=directory_guard(directory)?;ordinary(&path,false)?;
        let file=file_guard(&path,false)?;
        Self::connect(&path,file,guard,false)
    }
    /// Explicit install/bootstrap entry, never automatic repair. Admission of
    /// the private root and current broker protocol is a prerequisite. All
    /// existing native profiles must be readable and have NO ownership debt.
    /// create_new refuses to overwrite an old, empty or partially initialized
    /// coordinator. Failure leaves it for reviewed recovery, never deletion.
    pub fn initialize_new(directory:&Path)->Result<Self>{
        if !directory.is_absolute(){return Err(invalid());}
        ordinary(directory,true)?;let guard=directory_guard(directory)?;
        require_debt_free_profiles(directory)?;
        let path=directory.join("native-gamepad-ownership.sqlite3");
        let file=file_guard(&path,true)?;
        Self::connect(&path,file,guard,true)
    }
    fn connect(path:&Path,file:File,directory:File,initialize:bool)->Result<Self>{
        let mut connection=Connection::open_with_flags(&path,rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE|
            rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX).map_err(storage_error)?;
        connection.busy_timeout(std::time::Duration::from_millis(100)).map_err(storage_error)?;
        // Refuse a foreign database before changing its journal mode.
        if validate_header(&connection)?&&!initialize{return Err(invalid());}
        connection.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;").map_err(storage_error)?;
        let tx=connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage_error)?;
        if validate_header(&tx)?{
            if !initialize{return Err(invalid());}
            tx.execute_batch("CREATE TABLE gamepad_owner(singleton INTEGER PRIMARY KEY CHECK(singleton=1),record TEXT NOT NULL CHECK(length(CAST(record AS BLOB)) BETWEEN 1 AND 8192)) STRICT;
                PRAGMA application_id=1447839568; PRAGMA user_version=1;").map_err(storage_error)?;
        }
        tx.commit().map_err(storage_error)?;
        let store=Self{connection,_file:file,_directory:directory};store.durable()?;let _=store.pending()?;Ok(store)
    }
    fn durable(&self)->Result<()>{
        let mode:String=self.connection.query_row("PRAGMA journal_mode",[],|row|row.get(0)).map_err(storage_error)?;
        let synchronous:u32=self.connection.query_row("PRAGMA synchronous",[],|row|row.get(0)).map_err(storage_error)?;
        if !self.connection.is_autocommit()||mode!="wal"||!matches!(synchronous,2|3){return Err(invalid());}Ok(())
    }
    pub fn pending(&self)->Result<Option<Record>>{read(&self.connection).map(|value|value.map(|(_,record)|record))}
    /// After the profile prepared journal commits, before activation intent/OS
    /// creation. A process dying here leaves durable shared cleanup debt.
    pub fn reserve(&mut self,database:&Database,profile:&Snapshot)->Result<()>{
        self.durable()?;journal::require_current(database,profile)?;
        if profile.record().phase()!=Phase::Reserved||profile.record().activation_attempted(){return Err(invalid());}
        let tx=self.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage_error)?;
        if read(&tx)?.is_some(){return Err(busy());}
        tx.execute("INSERT INTO gamepad_owner(singleton,record) VALUES(1,?1)",[profile.raw()]).map_err(storage_error)?;
        tx.commit().map_err(storage_error)?;Ok(())
    }
    /// Mirror committed profile intent before sending Activate, then mirror
    /// presence/cleanup. Cross-database commit is not atomic: any uncertain side
    /// blocks reuse and must be reloaded, never assumed rolled back.
    pub fn mirror(&mut self,database:&Database,profile:&Snapshot)->Result<()>{
        self.durable()?;journal::require_current(database,profile)?;
        let tx=self.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage_error)?;
        let Some((raw,current))=read(&tx)?else{return Err(conflict())};
        if !same_group(&current,profile.record())||profile.record().revision()<current.revision()||
           (profile.record().revision()==current.revision()&&profile.raw()!=raw){return Err(conflict());}
        if tx.execute("UPDATE gamepad_owner SET record=?1 WHERE singleton=1 AND record=?2",
            params![profile.raw(),raw]).map_err(storage_error)?!=1{return Err(conflict());}
        tx.commit().map_err(storage_error)?;Ok(())
    }
    /// Only a CURRENT profile cleanup snapshot with its complete device set
    /// acknowledged removed and exact helper retired can release shared debt.
    /// Delete the shared row before deleting the profile's proof. A missing or
    /// corrupt profile is not absence evidence and cannot force-clear this row.
    pub fn release(&mut self,database:&Database,profile:&Snapshot,observer:&mut impl RecoveryObserver)->Result<()>{
        self.durable()?;journal::require_current(database,profile)?;
        if profile.record().phase()!=Phase::CleanupRequired||profile.record().devices().iter().any(|device|!device.removed())||
           !observer.helper_exited(profile.record().helper())?{return Err(busy());}
        let tx=self.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage_error)?;
        let Some((raw,current))=read(&tx)?else{tx.commit().map_err(storage_error)?;return Ok(())};
        if !same_group(&current,profile.record()){
            // Old retired ownership must not delete a replacement. Its own
            // profile proof can still be finalized without touching this row.
            tx.commit().map_err(storage_error)?;return Ok(());
        }
        if current.revision()>profile.record().revision(){return Err(conflict());}
        if tx.execute("DELETE FROM gamepad_owner WHERE singleton=1 AND record=?1",[raw]).map_err(storage_error)?!=1{return Err(conflict());}
        tx.commit().map_err(storage_error)?;Ok(())
    }
}
fn validate_header(connection:&Connection)->Result<bool>{
    let version:u32=connection.query_row("PRAGMA user_version",[],|row|row.get(0)).map_err(storage_error)?;
    let application:u32=connection.query_row("PRAGMA application_id",[],|row|row.get(0)).map_err(storage_error)?;
    if version==0&&application==0{
        let tables:u32=connection.query_row("SELECT count(*) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'",[],|row|row.get(0)).map_err(storage_error)?;
        if tables!=0{return Err(invalid());}return Ok(true);
    }
    if version!=1||application!=APPLICATION{return Err(invalid());}Ok(false)
}
fn same_group(left:&Record,right:&Record)->bool{
    left.reservation()==right.reservation()&&left.profile()==right.profile()&&left.device()==right.device()&&
        left.client()==right.client()&&left.owner()==right.owner()&&left.helper()==right.helper()&&
        left.helper_sha256()==right.helper_sha256()&&left.devices().iter().map(|device|device.identity()).eq(right.devices().iter().map(|device|device.identity()))
}
fn read(connection:&Connection)->Result<Option<(String,Record)>>{
    let mut statement=connection.prepare("SELECT singleton,length(CAST(record AS BLOB)),substr(record,1,8193) FROM gamepad_owner LIMIT 2").map_err(storage_error)?;
    let mut rows=statement.query([]).map_err(storage_error)?;
    let Some(row)=rows.next().map_err(storage_error)?else{return Ok(None)};
    let singleton:i64=row.get(0).map_err(storage_error)?;let bytes:i64=row.get(1).map_err(storage_error)?;
    let raw:String=row.get(2).map_err(storage_error)?;
    if singleton!=1||!(1..=8192).contains(&bytes)||raw.len()!=bytes as usize||rows.next().map_err(storage_error)?.is_some(){return Err(invalid());}
    let record:Record=serde_json::from_str(&raw).map_err(|_|invalid())?;record.validate()?;
    if serde_json::to_string(&record).map_err(storage_error)?!=raw{return Err(invalid());}Ok(Some((raw,record)))
}
fn ordinary(path:&Path,directory:bool)->Result<()>{
    let shape=std::fs::symlink_metadata(path).map_err(storage_error)?;
    let mut linked=shape.file_type().is_symlink();
    #[cfg(windows)]{use std::os::windows::fs::MetadataExt;linked|=shape.file_attributes()&0x400!=0;}
    if linked||(directory&&!shape.is_dir())||(!directory&&!shape.is_file()){return Err(invalid());}Ok(())
}
fn file_guard(path:&Path,create:bool)->Result<File>{
    let mut options=OpenOptions::new();options.read(true).write(true).create_new(create);
    #[cfg(windows)]{use std::os::windows::fs::OpenOptionsExt;
        // No FILE_SHARE_DELETE: retain the same named coordinator throughout
        // every live connection. OPEN_REPARSE_POINT refuses a link's target.
        options.share_mode(3).custom_flags(0x0020_0000);
    }
    #[cfg(unix)]{use std::os::unix::fs::OpenOptionsExt;options.mode(0o600).custom_flags(libc::O_NOFOLLOW|libc::O_CLOEXEC);}
    let file=options.open(path).map_err(storage_error)?;
    let shape=file.metadata().map_err(storage_error)?;
    if !shape.is_file(){return Err(invalid());}
    #[cfg(windows)]{use std::os::windows::fs::MetadataExt;if shape.file_attributes()&0x400!=0{return Err(invalid());}}
    Ok(file)
}
fn directory_guard(path:&Path)->Result<File>{
    let mut options=OpenOptions::new();options.read(true);
    #[cfg(windows)]{use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(3).custom_flags(0x0200_0000|0x0020_0000);
    }
    #[cfg(unix)]{use std::os::unix::fs::OpenOptionsExt;options.custom_flags(libc::O_DIRECTORY|libc::O_NOFOLLOW|libc::O_CLOEXEC);}
    let file=options.open(path).map_err(storage_error)?;let shape=file.metadata().map_err(storage_error)?;
    if !shape.is_dir(){return Err(invalid());}
    #[cfg(windows)]{use std::os::windows::fs::MetadataExt;if shape.file_attributes()&0x400!=0{return Err(invalid());}}
    Ok(file)
}
fn require_debt_free_profiles(directory:&Path)->Result<()>{
    use rusqlite::OptionalExtension;
    let profiles=directory.join("profiles");ordinary(&profiles,true)?;let _guard=directory_guard(&profiles)?;
    let mut count=0;
    for entry in std::fs::read_dir(&profiles).map_err(storage_error)?{
        count+=1;if count>4096{return Err(invalid());}
        let entry=entry.map_err(storage_error)?;let name=entry.file_name().into_string().map_err(|_|invalid())?;
        let profile=uuid::Uuid::parse_str(&name).map_err(|_|invalid())?;
        if profile.is_nil()||profile.to_string()!=name{return Err(invalid());}
        let root=entry.path();ordinary(&root,true)?;let _profile=directory_guard(&root)?;
        let database=root.join("omniplay.sqlite3");ordinary(&database,false)?;
        let connection=Connection::open_with_flags(database,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY|
            rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX).map_err(storage_error)?;
        connection.busy_timeout(std::time::Duration::from_millis(100)).map_err(storage_error)?;
        connection.execute_batch("PRAGMA trusted_schema=OFF;").map_err(storage_error)?;
        let version:u32=connection.query_row("PRAGMA user_version",[],|row|row.get(0)).map_err(storage_error)?;
        let(bytes,device):(i64,String)=connection.query_row("SELECT length(CAST(value AS BLOB)),substr(value,1,38) FROM preferences WHERE key='device_id'",[],|row|Ok((row.get(0)?,row.get(1)?))).map_err(storage_error)?;
        let id=uuid::Uuid::parse_str(&device).map_err(|_|invalid())?;
        if version!=crate::database::CURRENT_SCHEMA_VERSION||bytes!=36||id.is_nil()||id.to_string()!=device{return Err(invalid());}
        // Any row, including corrupt/unknown content, retains ownership. Do not
        // decode it as empty or run cleanup/migrations during bootstrap.
        let debt:Option<u32>=connection.query_row("SELECT 1 FROM preferences WHERE key='native_gamepad_ownership_v1'",[],|row|row.get(0)).optional().map_err(storage_error)?;
        if debt.is_some(){return Err(busy());}
    }
    Ok(())
}
