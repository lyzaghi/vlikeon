//! Native per-operation signal pipeline. No relay HTTP/client or product caller
//! is installed here. Never retain its outputs as indefinite session authority.
use crate::{Database,stream_credentials::{self as journal,Journal,Record,Phase},stream_credential_lease::CredentialLease};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_streaming::{RelayOperation,SignedDocument,MAX_PACKET_BYTES,
    native_credentials::{CredentialAddress,SecretKind,PairingSecretStore,load_pair},
    native_signal::{SignalMetadata,EncryptedSignal,NativeSignalCipher}};
use std::time::{Duration,Instant};
use uuid::Uuid;
use zeroize::Zeroizing;

/// Native observations must retain their original observation time. No serde
/// constructor, remote assertion or default authorized observation exists.
pub struct SignalObservation{address:CredentialAddress,session:Uuid,epoch:u64,checked_at:Instant,authorized:bool}
impl SignalObservation{
    pub fn from_native(address:CredentialAddress,session:Uuid,epoch:u64,checked_at:Instant,authorized:bool)->Self{
        Self{address,session,epoch,checked_at,authorized}
    }
}
pub trait NativeSignalAuthority{
    /// Must observe the real native document/session/pair and installed relay
    /// policy. A loaded credential or journal Active bit alone is insufficient.
    fn observe(&mut self,address:CredentialAddress,session:Uuid)->Result<SignalObservation>;
}
pub struct OutgoingSignal{address:CredentialAddress,epoch:u64,audience:String,sender:String,grant:SignedDocument,metadata:SignalMetadata,encrypted:EncryptedSignal,revision:u64}
impl OutgoingSignal{
    pub fn address(&self)->CredentialAddress{self.address}
    pub fn epoch(&self)->u64{self.epoch}
    pub fn audience(&self)->&str{&self.audience}
    pub fn sender(&self)->&str{&self.sender}
    pub fn metadata(&self)->&SignalMetadata{&self.metadata}
    pub fn revision(&self)->u64{self.revision}
    /// Native relay owner rechecks authority/deadline before each attempt. Retry
    /// this exact operation; never burn another sequence or reseal on retry.
    pub fn relay_operation(&self)->RelayOperation{
        RelayOperation::Send{grant:self.grant.clone(),message_id:self.metadata.message_id,session_id:self.metadata.session_id,
            sequence:self.metadata.sequence,expires_at_ms:self.metadata.expires_at_ms,iv:self.encrypted.iv.clone(),ciphertext:self.encrypted.ciphertext.clone()}
    }
}
pub struct CommittedSignal{session:Uuid,epoch:u64,revision:u64,body:Zeroizing<Vec<u8>>}
impl CommittedSignal{
    pub fn session(&self)->Uuid{self.session}
    pub fn epoch(&self)->u64{self.epoch}
    pub fn revision(&self)->u64{self.revision}
    /// Immediate native delivery only; recheck live authority if queued/delayed.
    pub fn body(&self)->&[u8]{&self.body}
}
struct Clock{now:u64,started:Instant,deadline:Instant}
impl Clock{
    fn new(now:u64,deadline:Instant)->Result<Self>{
        let started=Instant::now();
        if deadline<=started||deadline.saturating_duration_since(started)>Duration::from_secs(30){return Err(denied());}
        Ok(Self{now,started,deadline})
    }
    fn current(&self)->Result<u64>{
        if Instant::now()>=self.deadline{return Err(PlatformError::new(ErrorCode::DeadlineExceeded,"Native signaling exceeded its original operation deadline"));}
        self.now.checked_add(u64::try_from(self.started.elapsed().as_millis()).map_err(|_|denied())?).ok_or_else(denied)
    }
    fn authority(&self,owner:&mut dyn NativeSignalAuthority,address:CredentialAddress,session:Uuid,epoch:u64)->Result<()>{
        self.current()?;let observation=owner.observe(address,session)?;self.current()?;
        if session.is_nil()||epoch==0||epoch>10_000_000||observation.address!=address||observation.session!=session||observation.epoch!=epoch||
            !observation.authorized||observation.checked_at.elapsed()>Duration::from_millis(100)||observation.checked_at>Instant::now(){return Err(denied());}Ok(())
    }
}
fn denied()->PlatformError{PlatformError::new(ErrorCode::PermissionDenied,"The native signaling generation, session or current authority is unavailable")}
fn active(snapshot:&Journal,address:CredentialAddress,expected:u64,now:u64)->Result<&Record>{
    if snapshot.revision()!=expected||address.kind()!=SecretKind::PairEncryption{return Err(denied());}
    let record=snapshot.records().iter().find(|record|record.address()==address).ok_or_else(denied)?;
    if record.phase()!=Phase::Active||record.expires_at_ms().is_none_or(|expiry|expiry<=now){return Err(denied());}Ok(record)
}
fn cipher(store:&dyn PairingSecretStore,record:&Record,clock:&Clock)->Result<NativeSignalCipher>{
    let grant=record.grant().ok_or_else(denied)?;
    let material=load_pair(store,&record.address(),grant.document(),grant.audience(),record.public_key(),clock.current()?)?;
    NativeSignalCipher::new(material,grant.document(),grant.audience(),record.public_key(),clock.current()?)
}

pub fn prepare_send(database:&Database,profile:Uuid,address:CredentialAddress,session:Uuid,epoch:u64,expected:u64,
    store:&dyn PairingSecretStore,owner:&mut dyn NativeSignalAuthority,body:&[u8],now:u64,deadline:Instant)->Result<OutgoingSignal>{
    let clock=Clock::new(now,deadline)?;
    if body.is_empty()||body.len()>MAX_PACKET_BYTES-16{return Err(denied());}
    let lease=CredentialLease::acquire(database,profile)?;clock.authority(owner,address,session,epoch)?;
    let snapshot=journal::load(database,profile)?;active(&snapshot,address,expected,clock.current()?)?;
    // Reservation commits before key loading/encryption. Any later error burns
    // the sequence and leaves its committed revision for explicit reload.
    let(reserved,sequence)=journal::reserve_signal_sequence(database,&lease,profile,expected,address,clock.current()?)?;
    let record=active(&reserved,address,reserved.revision(),clock.current()?)?;
    let metadata=SignalMetadata{message_id:Uuid::new_v4(),session_id:session,sequence,
        expires_at_ms:clock.current()?.saturating_add(60_000).min(record.expires_at_ms().ok_or_else(denied)?)};
    let cipher=cipher(store,record,&clock)?;clock.authority(owner,address,session,epoch)?;
    let encrypted=cipher.seal_reserved(&metadata,body,clock.current()?)?;
    clock.authority(owner,address,session,epoch)?;lease.check(database,profile)?;
    let current=clock.current()?;
    active(&reserved,address,reserved.revision(),current)?;
    if metadata.expires_at_ms<=current{return Err(denied());}
    let grant=record.grant().ok_or_else(denied)?;
    Ok(OutgoingSignal{address,epoch,audience:grant.audience().into(),sender:record.public_key().into(),grant:grant.document().clone(),metadata,encrypted,revision:reserved.revision()})
}

pub fn accept_received(database:&Database,profile:Uuid,address:CredentialAddress,session:Uuid,epoch:u64,expected:u64,
    store:&dyn PairingSecretStore,owner:&mut dyn NativeSignalAuthority,sender:&str,cursor:u64,metadata:&SignalMetadata,
    encrypted:&EncryptedSignal,now:u64,deadline:Instant)->Result<CommittedSignal>{
    let clock=Clock::new(now,deadline)?;
    if metadata.session_id!=session{return Err(denied());}
    let lease=CredentialLease::acquire(database,profile)?;clock.authority(owner,address,session,epoch)?;
    let snapshot=journal::load(database,profile)?;let record=active(&snapshot,address,expected,clock.current()?)?;
    if cursor<=record.receive_cursor()||metadata.sequence<=record.receive_sequence(){return Err(denied());}
    let cipher=cipher(store,record,&clock)?;
    let body=cipher.open_uncommitted(sender,metadata,encrypted,clock.current()?)?;
    clock.authority(owner,address,session,epoch)?;
    // Publish no plaintext until authenticated cursor/sequence acceptance has
    // committed under the same lease/revision. Commit uncertainty is no delivery.
    let accepted=journal::accept_signal(database,&lease,profile,expected,address,sender,cursor,metadata,clock.current()?)?;
    clock.authority(owner,address,session,epoch)?;lease.check(database,profile)?;
    let current=clock.current()?;active(&accepted,address,accepted.revision(),current)?;
    if metadata.expires_at_ms<=current.saturating_sub(30_000){return Err(denied());}
    Ok(CommittedSignal{session,epoch,revision:accepted.revision(),body})
}
