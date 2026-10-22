use crate::{runtime::NativeAdapter,unavailable};
use omniplay_contracts::*;
use omniplay_core::{sessions::{RuntimeAdapter,RuntimeFactory},util::{bounded_read,hash_file,storage_error}};
use omniplay_updater::{channel::Channel,package::{PackageManifest,validate_relative},store::Store};
use serde::{Deserialize,Serialize};
use std::{fs::{File,OpenOptions},path::{Path,PathBuf}};
#[cfg(all(target_os="linux",target_arch="x86_64",target_env="gnu"))]
use std::os::unix::fs::{MetadataExt,OpenOptionsExt};
use uuid::Uuid;

#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeFile {pub path:String,pub sha256:ContentHash,pub bytes:u64}
#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FirmwareRequirement {pub role:String,pub filename:String,pub allowed_bytes:Vec<u64>,#[serde(default)]pub required_group:Option<String>,#[serde(default)]pub optional:bool}
#[derive(Clone,Copy,Default,PartialEq,Eq,Serialize,Deserialize)]
pub enum VideoBackend {#[default]#[serde(rename="software")]Software,#[serde(rename="opengl_core")]OpenGlCore}
impl VideoBackend{pub fn as_str(self)->&'static str{match self{Self::Software=>"software",Self::OpenGlCore=>"opengl_core"}}}
#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationPackage{pub protocol:u32,pub executable:String,pub maximum_width:u32,pub maximum_height:u32,pub memory_limit_mb:u32}
#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePackage {
    pub manifest:RuntimeManifest,pub worker:String,pub core_library:String,
    pub expected_library_name:String,pub expected_library_version:String,
    pub in_game_format_version:String,pub maximum_rom_bytes:u64,pub memory_limit_mb:u32,
    pub files:Vec<RuntimeFile>,
    #[serde(default)]pub core_options:std::collections::BTreeMap<String,String>,
    #[serde(default)]pub user_core_options:Vec<CoreOptionDefinition>,
    #[serde(default)]pub firmware_requirements:Vec<FirmwareRequirement>,
    #[serde(default)]pub content_formats:Vec<ContentFormat>,
    #[serde(default)]pub minimum_system_ram_bytes:u64,
    #[serde(default)]pub reset_after_ram_load:bool,
    #[serde(default)]pub filesystem_save_directory:Option<String>,
    #[serde(default)]pub core_save_directory:Option<String>,
    #[serde(default)]pub filesystem_save_memory:bool,
    #[serde(default)]pub file_io_protocol:u32,
    #[serde(default)]pub writable_files_protocol:u32,
    #[serde(default)]pub has_system_assets:bool,
    #[serde(default)]pub use_scratch_system_directory:bool,
    #[serde(default)]pub video_backend:VideoBackend,
    #[serde(default)]pub presentation:Option<PresentationPackage>,
    #[serde(default)]pub linux:Option<crate::linux_policy::LinuxPackage>,
    #[serde(default)]pub macos:Option<crate::mac_policy::MacPackage>,
    #[serde(default)]pub gpu_max_width:u32,
    #[serde(default)]pub gpu_max_height:u32,
    #[serde(default)]pub prepare_completion:bool,
    #[serde(default)]pub pause_barrier:bool,
    #[serde(default)]pub pause_barrier_protocol:u32,
    #[serde(default)]pub portable_in_game_format:Option<String>,
}
impl RuntimePackage{
    pub(crate) fn legacy_os_writes(&self)->bool{
        self.writable_files_protocol==0&&(self.filesystem_save_directory.is_some()||self.use_scratch_system_directory)
    }
    pub fn staging_budget(&self,game:&GameManifest)->Result<(u64,u32)>{
        game.validate()?;self.manifest.validate()?;if !self.manifest.systems.contains(&game.system)||game.byte_length>self.maximum_rom_bytes{return Err(PlatformError::new(ErrorCode::UnsupportedCapability,"The selected native runtime does not admit this game"));}
        let firmware=self.firmware_requirements.iter().try_fold(0u64,|total,requirement|total.checked_add(*requirement.allowed_bytes.iter().max().ok_or_else(||unavailable("Firmware staging shape is absent"))?).ok_or_else(||unavailable("Firmware staging size overflow")))?;
        let bytes=game.byte_length.checked_add(firmware).and_then(|bytes|bytes.checked_add(omniplay_core::native_work_storage::METADATA_RESERVE)).ok_or_else(||unavailable("Native staging size overflow"))?;let files=game.media.as_ref().map(|media|media.assets.len()).unwrap_or(1)+self.firmware_requirements.len();if bytes>8*1024*1024*1024||!(1..=144).contains(&files){return Err(PlatformError::new(ErrorCode::ResourceLimit,"The native staging declaration exceeds its storage policy"));}Ok((bytes,files as u32))
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {schema_version:u32,runtimes:Vec<RuntimePackage>}

pub struct NativeRegistry {root:Option<PathBuf>,packages:Vec<RuntimePackage>,profile_root:PathBuf,profile_id:Uuid,release_inventory:Option<PackageManifest>,#[cfg(all(target_os="linux",target_arch="x86_64",target_env="gnu"))]linux_context:Option<(omniplay_sandbox::linux_delegation::Delegation,crate::linux_image_cache::ProfileImageCache)>}
impl NativeRegistry {
    pub fn installed(profile_root:&Path,profile_id:Uuid)->Self {
        // Windows cleanup belongs to the broker's bounded asynchronous owner.
        // Do not run profile/process/filesystem recovery before native hello.
        #[cfg(not(windows))]collect_abandoned_work(profile_root,profile_id);
        let mut registry=Self{root:None,packages:Vec::new(),profile_root:profile_root.to_owned(),profile_id,release_inventory:None,#[cfg(all(target_os="linux",target_arch="x86_64",target_env="gnu"))]linux_context:None};
        // Catalog failure disables only native execution. The library, exports and
        // isolated web runtimes remain available instead of losing the whole profile.
        if let Ok((root,inventory))=installed_root(){
            if let Ok(packages)=load_catalog(&root,Some(&inventory)){registry.root=Some(root);registry.release_inventory=Some(inventory);registry.packages=packages;}
        }
        #[cfg(debug_assertions)]
        if registry.root.is_none(){
            if let Some(path)=std::env::var_os("OMNIPLAY_DEV_NATIVE_ROOT"){
                let root=PathBuf::from(path);
                if root.is_absolute(){if let Ok(root)=root.canonicalize(){if let Ok(packages)=load_catalog(&root,None){registry.root=Some(root);registry.packages=packages;}}}
            }
        }
        registry
    }
    fn available(&self)->bool{
        #[cfg(all(target_os="linux",target_arch="x86_64",target_env="gnu"))]{return self.linux_context.is_some()&&self.release_inventory.is_some()&&self.root.is_some();}
        #[cfg(not(all(target_os="linux",target_arch="x86_64",target_env="gnu")))]{omniplay_sandbox::supported()}
    }
    /// Explicit trusted-host composition only. No environment variable, renderer
    /// parameter or default installed() call can create/grant this delegation.
    #[cfg(all(target_os="linux",target_arch="x86_64",target_env="gnu"))]
    pub fn installed_linux(profile_root:&Path,profile_id:Uuid,delegation:omniplay_sandbox::linux_delegation::Delegation,cache_bytes:u64)->Result<Self>{
        if profile_id.is_nil()||delegation.identity().profile!=profile_id.to_string(){return Err(unavailable("The Linux delegation belongs to a different profile"));}let(root,inventory)=installed_root()?;let packages=load_catalog(&root,Some(&inventory))?;if packages.iter().any(|package|package.linux.is_none()){return Err(unavailable("Every selected Linux native package requires its explicit actor image policy"));}
        let profile_root=profile_root.canonicalize().map_err(storage_error)?;let cache=crate::linux_image_cache::ProfileImageCache::open(&profile_root,profile_id,cache_bytes)?;Ok(Self{root:Some(root),packages,profile_root,profile_id,release_inventory:Some(inventory),linux_context:Some((delegation,cache))})
    }
    /// Trusted service-owner entry point. Package admission precedes provisioning;
    /// retained work is audited before converting the profile to launch authority.
    /// The default native-messaging startup does not manufacture a host owner.
    #[cfg(all(target_os="linux",target_arch="x86_64",target_env="gnu"))]
    pub fn installed_linux_host(profile_root:&Path,profile_id:Uuid,host:&omniplay_sandbox::linux_host::NativeHostRoot,profile_lease:File,cache_bytes:u64)->Result<(Self,crate::LinuxRecoverySummary)>{
        if profile_id.is_nil(){return Err(unavailable("The Linux native host needs an exact profile owner"));}
        let(root,inventory)=installed_root()?;let packages=load_catalog(&root,Some(&inventory))?;if packages.iter().any(|package|package.linux.is_none()){return Err(unavailable("Every selected Linux native package requires its explicit actor image policy"));}
        let profile_root=profile_root.canonicalize().map_err(storage_error)?;let _profile=omniplay_sandbox::linux_launch::private_directory(&profile_root)?;
        let cache=crate::linux_image_cache::ProfileImageCache::open(&profile_root,profile_id,cache_bytes)?;
        let recovery=host.profile(&profile_id.to_string(),profile_lease)?;let summary=crate::linux_work::recover_linux_work(&profile_root,profile_id,&recovery)?;let delegation=recovery.finish()?;
        Ok((Self{root:Some(root),packages,profile_root,profile_id,release_inventory:Some(inventory),linux_context:Some((delegation,cache))},summary))
    }
    #[cfg(all(target_os="linux",target_arch="x86_64",target_env="gnu"))]
    pub fn installed_linux_service(profile_root:&Path,profile_id:Uuid,socket:&Path,cache_bytes:u64)->Result<(Self,crate::LinuxRecoverySummary)>{
        if profile_id.is_nil(){return Err(unavailable("The Linux native service needs an exact profile owner"));}let(root,inventory)=installed_root()?;let packages=load_catalog(&root,Some(&inventory))?;if packages.iter().any(|package|package.linux.is_none()){return Err(unavailable("Every selected Linux native package requires its explicit actor image policy"));}
        let profile_root=profile_root.canonicalize().map_err(storage_error)?;let _profile=omniplay_sandbox::linux_launch::private_directory(&profile_root)?;let profile_owner=std::fs::metadata(&profile_root).map_err(storage_error)?.uid();
        let lease_path=profile_root.join("linux-cgroup.lease");let lease=OpenOptions::new().read(true).write(true).create(true).mode(0o600).custom_flags(libc::O_CLOEXEC|libc::O_NOFOLLOW).open(&lease_path).map_err(storage_error)?;let value=lease.metadata().map_err(storage_error)?;if !value.is_file()||value.uid()!=profile_owner||value.mode()&0o7777!=0o600||value.nlink()!=1{return Err(unavailable("The Linux profile resource lease is not private"));}
        let directory=omniplay_sandbox::linux_host_channel::request(socket,&profile_id.to_string())?;let recovery=omniplay_sandbox::linux_delegation::RecoveryDelegation::admit(directory,&profile_id.to_string(),lease,omniplay_sandbox::linux_delegation::DelegationPolicy{maximum_sessions:4,maximum_reserved_bytes:14*1024*1024*1024})?;
        let summary=crate::linux_work::recover_linux_work(&profile_root,profile_id,&recovery)?;let delegation=recovery.finish()?;let cache=crate::linux_image_cache::ProfileImageCache::open(&profile_root,profile_id,cache_bytes)?;
        Ok((Self{root:Some(root),packages,profile_root,profile_id,release_inventory:Some(inventory),linux_context:Some((delegation,cache))},summary))
    }
}

fn collect_abandoned_work(profile_root:&Path,profile_id:Uuid){crate::work::collect_abandoned(profile_root,profile_id);}
impl RuntimeFactory for NativeRegistry {
    fn option_policy(&self,id:&str)->Result<CoreOptionPolicy>{
        if !self.available(){return Err(unavailable("Native core options are not installed for this platform"));}
        let package=self.packages.iter().find(|p|p.manifest.id==id).ok_or_else(||unavailable("The requested native runtime is not installed"))?;
        package.manifest.validate()?;
        let policy=CoreOptionPolicy{schema_version:1,definitions:package.user_core_options.clone()};policy.validate()?;Ok(policy)
    }
    fn list(&self)->Vec<RuntimeManifest>{if !self.available(){return Vec::new();}self.packages.iter().filter(|p|p.manifest.validate().is_ok()).map(|p|p.manifest.clone()).collect()}
    fn create(&self,id:&str)->Result<Box<dyn RuntimeAdapter>> {
        if !self.available(){return Err(unavailable("This platform's required native isolation is not available"));}
        let package=self.packages.iter().find(|p|p.manifest.id==id).cloned().ok_or_else(||unavailable("The requested native runtime is not installed"))?;
        package.manifest.validate()?;
        let root=self.root.clone().ok_or_else(||unavailable("The installed native runtime catalog is unavailable"))?;
        #[cfg(all(target_os="linux",target_arch="x86_64",target_env="gnu"))]{
            let(delegation,cache)=self.linux_context.as_ref().ok_or_else(||unavailable("The explicit Linux profile resource context is absent"))?;let inventory=self.release_inventory.clone().ok_or_else(||unavailable("The authenticated Linux release inventory is absent"))?;
            return Ok(Box::new(crate::linux_adapter::LinuxAdapter::new(root,package,inventory,self.profile_root.clone(),self.profile_id,delegation.clone(),cache.clone())));
        }
        #[cfg(not(all(target_os="linux",target_arch="x86_64",target_env="gnu")))]
        Ok(Box::new(NativeAdapter::new(root,package,self.profile_root.clone(),self.profile_id,self.release_inventory.clone())))
    }
}

fn installed_root()->Result<(PathBuf,PackageManifest)> {
    let store=Store::open(&omniplay_updater::install_root()?,Channel::embedded()?)?;let active=store.active()?;
    let root=store.version_directory(active.id);
    let expected=omniplay_updater::package::verify_entry(&root,&active.manifest,&active.manifest.broker_entry)?;
    if std::env::current_exe().and_then(|p|p.canonicalize()).map_err(storage_error)?!=expected{return Err(unavailable("Native runtimes must belong to the running broker's signed release"));}
    Ok((root,active.manifest))
}
fn load_catalog(root:&Path,inventory:Option<&PackageManifest>)->Result<Vec<RuntimePackage>> {
    let path="native-runtimes/catalog.json";
    let mut file=approved_file(root,path,inventory,None)?;
    let raw=bounded_read(&mut file,1024*1024)?;
    parse_catalog(&raw)
}
pub(crate) fn parse_catalog(raw:&[u8])->Result<Vec<RuntimePackage>>{
    if raw.len()>1024*1024{return Err(unavailable("The native runtime catalog exceeds its size bound"));}
    let catalog:Catalog=serde_json::from_slice(&raw).map_err(|_|unavailable("The native runtime catalog is invalid"))?;
    if catalog.schema_version!=1||catalog.runtimes.len()>32{return Err(unavailable("The native runtime catalog exceeds its version or size bound"));}
    let mut ids=std::collections::BTreeSet::new();
    for package in &catalog.runtimes {
        if package.pause_barrier_protocol>1||package.pause_barrier!=(package.pause_barrier_protocol==1){return Err(unavailable("Native pause requires the declared checked-barrier protocol; legacy void barriers must be rebuilt"));}
        if !ids.insert(&package.manifest.id)||!valid_identifier(&package.manifest.id)||package.manifest.kind!=RuntimeKind::NativeEmulator||package.manifest.platform!=format!("{}-{}",std::env::consts::OS,std::env::consts::ARCH)||!(256..=8192).contains(&package.memory_limit_mb)||package.maximum_rom_bytes==0||package.maximum_rom_bytes>MAX_ROM_BYTES||package.files.is_empty()||package.files.len()>256||package.expected_library_name.len()>128||package.expected_library_version.len()>128||package.in_game_format_version.is_empty(){return Err(unavailable("A native runtime declaration is outside its policy"));}
        let prefix=format!("native-runtimes/{}/",package.manifest.id);
        if let Some(linux)=&package.linux{linux.validate(package)?;}else if package.manifest.platform=="linux-x86_64"{return Err(unavailable("Linux native packages require their explicit actor image closure"));}
        if let Some(macos)=&package.macos{macos.validate(package)?;}else if package.manifest.platform=="macos-aarch64"{return Err(unavailable("macOS native packages require their explicit signed sandbox policy"));}
        if package.minimum_system_ram_bytes>64*1024*1024||package.core_options.len()>128||package.core_options.iter().any(|(k,v)|k.is_empty()||k.len()>128||v.len()>256){return Err(unavailable("Native core configuration exceeds its bounds"));}
        let options=CoreOptionPolicy{schema_version:1,definitions:package.user_core_options.clone()};options.validate()?;
        if options.definitions.iter().any(|d|package.core_options.get(&d.key)!=Some(&d.default_value)){return Err(unavailable("Native user option defaults differ from their signed fixed configuration"));}
        if options.definitions.iter().any(|d|d.key=="beetle_psx_enable_memcard1")&&(package.filesystem_save_directory.is_none()||!package.pause_barrier||!package.filesystem_save_memory||package.file_io_protocol!=1){return Err(unavailable("A selectable native second card requires checked combined memory/file saves"));}
        match package.video_backend{
            VideoBackend::Software if package.gpu_max_width!=0||package.gpu_max_height!=0=>return Err(unavailable("Software runtime declares an unrelated GPU target")),
            VideoBackend::OpenGlCore if !(16..=4096).contains(&package.gpu_max_width)||!(16..=4096).contains(&package.gpu_max_height)||u64::from(package.gpu_max_width)*u64::from(package.gpu_max_height)>16*1024*1024=>return Err(unavailable("Native GPU target exceeds its signed dimension policy")),
            _=>{}
        }
        if package.has_system_assets&&package.use_scratch_system_directory{return Err(unavailable("Native system assets and scratch directories are distinct declarations"));}
        if let Some(format)=&package.portable_in_game_format{
            let psp=format=="ppsspp-files-opsf2"&&package.in_game_format_version==*format&&package.filesystem_save_directory.as_deref()==Some("save/PSP/SAVEDATA")&&package.core_save_directory.is_none()&&!package.filesystem_save_memory&&package.pause_barrier&&package.manifest.systems.iter().all(|s|*s==GameSystem::Psp);
            let md=format=="ares-md-canonical-files-v1"&&package.in_game_format_version==*format&&package.filesystem_save_directory.as_deref()==Some("save/MD")&&package.core_save_directory.as_deref()==Some("save/MD")&&!package.filesystem_save_memory&&package.pause_barrier&&package.file_io_protocol==1&&package.manifest.id=="ares-md-native"&&package.expected_library_name=="OmniPlay ares MD"&&package.expected_library_version=="0.3.0+ares.9408cb43d494"&&package.manifest.version.to_string()=="0.3.0+ares.9408cb43d494"&&package.manifest.source_revision=="9408cb43d4948fc3ea6e152a307a34348df3fe04"&&package.manifest.systems.len()==1&&package.manifest.systems.contains(&GameSystem::MegaDrive);
            if !psp&&!md{return Err(unavailable("The native portable save format is not a supported complete codec declaration"));}
        }
        if let Some(directory)=&package.filesystem_save_directory{validate_relative(directory)?;if !directory.starts_with("save/")||directory.len()>256||directory.split('/').count()>8{return Err(unavailable("Native persistent files are outside their private save root"));}}
        if package.file_io_protocol>1||(package.file_io_protocol==1&&(package.filesystem_save_directory.is_none()||!package.pause_barrier))||package.filesystem_save_directory.is_some()&&!package.pause_barrier{return Err(unavailable("Native persistent files need their declared paused I/O contract"));}
        if package.writable_files_protocol>1{return Err(unavailable("Native memory files require a supported protocol"));}
        if package.writable_files_protocol==1{
            let md=package.manifest.id=="ares-md-native"&&package.portable_in_game_format.as_deref()==Some("ares-md-canonical-files-v1")&&package.filesystem_save_directory.as_deref()==Some("save/MD")&&package.core_save_directory.as_deref()==Some("save/MD");
            let ps1=package.manifest.id=="beetle-psx-native"&&package.expected_library_name=="Beetle PSX"&&package.expected_library_version=="0.9.44.1+omniplay.native3"&&package.manifest.version.to_string()=="0.9.44+1.omniplay.native3"&&package.manifest.source_revision=="df2b53b306ba52d2116f8e8cc93b9ee33d239be7"&&package.manifest.systems.len()==1&&package.manifest.systems.contains(&GameSystem::Ps1)&&package.filesystem_save_directory.as_deref()==Some("save/PSX")&&package.core_save_directory.as_deref()==Some("save/PSX")&&package.filesystem_save_memory&&package.portable_in_game_format.is_none()&&package.in_game_format_version=="beetle-psx-opnm1-cards1-native64";
            if !cfg!(windows)||!package.pause_barrier||package.file_io_protocol!=1||package.video_backend!=VideoBackend::Software||!md&&!ps1{return Err(unavailable("Native memory files require their exact admitted core and paused capture route"));}
        }
        if let Some(directory)=&package.core_save_directory{validate_relative(directory)?;if package.filesystem_save_directory.as_ref()!=Some(directory){return Err(unavailable("The native core save root differs from its captured private directory"));}}
        if package.filesystem_save_memory&&(package.filesystem_save_directory.is_none()||package.core_save_directory.is_none()||package.file_io_protocol!=1||package.portable_in_game_format.is_some()){return Err(unavailable("Combined native memory/files need checked I/O and their own nonportable format"));}
        if package.filesystem_save_directory.is_some()&&package.manifest.systems.contains(&GameSystem::Ps1)&&(!package.filesystem_save_memory||package.file_io_protocol!=1){return Err(unavailable("PS1 file-backed cards require complete checked memory and file progress"));}
        if package.firmware_requirements.len()>16{return Err(unavailable("Native firmware declaration exceeds its bound"));}
        let mut firmware_names=std::collections::BTreeSet::new();
        for fw in &package.firmware_requirements{validate_relative(&fw.filename)?;if fw.filename.contains('/')||fw.filename.starts_with("game.")||!(fw.filename.ends_with(".rom")||fw.filename.ends_with(".bin"))||!firmware_names.insert(&fw.filename)||fw.allowed_bytes.is_empty()||fw.allowed_bytes.len()>8||fw.allowed_bytes.iter().any(|n|*n==0||*n>32*1024*1024)||!valid_identifier(&fw.role)||fw.required_group.as_ref().is_some_and(|g|!valid_identifier(g)){return Err(unavailable("Native firmware declaration is invalid"));}}
        let mut paths=std::collections::BTreeSet::new();
        for file in &package.files {validate_relative(&file.path)?;ContentHash::parse(file.sha256.0.clone())?;if !file.path.starts_with(&prefix)||!paths.insert(&file.path)||file.bytes==0||file.bytes>2*1024*1024*1024{return Err(unavailable("Native runtime files are outside their package directory"));}}
        let worker=package.files.iter().find(|f|f.path==package.worker).ok_or_else(||unavailable("The native worker is absent from its inventory"))?;
        if worker.sha256!=package.manifest.executable_sha256||!paths.contains(&package.core_library){return Err(unavailable("The native runtime executable or core inventory is incomplete"));}
        if package.has_system_assets&&!package.files.iter().any(|f|f.path.starts_with(&format!("{prefix}assets/"))){return Err(unavailable("Declared native system assets are absent from their inventory"));}
        if Path::new(&package.worker).parent()!=Path::new(&package.core_library).parent(){return Err(unavailable("The native core library must reside beside its fixed worker executable"));}
        if let Some(presentation)=&package.presentation{
            validate_relative(&presentation.executable)?;
            if !(omniplay_sandbox::presentation_supported()||package.linux.is_some()&&package.manifest.platform=="linux-x86_64")||presentation.protocol!=2||package.video_backend!=VideoBackend::Software||!(1..=4096).contains(&presentation.maximum_width)||!(1..=4096).contains(&presentation.maximum_height)||!(256..=2048).contains(&presentation.memory_limit_mb)||u64::from(presentation.maximum_width)*u64::from(presentation.maximum_height)*16+128*1024*1024>u64::from(presentation.memory_limit_mb)*1024*1024||!paths.contains(&presentation.executable)||presentation.executable==package.worker||presentation.executable==package.core_library||Path::new(&presentation.executable).parent()!=Path::new(&package.worker).parent(){return Err(unavailable("The private presentation package differs from its protocol, software, inventory or memory policy"));}
        }
    }
    Ok(catalog.runtimes)
}

pub(crate) fn approved_file(root:&Path,path:&str,inventory:Option<&PackageManifest>,expected:Option<&RuntimeFile>)->Result<File> {
    validate_relative(path)?;
    let resolved=root.join(path).canonicalize().map_err(storage_error)?;
    let root=root.canonicalize().map_err(storage_error)?;
    if !resolved.starts_with(&root)||!std::fs::symlink_metadata(root.join(path)).map_err(storage_error)?.is_file(){return Err(unavailable("The runtime file is outside its installed release"));}
    let mut options=OpenOptions::new();options.read(true);
    #[cfg(windows)]{use std::os::windows::fs::OpenOptionsExt;options.share_mode(1);}
    let mut file=options.open(resolved).map_err(storage_error)?;
    let approved=inventory.map(|i|i.files.iter().find(|f|f.path==path).ok_or_else(||unavailable("Runtime file is not in the signed release inventory"))).transpose()?;
    let bound=approved.map(|f|f.bytes).or_else(||expected.map(|f|f.bytes)).unwrap_or(1024*1024);
    let(hash,bytes)=hash_file(&mut file,bound)?;
    if approved.is_some_and(|f|f.sha256!=hash||f.bytes!=bytes)||expected.is_some_and(|f|f.sha256!=hash||f.bytes!=bytes){return Err(unavailable("An installed native runtime file failed its integrity check"));}
    use std::io::{Seek,SeekFrom};file.seek(SeekFrom::Start(0)).map_err(storage_error)?;Ok(file)
}
