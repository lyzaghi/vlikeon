use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_core::util::{PendingFile,bounded_read,storage_error};
use serde::{Serialize,Deserialize};
use std::{fs::{self,File,OpenOptions},path::{Path,PathBuf}};
use uuid::Uuid;

#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Ownership{schema_version:u32,profile_id:Uuid,work_id:Uuid,container_name:String}
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Worker{schema_version:u32,work_id:Uuid,process_id:u32,creation_time:u64}
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Retirement{schema_version:u32,profile_id:Uuid,work_id:Uuid,retirement_confirmed:bool}
pub fn container_name(work_id:Uuid)->String{format!("OmniPlay.Session.{}",work_id.simple())}
fn record_bytes(work:&Path,name:&str,bytes:&[u8])->Result<()>{if bytes.len()>4096{return Err(storage_error("work record exceeds its bound"));}let mut pending=PendingFile::create(work)?;pending.write_all(bytes)?;pending.commit_new(&work.join(name))}
fn record(work:&Path,name:&str,value:&impl Serialize)->Result<()>{record_bytes(work,name,&serde_json::to_vec(value).map_err(storage_error)?)}
pub fn staging_ready(work:&Path,value:&impl Serialize)->Result<omniplay_contracts::ContentHash>{let bytes=serde_json::to_vec(value).map_err(storage_error)?;record_bytes(work,"staging-ready.json",&bytes)?;Ok(omniplay_core::util::digest(&bytes))}
pub fn creation_intent(work:&Path,profile:Uuid,id:Uuid)->Result<()>{record(work,"container-intent.json",&Ownership{schema_version:1,profile_id:profile,work_id:id,container_name:container_name(id)})}
pub fn created(work:&Path,profile:Uuid,id:Uuid)->Result<()>{record(work,"container-owned.json",&Ownership{schema_version:1,profile_id:profile,work_id:id,container_name:container_name(id)})}
pub fn launch_intent(work:&Path,id:Uuid)->Result<()>{record(work,"launch-intent.json",&serde_json::json!({"schema_version":1,"work_id":id}))}
pub fn process(work:&Path,id:Uuid,observed:omniplay_sandbox::ProcessIdentity)->Result<()>{record(work,"container-process.json",&Worker{schema_version:1,work_id:id,process_id:observed.process_id,creation_time:observed.creation_time})}
pub fn presentation_launch_intent(work:&Path,id:Uuid)->Result<()>{record(work,"presentation-launch-intent.json",&serde_json::json!({"schema_version":1,"work_id":id}))}
pub fn presentation_process(work:&Path,id:Uuid,observed:omniplay_sandbox::ProcessIdentity)->Result<()>{record(work,"presentation-process.json",&Worker{schema_version:1,work_id:id,process_id:observed.process_id,creation_time:observed.creation_time})}
fn ordinary(path:&Path,directory:bool)->Result<bool>{
    let value=match fs::symlink_metadata(path){Ok(value)=>value,Err(error)if error.kind()==std::io::ErrorKind::NotFound=>return Ok(false),Err(error)=>return Err(storage_error(error))};
    let mut linked=value.file_type().is_symlink();#[cfg(windows)]{use std::os::windows::fs::MetadataExt;linked|=value.file_attributes()&0x400!=0;}
    if linked||directory!=value.is_dir()||!directory&&!value.is_file(){return Err(storage_error("work entry is not an ordinary owned file or directory"));}Ok(true)
}
fn read<T:for<'a>Deserialize<'a>>(path:&Path)->Result<T>{if !ordinary(path,false)?{return Err(storage_error("work record is absent"));}let mut file=File::open(path).map_err(storage_error)?;let bytes=bounded_read(&mut file,4096)?;serde_json::from_slice(&bytes).map_err(storage_error)}
fn confined(profile:&Path,work:&Path)->Result<(PathBuf,Uuid)>{
    if !ordinary(profile,true)?{return Err(storage_error("profile is absent"));}let profile=profile.canonicalize().map_err(storage_error)?;let parent=profile.join("native-work");if !ordinary(&parent,true)?||!ordinary(work,true)?{return Err(storage_error("work root is absent"));}
    let parent=parent.canonicalize().map_err(storage_error)?;let resolved=work.canonicalize().map_err(storage_error)?;
    let name=work.file_name().and_then(|v|v.to_str()).ok_or_else(||storage_error("work name is invalid"))?;let id=Uuid::parse_str(name).map_err(storage_error)?;
    if id.to_string()!=name||resolved.parent()!=Some(parent.as_path()){return Err(storage_error("work root is outside the private profile"));}Ok((resolved,id))
}
pub fn remove(profile:&Path,work:&Path)->Result<()>{
    let(root,_)=confined(profile,work)?;let mut pending=vec![(root.clone(),0u8)];let mut entries=0usize;
    // The contained worker must already be gone. Refuse links before recursive
    // deletion, including Windows junction/reparse entries, and bound traversal.
    while let Some((directory,depth))=pending.pop(){if depth>32{return Err(storage_error("work cleanup depth exceeds its bound"));}
        for entry in fs::read_dir(&directory).map_err(storage_error)?{let path=entry.map_err(storage_error)?.path();entries+=1;if entries>20000{return Err(storage_error("work cleanup entries exceed its bound"));}
            let metadata=fs::symlink_metadata(&path).map_err(storage_error)?;if metadata.is_dir(){ordinary(&path,true)?;pending.push((path,depth+1));}else{ordinary(&path,false)?;}
        }
    }
    fs::remove_dir_all(root).map_err(storage_error)
}
#[cfg(windows)]
fn recovery_directory(profile:&Path)->Result<PathBuf>{
    if !ordinary(profile,true)?{return Err(storage_error("profile is absent"));}
    let profile=profile.canonicalize().map_err(storage_error)?;let root=profile.join("native-retirement");
    if !ordinary(&root,true)?{match fs::create_dir(&root){Ok(())=>{},Err(error)if error.kind()==std::io::ErrorKind::AlreadyExists=>{},Err(error)=>return Err(storage_error(error))}}
    if !ordinary(&root,true)?{return Err(storage_error("native recovery directory is absent"));}Ok(root)
}
#[cfg(windows)]
fn recovery_lease(profile:&Path)->Result<File>{
    use std::os::windows::fs::OpenOptionsExt;
    let root=recovery_directory(profile)?;let path=root.join("collector.lease");
    let _=ordinary(&path,false)?;
    OpenOptions::new().read(true).write(true).create(true).truncate(false).share_mode(0).open(path).map_err(storage_error)
}
/// New creators hold this until their exclusive per-work owner lease exists.
/// Collectors take it before admitting a missing/partly-created work. Sharing
/// denial is immediate; never wait on the native control thread for cleanup.
#[cfg(windows)]
pub(crate) fn creation_guard(profile:&Path)->Result<File>{
    use std::os::windows::fs::OpenOptionsExt;
    let path=recovery_directory(profile)?.join("creator.lease");let _=ordinary(&path,false)?;
    OpenOptions::new().read(true).write(true).create(true).truncate(false).share_mode(0).open(path).map_err(|error|{
        if matches!(error.raw_os_error(),Some(32)|Some(33)){
            PlatformError::new(ErrorCode::Busy,"Native session creation or recovery is using its creation lease").recover("Retry after the current native session operation finishes")
        }else{storage_error(error)}
    })
}
#[cfg(windows)]
fn retirement_path(profile:&Path,id:Uuid)->Result<PathBuf>{if id.is_nil(){return Err(storage_error("native recovery identity is invalid"));}Ok(recovery_directory(profile)?.join(format!("{id}.json")))}
#[cfg(windows)]
fn resume_retirement(profile:&Path,profile_id:Uuid,id:Uuid)->Result<()>{
    let proof=retirement_path(profile,id)?;let observed:Retirement=read(&proof)?;
    if observed.schema_version!=1||observed.profile_id!=profile_id||observed.work_id!=id||!observed.retirement_confirmed{return Err(storage_error("native retirement proof is inconsistent"));}
    let database=omniplay_core::Database::open(profile)?;let parent=database.root.join("native-work");let _=ordinary(&parent,true)?;let work=parent.join(id.to_string());
    if ordinary(&work,true)?{
        // A proof never permits deletion of a reserved/staged or reused UUID.
        // Only a debt already transitioned by the confirmed process owner can
        // continue after partial deletion removed its internal owner.lease.
        omniplay_core::native_work_storage::require_retiring(&database,profile_id,id)?;
        remove(profile,&work)?;
    }
    omniplay_core::native_work_storage::release_removed(&database,profile_id,id)?;
    fs::remove_file(proof).map_err(storage_error)
}
fn reclaim(profile:&Path,profile_id:Uuid,work:&Path)->Result<()>{
    #[cfg(windows)]let _creator=creation_guard(profile)?;
    #[cfg(windows)]{
        let name=work.file_name().and_then(|value|value.to_str()).ok_or_else(||storage_error("native recovery name is invalid"))?;
        let id=Uuid::parse_str(name).map_err(storage_error)?;
        if id.is_nil()||id.to_string()!=name{return Err(storage_error("native recovery name is invalid"));}
        let expected=profile.join("native-work").join(id.to_string());
        if work!=expected.as_path(){return Err(storage_error("native recovery work is outside its profile"));}
        let _=ordinary(&profile.join("native-work"),true)?;
        if ordinary(&retirement_path(profile,id)?,false)?{return resume_retirement(profile,profile_id,id);}
        let database=omniplay_core::Database::open(profile)?;
        let reservation=omniplay_core::native_work_storage::windows_recovery_reservation(&database,profile_id,id)?;
        if !ordinary(work,true)?{
            use omniplay_core::native_work_storage::RecoveryPhase;
            return match reservation{
                Some(row)if row.phase==RecoveryPhase::Reserved&&row.windows_creation_protocol==1=>omniplay_core::native_work_storage::release_uncreated(&database,profile_id,id),
                Some(row)if row.phase==RecoveryPhase::Retiring=>omniplay_core::native_work_storage::release_removed(&database,profile_id,id),
                _=>Err(storage_error("missing native work has no confirmed pre-launch or retirement authority")),
            };
        }
    }
    let(root,id)=confined(profile,work)?;
    if root.join("linux-work-owned.json").try_exists().map_err(storage_error)?||root.join("linux-work-intent.json").try_exists().map_err(storage_error)?{return Err(PlatformError::new(ErrorCode::UnsupportedCapability,"Linux native work requires its own confirmed process and resource recovery"));}
    #[cfg(windows)]let lease={
        use std::os::windows::fs::OpenOptionsExt;
        if ordinary(&root.join("owner.lease"),false)?{Some(OpenOptions::new().read(true).write(true).share_mode(0).open(root.join("owner.lease")).map_err(storage_error)?)}
        else{
            let database=omniplay_core::Database::open(profile)?;
            let reservation=omniplay_core::native_work_storage::windows_recovery_reservation(&database,profile_id,id)?;
            if !reservation.is_some_and(|row|row.phase==omniplay_core::native_work_storage::RecoveryPhase::Reserved&&row.windows_creation_protocol==1){return Err(storage_error("owner lease is absent without gated pre-launch authority"));}
            // A protocol-1 reserved work cannot have launched. Still reject any
            // conflicting OS journal instead of upgrading a damaged/legacy tree.
            for name in["container-intent.json","container-owned.json","launch-intent.json","container-process.json","presentation-launch-intent.json","presentation-process.json"]{
                if root.join(name).try_exists().map_err(storage_error)?{return Err(storage_error("pre-launch recovery conflicts with a container or process journal"));}
            }
            None
        }
    };
    // The creation handoff is now closed: either this collector owns the
    // per-work lease, or protocol-1 reserved authority proved no owner existed.
    // Do not hold the profile creation slot during potentially slow removal.
    #[cfg(windows)]drop(_creator);
    #[cfg(not(windows))]return Err(PlatformError::new(ErrorCode::UnsupportedCapability,"Native work reclamation is not implemented on this platform"));
    #[cfg(windows)]{
        let ownership=root.join("container-owned.json");
        if ownership.try_exists().map_err(storage_error)?{
            let owner:Ownership=read(&ownership)?;if owner.schema_version!=1||owner.profile_id!=profile_id||owner.work_id!=id||owner.container_name!=container_name(id){return Err(storage_error("work ownership is inconsistent"));}
            let presentation_path=root.join("presentation-process.json");
            if presentation_path.try_exists().map_err(storage_error)?{
                let observed:Worker=read(&presentation_path)?;if observed.schema_version!=1||observed.work_id!=id||observed.process_id==0||observed.creation_time==0{return Err(storage_error("presentation identity is invalid"));}
                omniplay_sandbox::confirmed_process_ended(omniplay_sandbox::ProcessIdentity{process_id:observed.process_id,creation_time:observed.creation_time})?;
            }else if root.join("presentation-launch-intent.json").try_exists().map_err(storage_error)?{return Err(storage_error("interrupted presentation launch has no confirmed process identity"));}
            let process_path=root.join("container-process.json");
            let observed=if process_path.try_exists().map_err(storage_error)?{let observed:Worker=read(&process_path)?;if observed.schema_version!=1||observed.work_id!=id||observed.process_id==0||observed.creation_time==0{return Err(storage_error("worker identity is invalid"));}Some(omniplay_sandbox::ProcessIdentity{process_id:observed.process_id,creation_time:observed.creation_time})}
            else{if root.join("launch-intent.json").try_exists().map_err(storage_error)?{return Err(storage_error("interrupted launch has no confirmed process identity"));}None};
            // Never kill a PID from disk. The OS query must establish that the
            // recorded process ended or that its PID belongs to a later process.
            omniplay_sandbox::retire_abandoned_profile(&owner.container_name,observed)?;
        }else if root.join("container-intent.json").try_exists().map_err(storage_error)?{
            return Err(storage_error("interrupted profile creation has no ownership acknowledgement"));
        }else{
            for name in["launch-intent.json","container-process.json","presentation-launch-intent.json","presentation-process.json"]{if root.join(name).try_exists().map_err(storage_error)?{return Err(storage_error("interrupted native work has no complete container ownership chain"));}}
        }
        // Publish the confirmed retirement outside the tree being removed. A
        // failed recursive removal may erase the internal lease/journal first;
        // the external proof and retiring ledger retain retry authority.
        let database=omniplay_core::Database::open(profile)?;omniplay_core::native_work_storage::retiring(&database,profile_id,id)?;
        omniplay_core::native_work_storage::require_retiring(&database,profile_id,id)?;
        let directory=recovery_directory(profile)?;
        record(&directory,&format!("{id}.json"),&Retirement{schema_version:1,profile_id,work_id:id,retirement_confirmed:true})?;
        drop(lease);resume_retirement(profile,profile_id,id)
    }
}
pub fn collect_abandoned(profile:&Path,profile_id:Uuid){
    #[cfg(windows)]{let _=recover_windows_work(profile,profile_id,&std::sync::atomic::AtomicBool::new(false),|_|{});}
    #[cfg(not(windows))]{let _=(profile,profile_id);}
}
#[cfg(windows)]
pub fn recover_windows_work(profile:&Path,profile_id:Uuid,cancel:&std::sync::atomic::AtomicBool,mut progress:impl FnMut(&omniplay_contracts::NativeWorkRecoverySummary))->Result<omniplay_contracts::NativeWorkRecoverySummary>{
    use std::sync::atomic::Ordering;
    let mut summary=omniplay_contracts::NativeWorkRecoverySummary::default();
    if profile_id.is_nil(){return Err(storage_error("native recovery profile is invalid"));}
    if cancel.load(Ordering::Acquire){summary.cancelled=true;return Ok(summary);}
    let _collector=recovery_lease(profile)?;let database=omniplay_core::Database::open(profile)?;
    let mut ids=omniplay_core::native_work_storage::windows_recovery_candidates(&database,profile_id)?.into_iter().collect::<std::collections::BTreeSet<_>>();let mut count=0usize;
    let work_root=profile.join("native-work");
    if ordinary(&work_root,true)?{for entry in fs::read_dir(&work_root).map_err(storage_error)?{
        if cancel.load(Ordering::Acquire){summary.cancelled=true;progress(&summary);return Ok(summary);}
        count+=1;if count>256{return Err(storage_error("native work inventory exceeds its bound"));}
        let entry=entry.map_err(storage_error)?;let Some(name)=entry.file_name().to_str().map(str::to_owned)else{summary.unknown_entries+=1;continue};
        match Uuid::parse_str(&name){Ok(id)if !id.is_nil()&&id.to_string()==name=>{ids.insert(id);},_=>summary.unknown_entries+=1}
    }}
    count=0;for entry in fs::read_dir(recovery_directory(profile)?).map_err(storage_error)?{
        if cancel.load(Ordering::Acquire){summary.cancelled=true;progress(&summary);return Ok(summary);}
        count+=1;if count>256{return Err(storage_error("native retirement inventory exceeds its bound"));}
        let entry=entry.map_err(storage_error)?;let Some(name)=entry.file_name().to_str().map(str::to_owned)else{summary.unknown_entries+=1;continue};
        if name=="collector.lease"||name=="creator.lease"{continue;}
        if let Some(raw)=name.strip_suffix(".json"){match Uuid::parse_str(raw){Ok(id)if !id.is_nil()&&id.to_string()==raw=>{ids.insert(id);},_=>summary.unknown_entries+=1}}
        else{summary.unknown_entries+=1;}
    }
    let cursor=omniplay_core::native_work_storage::windows_recovery_cursor(&database)?.and_then(|value|Uuid::parse_str(&value).ok());
    let ordered=ids.into_iter().collect::<Vec<_>>();let split=cursor.map(|cursor|ordered.partition_point(|id|*id<=cursor)).unwrap_or(0);
    let started=std::time::Instant::now();
    for id in ordered[split..].iter().chain(ordered[..split].iter()).take(32){
        if cancel.load(Ordering::Acquire)||started.elapsed()>=std::time::Duration::from_secs(30){break;}
        // Advance even when one work is live, unknown or malformed so it cannot
        // indefinitely starve a later acknowledged retirement in this profile.
        summary.attempted+=1;
        match reclaim(profile,profile_id,&work_root.join(id.to_string())){Ok(())=>summary.recovered+=1,Err(_)=>summary.retained+=1}
        omniplay_core::native_work_storage::set_windows_recovery_cursor(&database,*id)?;
        summary.more_available=(summary.attempted as usize)<ordered.len();progress(&summary);
    }
    summary.cancelled=cancel.load(Ordering::Acquire);summary.more_available=(summary.attempted as usize)<ordered.len();progress(&summary);Ok(summary)
}
pub fn collect_one(profile:&Path,profile_id:Uuid,work:&Path)->Result<()>{
    #[cfg(windows)]let _collector=recovery_lease(profile)?;
    reclaim(profile,profile_id,work)
}

#[cfg(all(test,windows))]
#[path="work_windows_tests.rs"]
mod windows_tests;
