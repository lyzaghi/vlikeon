//! Native-only pairing lifecycle metadata. No key material or renderer API.
//! Every transition commits before its corresponding vault/network side effect.
use crate::{Database,util::storage_error};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_streaming::{key,document_hash,verify_grant,SignedDocument,
    native_credentials::{CredentialAddress,SecretKind}};
use rusqlite::{OptionalExtension,Transaction,TransactionBehavior,params};
use serde::{Deserialize,Serialize};
use std::collections::BTreeSet;
use std::time::Instant;
use uuid::Uuid;

const KEY:&str="native_remote_play_credentials_v1";
const MAX_BYTES:usize=192*1024;
const MAX_RECORDS:usize=128;
const MAX_REVISION:u64=10_000_000;
fn invalid()->PlatformError{PlatformError::new(ErrorCode::CorruptData,"Native Remote Play credential lifecycle needs recovery")}
fn conflict()->PlatformError{PlatformError::new(ErrorCode::Conflict,"Native Remote Play credential state changed; reload before continuing")}

#[derive(Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Phase{Reserved,Writing,MaterialReady,Active,Revoking,Retired}
#[derive(Clone,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct Record{
    address:CredentialAddress,public_key:String,grant_hash:Option<String>,expires_at_ms:Option<u64>,
    phase:Phase,writer_retired:bool,material_absent:bool,local_retired:bool,relay_retired:bool,
}
impl Record{
    pub fn address(&self)->CredentialAddress{self.address}
    pub fn phase(&self)->Phase{self.phase}
    pub fn public_key(&self)->&str{&self.public_key}
    pub fn grant_hash(&self)->Option<&str>{self.grant_hash.as_deref()}
    pub fn expires_at_ms(&self)->Option<u64>{self.expires_at_ms}
    pub fn writer_retired(&self)->bool{self.writer_retired}
    pub fn material_absent(&self)->bool{self.material_absent}
    pub fn local_retired(&self)->bool{self.local_retired}
    pub fn relay_retired(&self)->bool{self.relay_retired}
}
#[derive(Clone,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct Journal{schema_version:u32,profile:Uuid,device:Uuid,revision:u64,records:Vec<Record>}
impl Journal{
    pub fn revision(&self)->u64{self.revision}
    pub fn records(&self)->&[Record]{&self.records}
    fn validate(&self)->Result<()>{
        if self.schema_version!=1||self.profile.is_nil()||self.device.is_nil()||self.revision>MAX_REVISION||self.records.len()>MAX_RECORDS||self.revision==0&&!self.records.is_empty(){return Err(invalid());}
        let mut generations=BTreeSet::new();let mut live=BTreeSet::new();
        for record in &self.records{
            let address=record.address;address.validate()?;key(&record.public_key)?;
            if address.profile()!=self.profile||address.device()!=self.device||!generations.insert(address.generation()){return Err(invalid());}
            let kind=match address.kind(){SecretKind::DeviceSigning=>0u8,SecretKind::PairEncryption=>1};
            if record.phase!=Phase::Retired&&!live.insert((kind,address.id())){return Err(invalid());}
            match address.kind(){
                SecretKind::DeviceSigning=>if record.grant_hash.is_some()||record.expires_at_ms.is_some(){return Err(invalid());},
                SecretKind::PairEncryption=>if record.expires_at_ms.is_none_or(|value|value==0)||record.grant_hash.as_ref().is_none_or(|hash|hash.len()!=64||!hash.bytes().all(|byte|byte.is_ascii_digit()||(b'a'..=b'f').contains(&byte))){return Err(invalid());},
            }
            match record.phase{
                Phase::Reserved|Phase::MaterialReady|Phase::Active=>if !record.writer_retired||record.material_absent||record.local_retired||record.relay_retired{return Err(invalid());},
                Phase::Writing=>if record.writer_retired||record.material_absent||record.local_retired||record.relay_retired{return Err(invalid());},
                Phase::Revoking=>if record.material_absent&&!record.writer_retired{return Err(invalid());},
                Phase::Retired=>if !record.writer_retired||!record.material_absent||!record.local_retired||!record.relay_retired{return Err(invalid());},
            }
        }
        for pair in self.records.iter().filter(|r|r.address.kind()==SecretKind::PairEncryption&&r.phase==Phase::Active){
            if !self.records.iter().any(|identity|identity.address.kind()==SecretKind::DeviceSigning&&identity.phase==Phase::Active&&identity.public_key==pair.public_key){return Err(invalid());}
        }
        Ok(())
    }
}
fn read(connection:&rusqlite::Connection,profile:Uuid,device:Uuid)->Result<Journal>{
    if profile.is_nil()||device.is_nil(){return Err(invalid());}
    // Bound the SQL value before fetching the full JSON string.
    let length:Option<i64>=connection.query_row("SELECT length(CAST(value AS BLOB)) FROM preferences WHERE key=?1",[KEY],|row|row.get(0)).optional().map_err(storage_error)?;
    let Some(length)=length else{return Ok(Journal{schema_version:1,profile,device,revision:0,records:Vec::new()});};
    if length<=0||length>MAX_BYTES as i64{return Err(invalid());}
    let json:String=connection.query_row("SELECT value FROM preferences WHERE key=?1",[KEY],|row|row.get(0)).map_err(storage_error)?;
    if json.len()>MAX_BYTES{return Err(invalid());}
    let journal:Journal=serde_json::from_str(&json).map_err(|_|invalid())?;journal.validate()?;
    if journal.profile!=profile||journal.device!=device{return Err(invalid());}Ok(journal)
}
pub fn load(database:&Database,profile:Uuid)->Result<Journal>{
    let tx=Transaction::new_unchecked(&database.connection,TransactionBehavior::Deferred).map_err(storage_error)?;
    let journal=read(&tx,profile,database.device_id)?;tx.commit().map_err(storage_error)?;Ok(journal)
}
fn update(database:&Database,profile:Uuid,expected:u64,change:impl FnOnce(&mut Journal)->Result<()>)->Result<Journal>{
    let tx=Transaction::new_unchecked(&database.connection,TransactionBehavior::Immediate).map_err(storage_error)?;
    let mut journal=read(&tx,profile,database.device_id)?;
    if journal.revision!=expected||expected>=MAX_REVISION{return Err(conflict());}
    change(&mut journal)?;journal.revision+=1;journal.validate()?;
    let json=serde_json::to_string(&journal).map_err(storage_error)?;
    if json.len()>MAX_BYTES{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Native pairing cleanup records exceed the profile bound"));}
    tx.execute("INSERT INTO preferences(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![KEY,json]).map_err(storage_error)?;
    tx.commit().map_err(storage_error)?;Ok(journal)
}
fn selected(journal:&mut Journal,address:CredentialAddress)->Result<&mut Record>{
    journal.records.iter_mut().find(|record|record.address==address).ok_or_else(conflict)
}
fn add(journal:&mut Journal,address:CredentialAddress,public_key:String,grant_hash:Option<String>,expires_at_ms:Option<u64>)->Result<()>{
    address.validate()?;key(&public_key)?;
    if address.profile()!=journal.profile||address.device()!=journal.device{return Err(conflict());}
    // Retired tombstones are intentionally retained; no age-based resurrection.
    if journal.records.len()>=MAX_RECORDS{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Native pairing generation history requires explicit maintenance before adding credentials"));}
    if journal.records.iter().any(|record|record.address.generation()==address.generation()||record.address.kind()==address.kind()&&record.address.id()==address.id()&&record.phase!=Phase::Retired){return Err(conflict());}
    journal.records.push(Record{address,public_key,grant_hash,expires_at_ms,phase:Phase::Reserved,
        writer_retired:true,material_absent:false,local_retired:false,relay_retired:false});Ok(())
}
pub fn reserve_identity(database:&Database,profile:Uuid,expected:u64,address:CredentialAddress,public_key:String)->Result<Journal>{
    if address.kind()!=SecretKind::DeviceSigning{return Err(invalid());}
    update(database,profile,expected,|journal|add(journal,address,public_key,None,None))
}
pub fn reserve_pair(database:&Database,profile:Uuid,expected:u64,address:CredentialAddress,local_public:String,document:&SignedDocument,audience:&str,now:u64)->Result<Journal>{
    let started=Instant::now();
    if address.kind()!=SecretKind::PairEncryption{return Err(invalid());}
    let grant=verify_grant(document,audience,now)?;
    if grant.id!=address.id()||(grant.host_public_key!=local_public&&grant.client_public_key!=local_public){return Err(invalid());}
    let hash=document_hash(document)?;
    update(database,profile,expected,|journal|{
        verify_grant(document,audience,after_wait(now,started)?)?;
        if !journal.records.iter().any(|record|record.address.kind()==SecretKind::DeviceSigning&&record.phase==Phase::Active&&record.public_key==local_public){return Err(conflict());}
        // A given grant ID never becomes usable again after local revocation.
        if journal.records.iter().any(|record|record.address.kind()==SecretKind::PairEncryption&&record.address.id()==address.id()){return Err(conflict());}
        add(journal,address,local_public,Some(hash),Some(grant.expires_at_ms))
    })
}
/// Commit BEFORE starting a single serialized vault writer. Restart recovery
/// must retire/contain that producer before recording writer retirement.
pub fn begin_write(database:&Database,profile:Uuid,expected:u64,address:CredentialAddress)->Result<Journal>{
    update(database,profile,expected,|journal|{let record=selected(journal,address)?;
        if record.phase!=Phase::Reserved{return Err(conflict());}record.phase=Phase::Writing;record.writer_retired=false;Ok(())})
}
/// Trusted native orchestrator calls only after exact material read-back AND
/// actual writer retirement. Caller observations are not cryptographic receipts.
pub fn material_ready(database:&Database,profile:Uuid,expected:u64,address:CredentialAddress)->Result<Journal>{
    update(database,profile,expected,|journal|{let record=selected(journal,address)?;
        if record.phase!=Phase::Writing{return Err(conflict());}record.phase=Phase::MaterialReady;record.writer_retired=true;Ok(())})
}
/// Identity requires native opt-in; a pair additionally requires authenticated
/// relay approval. This metadata transition does not perform either operation.
pub fn activate(database:&Database,profile:Uuid,expected:u64,address:CredentialAddress,now:u64)->Result<Journal>{
    let started=Instant::now();
    update(database,profile,expected,|journal|{let record=selected(journal,address)?;
        let current=after_wait(now,started)?;
        if record.phase!=Phase::MaterialReady||record.expires_at_ms.is_some_and(|expiry|expiry<=current){return Err(conflict());}
        record.phase=Phase::Active;Ok(())})
}
/// Durable fence first. Identity revocation atomically fences every pair too.
/// Repeating a fence never clears already retained cleanup observations.
pub fn revoke(database:&Database,profile:Uuid,expected:u64,address:CredentialAddress)->Result<Journal>{
    update(database,profile,expected,|journal|{
        let target=selected(journal,address)?;
        if target.phase==Phase::Retired{return Err(conflict());}
        let identity=target.address.kind()==SecretKind::DeviceSigning;
        for record in &mut journal.records{
            if (record.address==address||identity)&&!matches!(record.phase,Phase::Revoking|Phase::Retired){
                record.phase=Phase::Revoking;record.material_absent=false;record.local_retired=false;
                record.relay_retired=record.address.kind()==SecretKind::DeviceSigning;
            }
        }Ok(())
    })
}
#[derive(Clone,Copy)]pub enum CleanupObservation{WriterRetired,MaterialAbsent,LocalRetired,RelayRetired}
/// Evidence must come from native producer/vault/session/relay owners. There is
/// no renderer endpoint and no timestamp-as-retirement inference here.
pub fn observe_cleanup(database:&Database,profile:Uuid,expected:u64,address:CredentialAddress,observation:CleanupObservation)->Result<Journal>{
    update(database,profile,expected,|journal|{let record=selected(journal,address)?;
        if record.phase!=Phase::Revoking{return Err(conflict());}
        match observation{
            CleanupObservation::WriterRetired=>record.writer_retired=true,
            CleanupObservation::MaterialAbsent=>{if !record.writer_retired{return Err(conflict());}record.material_absent=true;},
            CleanupObservation::LocalRetired=>record.local_retired=true,
            CleanupObservation::RelayRetired=>record.relay_retired=true,
        }
        let complete=record.writer_retired&&record.material_absent&&record.local_retired&&record.relay_retired;
        if complete&&(address.kind()==SecretKind::PairEncryption||journal.records.iter().all(|other|other.address.kind()!=SecretKind::PairEncryption||other.phase==Phase::Retired)){
            selected(journal,address)?.phase=Phase::Retired;
        }
        Ok(())
    })
}
fn after_wait(now:u64,started:Instant)->Result<u64>{
    now.checked_add(u64::try_from(started.elapsed().as_millis()).map_err(|_|invalid())?).ok_or_else(invalid)
}
