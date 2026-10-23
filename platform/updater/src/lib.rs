#![forbid(unsafe_code)]

pub mod channel;
pub mod package;
pub mod store;
pub mod transport;

use omniplay_contracts::{ErrorCode,PlatformError,Result};
use std::path::PathBuf;

pub fn rejected(message:&str)->PlatformError {PlatformError::new(ErrorCode::UpdateRejected,message)}

pub fn install_root()->Result<PathBuf> {
    let base=if cfg!(windows){std::env::var_os("LOCALAPPDATA").map(PathBuf::from)}
        else if cfg!(target_os="macos"){std::env::var_os("HOME").map(|p|PathBuf::from(p).join("Library/Application Support"))}
        else{std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(||std::env::var_os("HOME").map(|p|PathBuf::from(p).join(".local/share")))};
    let base=base.filter(|path|path.is_absolute()).ok_or_else(||rejected("The user installation directory must be absolute"))?;
    Ok(base.join("OmniPlay").join("install"))
}

pub fn acknowledge_broker_health()->Result<()> {
    let Some(id)=std::env::var_os("OMNIPLAY_ACTIVE_RELEASE")else{return Ok(());};
    let id=id.to_str().and_then(|v|uuid::Uuid::parse_str(v).ok()).ok_or_else(||rejected("Active release context is invalid"))?;
    let store=store::Store::open(&install_root()?,channel::Channel::embedded()?)?;
    let release=store.active()?;
    if release.id!=id{return Err(rejected("The broker belongs to an inactive release"));}
    let expected=package::verify_entry(&store.version_directory(id),&release.manifest,&release.manifest.broker_entry)?;
    if std::env::current_exe().and_then(|p|p.canonicalize()).map_err(omniplay_core::util::storage_error)?!=expected{return Err(rejected("The running broker is not the installed entry point"));}
    store.acknowledge_health(id)
}
pub fn note_opened_profile_schema(schema:u32)->Result<()> {store::Store::open(&install_root()?,channel::Channel::embedded()?)?.note_opened_schema(schema)}
