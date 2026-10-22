//! Closed catalog declarations. Shape checks author no runtime availability.
use crate::{registry::{RuntimePackage,VideoBackend},unavailable};
use omniplay_contracts::{Capability,Result};
use omniplay_updater::package::validate_relative;
use serde::{Serialize,Deserialize};
use std::{collections::BTreeSet,path::Path};

#[derive(Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]#[serde(rename_all="snake_case")]pub enum LinuxImageRole{Worker,Loader,Core,Dependency,Asset}
#[derive(Clone,Serialize,Deserialize)]#[serde(deny_unknown_fields)]pub struct LinuxImage{pub source:String,pub target:String,pub role:LinuxImageRole}
#[derive(Clone,Serialize,Deserialize)]#[serde(deny_unknown_fields)]pub struct LinuxPackage{pub schema_version:u32,pub bootstrap:String,pub images:Vec<LinuxImage>,pub root_limit_mb:u32,pub scratch_limit_mb:u32,pub save_limit_mb:u32,pub maximum_file_mb:u32,pub cpu_cores:u8}
fn invalid()->omniplay_contracts::PlatformError{unavailable("The Linux native image declaration exceeds its closed package policy")}
impl LinuxPackage{
    pub fn validate(&self,package:&RuntimePackage)->Result<()>{
        if self.schema_version!=1||package.manifest.platform!="linux-x86_64"||package.video_backend!=VideoBackend::Software||package.manifest.capabilities.contains(&Capability::PerformanceMeasurement)||!(16..=1024).contains(&self.root_limit_mb)||!(4..=512).contains(&self.scratch_limit_mb)||!(4..=512).contains(&self.save_limit_mb)||!(1..=256).contains(&self.maximum_file_mb)||!(1..=8).contains(&self.cpu_cores)||!(3..=192).contains(&self.images.len()){return Err(invalid());}
        let presentation=package.presentation.as_ref().ok_or_else(invalid)?;if presentation.protocol!=2||BTreeSet::from([&self.bootstrap,&package.worker,&package.core_library,&presentation.executable]).len()!=4{return Err(invalid());}
        validate_relative(&self.bootstrap)?;let bootstrap=package.files.iter().find(|file|file.path==self.bootstrap).ok_or_else(invalid)?;if bootstrap.bytes==0||bootstrap.bytes>64*1024*1024{return Err(invalid());}
        let core=Path::new(&package.core_library).file_name().and_then(|value|value.to_str()).ok_or_else(invalid)?;if !core.contains(".so")||!core.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric){return Err(invalid());}let core_target=format!("runtime/{core}");let prefix=format!("native-runtimes/{}/",package.manifest.id);let(mut worker,mut loader,mut core_seen,mut bytes,mut assets,mut asset_bytes)=(0,0,0,0u64,0,0u64);let mut targets=BTreeSet::new();
        for image in &self.images{
            validate_relative(&image.source)?;if !omniplay_sandbox::linux_plan::root_path(&image.target)||!targets.insert(image.target.clone())||image.source==self.bootstrap||image.source==presentation.executable{return Err(invalid());}let source=package.files.iter().find(|file|file.path==image.source).ok_or_else(invalid)?;if source.bytes==0||source.bytes>256*1024*1024{return Err(invalid());}bytes=bytes.checked_add(source.bytes).ok_or_else(invalid)?;
            match image.role{
                LinuxImageRole::Worker=>{worker+=1;if image.source!=package.worker||image.target!="runtime/omniplay-native-worker"{return Err(invalid());}},
                LinuxImageRole::Loader=>{loader+=1;if image.target!="lib64/ld-linux-x86-64.so.2"{return Err(invalid());}},
                LinuxImageRole::Core=>{core_seen+=1;if image.source!=package.core_library||image.target!=core_target{return Err(invalid());}},
                LinuxImageRole::Dependency=>{let name=image.target.strip_prefix("runtime/").ok_or_else(invalid)?;if name.is_empty()||name.contains('/')||!name.contains(".so")||!name.as_bytes()[0].is_ascii_alphanumeric(){return Err(invalid());}},
                LinuxImageRole::Asset=>{let relative=image.source.strip_prefix(&prefix).ok_or_else(invalid)?;if !relative.starts_with("assets/")||image.target!=format!("runtime/{relative}")||source.bytes>32*1024*1024{return Err(invalid());}assets+=1;asset_bytes=asset_bytes.checked_add(source.bytes).ok_or_else(invalid)?;}
            }
        }
        if worker!=1||loader!=1||core_seen!=1||assets>128||asset_bytes>128*1024*1024||package.has_system_assets!=(assets>0)||bytes>u64::from(self.root_limit_mb)*1024*1024-2*1024*1024{return Err(invalid());}
        for target in &targets{if targets.range(format!("{target}/")..).next().is_some_and(|other|other.starts_with(&format!("{target}/"))){return Err(invalid());}}
        for file in &package.files{if file.path.starts_with(&format!("{prefix}assets/"))&&!self.images.iter().any(|image|image.role==LinuxImageRole::Asset&&image.source==file.path){return Err(invalid());}}Ok(())
    }
}
