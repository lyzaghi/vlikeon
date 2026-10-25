//! Durable, immutable work/leaf/launch acknowledgements for the lower-level
//! GNU/Linux pair. Not selected by the current product runtime factory.
use omniplay_contracts::{ContentHash,ErrorCode,PlatformError,Result};
use omniplay_core::util::{PendingFile,bounded_read,storage_error,sync_directory};
use omniplay_sandbox::{linux_delegation::{DelegationIdentity,GroupIdentity},linux_launch::{self,BootstrapSpawn,PresentedBootstrap,LinuxProcessIdentity},linux_presentation::PresentationSpawn,linux_sealed::SealedBudget};
use serde::{Deserialize,Serialize};
use std::{fs::{self,File,OpenOptions,DirBuilder},os::unix::fs::{DirBuilderExt,MetadataExt,OpenOptionsExt},path::{Path,PathBuf},sync::{Arc,atomic::AtomicBool}};
use uuid::Uuid;

#[derive(Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]#[serde(rename_all="snake_case")]enum Role{Actor,Presentation}
impl Role{fn group(self)->&'static str{match self{Self::Actor=>"linux-actor-group.json",Self::Presentation=>"linux-presentation-group.json"}}fn launch(self)->&'static str{match self{Self::Actor=>"linux-actor-launch-intent.json",Self::Presentation=>"linux-presentation-launch-intent.json"}}fn process(self)->&'static str{match self{Self::Actor=>"linux-actor-process.json",Self::Presentation=>"linux-presentation-process.json"}}}
#[derive(Clone,PartialEq,Eq,Serialize,Deserialize)]#[serde(deny_unknown_fields)]struct ParentRecord{profile:Uuid,device:u64,inode:u64}
#[derive(Clone,PartialEq,Eq,Serialize,Deserialize)]#[serde(deny_unknown_fields)]struct GroupRecord{parent:ParentRecord,work:Uuid,role:Role,device:u64,inode:u64,charged_memory:u64}
#[derive(Clone,PartialEq,Eq,Serialize,Deserialize)]#[serde(deny_unknown_fields)]struct Creator{process_id:u32,start_ticks:u64,boot_id:Uuid}
#[derive(PartialEq,Eq,Serialize,Deserialize)]#[serde(deny_unknown_fields)]struct Ownership{schema_version:u32,platform:String,profile:Uuid,work:Uuid,device:u64,inode:u64,#[serde(default,skip_serializing_if="Option::is_none")]creator:Option<Creator>}
#[derive(Serialize,Deserialize)]#[serde(deny_unknown_fields)]struct PairIntent{schema_version:u32,parent:ParentRecord,work:Uuid}
#[derive(Serialize,Deserialize)]#[serde(deny_unknown_fields)]struct GroupAcknowledgement{schema_version:u32,group:GroupRecord}
#[derive(Serialize,Deserialize)]#[serde(deny_unknown_fields)]struct LaunchIntent{schema_version:u32,group:GroupRecord,executable_sha256:ContentHash,executable_bytes:u64}
#[derive(Serialize,Deserialize)]#[serde(deny_unknown_fields)]struct ProcessAcknowledgement{schema_version:u32,group:GroupRecord,process_id:u32,start_ticks:u64,boot_id:Uuid}

pub(crate) struct LinuxWork{directory:PathBuf,profile:Uuid,id:Uuid,guard:File,_owner_lease:File,actor_lease:File,presentation_lease:File}
fn invalid()->PlatformError{PlatformError::new(ErrorCode::StorageFailure,"The private Linux session ownership record could not be established")}
fn uuid(raw:&str)->Result<Uuid>{let id=Uuid::parse_str(raw).map_err(|_|invalid())?;if id.is_nil()||id.to_string()!=raw{return Err(invalid());}Ok(id)}
fn parent(value:&DelegationIdentity)->Result<ParentRecord>{if value.inode==0{return Err(invalid());}Ok(ParentRecord{profile:uuid(&value.profile)?,device:value.device,inode:value.inode})}
fn group(value:&GroupIdentity)->Result<GroupRecord>{let parent=parent(&value.parent)?;if value.inode==0||value.inode==parent.inode||value.device!=parent.device||value.charged_memory==0||value.charged_memory>16*1024*1024*1024{return Err(invalid());}Ok(GroupRecord{parent,work:uuid(&value.session)?,role:if value.presentation{Role::Presentation}else{Role::Actor},device:value.device,inode:value.inode,charged_memory:value.charged_memory})}
fn ordinary(path:&Path)->Result<File>{linux_launch::private_record(path)}
fn same_file(left:&File,right:&File)->Result<bool>{let left=left.metadata().map_err(storage_error)?;let right=right.metadata().map_err(storage_error)?;Ok(left.dev()==right.dev()&&left.ino()==right.ino())}
fn lease(directory:&Path,name:&str)->Result<File>{let file=OpenOptions::new().read(true).write(true).create_new(true).mode(0o600).open(directory.join(name)).map_err(storage_error)?;file.try_lock().map_err(|_|invalid())?;file.sync_all().map_err(storage_error)?;Ok(file)}
fn create_directory(directory:&Path)->Result<()>{DirBuilder::new().mode(0o700).create(directory).map_err(storage_error)}
fn publish(directory:&Path,name:&str,value:&impl Serialize)->Result<()>{let raw=serde_json::to_vec(value).map_err(storage_error)?;if raw.len()>4096{return Err(invalid());}let mut pending=PendingFile::create(directory)?;pending.write_all(&raw)?;pending.commit_new(&directory.join(name))}
impl LinuxWork{
    pub fn create(profile_root:&Path,profile:Uuid,staging_bytes:u64,staging_files:u32)->Result<Self>{
        if profile.is_nil(){return Err(invalid());}let profile_root=profile_root.canonicalize().map_err(storage_error)?;let root=profile_root.join("native-work");
        match create_directory(&root){Ok(())=>sync_directory(&profile_root)?,Err(error)=>{if !root.try_exists().map_err(storage_error)?{return Err(error);}}}
        let parent=linux_launch::private_directory(&root)?;let id=Uuid::new_v4();let directory=root.join(id.to_string());let mut database=omniplay_core::Database::open(&profile_root)?;omniplay_core::native_work_storage::reserve(&mut database,profile,id,staging_bytes,staging_files)?;
        if let Err(error)=create_directory(&directory){let _=omniplay_core::native_work_storage::release_uncreated(&database,profile,id);return Err(error);}sync_directory(&root)?;let guard=linux_launch::private_directory(&directory)?;
        let shape=guard.metadata().map_err(storage_error)?;let creator=linux_launch::current_broker_identity()?;let ownership=Ownership{schema_version:2,platform:"linux-x86_64-gnu".into(),profile,work:id,device:shape.dev(),inode:shape.ino(),creator:Some(Creator{process_id:creator.process_id,start_ticks:creator.start_ticks,boot_id:uuid(&creator.boot_id)?})};publish(&directory,"linux-work-intent.json",&ownership)?;
        let owner=lease(&directory,"owner.lease")?;let actor_lease=lease(&directory,"actor.lease")?;let presentation_lease=lease(&directory,"presentation.lease")?;
        create_directory(&directory.join("root"))?;create_directory(&directory.join("presentation-tmp"))?;sync_directory(&directory)?;drop(parent);
        let result=Self{directory,profile,id,guard,_owner_lease:owner,actor_lease,presentation_lease};result.record("linux-work-owned.json",&ownership)?;Ok(result)
    }
    fn validate(&self)->Result<()>{
        let selected=linux_launch::private_directory(&self.directory)?;if self.directory.file_name().and_then(|value|value.to_str())!=Some(self.id.to_string().as_str())||!same_file(&selected,&self.guard)?{return Err(invalid());}Ok(())
    }
    fn record(&self,name:&str,value:&impl Serialize)->Result<()>{self.validate()?;publish(&self.directory,name,value)}
    fn read<T:for<'a>Deserialize<'a>>(&self,name:&str)->Result<T>{self.validate()?;let mut file=ordinary(&self.directory.join(name))?;let owner=self.guard.metadata().map_err(storage_error)?;if file.metadata().map_err(storage_error)?.uid()!=owner.uid(){return Err(invalid());}serde_json::from_slice(&bounded_read(&mut file,4096)?).map_err(storage_error)}
    pub fn session(&self)->String{self.id.to_string()}
    pub fn id(&self)->Uuid{self.id}
    pub fn profile_id(&self)->Uuid{self.profile}
    pub fn profile_root(&self)->Result<&Path>{self.directory.parent().and_then(Path::parent).ok_or_else(invalid)}
    pub fn check(&self)->Result<()>{self.validate()}
    pub fn directory(&self)->&Path{&self.directory}
    pub fn actor_lease(&self)->Result<File>{self.actor_lease.try_clone().map_err(storage_error)}
    pub fn presentation_lease(&self)->Result<File>{self.presentation_lease.try_clone().map_err(storage_error)}
    pub fn root_directory(&self)->Result<File>{self.validate()?;linux_launch::private_directory(&self.directory.join("root"))}
    pub fn presentation_directory(&self)->Result<(File,PathBuf)>{self.validate()?;let path=self.directory.join("presentation-tmp");Ok((linux_launch::private_directory(&path)?,path))}
    pub fn input_manifest(&self,value:&impl Serialize)->Result<ContentHash>{self.validate()?;let raw=serde_json::to_vec(value).map_err(storage_error)?;if raw.len()>64*1024{return Err(invalid());}let hash=omniplay_core::util::digest(&raw);let mut pending=PendingFile::create(&self.directory)?;pending.write_all(&raw)?;pending.commit_new(&self.directory.join("linux-inputs.json"))?;Ok(hash)}
    pub fn read_input_manifest(&self)->Result<Vec<u8>>{self.validate()?;bounded_read(&mut ordinary(&self.directory.join("linux-inputs.json"))?,64*1024)}
    pub fn pair_intent(&self,value:&DelegationIdentity,session:&str)->Result<()>{let parent=parent(value)?;if parent.profile!=self.profile||uuid(session)?!=self.id{return Err(invalid());}self.record("linux-pair-intent.json",&PairIntent{schema_version:1,parent,work:self.id})}
    pub fn group_created(&self,value:&GroupIdentity)->Result<()>{let group=group(value)?;let intent:PairIntent=self.read("linux-pair-intent.json")?;if intent.schema_version!=1||intent.work!=self.id||group.work!=self.id||intent.parent!=group.parent||group.parent.profile!=self.profile{return Err(invalid());}self.record(group.role.group(),&GroupAcknowledgement{schema_version:1,group})}
    fn admitted_group(&self,value:&GroupIdentity,role:Role)->Result<GroupRecord>{let group=group(value)?;let observed:GroupAcknowledgement=self.read(role.group())?;if observed.schema_version!=1||group!=observed.group||group.work!=self.id||group.parent.profile!=self.profile||group.role!=role{return Err(invalid());}Ok(group)}
    fn launch_intent(&self,group:GroupRecord,hash:&ContentHash,bytes:u64)->Result<()>{ContentHash::parse(hash.0.clone())?;if bytes==0||bytes>256*1024*1024{return Err(invalid());}self.record(group.role.launch(),&LaunchIntent{schema_version:1,group,executable_sha256:hash.clone(),executable_bytes:bytes})}
    fn observed(&self,role:Role,value:&LinuxProcessIdentity)->Result<()>{
        let intent:LaunchIntent=self.read(role.launch())?;let acknowledged:GroupAcknowledgement=self.read(role.group())?;ContentHash::parse(intent.executable_sha256.0.clone())?;if intent.schema_version!=1||acknowledged.schema_version!=1||intent.executable_bytes==0||intent.executable_bytes>256*1024*1024||intent.group!=acknowledged.group||intent.group.work!=self.id||intent.group.parent.profile!=self.profile||intent.group.role!=role||!(2..=i32::MAX as u32).contains(&value.process_id)||value.start_ticks==0{return Err(invalid());}
        self.record(role.process(),&ProcessAcknowledgement{schema_version:1,group:intent.group,process_id:value.process_id,start_ticks:value.start_ticks,boot_id:uuid(&value.boot_id)?})
    }
    /// Every launch intent is durable before the first child; each observed
    /// identity is committed by spawn_presented before its private start grant.
    pub fn launch_pair(&self,mut actor:BootstrapSpawn,helper:PresentationSpawn,inputs:&crate::linux_staging::StagedInputs,budget:&SealedBudget,cancel:Arc<AtomicBool>)->Result<PresentedBootstrap>{
        self.validate()?;if uuid(&actor.session)?!=self.id||uuid(&helper.session)?!=self.id||!same_file(&actor.work_lease,&self.actor_lease)?||!same_file(&helper.work_lease,&self.presentation_lease)?||!same_file(&actor.root_directory,&self.root_directory()?)?{return Err(invalid());}
        let(temporary,path)=self.presentation_directory()?;if !same_file(&helper.temporary_directory,&temporary)?||helper.temporary_path!=path{return Err(invalid());}
        inputs.attach(self,&mut actor)?;
        let actor_group=self.admitted_group(actor.resource_group.managed_identity().ok_or_else(invalid)?,Role::Actor)?;let helper_group=self.admitted_group(helper.resource_group.managed_identity().ok_or_else(invalid)?,Role::Presentation)?;
        self.launch_intent(actor_group,actor.executable.sha256(),actor.executable.bytes())?;self.launch_intent(helper_group,helper.executable.sha256(),helper.executable.bytes())?;
        linux_launch::spawn_presented(actor,helper,budget,cancel,|value|self.observed(Role::Actor,value),|value|self.observed(Role::Presentation,value))
    }
}

#[derive(Default)]pub struct LinuxRecoverySummary{pub inspected:u32,pub removed:u32,pub retained:u32}
fn exists(path:&Path)->Result<bool>{match fs::symlink_metadata(path){Ok(_)=>Ok(true),Err(error)if error.kind()==std::io::ErrorKind::NotFound=>Ok(false),Err(error)=>Err(storage_error(error))}}
fn recovery_read<T:for<'a>Deserialize<'a>>(work:&Path,name:&str)->Result<T>{serde_json::from_slice(&bounded_read(&mut ordinary(&work.join(name))?,4096)?).map_err(storage_error)}
fn recovery_group(record:&GroupRecord,profile:Uuid,work:Uuid,role:Role,parent:&DelegationIdentity)->Result<GroupIdentity>{
    if record.work!=work||record.parent.profile!=profile||record.role!=role{return Err(invalid());}let value=GroupIdentity{parent:DelegationIdentity{profile:record.parent.profile.to_string(),device:record.parent.device,inode:record.parent.inode},session:work.to_string(),presentation:role==Role::Presentation,device:record.device,inode:record.inode,charged_memory:record.charged_memory};if value.parent!=*parent||group(&value)?!=*record{return Err(invalid());}Ok(value)
}
fn recovery_process(work:&Path,group:&GroupRecord,owned:&Ownership)->Result<(Option<ProcessAcknowledgement>,bool)>{
    let intent=exists(&work.join(group.role.launch()))?;let observed=exists(&work.join(group.role.process()))?;if !intent&&!observed{return Ok((None,false));}if !intent{return Err(invalid());}
    let intent:LaunchIntent=recovery_read(work,group.role.launch())?;ContentHash::parse(intent.executable_sha256.0.clone())?;if intent.schema_version!=1||intent.group!=*group||intent.executable_bytes<64||intent.executable_bytes>256*1024*1024{return Err(invalid());}
    if !observed{creator_ended(owned)?;return Ok((None,true));}let observed:ProcessAcknowledgement=recovery_read(work,group.role.process())?;
    if observed.schema_version!=1||observed.group!=*group||observed.boot_id.is_nil(){return Err(invalid());}
    linux_launch::confirmed_process_ended(&LinuxProcessIdentity{process_id:observed.process_id,start_ticks:observed.start_ticks,boot_id:observed.boot_id.to_string()})?;Ok((Some(observed),false))
}
fn ownership_shape(owned:&Ownership)->Result<()>{match(owned.schema_version,&owned.creator){(1,None)=>Ok(()),(2,Some(creator))if(2..=i32::MAX as u32).contains(&creator.process_id)&&creator.start_ticks>0&&!creator.boot_id.is_nil()=>Ok(()),_=>Err(invalid())}}
fn creator_ended(owned:&Ownership)->Result<()>{ownership_shape(owned)?;if owned.schema_version!=2{return Err(invalid());}let creator=owned.creator.as_ref().ok_or_else(invalid)?;linux_launch::confirmed_process_ended(&LinuxProcessIdentity{process_id:creator.process_id,start_ticks:creator.start_ticks,boot_id:creator.boot_id.to_string()})}
#[derive(Serialize,Deserialize)]#[serde(deny_unknown_fields)]struct RetirementProof{schema_version:u32,owned:Ownership,actor:GroupRecord,presentation:GroupRecord,actor_process:Option<ProcessAcknowledgement>,presentation_process:Option<ProcessAcknowledgement>,#[serde(default)]actor_unacknowledged:bool,#[serde(default)]presentation_unacknowledged:bool}
fn retirement_root(profile_root:&Path)->Result<PathBuf>{let root=profile_root.join("native-retirements");if !exists(&root)?{create_directory(&root)?;sync_directory(profile_root)?;}let _directory=linux_launch::private_directory(&root)?;Ok(root)}
fn receipt_process(group:&GroupRecord,observed:&Option<ProcessAcknowledgement>,unacknowledged:bool,owned:&Ownership)->Result<()>{if unacknowledged{if observed.is_some(){return Err(invalid());}return creator_ended(owned);}if let Some(observed)=observed{if observed.schema_version!=1||observed.group!=*group||observed.boot_id.is_nil(){return Err(invalid());}linux_launch::confirmed_process_ended(&LinuxProcessIdentity{process_id:observed.process_id,start_ticks:observed.start_ticks,boot_id:observed.boot_id.to_string()})?;}Ok(())}
fn finish_retirement(profile_root:&Path,profile:Uuid,name:&str,resource:&omniplay_sandbox::linux_delegation::RecoveryDelegation)->Result<()>{
    let id=uuid(name.strip_suffix(".json").ok_or_else(invalid)?)?;let root=retirement_root(profile_root)?;let proof:RetirementProof=recovery_read(&root,name)?;let owned=&proof.owned;
    ownership_shape(owned)?;if proof.schema_version!=1||owned.platform!="linux-x86_64-gnu"||owned.profile!=profile||owned.work!=id||owned.inode==0{return Err(invalid());}let actor=recovery_group(&proof.actor,profile,id,Role::Actor,resource.identity())?;let presentation=recovery_group(&proof.presentation,profile,id,Role::Presentation,resource.identity())?;if actor.inode==presentation.inode{return Err(invalid());}
    receipt_process(&proof.actor,&proof.actor_process,proof.actor_unacknowledged,owned)?;receipt_process(&proof.presentation,&proof.presentation_process,proof.presentation_unacknowledged,owned)?;resource.confirm_empty(&actor)?;resource.confirm_empty(&presentation)?;resource.remove_empty(&actor)?;resource.remove_empty(&presentation)?;if !resource.charges_retired()?{return Err(invalid());}
    let work=profile_root.join("native-work").join(id.to_string());let mut leases=Vec::new();let mut held=None;
    if exists(&work)?{let directory=linux_launch::private_directory(&work)?;let metadata=directory.metadata().map_err(storage_error)?;if metadata.dev()!=owned.device||metadata.ino()!=owned.inode{return Err(invalid());}for name in["owner.lease","actor.lease","presentation.lease"]{if exists(&work.join(name))?{let lease=ordinary(&work.join(name))?;lease.try_lock().map_err(|_|invalid())?;leases.push(lease);}}held=Some(directory);}
    let database=omniplay_core::Database::open(profile_root)?;omniplay_core::native_work_storage::retiring(&database,profile,id)?;
    if let Some(directory)=held{let current=linux_launch::private_directory(&work)?;if !same_file(&directory,&current)?{return Err(invalid());}crate::work::remove(profile_root,&work)?;sync_directory(&profile_root.join("native-work"))?;}
    omniplay_core::native_work_storage::release_removed(&database,profile,id)?;fs::remove_file(root.join(name)).map_err(storage_error)?;sync_directory(&root)?;drop(leases);Ok(())
}
fn retirement_record(work:&Path,group:&GroupRecord)->Result<()>{
    let name=match group.role{Role::Actor=>"linux-actor-retire-intent.json",Role::Presentation=>"linux-presentation-retire-intent.json"};if exists(&work.join(name))?{let previous:GroupAcknowledgement=recovery_read(work,name)?;if previous.schema_version!=1||previous.group!=*group{return Err(invalid());}Ok(())}else{publish(work,name,&GroupAcknowledgement{schema_version:1,group:group.clone()})}
}
fn recover_partial_creation(profile_root:&Path,profile:Uuid,id:Uuid,path:&Path,directory:&File,shape:&fs::Metadata,resource:&omniplay_sandbox::linux_delegation::RecoveryDelegation)->Result<()>{
    let intent:Ownership=recovery_read(path,"linux-work-intent.json")?;
    if intent.platform!="linux-x86_64-gnu"||intent.profile!=profile||intent.work!=id||intent.device!=shape.dev()||intent.inode!=shape.ino(){return Err(invalid());}
    creator_ended(&intent)?;
    if exists(&path.join("linux-work-owned.json"))?{
        let owned:Ownership=recovery_read(path,"linux-work-owned.json")?;
        if owned!=intent{return Err(invalid());}
    }
    let mut leases=Vec::new();
    for name in["owner.lease","actor.lease","presentation.lease"]{
        if exists(&path.join(name))?{
            let lease=ordinary(&path.join(name))?;
            lease.try_lock().map_err(|_|invalid())?;
            leases.push(lease);
        }
    }
    for role in[Role::Actor,Role::Presentation]{
        if exists(&path.join(role.launch()))?||exists(&path.join(role.process()))?{return Err(invalid());}
    }
    if exists(&path.join("linux-pair-intent.json"))?{
        let pair:PairIntent=recovery_read(path,"linux-pair-intent.json")?;
        if pair.schema_version!=1||pair.work!=id||pair.parent!=parent(resource.identity())?{return Err(invalid());}
    }
    let session=id.to_string();
    for(role,presentation)in[(Role::Actor,false),(Role::Presentation,true)]{
        if exists(&path.join(role.group()))?{
            let group_ack:GroupAcknowledgement=recovery_read(path,role.group())?;
            if group_ack.schema_version!=1{return Err(invalid());}
            let identity=recovery_group(&group_ack.group,profile,id,role,resource.identity())?;
            resource.confirm_empty(&identity)?;
            retirement_record(path,&group_ack.group)?;
            resource.remove_empty(&identity)?;
        }else{
            resource.remove_unacknowledged_empty_leaf(&session,presentation)?;
        }
    }
    if !resource.charges_retired()?{return Err(invalid());}
    let current=linux_launch::private_directory(path)?;if !same_file(directory,&current)?{return Err(invalid());}
    let database=omniplay_core::Database::open(profile_root)?;
    omniplay_core::native_work_storage::retiring(&database,profile,id)?;
    crate::work::remove(profile_root,path)?;sync_directory(&profile_root.join("native-work"))?;
    omniplay_core::native_work_storage::release_removed(&database,profile,id)?;
    drop(leases);Ok(())
}
fn recover_one(profile_root:&Path,profile:Uuid,path:&Path,resource:&omniplay_sandbox::linux_delegation::RecoveryDelegation)->Result<()>{
    let id=uuid(path.file_name().and_then(|value|value.to_str()).ok_or_else(invalid)?)?;let directory=linux_launch::private_directory(path)?;let shape=directory.metadata().map_err(storage_error)?;
    if !exists(&path.join("linux-work-owned.json"))?||!exists(&path.join("linux-pair-intent.json"))?||!exists(&path.join(Role::Actor.group()))?||!exists(&path.join(Role::Presentation.group()))?{
        return recover_partial_creation(profile_root,profile,id,path,&directory,&shape,resource);
    }
    let owned:Ownership=recovery_read(path,"linux-work-owned.json")?;let intent:Ownership=recovery_read(path,"linux-work-intent.json")?;
    if owned!=intent{return Err(invalid());}for record in[&owned,&intent]{ownership_shape(record)?;if record.platform!="linux-x86_64-gnu"||record.profile!=profile||record.work!=id||record.device!=shape.dev()||record.inode!=shape.ino(){return Err(invalid());}}
    // Independently reopened descriptions must acquire all three locks; cloned
    // descriptions from a live session cannot be mistaken for abandoned work.
    let leases=[ordinary(&path.join("owner.lease"))?,ordinary(&path.join("actor.lease"))?,ordinary(&path.join("presentation.lease"))?];for lease in &leases{lease.try_lock().map_err(|_|invalid())?;}
    let pair:PairIntent=recovery_read(path,"linux-pair-intent.json")?;if pair.schema_version!=1||pair.work!=id||pair.parent!=parent(resource.identity())?{return Err(invalid());}
    let actor:GroupAcknowledgement=recovery_read(path,Role::Actor.group())?;let presentation:GroupAcknowledgement=recovery_read(path,Role::Presentation.group())?;if actor.schema_version!=1||presentation.schema_version!=1{return Err(invalid());}
    let actor_identity=recovery_group(&actor.group,profile,id,Role::Actor,resource.identity())?;let presentation_identity=recovery_group(&presentation.group,profile,id,Role::Presentation,resource.identity())?;if actor_identity.inode==presentation_identity.inode{return Err(invalid());}
    let(actor_process,actor_unacknowledged)=recovery_process(path,&actor.group,&owned)?;let(presentation_process,presentation_unacknowledged)=recovery_process(path,&presentation.group,&owned)?;resource.confirm_empty(&actor_identity)?;resource.confirm_empty(&presentation_identity)?;
    retirement_record(path,&actor.group)?;retirement_record(path,&presentation.group)?;resource.remove_empty(&actor_identity)?;resource.remove_empty(&presentation_identity)?;
    if !resource.charges_retired()?{return Err(invalid());}
    let current=linux_launch::private_directory(path)?;if !same_file(&directory,&current)?{return Err(invalid());}
    // Persist the complete proven retirement outside the tree to be erased.
    // A crash can then resume even after inner ownership/lease files disappear.
    let receipts=retirement_root(profile_root)?;let name=format!("{id}.json");if !exists(&receipts.join(&name))?{publish(&receipts,&name,&RetirementProof{schema_version:1,owned,actor:actor.group,presentation:presentation.group,actor_process,presentation_process,actor_unacknowledged,presentation_unacknowledged})?;}
    drop(leases);finish_retirement(profile_root,profile,&name,resource)
}
/// Explicit trusted-host recovery before new delegation admission. Unknown or
/// incomplete ownership/launch records stay on disk and count as retained work.
pub fn recover_linux_work(profile_root:&Path,profile:Uuid,resource:&omniplay_sandbox::linux_delegation::RecoveryDelegation)->Result<LinuxRecoverySummary>{
    #[derive(Default,Serialize,Deserialize)]#[serde(deny_unknown_fields)]struct Cursor{receipt:String,work:String}
    let _audit=resource.begin_audit()?;if profile.is_nil()||resource.identity().profile!=profile.to_string(){return Err(invalid());}let profile_root=profile_root.canonicalize().map_err(storage_error)?;let _profile=linux_launch::private_directory(&profile_root)?;let root=profile_root.join("native-work");let mut summary=LinuxRecoverySummary::default();let receipts=retirement_root(&profile_root)?;let mut resumed=std::collections::BTreeSet::new();let database=omniplay_core::Database::open(&profile_root)?;
    let saved=omniplay_core::native_work_storage::linux_recovery_cursor(&database)?;let mut cursor:Cursor=match saved{Some(value)=>serde_json::from_str(&value).map_err(storage_error)?,None=>Cursor::default()};if cursor.receipt.len()>768||cursor.work.len()>768{return Err(invalid());}
    for name in recovery_candidates(&receipts,&cursor.receipt)?{summary.inspected+=1;cursor.receipt=name.clone();if let Some(id)=name.strip_suffix(".json").and_then(|value|uuid(value).ok()){resumed.insert(id);}if finish_retirement(&profile_root,profile,&name,resource).is_ok(){summary.removed+=1;}else{summary.retained+=1;}}
    if exists(&root)?{let _root=linux_launch::private_directory(&root)?;for name in recovery_candidates(&root,&cursor.work)?{cursor.work=name.clone();if uuid(&name).is_ok_and(|id|resumed.contains(&id)){continue;}summary.inspected+=1;if recover_one(&profile_root,profile,&root.join(name),resource).is_ok(){summary.removed+=1;}else{summary.retained+=1;}}}
    for work_id in omniplay_core::native_work_storage::recovery_candidates(&database,profile)?{
        if resumed.contains(&work_id)||exists(&root.join(work_id.to_string()))?{continue;}
        summary.inspected+=1;
        let released=match omniplay_core::native_work_storage::recovery_reservation(&database,profile,work_id)?{
            Some(res)if res.phase==omniplay_core::native_work_storage::RecoveryPhase::Reserved=>omniplay_core::native_work_storage::release_uncreated(&database,profile,work_id),
            Some(res)if res.phase==omniplay_core::native_work_storage::RecoveryPhase::Retiring=>omniplay_core::native_work_storage::release_removed(&database,profile,work_id),
            _=>Err(invalid()),
        };
        if released.is_ok(){summary.removed+=1;}else{summary.retained+=1;}
    }
    let encoded=serde_json::to_string(&cursor).map_err(storage_error)?;omniplay_core::native_work_storage::set_linux_recovery_cursor(&database,&encoded)?;
    Ok(summary)
}
fn recovery_candidates(root:&Path,after:&str)->Result<Vec<String>>{
    let mut names=Vec::new();for entry in fs::read_dir(root).map_err(storage_error)?.take(257){if names.len()>=256{return Err(invalid());}let name=entry.map_err(storage_error)?.file_name().to_string_lossy().into_owned();if name.len()>768{return Err(invalid());}names.push(name);}names.sort();let at=names.partition_point(|name|name.as_str()<=after);names.rotate_left(at);names.truncate(32);Ok(names)
}
