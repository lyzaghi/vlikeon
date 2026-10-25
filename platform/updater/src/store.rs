use crate::{channel::Channel,package::{self,PackageManifest},rejected};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_core::{now_ms,updates::{ReleaseConstraints,ReleaseManifest,SignedEnvelope,TrustedRoot},util::{bounded_read,digest,storage_error,sync_directory}};
use rusqlite::{params,Connection,OptionalExtension};
use serde::{Deserialize,Serialize};
use std::{fs::File,path::{Path,PathBuf}};
use uuid::Uuid;

const SCHEMA:&str=r#"
BEGIN IMMEDIATE;
CREATE TABLE IF NOT EXISTS security(id INTEGER PRIMARY KEY CHECK(id=1),highest_metadata INTEGER NOT NULL DEFAULT 0,minimum_epoch INTEGER NOT NULL DEFAULT 0,latest_clock_ms INTEGER NOT NULL DEFAULT 0,data_schema_floor INTEGER NOT NULL DEFAULT __PROFILE_SCHEMA__);
INSERT OR IGNORE INTO security(id) VALUES(1);
CREATE TABLE IF NOT EXISTS observed(version INTEGER PRIMARY KEY,digest TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS roots(version INTEGER PRIMARY KEY,old_signed TEXT NOT NULL,new_signed TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS releases(id TEXT PRIMARY KEY,version TEXT NOT NULL,metadata_version INTEGER UNIQUE NOT NULL,epoch INTEGER NOT NULL,status TEXT NOT NULL CHECK(status IN ('staged','active','previous','rejected')),manifest TEXT NOT NULL,envelope TEXT NOT NULL,created_at_ms INTEGER NOT NULL,attempts INTEGER NOT NULL DEFAULT 0,healthy INTEGER NOT NULL DEFAULT 0);
CREATE UNIQUE INDEX IF NOT EXISTS one_active ON releases(status) WHERE status='active';
PRAGMA user_version=2;
COMMIT;
"#;

#[derive(Clone,Serialize,Deserialize)]
pub struct InstalledRelease {pub id:Uuid,pub version:semver::Version,pub metadata_version:u64,pub security_epoch:u64,pub status:String,pub attempts:u32,pub healthy:bool,pub manifest:PackageManifest}

#[derive(Serialize)]
pub struct UpdateStatus {pub channel:String,pub platform:String,pub configured:bool,pub highest_metadata_version:u64,pub minimum_security_epoch:u64,pub releases:Vec<InstalledRelease>}

pub struct Store {pub root:PathBuf,connection:Connection,pub channel:Channel}
pub struct UpdateLock {connection:Connection}
impl Drop for UpdateLock {fn drop(&mut self){let _=self.connection.execute_batch("ROLLBACK");}}

impl Store {
    pub fn open(root:&Path,channel:Channel)->Result<Self> {
        std::fs::create_dir_all(root).map_err(storage_error)?;
        if std::fs::symlink_metadata(root).map_err(storage_error)?.file_type().is_symlink(){return Err(rejected("Installation root must not be a symbolic link"));}
        #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(root,std::fs::Permissions::from_mode(0o700)).map_err(storage_error)?;}
        let root=root.canonicalize().map_err(storage_error)?;
        let connection=Connection::open(root.join("install.sqlite3")).map_err(storage_error)?;
        connection.busy_timeout(std::time::Duration::from_secs(5)).map_err(storage_error)?;
        connection.execute_batch("PRAGMA journal_mode=WAL;PRAGMA synchronous=FULL;PRAGMA trusted_schema=OFF;").map_err(storage_error)?;
        let version:u32=connection.query_row("PRAGMA user_version",[],|r|r.get(0)).map_err(storage_error)?;
        if version>2{return Err(rejected("Installation database requires a newer updater"));}
        if version==0{connection.execute_batch(&SCHEMA.replace("__PROFILE_SCHEMA__",&omniplay_core::database::CURRENT_SCHEMA_VERSION.to_string())).map_err(storage_error)?;}
        if version==1{connection.execute_batch("BEGIN IMMEDIATE;ALTER TABLE security ADD COLUMN data_schema_floor INTEGER NOT NULL DEFAULT 3;PRAGMA user_version=2;COMMIT;").map_err(storage_error)?;}
        Ok(Self{root,connection,channel})
    }

    pub fn lock(&self)->Result<UpdateLock> {
        // A separate SQLite lock does not block status/launch readers of the WAL database.
        // The OS releases it on a crash; there is no stale PID file or expiring lease race.
        let connection=Connection::open(self.root.join("operation-lock.sqlite3")).map_err(storage_error)?;
        connection.busy_timeout(std::time::Duration::from_millis(25)).map_err(storage_error)?;
        connection.execute_batch("CREATE TABLE IF NOT EXISTS operation(id INTEGER PRIMARY KEY);BEGIN EXCLUSIVE;").map_err(|_|PlatformError::new(ErrorCode::Busy,"Another installer or updater is active"))?;
        Ok(UpdateLock{connection})
    }

    pub fn trusted_root(&self)->Result<TrustedRoot> {
        let mut root=self.channel.require_enabled()?.clone();
        let mut query=self.connection.prepare("SELECT old_signed,new_signed FROM roots WHERE version>?1 ORDER BY version").map_err(storage_error)?;
        let rows=query.query_map([root.version],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(storage_error)?;
        // Stored rotations remain checkable after expiry; freshness is checked only at
        // the end of the chain when checking a new release. Installed versions run offline.
        for row in rows {let(old,new)=row.map_err(storage_error)?;let old:SignedEnvelope=serde_json::from_str(&old).map_err(|_|rejected("Stored root rotation is invalid"))?;let new:SignedEnvelope=serde_json::from_str(&new).map_err(|_|rejected("Stored root rotation is invalid"))?;root=root.rotate_historical(&old,&new)?;}
        Ok(root)
    }

    pub fn rotate_root(&mut self,old:&SignedEnvelope,new:&SignedEnvelope)->Result<u64> {
        let _lock=self.lock()?;
        let current=self.trusted_root()?;let next=current.rotate_historical(old,new)?;
        // Permit intermediate expired roots to recover an old installation. No release
        // can be downloaded until the final current root passes its expiry check.
        self.connection.execute("INSERT INTO roots(version,old_signed,new_signed) VALUES(?1,?2,?3)",params![next.version,serde_json::to_string(old).map_err(storage_error)?,serde_json::to_string(new).map_err(storage_error)?]).map_err(storage_error)?;
        Ok(next.version)
    }

    pub fn refresh_roots(&mut self,cancel:&std::sync::atomic::AtomicBool)->Result<()> {
        let Some(template)=self.channel.root_url_template.clone()else{return self.trusted_root()?.validate(now_ms());};
        for _ in 0..32 {
            if cancel.load(std::sync::atomic::Ordering::Relaxed){return Err(PlatformError::new(ErrorCode::Cancelled,"Update download was cancelled"));}
            let root=self.trusted_root()?;let version=root.version.checked_add(1).ok_or_else(||rejected("Root version overflow"))?;
            let url=template.replace("{version}",&version.to_string());
            let Some(rotation)=crate::transport::root_rotation(&url,&root)?else{return root.validate(now_ms());};
            self.rotate_root(&rotation.old_signed,&rotation.new_signed)?;
        }
        Err(rejected("Too many root rotations in one update; retry to continue recovery"))
    }

    pub fn status(&self)->Result<UpdateStatus> {
        let(highest,epoch):(u64,u64)=self.connection.query_row("SELECT highest_metadata,minimum_epoch FROM security WHERE id=1",[],|r|Ok((r.get(0)?,r.get(1)?))).map_err(storage_error)?;
        Ok(UpdateStatus{channel:self.channel.channel.clone(),platform:self.channel.platform.clone(),configured:self.channel.enabled,highest_metadata_version:highest,minimum_security_epoch:epoch,releases:self.releases()?})
    }

    pub fn releases(&self)->Result<Vec<InstalledRelease>> {
        let mut query=self.connection.prepare("SELECT id,version,metadata_version,epoch,status,attempts,healthy,manifest FROM releases ORDER BY metadata_version DESC LIMIT 100").map_err(storage_error)?;
        let rows=query.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get::<_,String>(7)?))).map_err(storage_error)?;
        rows.map(|row|{let(id,version,metadata_version,security_epoch,status,attempts,healthy,manifest)=row.map_err(storage_error)?;Ok(InstalledRelease{id:Uuid::parse_str(&id).map_err(|_|rejected("Installed release identity is corrupt"))?,version:semver::Version::parse(&version).map_err(|_|rejected("Installed version is corrupt"))?,metadata_version,security_epoch,status,attempts,healthy,manifest:serde_json::from_str(&manifest).map_err(|_|rejected("Installed inventory is corrupt"))?})}).collect()
    }

    pub fn active(&self)->Result<InstalledRelease> {self.releases()?.into_iter().find(|r|r.status=="active").ok_or_else(||PlatformError::new(ErrorCode::NotFound,"No active OmniPlay release is installed"))}

    fn verify_observe(&mut self,envelope:&SignedEnvelope)->Result<ReleaseManifest> {
        let root=self.trusted_root()?;let now=now_ms();let(highest,epoch,clock,schema):(u64,u64,u64,u32)=self.connection.query_row("SELECT highest_metadata,minimum_epoch,latest_clock_ms,data_schema_floor FROM security WHERE id=1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).map_err(storage_error)?;
        if now.saturating_add(300_000)<clock{return Err(rejected("System time moved backwards; correct the clock before updating"));}
        let raw=root.verify_payload(envelope,now)?;let hash=digest(&raw);
        let metadata:ReleaseManifest=serde_json::from_slice(&raw).map_err(|_|rejected("Signed release metadata is invalid"))?;
        let previous:Option<String>=self.connection.query_row("SELECT digest FROM observed WHERE version=?1",[metadata.metadata_version],|r|r.get(0)).optional().map_err(storage_error)?;
        let known=previous.as_deref()==Some(hash.0.as_str());
        if previous.is_some()&&!known{return Err(rejected("The same metadata version has conflicting signed payloads"));}
        let active=self.releases()?.into_iter().find(|r|r.status=="active");
        let current=active.map(|r|r.version).unwrap_or_else(||semver::Version::new(0,0,0));
        let policy=ReleaseConstraints{channel:&self.channel.channel,platform:&self.channel.platform,current_version:&current,minimum_security_epoch:epoch,highest_metadata_version:if known&&metadata.metadata_version==highest{highest.saturating_sub(1)}else{highest},data_schema:schema,now_ms:now};
        let release=root.verify_release(envelope,&policy)?;
        let tx=self.connection.transaction().map_err(storage_error)?;
        tx.execute("INSERT OR IGNORE INTO observed(version,digest) VALUES(?1,?2)",params![release.metadata_version,hash.0]).map_err(storage_error)?;
        tx.execute("UPDATE security SET highest_metadata=MAX(highest_metadata,?1),latest_clock_ms=MAX(latest_clock_ms,?2) WHERE id=1",params![release.metadata_version,now]).map_err(storage_error)?;
        tx.commit().map_err(storage_error)?;Ok(release)
    }

    pub fn stage_download(&mut self,envelope:&SignedEnvelope)->Result<InstalledRelease> {
        self.stage_download_cancellable(envelope,&std::sync::atomic::AtomicBool::new(false))
    }

    pub fn stage_download_cancellable(&mut self,envelope:&SignedEnvelope,cancel:&std::sync::atomic::AtomicBool)->Result<InstalledRelease> {
        let _lock=self.lock()?;let release=self.verify_observe(envelope)?;
        if let Some(existing)=self.releases()?.into_iter().find(|r|r.metadata_version==release.metadata_version){return Ok(existing);}
        let root=self.trusted_root()?;let pending=crate::transport::download_cancellable(&self.root.join("downloads"),&root,&release,cancel)?;
        if cancel.load(std::sync::atomic::Ordering::Relaxed){return Err(PlatformError::new(ErrorCode::Cancelled,"Update download was cancelled"));}
        self.stage_verified(&release,envelope,File::open(&pending.path).map_err(storage_error)?,cancel)
    }

    pub fn stage_file(&mut self,envelope:&SignedEnvelope,source:&Path)->Result<InstalledRelease> {
        let _lock=self.lock()?;let release=self.verify_observe(envelope)?;
        if let Some(existing)=self.releases()?.into_iter().find(|r|r.metadata_version==release.metadata_version){return Ok(existing);}
        // The source may reside on a removable or mutable filesystem. Copy and hash into
        // an owned pending file once, then extract that same inode; do not hash then reopen.
        let mut file=File::open(source).map_err(storage_error)?;
        let mut pending=omniplay_core::util::PendingFile::create(&self.root.join("downloads"))?;
        crate::transport::copy_verified(&mut file,&mut pending.file,&release)?;
        self.stage_verified(&release,envelope,File::open(&pending.path).map_err(storage_error)?,&std::sync::atomic::AtomicBool::new(false))
    }

    fn stage_verified(&mut self,release:&ReleaseManifest,envelope:&SignedEnvelope,mut source:File,cancel:&std::sync::atomic::AtomicBool)->Result<InstalledRelease> {
        let id=Uuid::new_v4();let staging=self.root.join("staging").join(id.to_string());
        std::fs::create_dir_all(staging.parent().expect("generated parent")).map_err(storage_error)?;
        std::fs::create_dir(&staging).map_err(storage_error)?;
        let result=(||{
            let manifest=package::unpack_cancellable(&mut source,&staging,release,cancel)?;
            if cancel.load(std::sync::atomic::Ordering::Relaxed){return Err(PlatformError::new(ErrorCode::Cancelled,"Update staging was cancelled"));}
            let versions=self.root.join("versions");std::fs::create_dir_all(&versions).map_err(storage_error)?;
            std::fs::rename(&staging,versions.join(id.to_string())).map_err(storage_error)?;sync_directory(&versions)?;
            let value=InstalledRelease{id,version:release.version.clone(),metadata_version:release.metadata_version,security_epoch:release.security_epoch,status:"staged".into(),attempts:0,healthy:false,manifest};
            self.connection.execute("INSERT INTO releases(id,version,metadata_version,epoch,status,manifest,envelope,created_at_ms) VALUES(?1,?2,?3,?4,'staged',?5,?6,?7)",params![id.to_string(),release.version.to_string(),release.metadata_version,release.security_epoch,serde_json::to_string(&value.manifest).map_err(storage_error)?,serde_json::to_string(envelope).map_err(storage_error)?,now_ms()]).map_err(storage_error)?;
            Ok(value)
        })();
        // Keep failed staging evidence for explicit maintenance, never recursively delete
        // an untrusted or computed destination during normal update activation.
        result
    }

    pub fn activate(&mut self,id:Uuid)->Result<InstalledRelease> {
        let _lock=self.lock()?;
        let mut candidate=self.releases()?.into_iter().find(|r|r.id==id&&r.status=="staged").ok_or_else(||rejected("Only a staged release can be activated"))?;
        let epoch:u64=self.connection.query_row("SELECT minimum_epoch FROM security WHERE id=1",[],|r|r.get(0)).map_err(storage_error)?;
        if candidate.security_epoch<epoch{return Err(rejected("Staged release is below the security floor"));}
        // Inspect every file again after staging: no package may be activated after it
        // was altered on disk. Browser launch checks entry points again as well.
        self.verify_installed(&candidate,true)?;
        let tx=self.connection.transaction().map_err(storage_error)?;
        tx.execute("UPDATE releases SET status='previous' WHERE status='active'",[]).map_err(storage_error)?;
        tx.execute("UPDATE releases SET status='active',attempts=0,healthy=0 WHERE id=?1",[id.to_string()]).map_err(storage_error)?;
        tx.execute("UPDATE security SET minimum_epoch=MAX(minimum_epoch,?1) WHERE id=1",[candidate.security_epoch]).map_err(storage_error)?;
        tx.commit().map_err(storage_error)?;candidate.status="active".into();Ok(candidate)
    }

    pub fn verify_installed(&self,release:&InstalledRelease,all:bool)->Result<()> {
        let schema:u32=self.connection.query_row("SELECT data_schema_floor FROM security WHERE id=1",[],|r|r.get(0)).map_err(storage_error)?;
        if schema<release.manifest.minimum_data_schema||schema>release.manifest.maximum_data_schema{return Err(rejected("The installed release cannot open the current profile schema"));}
        let directory=self.version_directory(release.id);
        package::verify_entry(&directory,&release.manifest,&release.manifest.browser_entry)?;
        package::verify_entry(&directory,&release.manifest,&release.manifest.broker_entry)?;
        if all {for entry in &release.manifest.files {let resolved=package::installed_file(&directory,&entry.path)?;let(hash,bytes)=omniplay_core::util::hash_file(&mut File::open(resolved).map_err(storage_error)?,entry.bytes)?;if hash!=entry.sha256||bytes!=entry.bytes{return Err(rejected("Installed file checksum failed"));}}}
        Ok(())
    }

    pub fn verify_release_files(&self,id:Uuid)->Result<InstalledRelease>{
        let _lock=self.lock()?;
        let release=self.releases()?.into_iter().find(|value|value.id==id).ok_or_else(||rejected("Installed release was not found"))?;
        self.verify_installed(&release,true)?;Ok(release)
    }

    pub fn note_launch(&self,id:Uuid)->Result<()> {self.connection.execute("UPDATE releases SET attempts=attempts+1 WHERE id=?1 AND status='active' AND healthy=0",[id.to_string()]).map_err(storage_error)?;Ok(())}
    pub fn note_opened_schema(&self,schema:u32)->Result<()> {if schema==0||schema>omniplay_core::database::CURRENT_SCHEMA_VERSION{return Err(rejected("The opened profile schema is invalid"));}self.connection.execute("UPDATE security SET data_schema_floor=MAX(data_schema_floor,?1) WHERE id=1",[schema]).map_err(storage_error)?;Ok(())}
    pub fn acknowledge_health(&self,id:Uuid)->Result<()> {
        let tx=self.connection.unchecked_transaction().map_err(storage_error)?;
        let count=tx.execute("UPDATE releases SET healthy=1 WHERE id=?1 AND status='active'",[id.to_string()]).map_err(storage_error)?;
        if count!=1{return Err(rejected("Health acknowledgement is not for the active release"));}
        tx.execute("UPDATE security SET data_schema_floor=MAX(data_schema_floor,?1) WHERE id=1",[omniplay_core::database::CURRENT_SCHEMA_VERSION]).map_err(storage_error)?;
        tx.commit().map_err(storage_error)?;Ok(())
    }

    pub fn rollback(&mut self)->Result<InstalledRelease> {
        let _lock=self.lock()?;let active=self.active()?;
        let floor:u64=self.connection.query_row("SELECT minimum_epoch FROM security WHERE id=1",[],|r|r.get(0)).map_err(storage_error)?;
        let schema:u32=self.connection.query_row("SELECT data_schema_floor FROM security WHERE id=1",[],|r|r.get(0)).map_err(storage_error)?;
        let mut previous=self.releases()?.into_iter().find(|r|r.status=="previous"&&r.security_epoch>=floor&&r.manifest.minimum_data_schema<=schema&&r.manifest.maximum_data_schema>=schema).ok_or_else(||rejected("No previous release satisfies the security and data-schema floors; install a newer signed repair"))?;
        self.verify_installed(&previous,true)?;
        let tx=self.connection.transaction().map_err(storage_error)?;
        tx.execute("UPDATE releases SET status='rejected' WHERE id=?1",[active.id.to_string()]).map_err(storage_error)?;
        tx.execute("UPDATE releases SET status='active' WHERE id=?1",[previous.id.to_string()]).map_err(storage_error)?;
        tx.commit().map_err(storage_error)?;previous.status="active".into();Ok(previous)
    }

    pub fn version_directory(&self,id:Uuid)->PathBuf {self.root.join("versions").join(id.to_string())}
    pub fn envelope_from_file(path:&Path)->Result<SignedEnvelope> {let raw=bounded_read(&mut File::open(path).map_err(storage_error)?,1024*1024)?;serde_json::from_slice(&raw).map_err(|_|rejected("Release envelope is invalid"))}
    pub fn delete_release_record(&mut self,id:Uuid)->Result<()>{self.connection.execute("DELETE FROM releases WHERE id=?1",[id.to_string()]).map_err(storage_error)?;Ok(())}
}
