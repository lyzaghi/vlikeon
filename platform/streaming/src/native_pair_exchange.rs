//! Single-use native P-256/HKDF exchange matching the browser signal-key KDF.
//! Local peer/comparison-code consent and relay invitation-secret proof remain
//! responsibilities of the native pairing controller; this is not a relay API.
use crate::{decode,document_hash,key,validate_audience,verify,verify_grant,invalid,unauthorized,
    SignedDocument,Invite,ClientHello,INVITE_DOMAIN,HELLO_DOMAIN,native_credentials::NativePairKey};
use base64::{Engine,engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{agreement,hkdf,rand::SystemRandom};
use sha2::{Digest,Sha256};
use std::time::{Duration,Instant};
use zeroize::{Zeroize,Zeroizing};
use serde::{Deserialize,Serialize};

/// Native host invitation. Share code deliberately contains bearer proof:
/// present only through an explicit native share action, never diagnostics.
pub struct NativeHostInvitation{document:SignedDocument,secret:Zeroizing<String>,host:String,audience:String,deadline:Instant,attempted:std::collections::BTreeSet<String>}
pub struct NativeHostPeerReview{pub client_public_key:String,pub label:String,pub comparison_code:String}
/// Locally consented host transcript, NOT relay approval. Persist material
/// before dispatching its public signed grant to ApprovePair.
pub struct NativeHostApproval{exchange:NativePairExchange,invite:SignedDocument,hello:SignedDocument,grant:SignedDocument,client:String,audience:String,deadline:Instant}
impl NativeHostInvitation{
    pub fn create(identity:&crate::native_credentials::NativeSigningIdentity,audience:&str,label:&str,now:u64)->Result<Self>{
        let started=Instant::now();validate_audience(audience)?;
        if label.is_empty()||label.len()>120||label.chars().any(char::is_control)||now>9_007_199_254_440_991{return Err(invalid());}
        let mut raw=Zeroizing::new([0u8;32]);getrandom::getrandom(&mut raw[..]).map_err(|_|unavailable())?;
        let invite=Invite{schema_version:1,id:uuid::Uuid::new_v4(),audience:audience.into(),host_public_key:identity.public_key(),label:label.into(),
            secret_hash:omniplay_contracts::ContentHash(format!("{:x}",Sha256::digest(&raw[..]))),created_at_ms:now,expires_at_ms:now+300_000};
        invite.validate(audience,now)?;
        let document=identity.sign(crate::native_credentials::SigningDomain::Invite,&serde_json::to_vec(&invite).map_err(|_|invalid())?)?;
        let deadline=started.checked_add(MAX_EXCHANGE_AGE).ok_or_else(invalid)?;
        if Instant::now()>=deadline{return Err(unauthorized());}
        Ok(Self{document,secret:Zeroizing::new(URL_SAFE_NO_PAD.encode(&raw[..])),host:identity.public_key(),audience:audience.into(),deadline,attempted:std::collections::BTreeSet::new()})
    }
    pub fn signed_invite(&self)->&SignedDocument{&self.document}
    pub fn share_code(&self,now:u64)->Result<Zeroizing<String>>{
        self.live(now)?;
        #[derive(Serialize)]struct Share<'a>{invite:&'a SignedDocument,secret:&'a str}
        let bytes=Zeroizing::new(serde_json::to_vec(&Share{invite:&self.document,secret:self.secret.as_str()}).map_err(|_|invalid())?);
        let mut code=Zeroizing::new(String::from("OPRP1."));URL_SAFE_NO_PAD.encode_string(&bytes[..],&mut code);
        if Instant::now()>=self.deadline{return Err(unauthorized());}Ok(code)
    }
    fn live(&self,now:u64)->Result<Invite>{
        if Instant::now()>=self.deadline{return Err(unauthorized());}
        let invite:Invite=verify(&self.host,INVITE_DOMAIN,&self.document,4096)?;invite.validate(&self.audience,now)?;Ok(invite)
    }
    pub fn review_client(&self,hello:&SignedDocument,expected_client:&str,now:u64)->Result<NativeHostPeerReview>{
        let started=Instant::now();let invite=self.live(now)?;key(expected_client)?;
        if expected_client==self.host{return Err(unauthorized());}
        let peer:ClientHello=verify(expected_client,HELLO_DOMAIN,hello,4096)?;peer.validate(invite.id,now)?;
        if peer.client_public_key!=expected_client||peer.created_at_ms<invite.created_at_ms.saturating_sub(30_000)||peer.created_at_ms>=invite.expires_at_ms{return Err(unauthorized());}
        let comparison=comparison_code(&self.document,hello)?;let _=self.live(elapsed_now(now,started)?)?;
        Ok(NativeHostPeerReview{client_public_key:peer.client_public_key,label:peer.label,comparison_code:comparison})
    }
    /// Real native peer/comparison/input-permission consent must precede this.
    /// One attempt per client key, at most eight; retain returned approval for
    /// persistence/retry, never regenerate a competing grant after uncertainty.
    pub fn prepare_approval_after_local_consent(&mut self,identity:&crate::native_credentials::NativeSigningIdentity,
        hello:&SignedDocument,expected_client:&str,input_allowed:bool,lifetime_ms:u64,now:u64)->Result<NativeHostApproval>{
        let started=Instant::now();let invite=self.live(now)?;let _=self.review_client(hello,expected_client,now)?;
        if identity.public_key()!=self.host||lifetime_ms==0||lifetime_ms>crate::MAX_PAIR_LIFETIME||now.checked_add(lifetime_ms).is_none_or(|expiry|expiry>9_007_199_254_740_991)||self.attempted.len()>=8||self.attempted.contains(expected_client){return Err(unauthorized());}
        self.attempted.insert(expected_client.into());
        let peer:ClientHello=verify(expected_client,HELLO_DOMAIN,hello,4096)?;
        let exchange=NativePairExchange::new(&self.host)?;
        let grant=crate::PairGrant{schema_version:1,id:uuid::Uuid::new_v4(),invite_id:invite.id,audience:self.audience.clone(),host_public_key:self.host.clone(),client_public_key:expected_client.into(),
            client_hello_hash:omniplay_contracts::ContentHash(document_hash(hello)?),client_ecdh:peer.client_ecdh,host_ecdh:exchange.public_key().into(),client_nonce:peer.client_nonce,host_nonce:exchange.nonce().into(),
            input_allowed,created_at_ms:now,expires_at_ms:now+lifetime_ms};
        grant.validate(&self.audience,now)?;
        let document=identity.sign(crate::native_credentials::SigningDomain::Grant,&serde_json::to_vec(&grant).map_err(|_|invalid())?)?;
        let _=self.live(elapsed_now(now,started)?)?;
        grant.validate(&self.audience,elapsed_now(now,started)?)?;
        Ok(NativeHostApproval{exchange,invite:self.document.clone(),hello:hello.clone(),grant:document,client:expected_client.into(),audience:self.audience.clone(),deadline:self.deadline})
    }
}
impl NativeHostApproval{
    pub fn signed_grant(&self)->&SignedDocument{&self.grant}
    pub fn native_deadline(&self)->Instant{self.deadline}
    pub fn finish_for_persistence(self,now:u64)->Result<NativePairKey>{
        if Instant::now()>=self.deadline{return Err(unauthorized());}
        let material=self.exchange.finish_host(&self.client,&self.invite,&self.hello,&self.grant,&self.audience,now)?;
        if Instant::now()>=self.deadline{return Err(unauthorized());}Ok(material)
    }
}
fn comparison_code(invite:&SignedDocument,hello:&SignedDocument)->Result<String>{
    let transcript=serde_json::to_vec(&["omniplay.streaming.pair-code.v1",invite.payload.as_str(),hello.payload.as_str()]).map_err(|_|invalid())?;
    let hash=Sha256::digest(&transcript);let number=u32::from_be_bytes([hash[0],hash[1],hash[2],hash[3]])%1_000_000;Ok(format!("{number:06}"))
}

#[derive(Deserialize)]#[serde(deny_unknown_fields)]
struct InvitationCode{invite:SignedDocument,secret:String}
impl Drop for InvitationCode{fn drop(&mut self){self.secret.zeroize();}}

/// Native pending client transcript, not peer approval or an active pair.
/// No Clone/Debug/Serialize; keeps the exact exchange, invite and signed hello.
/// The trusted controller must still submit JoinInvite proof and obtain local
/// comparison-code consent before accepting/activating an approved grant.
pub struct NativeClientPairing{
    exchange:NativePairExchange,invite:SignedDocument,hello:SignedDocument,
    expected_host:String,audience:String,comparison:String,deadline:Instant,expires:u64,
    _secret:Zeroizing<String>,
    joined:bool,approved:Option<SignedDocument>,declined:bool,
}
/// Public native UI metadata only. Approved still requires actual local peer
/// consent, exchange completion, vault/journal persistence and activation.
pub enum NativeClientPairingStatus{Pending,Declined,Approved{pair_id:uuid::Uuid,input_allowed:bool,expires_at_ms:u64}}
impl NativeClientPairing{
    /// expected_host and audience must be pinned by trusted native policy/UI,
    /// never inferred as authorized solely from the scanned invitation itself.
    /// This validates the invitation secret locally but sends nothing to relay.
    pub fn from_invitation_code(identity:&crate::native_credentials::NativeSigningIdentity,
        code:&str,expected_host:&str,audience:&str,label:&str,now:u64)->Result<Self>{
        let started=Instant::now();validate_audience(audience)?;key(expected_host)?;
        let encoded=code.strip_prefix("OPRP1.").ok_or_else(invalid)?;
        let bytes=Zeroizing::new(decode(encoded,8192)?);
        let mut invitation:InvitationCode=serde_json::from_slice(&bytes).map_err(|_|invalid())?;
        let invite:Invite=verify(expected_host,INVITE_DOMAIN,&invitation.invite,4096)?;
        invite.validate(audience,now)?;
        let local_public=identity.public_key();
        if invite.id.is_nil()||invite.host_public_key!=expected_host||local_public==expected_host||
            now>9_007_199_254_740_991||invite.expires_at_ms>9_007_199_254_740_991{return Err(unauthorized());}
        let secret=Zeroizing::new(decode(&invitation.secret,32)?);
        if secret.len()!=32||format!("{:x}",Sha256::digest(&secret[..]))!=invite.secret_hash.0{return Err(unauthorized());}
        if label.is_empty()||label.len()>120||label.chars().any(char::is_control){return Err(invalid());}
        let deadline=started.checked_add(Duration::from_millis(invite.expires_at_ms-now)).ok_or_else(invalid)?;
        // Exchange and nonce belong to this one pending transcript. The signed
        // hello cannot later be substituted by a caller when finishing it.
        let exchange=NativePairExchange::new(&local_public)?;
        let body=ClientHello{schema_version:1,invite_id:invite.id,client_public_key:local_public,
            client_ecdh:exchange.public_key().into(),client_nonce:exchange.nonce().into(),label:label.into(),created_at_ms:now};
        body.validate(invite.id,now)?;
        let hello=identity.sign(crate::native_credentials::SigningDomain::Hello,&serde_json::to_vec(&body).map_err(|_|invalid())?)?;
        let comparison=comparison_code(&invitation.invite,&hello)?;
        let current=elapsed_now(now,started)?;invite.validate(audience,current)?;
        if started.elapsed()>=MAX_EXCHANGE_AGE||Instant::now()>=deadline{return Err(unauthorized());}
        // Retain opaque proof for future native relay submission. Never expose
        // it via metadata getters/Debug/Serialize or leave its String on drop.
        let retained_secret=Zeroizing::new(std::mem::take(&mut invitation.secret));
        Ok(Self{exchange,invite:invitation.invite.clone(),hello,expected_host:expected_host.into(),audience:audience.into(),comparison,deadline,expires:invite.expires_at_ms,_secret:retained_secret,joined:false,approved:None,declined:false})
    }
    pub fn signed_invite(&self)->&SignedDocument{&self.invite}
    pub fn signed_hello(&self)->&SignedDocument{&self.hello}
    pub fn comparison_code(&self)->&str{&self.comparison}
    pub fn expires_at_ms(&self)->u64{self.expires}
    /// Native writer must retain this original bound through vault publication,
    /// not restart the invitation lifetime after ECDH or network completion.
    pub fn native_deadline(&self)->Instant{self.deadline}
    /// Signed public metadata for the native writer, not an activation receipt.
    pub fn approved_grant(&self)->Option<&SignedDocument>{self.approved.as_ref()}
    pub(crate) fn join_material<'a>(&'a self,identity:&crate::native_credentials::NativeSigningIdentity,now:u64)->Result<(&'a str,uuid::Uuid,&'a str,&'a SignedDocument)>{
        if Instant::now()>=self.deadline||now>=self.expires{return Err(unauthorized());}
        let invite:Invite=verify(&self.expected_host,INVITE_DOMAIN,&self.invite,4096)?;invite.validate(&self.audience,now)?;
        let hello:ClientHello=verify(&identity.public_key(),HELLO_DOMAIN,&self.hello,4096)?;hello.validate(invite.id,now)?;
        if hello.client_public_key!=identity.public_key(){return Err(unauthorized());}
        Ok((&self.audience,invite.id,self._secret.as_str(),&self.hello))
    }
    /// Explicit native controller dispatch; retry retains the same hello/proof.
    /// A pending receipt is not host approval, local consent or pair activation.
    pub fn submit_join(&mut self,relay:&crate::native_relay::NativePairRelay,identity:&crate::native_credentials::NativeSigningIdentity,
        now:u64,deadline:Instant,cancel:&std::sync::atomic::AtomicBool)->Result<()>{
        let started=Instant::now();
        if deadline<=started||deadline.saturating_duration_since(started)>Duration::from_secs(30){return Err(unauthorized());}
        relay.join_invitation(identity,self,now,deadline.min(self.deadline),cancel)?;
        self.joined=true;Ok(())
    }
    /// One explicit bounded poll, no background retry loop. Pin the first exact
    /// approved grant; a later conflicting or regressed response is refused.
    pub fn poll_status(&mut self,relay:&crate::native_relay::NativePairRelay,identity:&crate::native_credentials::NativeSigningIdentity,
        now:u64,deadline:Instant,cancel:&std::sync::atomic::AtomicBool)->Result<NativeClientPairingStatus>{
        let started=Instant::now();
        if !self.joined||deadline<=started||deadline.saturating_duration_since(started)>Duration::from_secs(30){return Err(unauthorized());}
        let response=relay.pairing_status(identity,self,now,deadline.min(self.deadline),cancel)?;
        let current=elapsed_now(now,started)?;
        let mut approved=None;
        let result=match response.state.as_str(){
            "pending"=>{if self.declined||self.approved.is_some(){return Err(unauthorized());}NativeClientPairingStatus::Pending},
            "declined"=>{if self.approved.is_some(){return Err(unauthorized());}NativeClientPairingStatus::Declined},
            "approved"=>{
                if self.declined{return Err(unauthorized());}
                let document=response.grant.ok_or_else(invalid)?;
                let grant:crate::PairGrant=verify(&self.expected_host,crate::GRANT_DOMAIN,&document,8192)?;grant.validate(&self.audience,current)?;
                let invite:Invite=verify(&self.expected_host,INVITE_DOMAIN,&self.invite,4096)?;invite.validate(&self.audience,current)?;
                let hello:ClientHello=verify(&identity.public_key(),HELLO_DOMAIN,&self.hello,4096)?;hello.validate(invite.id,current)?;
                if grant.id.is_nil()||grant.invite_id!=invite.id||grant.host_public_key!=self.expected_host||grant.client_public_key!=identity.public_key()||
                    grant.client_hello_hash.0!=document_hash(&self.hello)?||grant.client_ecdh!=hello.client_ecdh||grant.client_nonce!=hello.client_nonce||
                    grant.created_at_ms<invite.created_at_ms.saturating_sub(30_000)||grant.created_at_ms<hello.created_at_ms.saturating_sub(30_000)||grant.created_at_ms>=invite.expires_at_ms{return Err(unauthorized());}
                if let Some(previous)=&self.approved{if document_hash(previous)?!=document_hash(&document)?{return Err(unauthorized());}}
                let status=NativeClientPairingStatus::Approved{pair_id:grant.id,input_allowed:grant.input_allowed,expires_at_ms:grant.expires_at_ms};
                approved=Some(document);status
            },_=>return Err(invalid()),
        };
        if Instant::now()>=deadline.min(self.deadline)||cancel.load(std::sync::atomic::Ordering::Acquire){return Err(unauthorized());}
        if let Some(document)=&approved{
            let grant:crate::PairGrant=verify(&self.expected_host,crate::GRANT_DOMAIN,document,8192)?;grant.validate(&self.audience,elapsed_now(now,started)?)?;
        }
        let _=self.join_material(identity,elapsed_now(now,started)?)?;
        if Instant::now()>=deadline.min(self.deadline)||cancel.load(std::sync::atomic::Ordering::Acquire){return Err(unauthorized());}
        if let Some(document)=approved{self.approved=Some(document);}
        if matches!(&result,NativeClientPairingStatus::Declined){self.declined=true;}Ok(result)
    }
    /// Native controller calls only after actual local peer comparison/approval.
    /// Finishing consumes this pending transcript even if the grant is rejected.
    pub fn finish_after_local_consent(self,now:u64)->Result<NativePairKey>{
        let started=Instant::now();
        if !self.joined||Instant::now()>=self.deadline||now>=self.expires{return Err(unauthorized());}
        let grant=self.approved.as_ref().ok_or_else(unauthorized)?;
        let material=self.exchange.finish_client(&self.expected_host,&self.invite,&self.hello,grant,&self.audience,elapsed_now(now,started)?)?;
        if Instant::now()>=self.deadline{return Err(unauthorized());}Ok(material)
    }
}
fn elapsed_now(now:u64,started:Instant)->Result<u64>{
    now.checked_add(u64::try_from(started.elapsed().as_millis()).map_err(|_|invalid())?).ok_or_else(invalid)
}

const MAX_EXCHANGE_AGE:Duration=Duration::from_secs(300);
const INFO:&[u8]=b"omniplay.streaming.signal-key.v1";

/// No Clone, serialization or private-key export. Finishing consumes the
/// exchange, including all rejection/error paths; never retry a used scalar.
pub struct NativePairExchange{
    private:agreement::EphemeralPrivateKey,public:String,nonce:String,local_public:String,started:Instant,
}
impl NativePairExchange{
    pub fn new(local_public:&str)->Result<Self>{
        key(local_public)?;let started=Instant::now();
        let private=agreement::EphemeralPrivateKey::generate(&agreement::ECDH_P256,&SystemRandom::new()).map_err(|_|unavailable())?;
        let public=private.compute_public_key().map_err(|_|unavailable())?;
        if public.as_ref().len()!=65||public.as_ref()[0]!=4{return Err(unavailable());}
        let mut nonce=Zeroizing::new([0u8;32]);getrandom::getrandom(&mut nonce[..]).map_err(|_|unavailable())?;
        Ok(Self{private,public:URL_SAFE_NO_PAD.encode(public.as_ref()),nonce:URL_SAFE_NO_PAD.encode(&nonce[..]),local_public:local_public.into(),started})
    }
    pub fn public_key(&self)->&str{&self.public}
    pub fn nonce(&self)->&str{&self.nonce}
    /// expected_host is the peer pinned by the native invitation/verification
    /// flow, never a key accepted merely because an arriving grant claims it.
    pub fn finish_client(self,expected_host:&str,invite:&SignedDocument,hello:&SignedDocument,grant:&SignedDocument,audience:&str,now:u64)->Result<NativePairKey>{
        self.finish(false,expected_host,invite,hello,grant,audience,now)
    }
    /// expected_client is the locally approved hello peer. Host ECDH and nonce
    /// must come from this exact native exchange in its signed grant.
    pub fn finish_host(self,expected_client:&str,invite:&SignedDocument,hello:&SignedDocument,grant:&SignedDocument,audience:&str,now:u64)->Result<NativePairKey>{
        self.finish(true,expected_client,invite,hello,grant,audience,now)
    }
    fn finish(self,host:bool,expected_peer:&str,invite_document:&SignedDocument,hello_document:&SignedDocument,grant_document:&SignedDocument,audience:&str,now:u64)->Result<NativePairKey>{
        let operation_started=Instant::now();
        if self.started.elapsed()>=MAX_EXCHANGE_AGE{return Err(unauthorized());}
        key(expected_peer)?;validate_audience(audience)?;
        if expected_peer==self.local_public{return Err(unauthorized());}
        let(host_public,client_public)=if host{(self.local_public.as_str(),expected_peer)}else{(expected_peer,self.local_public.as_str())};
        let invite:Invite=verify(host_public,INVITE_DOMAIN,invite_document,4096)?;invite.validate(audience,now)?;
        if invite.id.is_nil()||invite.host_public_key!=host_public{return Err(unauthorized());}
        let hello:ClientHello=verify(client_public,HELLO_DOMAIN,hello_document,4096)?;hello.validate(invite.id,now)?;
        if hello.client_public_key!=client_public||hello.created_at_ms<invite.created_at_ms.saturating_sub(30_000)||hello.created_at_ms>=invite.expires_at_ms{return Err(unauthorized());}
        let grant=verify_grant(grant_document,audience,now)?;
        if grant.id.is_nil()||grant.invite_id!=invite.id||grant.host_public_key!=host_public||grant.client_public_key!=client_public||
            grant.client_hello_hash.0!=document_hash(hello_document)?||grant.client_ecdh!=hello.client_ecdh||grant.client_nonce!=hello.client_nonce||
            grant.created_at_ms<invite.created_at_ms.saturating_sub(30_000)||grant.created_at_ms<hello.created_at_ms.saturating_sub(30_000)||grant.created_at_ms>=invite.expires_at_ms{return Err(unauthorized());}
        let peer=if host{
            if grant.host_ecdh!=self.public||grant.host_nonce!=self.nonce{return Err(unauthorized());}
            &grant.client_ecdh
        }else{
            if hello.client_ecdh!=self.public||hello.client_nonce!=self.nonce{return Err(unauthorized());}
            &grant.host_ecdh
        };
        let peer=decode(peer,65)?;
        if peer.len()!=65||peer[0]!=4||URL_SAFE_NO_PAD.encode(&peer)==self.public{return Err(invalid());}
        // Browser salt is SHA256(UTF8(JSON.stringify({payload,signature}))).
        // SignedDocument declares that exact field order; no parsed/re-encoded
        // grant payload is substituted into the KDF transcript.
        let transcript=serde_json::to_vec(grant_document).map_err(|_|invalid())?;
        let salt=Sha256::digest(&transcript);
        let peer=agreement::UnparsedPublicKey::new(&agreement::ECDH_P256,&peer);
        let bytes=agreement::agree_ephemeral(self.private,&peer,|material|{
            let salt=hkdf::Salt::new(hkdf::HKDF_SHA256,&salt);let extracted=salt.extract(material);
            let info=[INFO];let output=extracted.expand(&info,SignalKeyLength).map_err(|_|unavailable())?;
            let mut bytes=Zeroizing::new([0u8;32]);output.fill(&mut bytes[..]).map_err(|_|unavailable())?;
            Ok::<_,omniplay_contracts::PlatformError>(bytes)
        }).map_err(|_|unauthorized())??;
        let elapsed=u64::try_from(operation_started.elapsed().as_millis()).map_err(|_|invalid())?;
        let current=now.checked_add(elapsed).ok_or_else(invalid)?;
        // Derivation does not extend the invite or grant's original lifetime.
        invite.validate(audience,current)?;grant.validate(audience,current)?;
        if self.started.elapsed()>=MAX_EXCHANGE_AGE{return Err(unauthorized());}
        Ok(NativePairKey::from_derived(bytes))
    }
}
struct SignalKeyLength;
impl hkdf::KeyType for SignalKeyLength{fn len(&self)->usize{32}}
fn unavailable()->omniplay_contracts::PlatformError{
    omniplay_contracts::PlatformError::new(omniplay_contracts::ErrorCode::RuntimeUnavailable,"Native pairing cryptography is unavailable")
}
