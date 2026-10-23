//! Read-only source descriptors; the authenticated XPC owner copies and hashes
//! the bytes into its own container. The broker never opens that container.
use crate::{mac_work::MacWork,registry::RuntimePackage,unavailable};
use omniplay_contracts::*;
use omniplay_core::{Database,firmware::FirmwareVault};
use omniplay_sandbox::macos::{MacConnection,StagedAsset};
use std::{collections::BTreeMap,fs::File,path::Path,sync::atomic::{AtomicBool,Ordering}};
use uuid::Uuid;
pub(crate) struct StagedInputs{work:MacWork,session:String,game:Uuid,game_hash:ContentHash,filename:String,primary_bytes:u64,firmware:BTreeMap<String,ContentHash>}
fn invalid()->PlatformError{unavailable("The macOS game input declaration is invalid")}
fn cancelled(cancel:&AtomicBool)->Result<()>{if cancel.load(Ordering::Acquire){Err(PlatformError::new(ErrorCode::Cancelled,"macOS input staging was cancelled"))}else{Ok(())}}
impl StagedInputs{
    pub fn prepare(connection:&MacConnection,profile:&Path,package:&RuntimePackage,game:&GameManifest,primary:File,cancel:&AtomicBool)->Result<Self>{
        cancelled(cancel)?;game.validate()?;let reservation=package.staging_budget(game)?;
        package.macos.as_ref().ok_or_else(invalid)?.validate(package)?;
        let format=game.media.as_ref().map(|m|m.format).unwrap_or(ContentFormat::RawRom);
        if !package.content_formats.is_empty()&&!package.content_formats.contains(&format){return Err(PlatformError::new(ErrorCode::UnsupportedCapability,"The macOS core does not support this media format"));}
        let extension=format.extension(game.system)?;let mut sources=BTreeMap::new();
        let(filename,primary_bytes)=if let Some(media)=&game.media{
            let mut primary=Some(primary);
            for asset in &media.assets{cancelled(cancel)?;
                let source=if asset.name==media.entry_name{primary.take().ok_or_else(invalid)?}else{omniplay_sandbox::macos::profile_asset(profile,&asset.content_hash.0)?};
                if sources.insert(asset.name.clone(),StagedAsset{name:asset.name.clone(),sha256:asset.content_hash.0.clone(),bytes:asset.byte_length,source}).is_some(){return Err(invalid());}
            }(media.entry_name.clone(),media.entry()?.byte_length)
        }else{let name=format!("game.{extension}");sources.insert(name.clone(),StagedAsset{name:name.clone(),sha256:game.content_hash.0.clone(),bytes:game.byte_length,source:primary});(name,game.byte_length)};
        let mut database=Database::open(profile)?;let mut groups=BTreeMap::new();let mut firmware=BTreeMap::new();
        for requirement in &package.firmware_requirements{
            cancelled(cancel)?;if let Some(group)=&requirement.required_group{groups.entry(group.clone()).or_insert(false);}
            let selected=FirmwareVault::new(&mut database).selected(game.system,&requirement.role);
            let(metadata,source)=match selected{Ok(value)=>value,Err(error)if error.code==ErrorCode::NotFound&&(requirement.optional||requirement.required_group.is_some())=>continue,Err(error)=>return Err(error)};
            let name=&requirement.filename;
            if !requirement.allowed_bytes.contains(&metadata.byte_length)||metadata.byte_length>32*1024*1024||name.starts_with("game.")||!(name.ends_with(".rom")||name.ends_with(".bin")){return Err(invalid());}
            if sources.insert(name.clone(),StagedAsset{name:name.clone(),sha256:metadata.content_hash.0.clone(),bytes:metadata.byte_length,source}).is_some(){return Err(invalid());}
            firmware.insert(requirement.role.clone(),metadata.content_hash);if let Some(group)=&requirement.required_group{groups.insert(group.clone(),true);}
        }
        if groups.values().any(|present|!*present){return Err(PlatformError::new(ErrorCode::NotFound,"The selected macOS core needs user-imported firmware"));}
        cancelled(cancel)?;let assets:Vec<_>=sources.into_values().collect();
        let bytes=assets.iter().try_fold(0u64,|total,asset|total.checked_add(asset.bytes).ok_or_else(invalid))?;
        if bytes.checked_add(omniplay_core::native_work_storage::METADATA_RESERVE).is_none_or(|bytes|bytes>reservation.0)||assets.len()>reservation.1 as usize{return Err(invalid());}
        let work=MacWork::create(connection.clone(),profile,&package.manifest.id,reservation.0,reservation.1)?;
        let session=work.session();connection.stage(&session,&package.manifest.id,&assets)?;
        work.staged()?;cancelled(cancel)?;
        Ok(Self{work,session,game:game.id,game_hash:game.content_hash.clone(),filename,primary_bytes,firmware})
    }
    pub fn session(&self)->&str{&self.session}
    pub fn launch_intent(&mut self)->Result<()>{self.work.launch_intent()}
    pub fn retire_confirmed(&mut self)->Result<()>{self.work.retire_confirmed()}
    pub fn filename(&self)->&str{&self.filename}
    pub fn primary_bytes(&self)->u64{self.primary_bytes}
    pub fn firmware_hashes(&self)->&BTreeMap<String,ContentHash>{&self.firmware}
    pub fn matches_game(&self,game:&GameManifest)->bool{self.game==game.id&&self.game_hash==game.content_hash}
}
