//! One durable controller group across cooperating profiles/processes. Every
//! broker must use the same native-selected per-user directory. Not a driver
//! ACL, a machine-wide service, or authority over other applications' devices.
use crate::{Database,stream_gamepad_journal::{self as journal,Record,Snapshot,Phase,RecoveryObserver},util::storage_error};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use rusqlite::{Connection,TransactionBehavior,params};
use std::path::Path;
const APPLICATION:u32=0x564c4750;
fn invalid()->PlatformError{PlatformError::new(ErrorCode::CorruptData,"Shared gamepad ownership requires confirmed native recovery")}
fn busy()->PlatformError{PlatformError::new(ErrorCode::Busy,"Another native gamepad group still owns the shared reservation")}
fn conflict()->PlatformError{PlatformError::new(ErrorCode::Conflict,"Shared gamepad ownership changed; reload its authoritative record")}

/// One group of up to four slots, matching the current single-group host model.
/// Persisted ownership is never deleted by object Drop, expiry or process loss.
pub struct Arbitration{connection:Connection}
impl Arbitration{
    /// The install service supplies an existing private coordination directory.
    /// Its ACL/ancestor integrity is a prerequisite; no renderer-selected path,
    /// environment override or directory creation is accepted by this API.
    pub fn open(directory:&Path)->Result<Self>{
        if !directory.is_absolute(){return Err(invalid());}
        ordinary(directory,true)?;let path=directory.join("native-gamepad-ownership.sqlite3");
        match std::fs::symlink_metadata(&path){Ok(_)=>ordinary(&path,false)?,
            Err(error)if error.kind()==std::io::ErrorKind::NotFound=>{},Err(error)=>return Err(storage_error(error))}
        let mut connection=Connection::open_with_flags(&path,rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE|
            rusqlite::OpenFlags::SQLITE_OPEN_CREATE|rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX).map_err(storage_error)?;
        connection.busy_timeout(std::time::Duration::from_millis(100)).map_err(storage_error)?;
        // Refuse a foreign database before changing its journal mode.
        validate_header(&connection)?;
        connection.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;").map_err(storage_error)?;
        let tx=connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage_error)?;
        if validate_header(&tx)?{
            tx.execute_batch("CREATE TABLE gamepad_owner(singleton INTEGER PRIMARY KEY CHECK(singleton=1),record TEXT NOT NULL CHECK(length(CAST(record AS BLOB)) BETWEEN 1 AND 8192)) STRICT;
                PRAGMA application_id=1447839568; PRAGMA user_version=1;").map_err(storage_error)?;
        }
        tx.commit().map_err(storage_error)?;
        let store=Self{connection};store.durable()?;let _=store.pending()?;Ok(store)
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
