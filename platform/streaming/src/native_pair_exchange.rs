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
use serde::Deserialize;

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
    joined:bool,
}
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
        let transcript=serde_json::to_vec(&["omniplay.streaming.pair-code.v1",invitation.invite.payload.as_str(),hello.payload.as_str()]).map_err(|_|invalid())?;
        let hash=Sha256::digest(&transcript);
        let number=u32::from_be_bytes([hash[0],hash[1],hash[2],hash[3]])%1_000_000;
        let comparison=format!("{number:06}");
        let current=elapsed_now(now,started)?;invite.validate(audience,current)?;
        if started.elapsed()>=MAX_EXCHANGE_AGE||Instant::now()>=deadline{return Err(unauthorized());}
        // Retain opaque proof for future native relay submission. Never expose
        // it via metadata getters/Debug/Serialize or leave its String on drop.
        let retained_secret=Zeroizing::new(std::mem::take(&mut invitation.secret));
        Ok(Self{exchange,invite:invitation.invite.clone(),hello,expected_host:expected_host.into(),audience:audience.into(),comparison,deadline,expires:invite.expires_at_ms,_secret:retained_secret,joined:false})
    }
    pub fn signed_invite(&self)->&SignedDocument{&self.invite}
    pub fn signed_hello(&self)->&SignedDocument{&self.hello}
    pub fn comparison_code(&self)->&str{&self.comparison}
    pub fn expires_at_ms(&self)->u64{self.expires}
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
    /// Native controller calls only after actual local peer comparison/approval.
    /// Finishing consumes this pending transcript even if the grant is rejected.
    pub fn finish_after_local_consent(self,grant:&SignedDocument,now:u64)->Result<NativePairKey>{
        let started=Instant::now();
        if !self.joined||Instant::now()>=self.deadline||now>=self.expires{return Err(unauthorized());}
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
