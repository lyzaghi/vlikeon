//! Broker-private copy debt and boot provenance. These files are outside the
//! sandboxed owner/actor container. Never accept a container marker as proof of
//! an earlier boot, and never reconstruct process authority from a disk PID.
use crate::unavailable;
use omniplay_contracts::*;
use omniplay_core::{Database,native_work_storage::{self as storage,RecoveryPhase,MacWorkJournal as Intent},util::{bounded_read,digest,storage_error}};
use omniplay_sandbox::macos::{MacConnection,kernel_boot_session};
use std::{fs::{self,File,OpenOptions},io::Write,os::unix::fs::{DirBuilderExt,MetadataExt,OpenOptionsExt},path::{Path,PathBuf}};
use uuid::Uuid;

pub(crate) struct MacWork{connection:MacConnection,profile_root:PathBuf,intent:Intent,hash:ContentHash,launch_attempted:bool,closed:bool}
#[derive(Default)]pub(crate) struct RecoverySummary{pub inspected:u32,pub removed:u32,pub retained:u32,pub remaining:u32}
fn invalid()->PlatformError{unavailable("macOS native work needs confirmed recovery before reuse")}
fn identifier(value:&str)->bool{!value.is_empty()&&value.len()<=64&&value.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'_'|b'-'))}
fn private_directory(path:&Path)->Result<File>{
    let file=OpenOptions::new().read(true).custom_flags(libc::O_DIRECTORY|libc::O_NOFOLLOW|libc::O_CLOEXEC).open(path).map_err(storage_error)?;
    let shape=file.metadata().map_err(storage_error)?;
    let parent=path.parent().ok_or_else(invalid)?;let owner=fs::metadata(parent).map_err(storage_error)?.uid();
    if !shape.is_dir()||shape.uid()!=owner||shape.mode()&0o7777!=0o700{return Err(invalid());}Ok(file)
}
fn create_directory(path:&Path)->Result<()>{fs::DirBuilder::new().mode(0o700).create(path).map_err(storage_error)}
fn root(profile:&Path)->Result<PathBuf>{
    let result=profile.join("native-work");
    match fs::symlink_metadata(&result){Err(error)if error.kind()==std::io::ErrorKind::NotFound=>{create_directory(&result)?;File::open(profile).and_then(|file|file.sync_all()).map_err(storage_error)?;},Err(error)=>return Err(storage_error(error)),Ok(_)=>{}}
    private_directory(&result)?;Ok(result)
}
fn present(path:&Path)->Result<bool>{match fs::symlink_metadata(path){Ok(_)=>Ok(true),Err(error)if error.kind()==std::io::ErrorKind::NotFound=>Ok(false),Err(error)=>Err(storage_error(error))}}
fn read_intent(directory:&Path)->Result<(Intent,ContentHash)>{
    let owner=private_directory(directory)?.metadata().map_err(storage_error)?.uid();
    let mut file=OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW|libc::O_CLOEXEC).open(directory.join("mac-work.json")).map_err(storage_error)?;
    let shape=file.metadata().map_err(storage_error)?;
    if !shape.is_file()||shape.uid()!=owner||shape.nlink()!=1||shape.mode()&0o7777!=0o600||shape.len()==0||shape.len()>4096{return Err(invalid());}
    let raw=bounded_read(&mut file,4096)?;let intent:Intent=serde_json::from_slice(&raw).map_err(|_|invalid())?;
    if intent.schema_version!=1||intent.profile.is_nil()||intent.session.is_nil()||intent.boot.is_nil()||!identifier(&intent.runtime)||intent.reserved_bytes==0||intent.reserved_bytes>8*1024*1024*1024||!(1..=144).contains(&intent.files)||serde_json::to_vec(&intent).map_err(|_|invalid())?!=raw{return Err(invalid());}
    Ok((intent,digest(&raw)))
}
// Exact flat broker-owned shape only. No recursive removal and no native input,
// user save, foreign platform journal or arbitrary path is accepted here.
fn remove_local(profile:&Path,session:Uuid)->Result<()>{
    let parent=root(profile)?;let path=parent.join(session.to_string());if !present(&path)?{return Ok(());}
    let directory=private_directory(&path)?;let original=directory.metadata().map_err(storage_error)?;
    let mut selected=None;
    for entry in fs::read_dir(&path).map_err(storage_error)?{
        let entry=entry.map_err(storage_error)?;if selected.is_some()||entry.file_name().to_str()!=Some("mac-work.json"){return Err(invalid());}
        let file=OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW|libc::O_CLOEXEC).open(entry.path()).map_err(storage_error)?;let shape=file.metadata().map_err(storage_error)?;
        if !shape.is_file()||shape.uid()!=original.uid()||shape.nlink()!=1||shape.mode()&0o7777!=0o600||shape.len()>4096{return Err(invalid());}selected=Some((entry.path(),file,shape));
    }
    let current=fs::symlink_metadata(&path).map_err(storage_error)?;
    if !current.is_dir()||current.dev()!=original.dev()||current.ino()!=original.ino(){return Err(invalid());}
    if let Some((file_path,_held,expected))=selected{
        let current=fs::symlink_metadata(&file_path).map_err(storage_error)?;
        if !current.is_file()||current.dev()!=expected.dev()||current.ino()!=expected.ino(){return Err(invalid());}
        fs::remove_file(file_path).map_err(storage_error)?;
    }
    directory.sync_all().map_err(storage_error)?;fs::remove_dir(&path).map_err(storage_error)?;
    private_directory(&parent)?.sync_all().map_err(storage_error)
}
impl MacWork{
    pub fn create(connection:MacConnection,profile_root:&Path,runtime:&str,bytes:u64,files:u32)->Result<Self>{
        let profile=Uuid::parse_str(connection.profile()).map_err(|_|invalid())?;
        if profile.is_nil()||!identifier(runtime){return Err(invalid());}
        let profile_root=profile_root.canonicalize().map_err(storage_error)?;let parent=root(&profile_root)?;
        let boot=Uuid::parse_str(&kernel_boot_session()?).map_err(|_|invalid())?;let session=Uuid::new_v4();
        let intent=Intent{schema_version:1,profile,session,runtime:runtime.to_owned(),boot,reserved_bytes:bytes,files};
        let raw=serde_json::to_vec(&intent).map_err(|_|invalid())?;if raw.len()>4096{return Err(invalid());}
        let mut database=Database::open(&profile_root)?;storage::reserve_macos(&mut database,&intent)?;
        let directory=parent.join(session.to_string());
        if let Err(error)=create_directory(&directory){let _=storage::release_uncreated(&database,profile,session);return Err(error);}
        let result=Self{connection,profile_root,intent,hash:digest(&raw),launch_attempted:false,closed:false};
        private_directory(&parent)?.sync_all().map_err(storage_error)?;
        let mut file=OpenOptions::new().write(true).create_new(true).mode(0o600).custom_flags(libc::O_CLOEXEC|libc::O_NOFOLLOW).open(directory.join("mac-work.json")).map_err(storage_error)?;
        file.write_all(&raw).and_then(|_|file.sync_all()).map_err(storage_error)?;
        private_directory(&directory)?.sync_all().map_err(storage_error)?;Ok(result)
    }
    pub fn session(&self)->String{self.intent.session.to_string()}
    pub fn staged(&self)->Result<()>{let db=Database::open(&self.profile_root)?;storage::require_reserved(&db,self.intent.profile,self.intent.session,self.intent.reserved_bytes,self.intent.files)?;storage::staged(&db,self.intent.profile,self.intent.session,&self.hash)}
    pub fn launch_intent(&mut self)->Result<()>{
        if self.closed||self.launch_attempted||!self.connection.execution_available()||kernel_boot_session()?!=self.intent.boot.to_string(){return Err(invalid());}
        let db=Database::open(&self.profile_root)?;
        let (observed,hash)=read_intent(&self.profile_root.join("native-work").join(self.session()))?;
        if observed.session!=self.intent.session||observed.profile!=self.intent.profile||hash!=self.hash{return Err(invalid());}
        storage::require_staged(&db,self.intent.profile,self.intent.session,&self.hash,self.intent.reserved_bytes,self.intent.files)?;
        // The FULL-synchronous staged transaction and protected boot provenance
        // precede the first request which can cause an owner to spawn children.
        self.launch_attempted=true;Ok(())
    }
    pub fn retire_confirmed(&mut self)->Result<()>{
        if self.closed{return Ok(());}
        if self.launch_attempted&&!self.connection.confirmed_session_ended(&self.session())?{return Err(invalid());}
        self.connection.discard(&self.session())?;
        let db=Database::open(&self.profile_root)?;storage::retiring(&db,self.intent.profile,self.intent.session)?;
        remove_local(&self.profile_root,self.intent.session)?;storage::release_macos_removed(&db,self.intent.profile,self.intent.session)?;self.closed=true;Ok(())
    }
}
impl Drop for MacWork{fn drop(&mut self){
    if self.closed{return;}
    // A live creator knows whether it ever issued launch. After a crash, recovery
    // uses the durable phase and protected boot UUID instead of this memory bit.
    if !self.launch_attempted||self.connection.confirmed_session_ended(&self.session()).unwrap_or(false){let _=self.retire_confirmed();}
}}
pub(crate) fn recover(connection:&MacConnection,profile_root:&Path)->Result<RecoverySummary>{
    let profile=Uuid::parse_str(connection.profile()).map_err(|_|invalid())?;let database=Database::open(profile_root)?;
    let candidates=storage::recovery_candidates(&database,profile)?;let mut result=RecoverySummary::default();
    let current=kernel_boot_session()?;let cursor=storage::macos_recovery_cursor(&database)?;
    let start=cursor.and_then(|cursor|candidates.iter().position(|id|*id>cursor)).unwrap_or(0);
    for offset in 0..candidates.len().min(4){
        let session=candidates[(start+offset)%candidates.len()];result.inspected+=1;
        let recovered=(||->Result<()>{
            let row=storage::recovery_reservation(&database,profile,session)?.ok_or_else(invalid)?;
            if row.windows_creation_protocol!=0{return Err(invalid());}
            let intent=storage::macos_journal(&database,profile,session)?.ok_or_else(invalid)?;
            let hash=digest(&serde_json::to_vec(&intent).map_err(|_|invalid())?);
            if row.phase==RecoveryPhase::Retiring{
                remove_local(profile_root,session)?;return storage::release_macos_removed(&database,profile,session);
            }
            if row.phase==RecoveryPhase::Reserved{
                // No launch path is allowed before the durable staged phase.
                connection.discard(&session.to_string())?;
            }else{
                if intent.profile!=profile||intent.session!=session||intent.boot.to_string()==current{return Err(invalid());}
                storage::require_staged(&database,profile,session,&hash,intent.reserved_bytes,intent.files)?;
                connection.recover_previous_boot(&session.to_string(),&intent.runtime,&intent.boot.to_string())?;
            }
            storage::retiring(&database,profile,session)?;remove_local(profile_root,session)?;
            storage::release_macos_removed(&database,profile,session)
        })();
        if recovered.is_ok(){result.removed+=1;}else{result.retained+=1;}
        storage::set_macos_recovery_cursor(&database,session)?;
    }
    result.remaining=storage::status(&database,profile)?.works;Ok(result)
}
