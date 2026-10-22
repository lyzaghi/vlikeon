//! Private native media: verify one bounded set off the control thread, then
//! grant positional reads by owner/set/index. No ROM copy or HTTP route.
use base64::{Engine,engine::general_purpose::STANDARD};
use omniplay_contracts::*;
use omniplay_core::{Database,library::Library,util::storage_error};
use serde::Serialize;
use sha2::{Digest,Sha256};
use std::{collections::{HashMap,VecDeque},fs::{File,Metadata,OpenOptions},io::{Read,Seek,SeekFrom},path::PathBuf,sync::{Arc,Mutex,mpsc::{self,SyncSender,TrySendError}},sync::atomic::{AtomicBool,AtomicUsize,Ordering},time::{Duration,Instant,SystemTime}};
use uuid::Uuid;

const READ_LIMIT:u32=64*1024;
const LEASE:Duration=Duration::from_secs(120);
fn failure(code:ErrorCode,message:&str)->PlatformError{PlatformError::new(code,message)}
fn cancelled()->PlatformError{failure(ErrorCode::Cancelled,"Native media access was cancelled")}
fn missing()->PlatformError{failure(ErrorCode::PermissionExpired,"Native media permission expired; reopen its verified set")}

#[derive(Clone,Serialize)]pub struct RangeAsset{pub index:u16,pub name:String,pub sha256:ContentHash,pub byte_length:u64}
#[derive(Clone,Serialize)]pub struct RangeSetInfo{pub set_id:Uuid,pub game_id:Uuid,pub game_content_hash:ContentHash,pub byte_length:u64,pub assets:Vec<RangeAsset>,pub read_limit:u32,pub lease_ms:u64}
#[derive(Clone,Serialize)]pub struct RangeJob{pub id:Uuid,pub game_id:Uuid,pub state:String,pub bytes_hashed:u64,pub total_bytes:u64,pub asset_index:u16,pub asset_count:u16,pub result_available:bool,pub error:Option<PlatformError>}
#[derive(Serialize)]pub struct RangeChunk{pub set_id:Uuid,pub asset_index:u16,pub offset:u64,pub payload_base64:String,pub finished:bool}
#[derive(Clone,PartialEq,Eq)]struct Stamp{
    length:u64,modified:Option<SystemTime>,
    #[cfg(unix)]unix:[u64;6],
    #[cfg(windows)]windows:[u64;3],
}
impl Stamp{fn of(meta:&Metadata)->Result<Self>{
    if !meta.is_file()||meta.file_type().is_symlink(){return Err(failure(ErrorCode::PermissionDenied,"Native media must be an ordinary immutable asset"));}
    #[cfg(unix)]let unix={use std::os::unix::fs::MetadataExt;[meta.dev(),meta.ino(),meta.mtime()as u64,meta.mtime_nsec()as u64,meta.ctime()as u64,meta.ctime_nsec()as u64]};
    #[cfg(windows)]let windows={use std::os::windows::fs::MetadataExt;if meta.file_attributes()&0x400!=0{return Err(failure(ErrorCode::PermissionDenied,"Native media may not be a reparse point"));}[meta.creation_time(),meta.last_write_time(),meta.file_attributes()as u64]};
    Ok(Self{length:meta.len(),modified:meta.modified().ok(),#[cfg(unix)]unix,#[cfg(windows)]windows})
}}
struct Reader{file:Mutex<File>,stamp:Stamp,cancel:Arc<AtomicBool>}
struct Set{owner:Uuid,view:RangeSetInfo,readers:Vec<Arc<Reader>>,expires:Instant}
impl Set{fn cancel(&self){for reader in &self.readers{reader.cancel.store(true,Ordering::Release);}}}
struct Job{owner:Uuid,view:RangeJob,cancel:Arc<AtomicBool>,ready:Option<Set>,expires:Option<Instant>}
struct Spec{path:PathBuf,asset:RangeAsset}
struct Plan{owner:Uuid,job_id:Uuid,view:RangeSetInfo,files:Vec<Spec>,cancel:Arc<AtomicBool>}
enum Work{Prepare(Plan),Retire(Set)}
struct State{job:Option<Job>,set:Option<Set>,retired:Vec<Set>,requests:HashMap<(Uuid,Uuid),[u8;32]>,order:VecDeque<(Uuid,Uuid)>}
impl Default for State{fn default()->Self{Self{job:None,set:None,retired:Vec::new(),requests:HashMap::new(),order:VecDeque::new()}}}
pub struct AssetRanges{state:Mutex<State>,sender:Mutex<Option<SyncSender<Work>>>,io:AtomicUsize,reading:AtomicBool}
impl Default for AssetRanges{fn default()->Self{Self{state:Mutex::new(State::default()),sender:Mutex::new(None),io:AtomicUsize::new(0),reading:AtomicBool::new(false)}}}
impl AssetRanges{
    fn sender(self:&Arc<Self>)->Result<SyncSender<Work>>{
        let mut sender=self.sender.lock().map_err(|_|cancelled())?;if let Some(sender)=sender.as_ref(){return Ok(sender.clone());}
        let(tx,rx)=mpsc::sync_channel::<Work>(2);let weak=Arc::downgrade(self);
        std::thread::Builder::new().name("omniplay-media-files".into()).spawn(move||loop{
            match rx.recv_timeout(Duration::from_secs(1)){
                Ok(work)=>{let Some(service)=weak.upgrade()else{drop(work);break;};match work{Work::Prepare(plan)=>service.verify(plan),Work::Retire(set)=>{set.cancel();drop(set);}}service.io.fetch_sub(1,Ordering::AcqRel);}
                Err(mpsc::RecvTimeoutError::Timeout)=>{let Some(service)=weak.upgrade()else{break;};service.io.fetch_add(1,Ordering::AcqRel);let retired=service.take_expired();for set in retired{set.cancel();drop(set);}service.io.fetch_sub(1,Ordering::AcqRel);}
                Err(mpsc::RecvTimeoutError::Disconnected)=>break,
            }
        }).map_err(|_|failure(ErrorCode::ResourceLimit,"The native media worker could not start"))?;
        *sender=Some(tx.clone());Ok(tx)
    }
    fn queue_retire(self:&Arc<Self>,set:Set){
        set.cancel();let sender=self.sender.lock().ok().and_then(|value|value.clone());
        if let Some(sender)=sender{self.io.fetch_add(1,Ordering::AcqRel);match sender.try_send(Work::Retire(set)){Ok(())=>return,Err(TrySendError::Full(Work::Retire(set))|TrySendError::Disconnected(Work::Retire(set)))=>{self.io.fetch_sub(1,Ordering::AcqRel);self.state.lock().unwrap_or_else(|error|error.into_inner()).retired.push(set);return;},Err(_)=>unreachable!()}}
        self.state.lock().unwrap_or_else(|error|error.into_inner()).retired.push(set);
    }
    fn take_expired(&self)->Vec<Set>{
        let mut state=self.state.lock().unwrap_or_else(|error|error.into_inner());let mut retired=std::mem::take(&mut state.retired);let now=Instant::now();
        if state.set.as_ref().is_some_and(|set|set.expires<=now){if let Some(set)=state.set.take(){retired.push(set);}}
        if let Some(job)=state.job.as_mut(){if job.expires.is_some_and(|expires|expires<=now){if let Some(set)=job.ready.take(){retired.push(set);}job.view.result_available=false;job.view.state="expired".into();job.expires=None;}}
        retired
    }
    pub fn expire(self:&Arc<Self>){for set in self.take_expired(){self.queue_retire(set);}}
    pub fn busy(&self)->bool{self.io.load(Ordering::Acquire)!=0||self.state.lock().map(|state|!state.retired.is_empty()).unwrap_or(true)}
    pub fn start(self:&Arc<Self>,database:&Database,owner:Uuid,game_id:Uuid)->Result<RangeJob>{
        self.expire();let library=Library::new(database);let game=library.get(game_id)?;game.validate()?;
        if matches!(game.system,GameSystem::Pc|GameSystem::Web){return Err(failure(ErrorCode::UnsupportedCapability,"Only imported emulator media can be read through this capability"));}
        let assets=if let Some(media)=&game.media{media.assets.clone()}else{vec![MediaAsset{name:format!("game.{}",ContentFormat::RawRom.extension(game.system)?),content_hash:game.content_hash.clone(),byte_length:game.byte_length}]};
        let mut files=Vec::with_capacity(assets.len());let mut exposed=Vec::with_capacity(assets.len());
        for(index,asset)in assets.iter().enumerate(){let(path,hash,length)=library.asset_details(&game,if game.media.is_some(){Some(asset.name.as_str())}else{None})?;let view=RangeAsset{index:index as u16,name:asset.name.clone(),sha256:hash,byte_length:length};files.push(Spec{path,asset:view.clone()});exposed.push(view);}
        let sender=self.sender()?;let mut state=self.state.lock().map_err(|_|cancelled())?;
        if self.io.load(Ordering::Acquire)!=0||state.set.is_some()||!state.retired.is_empty()||state.job.as_ref().is_some_and(|job|job.ready.is_some()||job.view.state=="hashing") {return Err(failure(ErrorCode::Busy,"Close or finish the current native media set first"));}
        let view=RangeJob{id:Uuid::new_v4(),game_id,state:"hashing".into(),bytes_hashed:0,total_bytes:game.byte_length,asset_index:0,asset_count:files.len()as u16,result_available:false,error:None};let cancel=Arc::new(AtomicBool::new(false));
        let plan=Plan{owner,job_id:view.id,view:RangeSetInfo{set_id:Uuid::new_v4(),game_id,game_content_hash:game.content_hash,byte_length:game.byte_length,assets:exposed,read_limit:READ_LIMIT,lease_ms:120000},files,cancel:cancel.clone()};
        state.job=Some(Job{owner,view:view.clone(),cancel,ready:None,expires:None});self.io.fetch_add(1,Ordering::AcqRel);
        if sender.try_send(Work::Prepare(plan)).is_err(){self.io.fetch_sub(1,Ordering::AcqRel);state.job=None;return Err(failure(ErrorCode::Busy,"Native media work could not be admitted"));}Ok(view)
    }
    fn verify(&self,plan:Plan){
        let started=Instant::now();let mut hashed=0u64;
        let checked=(||{
            let mut readers=Vec::with_capacity(plan.files.len());let mut block=[0u8;64*1024];
            for(index,spec)in plan.files.iter().enumerate(){
                if plan.cancel.load(Ordering::Acquire){return Err(cancelled());}
                let named=std::fs::symlink_metadata(&spec.path).map_err(storage_error)?;let _=Stamp::of(&named)?;
                let mut options=OpenOptions::new();options.read(true);#[cfg(windows)]{use std::os::windows::fs::OpenOptionsExt;options.share_mode(1);}
                let mut file=options.open(&spec.path).map_err(storage_error)?;let stamp=Stamp::of(&file.metadata().map_err(storage_error)?)?;
                if stamp!=Stamp::of(&named)?||stamp.length!=spec.asset.byte_length||stamp.length==0||stamp.length>MAX_ROM_BYTES{return Err(failure(ErrorCode::ContentMismatch,"Native asset identity or size changed before verification"));}
                let mut digest=Sha256::new();let mut length=0u64;
                loop{
                    if plan.cancel.load(Ordering::Acquire){return Err(cancelled());}if started.elapsed()>Duration::from_secs(1800){return Err(failure(ErrorCode::DeadlineExceeded,"Native media verification exceeded its deadline"));}
                    let count=file.read(&mut block).map_err(storage_error)?;if plan.cancel.load(Ordering::Acquire){return Err(cancelled());}if count==0{break;}
                    length=length.checked_add(count as u64).ok_or_else(||failure(ErrorCode::ResourceLimit,"Native media byte count overflowed"))?;if length>spec.asset.byte_length{return Err(failure(ErrorCode::ContentMismatch,"Native asset grew during verification"));}
                    digest.update(&block[..count]);hashed+=count as u64;
                    if let Ok(mut state)=self.state.lock(){if let Some(job)=state.job.as_mut().filter(|job|job.view.id==plan.job_id&&job.owner==plan.owner){job.view.bytes_hashed=hashed;job.view.asset_index=index as u16+1;}}
                }
                if length!=spec.asset.byte_length||format!("{:x}",digest.finalize())!=spec.asset.sha256.0||stamp!=Stamp::of(&file.metadata().map_err(storage_error)?)?{return Err(failure(ErrorCode::ContentMismatch,"Native media integrity changed during verification"));}
                file.seek(SeekFrom::Start(0)).map_err(storage_error)?;readers.push(Arc::new(Reader{file:Mutex::new(file),stamp,cancel:plan.cancel.clone()}));
            }
            Ok(Set{owner:plan.owner,view:plan.view.clone(),readers,expires:Instant::now()+LEASE})
        })();
        let mut state=self.state.lock().unwrap_or_else(|error|error.into_inner());
        if let Some(job)=state.job.as_mut().filter(|job|job.view.id==plan.job_id&&job.owner==plan.owner){
            match checked{Ok(set)if !plan.cancel.load(Ordering::Acquire)=>{job.ready=Some(set);job.view.state="ready".into();job.view.result_available=true;job.expires=Some(Instant::now()+LEASE);},Ok(set)=>{drop(state);set.cancel();drop(set);},Err(error)=>{job.view.state=if error.code==ErrorCode::Cancelled{"cancelled"}else{"failed"}.into();job.view.error=Some(error);}}
        }else{drop(state);if let Ok(set)=checked{set.cancel();drop(set);}}
    }
    pub fn status(self:&Arc<Self>,owner:Uuid,id:Uuid)->Result<RangeJob>{self.expire();let state=self.state.lock().map_err(|_|cancelled())?;let job=state.job.as_ref().filter(|job|job.owner==owner&&job.view.id==id).ok_or_else(missing)?;Ok(job.view.clone())}
    pub fn collect(self:&Arc<Self>,owner:Uuid,id:Uuid)->Result<RangeSetInfo>{
        self.expire();let mut state=self.state.lock().map_err(|_|cancelled())?;
        if let Some(set)=state.set.as_ref(){if set.owner==owner&&state.job.as_ref().is_some_and(|job|job.owner==owner&&job.view.id==id&&job.view.game_id==set.view.game_id){return Ok(set.view.clone());}return Err(failure(ErrorCode::Busy,"A native media set is already open"));}
        let job=state.job.as_mut().filter(|job|job.owner==owner&&job.view.id==id).ok_or_else(missing)?;let mut set=job.ready.take().ok_or_else(missing)?;set.expires=Instant::now()+LEASE;job.view.result_available=false;job.expires=None;let view=set.view.clone();state.set=Some(set);Ok(view)
    }
    pub fn cancel_job(self:&Arc<Self>,owner:Uuid,id:Uuid)->Result<RangeJob>{
        let set;let active;let view;{let mut state=self.state.lock().map_err(|_|cancelled())?;let job=state.job.as_mut().filter(|job|job.owner==owner&&job.view.id==id).ok_or_else(missing)?;job.cancel.store(true,Ordering::Release);set=job.ready.take();job.view.result_available=false;if job.view.state!="hashing"{job.view.state="cancelled".into();}job.expires=None;view=job.view.clone();active=if state.set.as_ref().is_some_and(|set|set.owner==owner&&set.view.game_id==view.game_id){state.set.take()}else{None};}for set in set.into_iter().chain(active){self.queue_retire(set);}Ok(view)
    }
    pub fn keep_alive(self:&Arc<Self>,owner:Uuid,id:Uuid)->Result<RangeSetInfo>{self.expire();let mut state=self.state.lock().map_err(|_|cancelled())?;let set=state.set.as_mut().filter(|set|set.owner==owner&&set.view.set_id==id).ok_or_else(missing)?;set.expires=Instant::now()+LEASE;Ok(set.view.clone())}
    pub fn close_set(self:&Arc<Self>,owner:Uuid,id:Uuid)->Result<()>{
        let set;{let mut state=self.state.lock().map_err(|_|cancelled())?;if !state.set.as_ref().is_some_and(|set|set.owner==owner&&set.view.set_id==id){return Err(missing());}set=state.set.take();}if let Some(set)=set{self.queue_retire(set);}Ok(())
    }
    pub fn close_owner(self:&Arc<Self>,owner:Uuid){
        let mut retired=Vec::new();{let mut state=self.state.lock().unwrap_or_else(|error|error.into_inner());if state.job.as_ref().is_some_and(|job|job.owner==owner){if let Some(mut job)=state.job.take(){job.cancel.store(true,Ordering::Release);if let Some(set)=job.ready.take(){retired.push(set);}}}if state.set.as_ref().is_some_and(|set|set.owner==owner){if let Some(set)=state.set.take(){retired.push(set);}}state.requests.retain(|(saved_owner,_),_|*saved_owner!=owner);state.order.retain(|(saved_owner,_)|*saved_owner!=owner);}for set in retired{self.queue_retire(set);}
    }
    pub fn read(self:&Arc<Self>,owner:Uuid,request_id:Uuid,fingerprint:[u8;32],set_id:Uuid,index:u16,offset:u64,limit:u32,deadline_ms:u64)->Result<ReadOperation>{
        self.expire();if limit==0||limit>READ_LIMIT{return Err(failure(ErrorCode::ResourceLimit,"A native media read exceeds its byte limit"));}
        let remaining=deadline_ms.saturating_sub(omniplay_core::now_ms()).min(30000);if remaining==0{return Err(failure(ErrorCode::DeadlineExceeded,"Native media read deadline expired"));}
        let mut state=self.state.lock().map_err(|_|cancelled())?;let key=(owner,request_id);
        if state.requests.get(&key).is_some_and(|previous|previous!=&fingerprint){return Err(failure(ErrorCode::Conflict,"Native media request ID was reused with changed content"));}
        let set=state.set.as_mut().filter(|set|set.owner==owner&&set.view.set_id==set_id).ok_or_else(missing)?;let reader=set.readers.get(index as usize).ok_or_else(missing)?.clone();
        if offset>reader.stamp.length||reader.cancel.load(Ordering::Acquire){return Err(missing());}set.expires=Instant::now()+LEASE;
        if self.reading.compare_exchange(false,true,Ordering::AcqRel,Ordering::Acquire).is_err(){return Err(failure(ErrorCode::Busy,"A native media read is still finishing"));}
        if !state.requests.contains_key(&key){if state.order.len()>=32{if let Some(old)=state.order.pop_front(){state.requests.remove(&old);}}state.order.push_back(key);state.requests.insert(key,fingerprint);}
        self.io.fetch_add(1,Ordering::AcqRel);Ok(ReadOperation{service:self.clone(),reader:Some(reader),set_id,index,offset,limit,deadline:Instant::now()+Duration::from_millis(remaining)})
    }
    pub fn check_replay(&self,owner:Uuid,id:Uuid,raw:&[u8])->Result<()>{let digest:[u8;32]=Sha256::digest(raw).into();let state=self.state.lock().map_err(|_|cancelled())?;if state.requests.get(&(owner,id)).is_some_and(|previous|previous!=&digest){return Err(failure(ErrorCode::Conflict,"Native media request ID was reused with changed content"));}Ok(())}
}
pub struct ReadOperation{service:Arc<AssetRanges>,reader:Option<Arc<Reader>>,set_id:Uuid,index:u16,offset:u64,limit:u32,deadline:Instant}
impl ReadOperation{pub fn run(&self)->Result<RangeChunk>{
    let reader=self.reader.as_ref().ok_or_else(missing)?;let check=||{if reader.cancel.load(Ordering::Acquire){Err(cancelled())}else if Instant::now()>=self.deadline{Err(failure(ErrorCode::DeadlineExceeded,"Native media read deadline expired"))}else{Ok(())}};
    check()?;let mut file=reader.file.try_lock().map_err(|_|failure(ErrorCode::Busy,"A native read of this asset is still finishing"))?;
    if reader.stamp!=Stamp::of(&file.metadata().map_err(storage_error)?)?{reader.cancel.store(true,Ordering::Release);return Err(failure(ErrorCode::ContentMismatch,"Native media changed after verification"));}
    let length=(reader.stamp.length-self.offset).min(self.limit as u64)as usize;let mut bytes=vec![0u8;length];file.seek(SeekFrom::Start(self.offset)).map_err(storage_error)?;check()?;file.read_exact(&mut bytes).map_err(storage_error)?;check()?;
    if reader.stamp!=Stamp::of(&file.metadata().map_err(storage_error)?)?{reader.cancel.store(true,Ordering::Release);return Err(failure(ErrorCode::ContentMismatch,"Native media changed during a positional read"));}
    Ok(RangeChunk{set_id:self.set_id,asset_index:self.index,offset:self.offset,payload_base64:STANDARD.encode(bytes),finished:self.offset+length as u64==reader.stamp.length})
}}
impl Drop for ReadOperation{fn drop(&mut self){drop(self.reader.take());self.service.reading.store(false,Ordering::Release);self.service.io.fetch_sub(1,Ordering::AcqRel);}}
