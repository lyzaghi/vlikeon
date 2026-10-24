//! Native credential candidate orchestration. No renderer, relay, consent UI or
//! startup endpoint is provided here; candidates are not active pairing grants.
use crate::{Database,stream_credentials::{self as journal,Phase},stream_credential_lease::CredentialLease};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_streaming::{SignedDocument,verify_grant,validate_audience,native_credentials::{
    CredentialAddress,SecretKind,NativeSigningIdentity,NativePairKey,PairingSecretStore,
    store_identity_candidate,store_pair_candidate,load_identity}};
use std::time::{Duration,Instant};
use uuid::Uuid;

/// Public metadata for a durable MaterialReady generation, not secret export,
/// consent, relay confirmation or a live signing/session authority capability.
pub struct Candidate{address:CredentialAddress,public_key:String,revision:u64}
impl Candidate{
    pub fn address(&self)->CredentialAddress{self.address}
    pub fn public_key(&self)->&str{&self.public_key}
    pub fn revision(&self)->u64{self.revision}
}
/// Owns the same lease across journal intent, actual synchronous vault work and
/// publication. A blocked provider keeps this owner/lease on its original stack;
/// there is no detached worker whose lock is dropped before it really exits.
pub struct NativeCredentialWriter<'a>{database:&'a Database,profile:Uuid,store:&'a dyn PairingSecretStore,lease:CredentialLease}
impl<'a> NativeCredentialWriter<'a>{
    pub fn open(database:&'a Database,profile:Uuid,store:&'a dyn PairingSecretStore)->Result<Self>{
        let lease=CredentialLease::acquire(database,profile)?;
        // Refuse corrupt/mismatched history before generating a replacement key.
        let _=journal::load(database,profile)?;
        Ok(Self{database,profile,store,lease})
    }
    pub fn prepare_identity(&mut self,expected:u64,deadline:Instant)->Result<Candidate>{
        begin(deadline)?;
        let snapshot=journal::load(self.database,self.profile)?;check(deadline)?;
        if snapshot.revision()!=expected||snapshot.records().iter().any(|record|record.address().kind()==SecretKind::DeviceSigning&&record.phase()!=Phase::Retired){return Err(conflict());}
        let identity=NativeSigningIdentity::generate()?;check(deadline)?;
        let public_key=identity.public_key();let address=CredentialAddress::new_identity(self.profile,self.database.device_id)?;
        let reserved=journal::reserve_identity(self.database,&self.lease,self.profile,expected,address,public_key.clone())?;
        check(deadline)?;
        let writing=journal::begin_write(self.database,&self.lease,self.profile,reserved.revision(),address)?;
        check(deadline)?;self.lease.check(self.database,self.profile)?;
        store_identity_candidate(self.store,&address,&identity)?;
        // Even a successful vault write is not published after a late result.
        // Error/timeout keeps Writing (or its uncertain committed successor).
        check(deadline)?;self.lease.check(self.database,self.profile)?;
        let ready=journal::material_ready(self.database,&self.lease,self.profile,writing.revision(),address)?;
        check(deadline)?;
        Ok(Candidate{address,public_key,revision:ready.revision()})
    }
    /// Native host calls only AFTER its actual local opt-in. This method is not
    /// a consent dialog and must never be exposed as a renderer signing API.
    /// The admitted identity is reloaded with its pinned public key before the
    /// journal transition; no existing peer/pair is migrated or approved here.
    pub fn activate_identity_after_local_consent(&mut self,address:CredentialAddress,expected:u64,now:u64,deadline:Instant)->Result<u64>{
        let started=begin(deadline)?;
        let snapshot=journal::load(self.database,self.profile)?;check(deadline)?;
        if snapshot.revision()!=expected{return Err(conflict());}
        let record=snapshot.records().iter().find(|record|record.address()==address).ok_or_else(conflict)?;
        if address.kind()!=SecretKind::DeviceSigning||record.phase()!=Phase::MaterialReady{return Err(conflict());}
        let identity=load_identity(self.store,&address,record.public_key())?;
        check(deadline)?;self.lease.check(self.database,self.profile)?;
        let updated=journal::activate(self.database,&self.lease,self.profile,expected,address,elapsed_now(now,started)?)?;
        drop(identity);check(deadline)?;Ok(updated.revision())
    }
    /// The key must come from the native authenticated exchange. This primitive
    /// does not derive it or prove the exchange; no browser key import exists.
    /// Approval is still pending when the returned candidate is MaterialReady.
    pub fn prepare_pair(&mut self,expected:u64,identity_address:CredentialAddress,document:&SignedDocument,
        audience:&str,key:&NativePairKey,now:u64,deadline:Instant)->Result<Candidate>{
        let started=begin(deadline)?;validate_audience(audience)?;
        let snapshot=journal::load(self.database,self.profile)?;check(deadline)?;
        if snapshot.revision()!=expected{return Err(conflict());}
        let identity_record=snapshot.records().iter().find(|record|record.address()==identity_address).ok_or_else(conflict)?;
        if identity_address.kind()!=SecretKind::DeviceSigning||identity_record.phase()!=Phase::Active{return Err(conflict());}
        let public_key=identity_record.public_key().to_owned();
        // Missing/locked identity cannot silently become a new native device.
        let identity=load_identity(self.store,&identity_address,&public_key)?;
        check(deadline)?;
        let grant=verify_grant(document,audience,elapsed_now(now,started)?)?;
        if grant.host_public_key!=public_key&&grant.client_public_key!=public_key{return Err(conflict());}
        let address=CredentialAddress::new_pair(self.profile,self.database.device_id,grant.id)?;
        let reserved=journal::reserve_pair(self.database,&self.lease,self.profile,expected,address,public_key.clone(),document,audience,elapsed_now(now,started)?)?;
        check(deadline)?;
        let writing=journal::begin_write(self.database,&self.lease,self.profile,reserved.revision(),address)?;
        check(deadline)?;self.lease.check(self.database,self.profile)?;
        store_pair_candidate(self.store,&address,document,audience,&public_key,elapsed_now(now,started)?,key)?;
        check(deadline)?;self.lease.check(self.database,self.profile)?;
        verify_grant(document,audience,elapsed_now(now,started)?)?;
        let ready=journal::material_ready(self.database,&self.lease,self.profile,writing.revision(),address)?;
        drop(identity);check(deadline)?;
        // A slow journal commit must not deliver already expired pair metadata.
        verify_grant(document,audience,elapsed_now(now,started)?)?;
        Ok(Candidate{address,public_key,revision:ready.revision()})
    }
}
fn conflict()->PlatformError{PlatformError::new(ErrorCode::Conflict,"Reload the native credential lifecycle before preparing or activating a generation")}
fn check(deadline:Instant)->Result<()>{
    if Instant::now()>=deadline{return Err(PlatformError::new(ErrorCode::DeadlineExceeded,"Native credential preparation exceeded its original deadline"));}Ok(())
}
fn begin(deadline:Instant)->Result<Instant>{
    let now=Instant::now();check(deadline)?;
    if deadline.saturating_duration_since(now)>Duration::from_secs(30){return Err(PlatformError::new(ErrorCode::InvalidRequest,"Native credential preparation requires a bounded operation deadline"));}Ok(now)
}
fn elapsed_now(now:u64,started:Instant)->Result<u64>{
    now.checked_add(u64::try_from(started.elapsed().as_millis()).map_err(|_|conflict())?).ok_or_else(conflict)
}
