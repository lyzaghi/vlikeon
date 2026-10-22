//! Authenticated catalog and held-file/immutable DT_NEEDED closure preparation.
//! The caller supplies Store's already verified outer release inventory. This
//! source is not yet selected by NativeRegistry/NativeAdapter.
use crate::{linux_image_cache::ProfileImageCache,linux_policy::{LinuxImage,LinuxImageRole},registry::{RuntimePackage,parse_catalog},unavailable};
use omniplay_contracts::{ErrorCode,PlatformError,Result,valid_identifier};
use omniplay_core::util::{bounded_read,digest};
use omniplay_sandbox::{linux_elf::{BootstrapExecutable,PresentationExecutable},linux_images::{self,ImageRole,ReleaseDirectory},linux_launch::LaunchFile,linux_sealed::SealedObject};
use omniplay_updater::package::PackageManifest;
use std::{collections::{BTreeMap,BTreeSet},fs::File,path::Path,sync::atomic::{AtomicBool,Ordering}};

struct PreparedImage{declaration:LinuxImage,object:SealedObject}
pub(crate) struct PreparedLinuxPackage{pub package:RuntimePackage,pub bootstrap:BootstrapExecutable,pub presentation:PresentationExecutable,pub presentation_directory:File,images:Vec<PreparedImage>,pub cache:ProfileImageCache}
fn invalid()->PlatformError{unavailable("The Linux native package differs from its authenticated image closure")}
fn cancelled(cancel:&AtomicBool)->Result<()>{if cancel.load(Ordering::Acquire){Err(PlatformError::new(ErrorCode::Cancelled,"Linux native package preparation was cancelled"))}else{Ok(())}}
fn role(value:LinuxImageRole)->ImageRole{match value{LinuxImageRole::Worker=>ImageRole::Worker,LinuxImageRole::Loader=>ImageRole::Loader,LinuxImageRole::Core=>ImageRole::Core,LinuxImageRole::Dependency=>ImageRole::Dependency,LinuxImageRole::Asset=>ImageRole::Asset}}
fn closure(images:&[PreparedImage])->Result<()>{
    let mut providers=BTreeMap::new();let mut dependencies=BTreeMap::new();let mut pending=Vec::new();
    for image in images{let declaration=&image.declaration;let Some(inspected)=linux_images::inspect(&image.object,role(declaration.role))?else{continue;};let basename=declaration.target.rsplit('/').next().ok_or_else(invalid)?;
        if declaration.role==LinuxImageRole::Worker{if inspected.soname.is_some(){return Err(invalid());}}
        else{if inspected.soname.as_deref().is_some_and(|name|name!=basename)||matches!(declaration.role,LinuxImageRole::Loader|LinuxImageRole::Dependency)&&inspected.soname.as_deref()!=Some(basename)||!linux_images::basename(basename)||providers.insert(basename.to_owned(),declaration.target.clone()).is_some(){return Err(invalid());}}
        if matches!(declaration.role,LinuxImageRole::Worker|LinuxImageRole::Loader|LinuxImageRole::Core){pending.push(declaration.target.clone());}dependencies.insert(declaration.target.clone(),inspected.needed);
    }
    for names in dependencies.values(){if names.iter().any(|name|!providers.contains_key(name)){return Err(invalid());}}
    let mut reachable=BTreeSet::new();while let Some(target)=pending.pop(){if !reachable.insert(target.clone()){continue;}let needed=dependencies.get(&target).ok_or_else(invalid)?;for name in needed{pending.push(providers.get(name).ok_or_else(invalid)?.clone());}}
    if dependencies.keys().any(|target|!reachable.contains(target)){return Err(invalid());}Ok(())
}
impl PreparedLinuxPackage{
    pub fn admit(root:&Path,runtime:&str,inventory:&PackageManifest,cache:ProfileImageCache,cancel:&AtomicBool)->Result<Self>{
        cancelled(cancel)?;if !root.is_absolute()||inventory.platform!="linux-x86_64"||!valid_identifier(runtime){return Err(invalid());}let release=ReleaseDirectory::open(root)?;
        let catalog=inventory.files.iter().find(|file|file.path=="native-runtimes/catalog.json"&&!file.executable).ok_or_else(invalid)?;if catalog.bytes==0||catalog.bytes>1024*1024{return Err(invalid());}let raw=bounded_read(&mut release.file(&catalog.path)?,catalog.bytes)?;cancelled(cancel)?;if raw.len()as u64!=catalog.bytes||digest(&raw)!=catalog.sha256{return Err(invalid());}
        let package=parse_catalog(&raw)?.into_iter().find(|package|package.manifest.id==runtime).ok_or_else(invalid)?;package.manifest.validate()?;let linux=package.linux.as_ref().ok_or_else(invalid)?;linux.validate(&package)?;
        for file in &package.files{if !inventory.files.iter().any(|outer|outer.path==file.path&&outer.bytes==file.bytes&&outer.sha256==file.sha256){return Err(invalid());}}
        let helper=package.presentation.as_ref().ok_or_else(invalid)?;for path in[&linux.bootstrap,&package.worker,&helper.executable]{if !inventory.files.iter().any(|file|&file.path==path&&file.executable){return Err(invalid());}}
        let bootstrap=package.files.iter().find(|file|file.path==linux.bootstrap).ok_or_else(invalid)?;let bootstrap=BootstrapExecutable::admit(release.file(&bootstrap.path)?,&bootstrap.sha256,bootstrap.bytes,cancel)?;
        let presentation=package.files.iter().find(|file|file.path==helper.executable).ok_or_else(invalid)?;let presentation=PresentationExecutable::admit(release.file(&presentation.path)?,&presentation.sha256,presentation.bytes,cancel)?;
        let directory=Path::new(&package.worker).parent().and_then(|path|path.to_str()).ok_or_else(invalid)?;let presentation_directory=release.subdirectory(directory)?;let mut images=Vec::new();
        for declaration in &linux.images{cancelled(cancel)?;let file=package.files.iter().find(|file|file.path==declaration.source).ok_or_else(invalid)?;let object=cache.image(&file.sha256,file.bytes,cancel,||release.file(&file.path))?;images.push(PreparedImage{declaration:declaration.clone(),object});}
        closure(&images)?;cancelled(cancel)?;Ok(Self{package,bootstrap,presentation,presentation_directory,images,cache})
    }
    pub fn launch_images(&self)->Vec<LaunchFile>{self.images.iter().map(|image|LaunchFile::Image{path:image.declaration.target.clone(),executable:matches!(image.declaration.role,LinuxImageRole::Worker|LinuxImageRole::Loader),object:image.object.clone()}).collect()}
}
