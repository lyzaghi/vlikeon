//! Shared broker/profile immutable-image cache. Active images retain this owner
//! through the pidfd reaper; eviction never releases an active image's budget.
use omniplay_contracts::{ContentHash,ErrorCode,PlatformError,Result};
use omniplay_sandbox::{linux_launch,linux_sealed::{SealedBudget,SealedObject}};
use std::{collections::HashMap,fs::File,os::unix::fs::MetadataExt,path::Path,sync::{Arc,Mutex,Condvar,OnceLock,Weak,atomic::{AtomicBool,Ordering}},time::Duration};
use uuid::Uuid;

#[derive(Clone,PartialEq,Eq,Hash)]struct Key{hash:String,bytes:u64}
struct Cached{object:SealedObject,used:u64}
struct Entries{objects:HashMap<Key,Cached>,pending:HashMap<Key,()>,bytes:u64,clock:u64,waiters:usize}
struct Cache{_directory:File,profile:Uuid,maximum_bytes:u64,budget:SealedBudget,entries:Mutex<Entries>,changed:Condvar}
#[derive(Clone)]pub(crate) struct ProfileImageCache{inner:Arc<Cache>}
struct Flight{cache:Arc<Cache>,key:Key,complete:bool}
static CACHES:OnceLock<Mutex<HashMap<(u64,u64),Weak<Cache>>>>=OnceLock::new();
fn limit()->PlatformError{PlatformError::new(ErrorCode::ResourceLimit,"The profile's immutable Linux image cache is full")}
fn invalid()->PlatformError{PlatformError::new(ErrorCode::RuntimeUnavailable,"The shared Linux image cache could not be established")}
fn cancelled(cancel:&AtomicBool)->Result<()>{if cancel.load(Ordering::Acquire){Err(PlatformError::new(ErrorCode::Cancelled,"Linux image preparation was cancelled"))}else{Ok(())}}
impl Drop for Flight{fn drop(&mut self){if !self.complete{let mut entries=self.cache.entries.lock().unwrap_or_else(|error|error.into_inner());if entries.pending.remove(&self.key).is_some(){entries.bytes=entries.bytes.saturating_sub(self.key.bytes);}self.cache.changed.notify_all();}}}
impl ProfileImageCache{
    pub fn open(profile_root:&Path,profile:Uuid,maximum_bytes:u64)->Result<Self>{
        if profile.is_nil()||!(16*1024*1024..=896*1024*1024).contains(&maximum_bytes)||maximum_bytes&4095!=0{return Err(limit());}let directory=linux_launch::private_directory(profile_root)?;let shape=directory.metadata().map_err(|_|invalid())?;let key=(shape.dev(),shape.ino());
        let mut caches=CACHES.get_or_init(||Mutex::new(HashMap::new())).lock().map_err(|_|invalid())?;caches.retain(|_,cache|cache.strong_count()>0);if let Some(inner)=caches.get(&key).and_then(Weak::upgrade){if inner.profile!=profile||inner.maximum_bytes!=maximum_bytes{return Err(invalid());}return Ok(Self{inner});}if caches.len()>=64{return Err(limit());}
        let inner=Arc::new(Cache{_directory:directory,profile,maximum_bytes,budget:SealedBudget::new(maximum_bytes+128*1024*1024,256)?,entries:Mutex::new(Entries{objects:HashMap::new(),pending:HashMap::new(),bytes:0,clock:0,waiters:0}),changed:Condvar::new()});caches.insert(key,Arc::downgrade(&inner));Ok(Self{inner})
    }
    pub fn profile(&self)->Uuid{self.inner.profile}
    pub fn plan_budget(&self)->SealedBudget{self.inner.budget.clone()}
    /// The expected key must come from the authenticated outer release. A cache
    /// hit returns those already verified immutable bytes, never the host path.
    pub fn image(&self,expected:&ContentHash,bytes:u64,cancel:&AtomicBool,source:impl FnOnce()->Result<File>)->Result<SealedObject>{
        ContentHash::parse(expected.0.clone())?;if bytes==0||bytes>256*1024*1024||bytes>self.inner.maximum_bytes{return Err(limit());}let key=Key{hash:expected.0.clone(),bytes};let mut flight=loop{
            cancelled(cancel)?;let mut entries=self.inner.entries.lock().map_err(|_|invalid())?;entries.clock=entries.clock.checked_add(1).ok_or_else(invalid)?;let used=entries.clock;
            if let Some(found)=entries.objects.get_mut(&key){found.used=used;return Ok(found.object.retain_owner(self.inner.clone()));}
            if entries.pending.contains_key(&key){if entries.waiters>=64{return Err(PlatformError::new(ErrorCode::Busy,"The Linux image preparation queue is busy"));}entries.waiters+=1;let(mut next,_)=self.inner.changed.wait_timeout(entries,Duration::from_millis(100)).map_err(|_|invalid())?;next.waiters-=1;drop(next);continue;}
            while entries.objects.len()+entries.pending.len()>=192||entries.bytes.checked_add(bytes).is_none_or(|total|total>self.inner.maximum_bytes){let victim=entries.objects.iter().filter(|(_,value)|value.object.cache_unused()).min_by_key(|(_,value)|value.used).map(|(key,_)|key.clone()).ok_or_else(limit)?;let removed=entries.objects.remove(&victim).ok_or_else(invalid)?;entries.bytes=entries.bytes.checked_sub(removed.object.bytes()).ok_or_else(invalid)?;}
            entries.bytes=entries.bytes.checked_add(bytes).ok_or_else(limit)?;entries.pending.insert(key.clone(),());break Flight{cache:self.inner.clone(),key:key.clone(),complete:false};
        };
        let object=self.inner.budget.image(source()?,expected,bytes,cancel)?;cancelled(cancel)?;let mut entries=self.inner.entries.lock().map_err(|_|invalid())?;entries.clock=entries.clock.checked_add(1).ok_or_else(invalid)?;let used=entries.clock;if entries.pending.remove(&key).is_none(){return Err(invalid());}entries.objects.insert(key,Cached{object:object.clone(),used});flight.complete=true;self.inner.changed.notify_all();Ok(object.retain_owner(self.inner.clone()))
    }
}
