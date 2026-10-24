//! Native per-operation signal pipeline with paired HTTP send/receive orchestration.
//! No product caller is installed. Outputs are not indefinite session authority.
use crate::{Database,stream_credentials::{self as journal,Journal,Record,Phase},stream_credential_lease::CredentialLease};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_streaming::{RelayOperation,RelayPacket,SignedDocument,MAX_PACKET_BYTES,document_hash,verify_grant,decode,native_relay::NativePairRelay,
    native_credentials::{CredentialAddress,SecretKind,PairingSecretStore,load_pair,load_identity},
    native_signal::{SignalMetadata,EncryptedSignal,NativeSignalCipher}};
use std::time::{Duration,Instant};
use uuid::Uuid;
use serde::Deserialize;
use std::sync::atomic::{AtomicBool,Ordering};
use std::collections::BTreeSet;
use zeroize::Zeroizing;
use omniplay_streaming::native_ice::{self,NativeIceConfiguration};

pub struct PreparedIce{address:CredentialAddress,session:Uuid,epoch:u64,configuration:NativeIceConfiguration}
impl PreparedIce{
    /// Native ICE adapter must additionally recheck its live authority immediately
    /// before use and retire allocations on session/pair/epoch revocation.
    pub fn for_session(&self,address:CredentialAddress,session:Uuid,epoch:u64,now:u64)->Result<&NativeIceConfiguration>{
        if self.address!=address||self.session!=session||self.epoch!=epoch{return Err(denied());}
        self.configuration.require_live(now)?;Ok(&self.configuration)
    }
}

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
#[derive(Deserialize)]#[serde(deny_unknown_fields)]
struct SendAcknowledgement{accepted:bool,cursor:u64}
pub struct SentSignal{message_id:Uuid,sequence:u64,cursor:u64}
impl SentSignal{
    pub fn message_id(&self)->Uuid{self.message_id}
    pub fn sequence(&self)->u64{self.sequence}
    pub fn cursor(&self)->u64{self.cursor}
}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]
struct ReceiveReply{packets:Vec<RelayPacket>}
/// Encrypted, unacknowledged batch. Cursor remains unchanged until accept_next
/// authenticates and commits each head packet. Never skip a foreign-session
/// head: the native receiver controller must explicitly route/recover it.
pub struct PolledSignals{address:CredentialAddress,epoch:u64,revision:u64,deadline:Instant,packets:Vec<RelayPacket>,next:usize}
impl PolledSignals{
    pub fn next_packet(&self)->Option<&RelayPacket>{self.packets.get(self.next)}
    pub fn revision(&self)->u64{self.revision}
    pub fn remaining(&self)->usize{self.packets.len().saturating_sub(self.next)}
    /// Caller supplies the locally admitted session, not a remote-created local
    /// authority. Failure leaves the batch index and expected revision intact;
    /// a commit may be uncertain, so reload/re-poll instead of skipping ahead.
    pub fn accept_next(&mut self,database:&Database,profile:Uuid,session:Uuid,store:&dyn PairingSecretStore,
        owner:&mut dyn NativeSignalAuthority,now:u64,deadline:Instant,cancel:&AtomicBool)->Result<CommittedSignal>{
        if cancel.load(Ordering::Acquire){return Err(PlatformError::new(ErrorCode::Cancelled,"Native signaling batch was cancelled"));}
        if deadline>self.deadline||Instant::now()>=self.deadline{return Err(denied());}
        let packet=self.next_packet().ok_or_else(denied)?;
        if packet.session_id!=session{return Err(denied());}
        let metadata=SignalMetadata{message_id:packet.message_id,session_id:packet.session_id,sequence:packet.sequence,expires_at_ms:packet.expires_at_ms};
        let encrypted=EncryptedSignal{iv:packet.iv.clone(),ciphertext:packet.ciphertext.clone()};
        let accepted=accept_received(database,profile,self.address,session,self.epoch,self.revision,store,owner,
            &packet.sender_public_key,packet.cursor,&metadata,&encrypted,now,deadline)?;
        if cancel.load(Ordering::Acquire){return Err(PlatformError::new(ErrorCode::Cancelled,"Native signaling delivery was cancelled after acceptance; reload its committed cursor"));}
        self.revision=accepted.revision();self.next+=1;Ok(accepted)
    }
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
fn owned(record:&Record,session:Uuid,epoch:u64)->Result<()>{
    if session.is_nil()||epoch==0||record.signal_session()!=Some(session)||record.signal_epoch()!=epoch{return Err(denied());}Ok(())
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
    let snapshot=journal::load(database,profile)?;owned(active(&snapshot,address,expected,clock.current()?)?,session,epoch)?;
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
    owned(record,session,epoch)?;
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

/// Sends the original prepared ciphertext exactly once per call. Retry may call
/// this again with that same prepared value and a current journal revision; the
/// HTTP envelope has a fresh nonce/signature but payload identity is unchanged.
/// Unknown/late replies never acknowledge send; the server may already have it.
pub fn send_prepared(database:&Database,profile:Uuid,prepared:&OutgoingSignal,expected:u64,store:&dyn PairingSecretStore,
    owner:&mut dyn NativeSignalAuthority,relay:&NativePairRelay,now:u64,deadline:Instant,cancel:&AtomicBool)->Result<SentSignal>{
    let clock=Clock::new(now,deadline)?;let address=prepared.address;
    let session=prepared.metadata.session_id;let epoch=prepared.epoch;
    let lease=CredentialLease::acquire(database,profile)?;clock.authority(owner,address,session,epoch)?;
    let snapshot=journal::load(database,profile)?;let record=active(&snapshot,address,expected,clock.current()?)?;
    owned(record,session,epoch)?;
    let grant=record.grant().ok_or_else(denied)?;
    if prepared.revision>expected||prepared.metadata.sequence==0||prepared.metadata.sequence>record.send_sequence()||
        prepared.sender!=record.public_key()||prepared.audience!=grant.audience()||
        Some(document_hash(&prepared.grant)?.as_str())!=record.grant_hash()||prepared.metadata.expires_at_ms<=clock.current()?{return Err(denied());}
    let identity_record=snapshot.records().iter().find(|identity|identity.address().kind()==SecretKind::DeviceSigning&&identity.phase()==Phase::Active&&identity.public_key()==record.public_key()).ok_or_else(denied)?;
    let identity=load_identity(store,&identity_record.address(),identity_record.public_key())?;
    clock.authority(owner,address,session,epoch)?;lease.check(database,profile)?;
    if prepared.metadata.expires_at_ms<=clock.current()?{return Err(denied());}
    let response=relay.request(&identity,&prepared.audience,prepared.relay_operation(),clock.current()?,deadline,cancel)?;
    let acknowledgement:SendAcknowledgement=serde_json::from_value(response).map_err(|_|denied())?;
    if !acknowledgement.accepted||acknowledgement.cursor==0||acknowledgement.cursor>9_007_199_254_740_991{return Err(denied());}
    clock.authority(owner,address,session,epoch)?;lease.check(database,profile)?;
    let current=clock.current()?;active(&snapshot,address,expected,current)?;
    if prepared.metadata.expires_at_ms<=current{return Err(denied());}
    if cancel.load(Ordering::Acquire){return Err(PlatformError::new(ErrorCode::Cancelled,"Native signaling send was cancelled"));}
    Ok(SentSignal{message_id:prepared.metadata.message_id,sequence:prepared.metadata.sequence,cursor:acknowledgement.cursor})
}

/// Polls at most eight packets without acknowledging any of them. Native owner
/// must provide receiver/session lifecycle authority; no autonomous poll loop,
/// ownership renewal or unknown-session acceptance is supplied by this call.
pub fn poll_received(database:&Database,profile:Uuid,address:CredentialAddress,session:Uuid,epoch:u64,expected:u64,
    store:&dyn PairingSecretStore,owner:&mut dyn NativeSignalAuthority,relay:&NativePairRelay,now:u64,deadline:Instant,cancel:&AtomicBool)->Result<PolledSignals>{
    let clock=Clock::new(now,deadline)?;
    let lease=CredentialLease::acquire(database,profile)?;clock.authority(owner,address,session,epoch)?;
    let snapshot=journal::load(database,profile)?;let record=active(&snapshot,address,expected,clock.current()?)?;
    owned(record,session,epoch)?;
    let metadata=record.grant().ok_or_else(denied)?;
    let grant=verify_grant(metadata.document(),metadata.audience(),clock.current()?)?;
    let peer=if record.public_key()==grant.host_public_key{&grant.client_public_key}else{&grant.host_public_key};
    let identity_record=snapshot.records().iter().find(|identity|identity.address().kind()==SecretKind::DeviceSigning&&identity.phase()==Phase::Active&&identity.public_key()==record.public_key()).ok_or_else(denied)?;
    let identity=load_identity(store,&identity_record.address(),identity_record.public_key())?;
    clock.authority(owner,address,session,epoch)?;lease.check(database,profile)?;
    let response=relay.request(&identity,metadata.audience(),RelayOperation::Receive{grant:metadata.document().clone(),after:record.receive_cursor(),limit:8},clock.current()?,deadline,cancel)?;
    let response:ReceiveReply=serde_json::from_value(response).map_err(|_|denied())?;
    if response.packets.len()>8{return Err(denied());}
    let mut cursor=record.receive_cursor();let mut sequence=record.receive_sequence();let mut messages=BTreeSet::new();
    for packet in &response.packets{
        let current=clock.current()?;
        if packet.cursor<=cursor||packet.cursor>9_007_199_254_740_991||packet.sequence<=sequence||packet.sequence>1_000_000||
            packet.message_id.is_nil()||packet.session_id.is_nil()||packet.sender_public_key!=*peer||!messages.insert(packet.message_id)||
            packet.expires_at_ms==0||packet.expires_at_ms>9_007_199_254_740_991||packet.expires_at_ms<=current.saturating_sub(30_000)||
            packet.expires_at_ms>current.saturating_add(90_000)||packet.expires_at_ms>grant.expires_at_ms{return Err(denied());}
        if decode(&packet.iv,12)?.len()!=12||decode(&packet.ciphertext,MAX_PACKET_BYTES)?.len()<16{return Err(denied());}
        cursor=packet.cursor;sequence=packet.sequence;
    }
    clock.authority(owner,address,session,epoch)?;lease.check(database,profile)?;
    active(&snapshot,address,expected,clock.current()?)?;
    if cancel.load(Ordering::Acquire){return Err(PlatformError::new(ErrorCode::Cancelled,"Native signaling receive was cancelled"));}
    Ok(PolledSignals{address,epoch,revision:expected,deadline,packets:response.packets,next:0})
}

/// TURN URLs and relay-only requirement come from the admitted native policy/
/// local setting, not the remote endpoint. No ICE agent is created by this call.
pub fn request_ice(database:&Database,profile:Uuid,address:CredentialAddress,session:Uuid,epoch:u64,expected:u64,
    store:&dyn PairingSecretStore,owner:&mut dyn NativeSignalAuthority,relay:&NativePairRelay,allowed_turn_urls:&[String],
    require_relay:bool,now:u64,deadline:Instant,cancel:&AtomicBool)->Result<PreparedIce>{
    let clock=Clock::new(now,deadline)?;
    if allowed_turn_urls.len()>32||require_relay&&allowed_turn_urls.is_empty(){return Err(denied());}
    for url in allowed_turn_urls{native_ice::validate_turn_uri(url)?;}
    let lease=CredentialLease::acquire(database,profile)?;clock.authority(owner,address,session,epoch)?;
    let snapshot=journal::load(database,profile)?;let record=active(&snapshot,address,expected,clock.current()?)?;
    owned(record,session,epoch)?;
    let grant=record.grant().ok_or_else(denied)?;
    let identity_record=snapshot.records().iter().find(|identity|identity.address().kind()==SecretKind::DeviceSigning&&identity.phase()==Phase::Active&&identity.public_key()==record.public_key()).ok_or_else(denied)?;
    let identity=load_identity(store,&identity_record.address(),identity_record.public_key())?;
    clock.authority(owner,address,session,epoch)?;lease.check(database,profile)?;
    let response=relay.request(&identity,grant.audience(),RelayOperation::IceCredentials{grant:grant.document().clone()},clock.current()?,deadline,cancel)?;
    let configuration=native_ice::admit(response,grant.document(),grant.audience(),record.public_key(),allowed_turn_urls,clock.current()?)?;
    if require_relay&&!configuration.relay_available(){return Err(PlatformError::new(ErrorCode::UnsupportedCapability,"The configured native relay cannot provide the required TURN service"));}
    clock.authority(owner,address,session,epoch)?;lease.check(database,profile)?;
    let current=clock.current()?;active(&snapshot,address,expected,current)?;configuration.require_live(current)?;
    if cancel.load(Ordering::Acquire){return Err(PlatformError::new(ErrorCode::Cancelled,"Native ICE authorization was cancelled"));}
    Ok(PreparedIce{address,session,epoch,configuration})
}
