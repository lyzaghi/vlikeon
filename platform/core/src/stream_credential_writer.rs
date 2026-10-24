//! Native credential candidate orchestration. No renderer, relay, consent UI or
//! startup endpoint is provided here; candidates are not active pairing grants.
use crate::{Database,stream_credentials::{self as journal,Phase},stream_credential_lease::CredentialLease};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_streaming::{SignedDocument,verify_grant,validate_audience,native_credentials::{
    CredentialAddress,SecretKind,NativeSigningIdentity,NativePairKey,PairingSecretStore,
    store_identity_candidate,store_pair_candidate,load_identity,load_pair}};
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
        let _=journal::load_scoped(database,&lease,profile)?;
        Ok(Self{database,profile,store,lease})
    }
    pub fn prepare_identity(&mut self,expected:u64,deadline:Instant)->Result<Candidate>{
        begin(deadline)?;
        let snapshot=journal::load_scoped(self.database,&self.lease,self.profile)?;check(deadline)?;
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
    fn active_signer(&self,expected:u64,address:CredentialAddress)->Result<NativeSigningIdentity>{
        let snapshot=journal::load_scoped(self.database,&self.lease,self.profile)?;
        if snapshot.revision()!=expected{return Err(conflict());}
        let record=snapshot.records().iter().find(|record|record.address()==address&&record.phase()==Phase::Active&&address.kind()==SecretKind::DeviceSigning).ok_or_else(conflict)?;
        load_identity(self.store,&address,record.public_key())
    }
    pub fn begin_client(&self,expected:u64,address:CredentialAddress,code:&str,host:&str,audience:&str,label:&str,
        now:u64,deadline:Instant)->Result<omniplay_streaming::native_pair_exchange::NativeClientPairing>{
        let started=begin(deadline)?;let signer=self.active_signer(expected,address)?;check(deadline)?;
        let pending=omniplay_streaming::native_pair_exchange::NativeClientPairing::from_invitation_code(&signer,code,host,audience,label,elapsed_now(now,started)?)?;
        self.lease.check(self.database,self.profile)?;check(deadline)?;Ok(pending)
    }
    /// Local invitation creation only. Registration and sharing remain explicit.
    pub fn begin_host(&self,expected:u64,address:CredentialAddress,audience:&str,label:&str,now:u64,deadline:Instant)
        ->Result<omniplay_streaming::native_pair_exchange::NativeHostInvitation>{
        let started=begin(deadline)?;
        crate::stream_signal_authority::require_installed_host_grant(audience,false)?;
        let signer=self.active_signer(expected,address)?;check(deadline)?;
        let invitation=omniplay_streaming::native_pair_exchange::NativeHostInvitation::create(&signer,audience,label,elapsed_now(now,started)?)?;
        self.lease.check(self.database,self.profile)?;check(deadline)?;Ok(invitation)
    }
    /// Caller retains the invitation even on uncertain registration outcomes.
    pub fn register_host(&self,expected:u64,address:CredentialAddress,invitation:&mut omniplay_streaming::native_pair_exchange::NativeHostInvitation,
        relay:&omniplay_streaming::native_relay::NativePairRelay,now:u64,deadline:Instant,cancel:&std::sync::atomic::AtomicBool)->Result<()>{
        let started=begin(deadline)?;
        let metadata:omniplay_streaming::Invite=omniplay_streaming::inspect(invitation.signed_invite(),4096)?;
        crate::stream_signal_authority::require_installed_host_grant(&metadata.audience,false)?;
        if cancel.load(std::sync::atomic::Ordering::Acquire){return Err(conflict());}
        let signer=self.active_signer(expected,address)?;check(deadline)?;
        invitation.register(relay,&signer,elapsed_now(now,started)?,deadline,cancel)?;
        self.lease.check(self.database,self.profile)?;check(deadline)?;
        if cancel.load(std::sync::atomic::Ordering::Acquire){return Err(conflict());}Ok(())
    }
    /// Explicit bounded host poll; peer keys/labels are not user-approved here.
    pub fn poll_host(&self,expected:u64,address:CredentialAddress,invitation:&mut omniplay_streaming::native_pair_exchange::NativeHostInvitation,
        relay:&omniplay_streaming::native_relay::NativePairRelay,now:u64,deadline:Instant,cancel:&std::sync::atomic::AtomicBool)
        ->Result<Vec<omniplay_streaming::native_pair_exchange::NativeHostPeerReview>>{
        let started=begin(deadline)?;
        let metadata:omniplay_streaming::Invite=omniplay_streaming::inspect(invitation.signed_invite(),4096)?;
        crate::stream_signal_authority::require_installed_host_grant(&metadata.audience,false)?;
        if cancel.load(std::sync::atomic::Ordering::Acquire){return Err(conflict());}
        let signer=self.active_signer(expected,address)?;check(deadline)?;
        let reviews=invitation.poll_requests(relay,&signer,elapsed_now(now,started)?,deadline,cancel)?;
        self.lease.check(self.database,self.profile)?;check(deadline)?;
        if cancel.load(std::sync::atomic::Ordering::Acquire){return Err(conflict());}Ok(reviews)
    }
    /// Exact retained request only; uncertainty must retain local decline fence.
    pub fn decline_host_peer(&self,expected:u64,address:CredentialAddress,invitation:&mut omniplay_streaming::native_pair_exchange::NativeHostInvitation,
        client:&str,relay:&omniplay_streaming::native_relay::NativePairRelay,now:u64,deadline:Instant,cancel:&std::sync::atomic::AtomicBool)->Result<()>{
        let started=begin(deadline)?;
        let metadata:omniplay_streaming::Invite=omniplay_streaming::inspect(invitation.signed_invite(),4096)?;
        crate::stream_signal_authority::require_installed_host_grant(&metadata.audience,false)?;
        if cancel.load(std::sync::atomic::Ordering::Acquire){return Err(conflict());}
        let signer=self.active_signer(expected,address)?;check(deadline)?;
        invitation.decline_client(relay,&signer,client,elapsed_now(now,started)?,deadline,cancel)?;
        self.lease.check(self.database,self.profile)?;check(deadline)?;
        if cancel.load(std::sync::atomic::Ordering::Acquire){return Err(conflict());}Ok(())
    }
    /// Live Active-identity close path; retain the invitation on uncertain reply.
    /// Does not revoke approved pairs or implement crash/revoking-identity cleanup.
    pub fn close_host(&self,expected:u64,address:CredentialAddress,invitation:&mut omniplay_streaming::native_pair_exchange::NativeHostInvitation,
        relay:&omniplay_streaming::native_relay::NativePairRelay,now:u64,deadline:Instant,cancel:&std::sync::atomic::AtomicBool)->Result<()>{
        let started=begin(deadline)?;
        let metadata:omniplay_streaming::Invite=omniplay_streaming::inspect(invitation.signed_invite(),4096)?;
        if !crate::stream_signal_authority::installed_retirement_origins()?.contains(&metadata.audience){return Err(conflict());}
        let signer=self.active_signer(expected,address)?;check(deadline)?;
        // close() fences locally before remote cancellation/deadline admission.
        invitation.close(relay,&signer,elapsed_now(now,started)?,deadline,cancel)?;
        self.lease.check(self.database,self.profile)?;check(deadline)?;
        if cancel.load(std::sync::atomic::Ordering::Acquire){return Err(conflict());}Ok(())
    }
    pub fn join_client(&self,expected:u64,address:CredentialAddress,pending:&mut omniplay_streaming::native_pair_exchange::NativeClientPairing,
        relay:&omniplay_streaming::native_relay::NativePairRelay,now:u64,deadline:Instant,cancel:&std::sync::atomic::AtomicBool)->Result<()>{
        let started=begin(deadline)?;let signer=self.active_signer(expected,address)?;check(deadline)?;
        pending.submit_join(relay,&signer,elapsed_now(now,started)?,deadline,cancel)?;
        self.lease.check(self.database,self.profile)?;check(deadline)?;
        if cancel.load(std::sync::atomic::Ordering::Acquire){return Err(conflict());}Ok(())
    }
    pub fn poll_client(&self,expected:u64,address:CredentialAddress,pending:&mut omniplay_streaming::native_pair_exchange::NativeClientPairing,
        relay:&omniplay_streaming::native_relay::NativePairRelay,now:u64,deadline:Instant,cancel:&std::sync::atomic::AtomicBool)
        ->Result<omniplay_streaming::native_pair_exchange::NativeClientPairingStatus>{
        let started=begin(deadline)?;let signer=self.active_signer(expected,address)?;check(deadline)?;
        let result=pending.poll_status(relay,&signer,elapsed_now(now,started)?,deadline,cancel)?;
        self.lease.check(self.database,self.profile)?;check(deadline)?;
        if cancel.load(std::sync::atomic::Ordering::Acquire){return Err(conflict());}Ok(result)
    }
    /// Native host calls only AFTER its actual local opt-in. This method is not
    /// a consent dialog and must never be exposed as a renderer signing API.
    /// The admitted identity is reloaded with its pinned public key before the
    /// journal transition; no existing peer/pair is migrated or approved here.
    pub fn activate_identity_after_local_consent(&mut self,address:CredentialAddress,expected:u64,now:u64,deadline:Instant)->Result<u64>{
        let started=begin(deadline)?;
        let snapshot=journal::load_scoped(self.database,&self.lease,self.profile)?;check(deadline)?;
        if snapshot.revision()!=expected{return Err(conflict());}
        let record=snapshot.records().iter().find(|record|record.address()==address).ok_or_else(conflict)?;
        if address.kind()!=SecretKind::DeviceSigning||record.phase()!=Phase::MaterialReady{return Err(conflict());}
        let identity=load_identity(self.store,&address,record.public_key())?;
        check(deadline)?;self.lease.check(self.database,self.profile)?;
        let updated=journal::activate(self.database,&self.lease,self.profile,expected,address,elapsed_now(now,started)?)?;
        drop(identity);check(deadline)?;Ok(updated.revision())
    }
    /// Completes the exact polled client transcript only after the real native
    /// comparison/peer consent UI approves it. Caller supplies no replacement
    /// grant, audience or key. Candidate remains MaterialReady, not Active.
    pub fn prepare_client_after_local_consent(&mut self,expected:u64,identity_address:CredentialAddress,
        pending:omniplay_streaming::native_pair_exchange::NativeClientPairing,now:u64,deadline:Instant)->Result<Candidate>{
        let started=begin(deadline)?;let deadline=deadline.min(pending.native_deadline());check(deadline)?;
        let snapshot=journal::load_scoped(self.database,&self.lease,self.profile)?;check(deadline)?;
        if snapshot.revision()!=expected{return Err(conflict());}
        let record=snapshot.records().iter().find(|record|record.address()==identity_address).ok_or_else(conflict)?;
        if identity_address.kind()!=SecretKind::DeviceSigning||record.phase()!=Phase::Active{return Err(conflict());}
        // Prove current identity possession before consuming the one-use scalar.
        let identity=load_identity(self.store,&identity_address,record.public_key())?;
        check(deadline)?;self.lease.check(self.database,self.profile)?;
        let hello:omniplay_streaming::ClientHello=omniplay_streaming::verify(record.public_key(),omniplay_streaming::HELLO_DOMAIN,pending.signed_hello(),4096)?;
        if hello.client_public_key!=identity.public_key(){return Err(conflict());}
        let document=pending.approved_grant().ok_or_else(conflict)?.clone();
        let grant:omniplay_streaming::PairGrant=omniplay_streaming::inspect(&document,8192)?;
        if grant.client_public_key!=identity.public_key(){return Err(conflict());}
        // finish rechecks the separately pinned host/transcript, joined/polled
        // approval and original monotonic invite bound before deriving material.
        let material=pending.finish_after_local_consent(elapsed_now(now,started)?)?;
        check(deadline)?;self.lease.check(self.database,self.profile)?;
        drop(identity);
        self.prepare_pair(expected,identity_address,&document,&grant.audience,&material,elapsed_now(now,started)?,deadline)
    }
    /// Actual native peer/comparison/input consent to the exact polled hello.
    /// One lease spans signer reload, single-use grant creation and persistence.
    pub fn prepare_reviewed_host_peer(&mut self,expected:u64,identity_address:CredentialAddress,
        invitation:&mut omniplay_streaming::native_pair_exchange::NativeHostInvitation,client:&str,reviewed_hello_hash:&str,
        input_allowed:bool,lifetime_ms:u64,now:u64,deadline:Instant,cancel:&std::sync::atomic::AtomicBool)->Result<Candidate>{
        let started=begin(deadline)?;
        let metadata:omniplay_streaming::Invite=omniplay_streaming::inspect(invitation.signed_invite(),4096)?;
        crate::stream_signal_authority::require_installed_host_grant(&metadata.audience,input_allowed)?;
        if cancel.load(std::sync::atomic::Ordering::Acquire){return Err(conflict());}
        let hello=invitation.signed_request(client).ok_or_else(conflict)?.clone();
        if omniplay_streaming::document_hash(&hello)?!=reviewed_hello_hash{return Err(conflict());}
        let signer=self.active_signer(expected,identity_address)?;check(deadline)?;
        let approval=invitation.prepare_approval_after_local_consent(&signer,&hello,client,input_allowed,lifetime_ms,elapsed_now(now,started)?)?;
        drop(signer);self.lease.check(self.database,self.profile)?;check(deadline)?;
        if cancel.load(std::sync::atomic::Ordering::Acquire){return Err(conflict());}
        let candidate=self.prepare_host_after_local_consent(expected,identity_address,approval,elapsed_now(now,started)?,deadline)?;
        if cancel.load(std::sync::atomic::Ordering::Acquire){return Err(PlatformError::new(ErrorCode::Cancelled,"Host candidate may have committed; reload native pairing state"));}
        Ok(candidate)
    }
    /// Persists a locally consented host approval BEFORE any ApprovePair request.
    /// The immutable signed grant goes into journal recovery metadata; no relay
    /// acknowledgment or Active transition is implied by MaterialReady.
    pub fn prepare_host_after_local_consent(&mut self,expected:u64,identity_address:CredentialAddress,
        approval:omniplay_streaming::native_pair_exchange::NativeHostApproval,now:u64,deadline:Instant)->Result<Candidate>{
        let started=begin(deadline)?;let deadline=deadline.min(approval.native_deadline());check(deadline)?;
        let snapshot=journal::load_scoped(self.database,&self.lease,self.profile)?;check(deadline)?;
        if snapshot.revision()!=expected{return Err(conflict());}
        let identity=snapshot.records().iter().find(|record|record.address()==identity_address).ok_or_else(conflict)?;
        if identity_address.kind()!=SecretKind::DeviceSigning||identity.phase()!=Phase::Active{return Err(conflict());}
        let signer=load_identity(self.store,&identity_address,identity.public_key())?;
        let document=approval.signed_grant().clone();
        let metadata:omniplay_streaming::PairGrant=omniplay_streaming::inspect(&document,8192)?;
        let grant=verify_grant(&document,&metadata.audience,elapsed_now(now,started)?)?;
        if grant.host_public_key!=signer.public_key(){return Err(conflict());}
        crate::stream_signal_authority::require_installed_host_grant(&grant.audience,grant.input_allowed)?;
        check(deadline)?;self.lease.check(self.database,self.profile)?;
        let material=approval.finish_for_persistence(elapsed_now(now,started)?)?;
        check(deadline)?;drop(signer);
        self.prepare_pair(expected,identity_address,&document,&grant.audience,&material,elapsed_now(now,started)?,deadline)
    }
    /// Re-review after interruption: public signed metadata only, no activation.
    /// Reload vault material under the scoped lease before presenting a candidate.
    pub fn review_client_candidate(&self,address:CredentialAddress,expected:u64,now:u64,deadline:Instant)->Result<serde_json::Value>{
        let started=begin(deadline)?;
        let snapshot=journal::load_scoped(self.database,&self.lease,self.profile)?;
        if snapshot.revision()!=expected{return Err(conflict());}
        let pair=snapshot.records().iter().find(|record|record.address()==address).ok_or_else(conflict)?;
        if address.kind()!=SecretKind::PairEncryption||pair.phase()!=Phase::MaterialReady||pair.signal_session().is_some(){return Err(conflict());}
        let metadata=pair.grant().ok_or_else(conflict)?;
        let grant=verify_grant(metadata.document(),metadata.audience(),elapsed_now(now,started)?)?;
        if grant.id!=address.id()||grant.client_public_key!=pair.public_key(){return Err(conflict());}
        let identity=snapshot.records().iter().find(|record|record.phase()==Phase::Active&&record.address().kind()==SecretKind::DeviceSigning&&record.public_key()==pair.public_key()).ok_or_else(conflict)?;
        let signer=load_identity(self.store,&identity.address(),identity.public_key())?;
        let material=load_pair(self.store,&address,metadata.document(),metadata.audience(),pair.public_key(),elapsed_now(now,started)?)?;
        let hash=omniplay_streaming::document_hash(metadata.document())?;
        if pair.grant_hash()!=Some(hash.as_str()){return Err(conflict());}
        drop(material);drop(signer);check(deadline)?;self.lease.check(self.database,self.profile)?;
        grant.validate(metadata.audience(),elapsed_now(now,started)?)?;
        let fingerprint=omniplay_streaming::key_id(&grant.host_public_key)?;
        Ok(serde_json::json!({"credential_protocol":1,"address":address,"revision":expected,"phase":"material_ready",
            "grant_hash":hash,"host_public_key":grant.host_public_key,"host_fingerprint":fingerprint,
            "pair_id":grant.id,"input_allowed":grant.input_allowed,"expires_at_ms":grant.expires_at_ms,
            "audience":metadata.audience(),"local_consent_required":true,"comparison_available":false}))
    }
    /// Explicit native client consent only. Reload exact stored material and
    /// obtain fresh bound relay approval before publishing Active. No replacement
    /// generation, generic boolean receipt or transport/session grant is created.
    pub fn activate_client_after_local_consent(&mut self,address:CredentialAddress,expected:u64,
        relay:&omniplay_streaming::native_relay::NativePairRelay,now:u64,deadline:Instant,cancel:&std::sync::atomic::AtomicBool)->Result<u64>{
        self.activate_consented_pair(address,expected,relay,false,now,deadline,cancel)
    }
    /// Mobile native consent binds activation to the exact reviewed signed grant.
    /// The same writer lease remains held through the subsequent activation.
    pub fn activate_reviewed_client_after_local_consent(&mut self,address:CredentialAddress,expected:u64,reviewed_hash:&str,
        relay:&omniplay_streaming::native_relay::NativePairRelay,now:u64,deadline:Instant,cancel:&std::sync::atomic::AtomicBool)->Result<u64>{
        check(deadline)?;
        let snapshot=journal::load_scoped(self.database,&self.lease,self.profile)?;
        let pair=snapshot.records().iter().find(|record|record.address()==address).ok_or_else(conflict)?;
        if snapshot.revision()!=expected||pair.grant_hash()!=Some(reviewed_hash){return Err(conflict());}
        self.activate_consented_pair(address,expected,relay,false,now,deadline,cancel)
    }
    /// Exact already-persisted host candidate only, after actual native consent.
    /// Reuses the retained signed grant for approval/reconciliation, not a new key.
    pub fn activate_host_after_local_consent(&mut self,address:CredentialAddress,expected:u64,
        relay:&omniplay_streaming::native_relay::NativePairRelay,now:u64,deadline:Instant,cancel:&std::sync::atomic::AtomicBool)->Result<u64>{
        self.activate_consented_pair(address,expected,relay,true,now,deadline,cancel)
    }
    fn activate_consented_pair(&mut self,address:CredentialAddress,expected:u64,
        relay:&omniplay_streaming::native_relay::NativePairRelay,host:bool,now:u64,deadline:Instant,cancel:&std::sync::atomic::AtomicBool)->Result<u64>{
        let started=begin(deadline)?;
        if cancel.load(std::sync::atomic::Ordering::Acquire){return Err(conflict());}
        let snapshot=journal::load_scoped(self.database,&self.lease,self.profile)?;check(deadline)?;
        if snapshot.revision()!=expected{return Err(conflict());}
        let pair=snapshot.records().iter().find(|record|record.address()==address).ok_or_else(conflict)?;
        if address.kind()!=SecretKind::PairEncryption||pair.phase()!=Phase::MaterialReady{return Err(conflict());}
        let metadata=pair.grant().ok_or_else(conflict)?;
        let grant=verify_grant(metadata.document(),metadata.audience(),elapsed_now(now,started)?)?;
        if (if host{&grant.host_public_key}else{&grant.client_public_key})!=pair.public_key(){return Err(conflict());}
        if host{crate::stream_signal_authority::require_installed_host_grant(metadata.audience(),grant.input_allowed)?;}
        let identity=snapshot.records().iter().find(|record|record.address().kind()==SecretKind::DeviceSigning&&
            record.phase()==Phase::Active&&record.public_key()==pair.public_key()).ok_or_else(conflict)?;
        let signer=load_identity(self.store,&identity.address(),identity.public_key())?;
        let material=load_pair(self.store,&address,metadata.document(),metadata.audience(),pair.public_key(),elapsed_now(now,started)?)?;
        check(deadline)?;self.lease.check(self.database,self.profile)?;
        if host{relay.approve_host_grant(&signer,metadata.audience(),metadata.document(),elapsed_now(now,started)?,deadline,cancel)?;}
        else{relay.confirm_client_grant(&signer,metadata.audience(),metadata.document(),elapsed_now(now,started)?,deadline,cancel)?;}
        check(deadline)?;self.lease.check(self.database,self.profile)?;
        if cancel.load(std::sync::atomic::Ordering::Acquire){return Err(conflict());}
        let updated=journal::activate(self.database,&self.lease,self.profile,expected,address,elapsed_now(now,started)?)?;
        drop(material);drop(signer);check(deadline)?;
        grant.validate(metadata.audience(),elapsed_now(now,started)?)?;
        if cancel.load(std::sync::atomic::Ordering::Acquire){return Err(PlatformError::new(ErrorCode::Cancelled,"Pair activation may have committed; reload native pairing state"));}
        Ok(updated.revision())
    }
    /// The key must come from the native authenticated exchange. This primitive
    /// does not derive it or prove the exchange; no browser key import exists.
    /// Approval is still pending when the returned candidate is MaterialReady.
    pub fn prepare_pair(&mut self,expected:u64,identity_address:CredentialAddress,document:&SignedDocument,
        audience:&str,key:&NativePairKey,now:u64,deadline:Instant)->Result<Candidate>{
        let started=begin(deadline)?;validate_audience(audience)?;
        let snapshot=journal::load_scoped(self.database,&self.lease,self.profile)?;check(deadline)?;
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
