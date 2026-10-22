//! RuntimeAdapter composition for an explicitly admitted Linux profile context.
//! The default installed factory supplies no delegation and stays unavailable.
use crate::{linux_image_cache::ProfileImageCache,linux_package::PreparedLinuxPackage,linux_session::LinuxSession,linux_staging::StagedInputs,linux_work::LinuxWork,registry::RuntimePackage,unavailable};
use omniplay_contracts::*;
use omniplay_core::{Database,sessions::{RuntimeAdapter,RuntimeObservation}};
use omniplay_sandbox::linux_delegation::Delegation;
use omniplay_updater::package::PackageManifest;
use std::{fs::File,path::PathBuf,sync::{Arc,atomic::{AtomicBool,Ordering}},time::Duration};
use uuid::Uuid;

pub(crate) struct LinuxAdapter{root:PathBuf,package:RuntimePackage,declaration:RuntimePackage,inventory:PackageManifest,profile_root:PathBuf,profile:Uuid,delegation:Delegation,cache:ProfileImageCache,session:Option<LinuxSession>}
fn invalid()->PlatformError{unavailable("The Linux runtime adapter has no prepared owned session")}
static NEVER_CANCELLED:AtomicBool=AtomicBool::new(false);
impl LinuxAdapter{
    pub fn new(root:PathBuf,package:RuntimePackage,inventory:PackageManifest,profile_root:PathBuf,profile:Uuid,delegation:Delegation,cache:ProfileImageCache)->Self{Self{root,declaration:package.clone(),package,inventory,profile_root,profile,delegation,cache,session:None}}
    fn session(&mut self)->Result<&mut LinuxSession>{self.session.as_mut().ok_or_else(invalid)}
    fn prepare_inner(&mut self,game:&GameManifest,asset:File,config:&SessionConfig,cancel:Arc<AtomicBool>)->Result<()>{
        if self.session.is_some(){return Err(PlatformError::new(ErrorCode::InvalidState,"The Linux native adapter is already prepared"));}game.validate()?;config.validate()?;if cancel.load(Ordering::Acquire){return Err(PlatformError::new(ErrorCode::Cancelled,"Linux runtime preparation was cancelled"));}
        let prepared=PreparedLinuxPackage::admit(&self.root,&self.package.manifest.id,&self.inventory,self.cache.clone(),&cancel)?;
        // Compare all catalog fields, not just runtime ID: a factory-created
        // adapter cannot silently adopt a different declaration during prepare.
        if serde_json::to_value(&prepared.package).map_err(|_|invalid())?!=serde_json::to_value(&self.declaration).map_err(|_|invalid())?{return Err(invalid());}
        let database=Database::open(&self.profile_root)?;let input=omniplay_core::input::load(&database,game.id)?;input.validate()?;drop(database);
        let(bytes,files)=StagedInputs::reservation(&prepared.package,game)?;let work=LinuxWork::create(&self.profile_root,self.profile,bytes,files)?;let inputs=StagedInputs::prepare(&work,&prepared.package,game,asset,&cancel)?;
        let mut session=LinuxSession::launch(prepared,work,inputs,&self.delegation,config,input.haptics_enabled,cancel.clone())?;session.prepare_core(game,config,input,&cancel)?;self.package=session.package.clone();self.session=Some(session);Ok(())
    }
}
impl RuntimeAdapter for LinuxAdapter{
    fn manifest(&self)->&RuntimeManifest{&self.package.manifest}
    fn prepare(&mut self,game:&GameManifest,asset:File,config:&SessionConfig)->Result<()>{self.prepare_cancellable(game,asset,config,Arc::new(AtomicBool::new(false)))}
    fn prepare_cancellable(&mut self,game:&GameManifest,asset:File,config:&SessionConfig,cancel:Arc<AtomicBool>)->Result<()>{if self.session.is_some(){return Err(PlatformError::new(ErrorCode::InvalidState,"The Linux native adapter is already prepared"));}self.package=self.declaration.clone();let result=self.prepare_inner(game,asset,config,cancel);if result.is_err(){if let Some(mut session)=self.session.take(){session.stop_owned();}}result}
    fn start(&mut self)->Result<()>{self.session()?.start(&NEVER_CANCELLED)}
    fn pause(&mut self)->Result<()>{self.session()?.pause(&NEVER_CANCELLED)}
    fn resume(&mut self)->Result<()>{self.session()?.resume(&NEVER_CANCELLED)}
    fn stop(&mut self)->Result<()>{if let Some(mut session)=self.session.take(){session.stop(&NEVER_CANCELLED)}else{Ok(())}}
    fn save(&mut self,kind:SaveKind)->Result<Vec<u8>>{self.session()?.save(kind,&NEVER_CANCELLED)}
    fn load(&mut self,kind:SaveKind,payload:&[u8])->Result<()>{self.session()?.load(kind,payload,&NEVER_CANCELLED)}
    fn compatibility(&self,game:&GameManifest,config:&SessionConfig,kind:SaveKind)->Result<SaveCompatibility>{self.session.as_ref().ok_or_else(invalid)?.compatibility_for_config(game,config,kind)}
    fn metrics(&mut self,session:Uuid)->Result<Option<FrameMetrics>>{self.session()?.metrics(session,&NEVER_CANCELLED).map(Some)}
    fn observe(&mut self)->Result<RuntimeObservation>{match self.session.as_mut(){Some(session)=>session.observe(&NEVER_CANCELLED),None=>Ok(RuntimeObservation::default())}}
    fn cancel_close(&mut self)->Result<()>{self.session()?.call("cancel_close",serde_json::json!({}),Vec::new(),Duration::from_secs(5),&NEVER_CANCELLED).map(|_|())}
    fn disc_status(&mut self)->Result<DiscStatus>{self.session()?.disc_status(None,&NEVER_CANCELLED)}
    fn select_disc(&mut self,index:u32)->Result<DiscStatus>{self.session()?.disc_status(Some(index),&NEVER_CANCELLED)}
}
