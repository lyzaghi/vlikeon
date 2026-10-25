use crate::rejected;
use omniplay_contracts::{ContentHash,ErrorCode,PlatformError,Result};
use omniplay_core::{updates::ReleaseManifest,util::{hash_file,storage_error,sync_directory}};
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
use std::{collections::{BTreeMap,BTreeSet},fs::{File,OpenOptions},io::{Read,Write},path::{Path,PathBuf}};

const MAGIC:&[u8;8]=b"OPPK0001";
const MAX_INVENTORY:usize=16*1024*1024;
pub const MAX_PACKAGE_BYTES:u64=2*1024*1024*1024;
const MAX_FILES:usize=100_000;
pub const LINUX_NATIVE_HOST_ENTRY:&str="libexec/omniplay-native-host";
pub const LINUX_NATIVE_HOST_SERVICE:&str="share/systemd/user/omniplay-native-host.service";
pub const LINUX_NATIVE_HOST_SOCKET:&str="share/systemd/user/omniplay-native-host.socket";
const LINUX_SERVICE_ROUTE:&str="ExecStart=@OMNIPLAY_LAUNCHER@ --linux-native-host-service";

#[derive(Clone,Debug,Deserialize,Serialize)]
#[serde(deny_unknown_fields)]
pub struct PackageFile {pub path:String,pub bytes:u64,pub sha256:ContentHash,pub executable:bool}

#[derive(Clone,Debug,Deserialize,Serialize)]
#[serde(deny_unknown_fields)]
pub struct PackageManifest {
    pub schema_version:u32,pub channel:String,pub platform:String,pub version:semver::Version,
    pub chromium_revision:String,pub minimum_data_schema:u32,pub maximum_data_schema:u32,
    pub browser_entry:String,pub broker_entry:String,pub files:Vec<PackageFile>,
}

pub fn validate_relative(value:&str)->Result<()> {
    if value.is_empty()||value.len()>240||!value.is_ascii()||value.contains('\\')||value.starts_with('/') {return Err(rejected("Package path is invalid"));}
    for part in value.split('/') {
        if part.is_empty()||part=="."||part==".."||part.ends_with('.')||part.ends_with(' ')||!part.bytes().all(|c|c.is_ascii_alphanumeric()||[b'-',b'_',b'+',b'.',b' '].contains(&c)){return Err(rejected("Package path contains an unsafe component"));}
        let stem=part.split('.').next().unwrap_or("").to_ascii_uppercase();
        if ["CON","PRN","AUX","NUL","CLOCK$"].contains(&stem.as_str())||(stem.len()==4&&(stem.starts_with("COM")||stem.starts_with("LPT"))&&stem.as_bytes()[3].is_ascii_digit()){return Err(rejected("Package path contains a reserved device name"));}
    }
    Ok(())
}

impl PackageManifest {
    pub fn validate(&self,release:&ReleaseManifest)->Result<()> {
        if self.schema_version!=1||self.channel!=release.channel||self.platform!=release.platform||self.version!=release.version||self.chromium_revision!=release.chromium_revision||self.minimum_data_schema!=release.minimum_data_schema||self.maximum_data_schema!=release.maximum_data_schema||self.files.is_empty()||self.files.len()>MAX_FILES{return Err(rejected("Package identity does not match the signed release"));}
        validate_relative(&self.browser_entry)?;validate_relative(&self.broker_entry)?;
        if self.browser_entry==self.broker_entry{return Err(rejected("Browser and broker entry points must be distinct"));}
        let mut names=BTreeSet::new();let mut bytes=0u64;let mut executables=BTreeSet::new();
        for file in &self.files {
            validate_relative(&file.path)?;ContentHash::parse(file.sha256.0.clone()).map_err(|_|rejected("Package file digest is invalid"))?;
            let lower=file.path.to_ascii_lowercase();
            if !names.insert(lower.clone()){return Err(rejected("Package contains colliding file paths"));}
            for (index,_) in lower.match_indices('/') {if names.contains(&lower[..index]){return Err(rejected("Package path crosses another file"));}}
            if file.executable{executables.insert(file.path.as_str());}
            bytes=bytes.checked_add(file.bytes).ok_or_else(||rejected("Package size overflow"))?;
            if bytes>MAX_PACKAGE_BYTES{return Err(rejected("Package exceeds its unpacked size limit"));}
        }
        // The inverse prefix case must also be checked when the parent file occurs last.
        for path in &names {if names.range(format!("{path}/")..).next().is_some_and(|p|p.starts_with(&format!("{path}/"))){return Err(rejected("Package file is also used as a directory"));}}
        if !executables.contains(self.browser_entry.as_str())||!executables.contains(self.broker_entry.as_str()) {return Err(rejected("Package is missing an executable entry point"));}
        if self.platform=="linux-x86_64"&&(!executables.contains(LINUX_NATIVE_HOST_ENTRY)||!names.contains(LINUX_NATIVE_HOST_SERVICE)||!names.contains(LINUX_NATIVE_HOST_SOCKET)){return Err(rejected("The Linux release package is missing its delegated native-host service"));}
        if !names.contains("licenses/third_party_notices.txt")||!names.contains("licenses/source_offer.json"){return Err(rejected("Release package is missing its notices or corresponding-source declaration"));}
        Ok(())
    }
}

pub fn read_manifest(file:&mut File)->Result<PackageManifest> {
    let mut magic=[0u8;8];file.read_exact(&mut magic).map_err(storage_error)?;
    if &magic!=MAGIC{return Err(rejected("Package magic is invalid"));}
    let mut size=[0u8;4];file.read_exact(&mut size).map_err(storage_error)?;let size=u32::from_le_bytes(size)as usize;
    if size==0||size>MAX_INVENTORY{return Err(rejected("Package inventory exceeds its bound"));}
    let mut raw=vec![0u8;size];file.read_exact(&mut raw).map_err(storage_error)?;
    serde_json::from_slice(&raw).map_err(|_|rejected("Package inventory is invalid"))
}

pub fn unpack(mut source:File,directory:&Path,release:&ReleaseManifest)->Result<PackageManifest> {
    unpack_cancellable(&mut source,directory,release,&std::sync::atomic::AtomicBool::new(false))
}

fn check_cancel(cancel:&std::sync::atomic::AtomicBool)->Result<()>{if cancel.load(std::sync::atomic::Ordering::Relaxed){Err(PlatformError::new(ErrorCode::Cancelled,"Update extraction was cancelled"))}else{Ok(())}}

pub fn unpack_cancellable(source:&mut File,directory:&Path,release:&ReleaseManifest,cancel:&std::sync::atomic::AtomicBool)->Result<PackageManifest> {
    check_cancel(cancel)?;
    let manifest=read_manifest(source)?;manifest.validate(release)?;
    unpack_payload_cancellable(source,directory,&manifest,cancel)?;
    Ok(manifest)
}

pub fn unpack_matching_manifest(source:&mut File,directory:&Path,expected:&PackageManifest,cancel:&std::sync::atomic::AtomicBool)->Result<PackageManifest> {
    check_cancel(cancel)?;
    let manifest=read_manifest(source)?;
    if manifest.schema_version!=expected.schema_version||manifest.channel!=expected.channel||manifest.platform!=expected.platform||manifest.version!=expected.version||manifest.chromium_revision!=expected.chromium_revision||manifest.files.len()!=expected.files.len() {
        return Err(rejected("Repair package manifest does not match the installed release"));
    }
    for (lhs,rhs) in manifest.files.iter().zip(expected.files.iter()) {
        if lhs.path!=rhs.path||lhs.bytes!=rhs.bytes||lhs.sha256!=rhs.sha256||lhs.executable!=rhs.executable {
            return Err(rejected("Repair package file inventory does not match the installed release"));
        }
    }
    unpack_payload_cancellable(source,directory,&manifest,cancel)?;
    Ok(manifest)
}

fn unpack_payload_cancellable(source:&mut File,directory:&Path,manifest:&PackageManifest,cancel:&std::sync::atomic::AtomicBool)->Result<()> {
    // Caller creates a fresh UUID directory under its private install root. Never merge
    // package files into an existing tree; no links or archive-supplied permissions exist.
    if std::fs::read_dir(directory).map_err(storage_error)?.next().is_some(){return Err(rejected("Update staging directory is not empty"));}
    let mut created=BTreeSet::new();
    for entry in &manifest.files {
        check_cancel(cancel)?;
        let path=directory.join(&entry.path);let parent=path.parent().ok_or_else(||rejected("Package file has no parent"))?;
        std::fs::create_dir_all(parent).map_err(storage_error)?;
        let mut options=OpenOptions::new();options.create_new(true).write(true);
        #[cfg(unix)]{use std::os::unix::fs::OpenOptionsExt;options.mode(if entry.executable{0o700}else{0o600});}
        let mut target=options.open(&path).map_err(storage_error)?;let mut hash=Sha256::new();let mut left=entry.bytes;let mut buffer=[0u8;65536];
        while left>0 {check_cancel(cancel)?;let request=left.min(buffer.len()as u64)as usize;let read=source.read(&mut buffer[..request]).map_err(storage_error)?;if read==0{return Err(rejected("Package payload is truncated"));}target.write_all(&buffer[..read]).map_err(storage_error)?;hash.update(&buffer[..read]);left-=read as u64;}
        if format!("{:x}",hash.finalize())!=entry.sha256.0{return Err(rejected("Package file checksum failed"));}
        #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;target.set_permissions(std::fs::Permissions::from_mode(if entry.executable{0o500}else{0o400})).map_err(storage_error)?;}
        target.sync_all().map_err(storage_error)?;created.insert(parent.to_owned());
    }
    let mut tail=[0u8;1];if source.read(&mut tail).map_err(storage_error)?!=0{return Err(rejected("Package has unlisted trailing data"));}
    if manifest.platform=="linux-x86_64"{validate_linux_registration(directory)?;}
    for parent in created.into_iter().rev(){check_cancel(cancel)?;sync_directory(&parent)?;}sync_directory(directory)?;check_cancel(cancel)?;
    Ok(())
}

fn validate_linux_registration(directory:&Path)->Result<()> {
    let service=std::fs::read(directory.join(LINUX_NATIVE_HOST_SERVICE)).map_err(storage_error)?;let socket=std::fs::read(directory.join(LINUX_NATIVE_HOST_SOCKET)).map_err(storage_error)?;
    if service.len()>65536||socket.len()>65536{return Err(rejected("Linux native-host registration exceeds its size bound"));}
    let service=std::str::from_utf8(&service).map_err(|_|rejected("Linux native-host service is not UTF-8"))?;let socket=std::str::from_utf8(&socket).map_err(|_|rejected("Linux native-host socket is not UTF-8"))?;
    if service.lines().filter(|line|*line==LINUX_SERVICE_ROUTE).count()!=1||service.lines().filter(|line|*line=="RefuseManualStart=yes").count()!=1||service.lines().filter(|line|*line=="RefuseManualStop=yes").count()!=1||service.matches("@OMNIPLAY_LAUNCHER@").count()!=1||service.contains("[Install]")||service.lines().any(|line|line.starts_with("ExecStartPre=")||line.starts_with("ExecStartPost=")||line.starts_with("ExecStop=")||((line.starts_with("Requires=")||line.starts_with("BindsTo="))&&line.contains("omniplay-native-host.socket"))){return Err(rejected("Linux native-host service template has an unsafe launch route"));}
    if socket.lines().filter(|line|*line=="ListenStream=%t/omniplay/native-host.sock").count()!=1||socket.lines().filter(|line|*line=="Service=omniplay-native-host.service").count()!=1||socket.contains("@OMNIPLAY_LAUNCHER@")||socket.lines().any(|line|line.starts_with("Exec")){return Err(rejected("Linux native-host socket template is invalid"));}
    Ok(())
}

pub fn verify_entry(directory:&Path,manifest:&PackageManifest,entry:&str)->Result<PathBuf> {
    validate_relative(entry)?;
    let file=manifest.files.iter().find(|f|f.path==entry&&f.executable).ok_or_else(||rejected("Entry point is not in the installed inventory"))?;
    let resolved=installed_file(directory,entry)?;
    let mut opened=File::open(&resolved).map_err(storage_error)?;let(hash,bytes)=hash_file(&mut opened,file.bytes)?;
    if hash!=file.sha256||bytes!=file.bytes{return Err(rejected("Installed entry point checksum failed"));}
    Ok(resolved)
}

// Inspect every component, not only the final file: an intermediate link must
// not redirect an otherwise correctly signed executable outside its inventory.
pub fn installed_file(directory:&Path,entry:&str)->Result<PathBuf>{
    validate_relative(entry)?;
    let check=|path:&Path,is_directory:bool|->Result<()>{let metadata=std::fs::symlink_metadata(path).map_err(storage_error)?;
        #[cfg(windows)]{use std::os::windows::fs::MetadataExt;if metadata.file_attributes()&0x400!=0{return Err(rejected("Installed paths cannot contain reparse points"));}}
        let wrong_type=if is_directory{!metadata.is_dir()}else{!metadata.is_file()};
        if metadata.file_type().is_symlink()||wrong_type{return Err(rejected("Installed paths must contain ordinary directories and files"));}Ok(())};
    if !directory.is_absolute(){return Err(rejected("Installed version directory must be absolute"));}
    for ancestor in directory.ancestors(){check(ancestor,true)?;}
    let base=directory.canonicalize().map_err(storage_error)?;let mut path=base.clone();let parts=entry.split('/').collect::<Vec<_>>();
    for(index,part)in parts.iter().enumerate(){path.push(part);check(&path,index+1<parts.len())?;}
    let resolved=path.canonicalize().map_err(storage_error)?;if !resolved.starts_with(&base){return Err(rejected("Installed file escapes its version directory"));}Ok(resolved)
}

pub fn inventory_hashes(manifest:&PackageManifest)->BTreeMap<&str,&ContentHash>{manifest.files.iter().map(|f|(f.path.as_str(),&f.sha256)).collect()}
