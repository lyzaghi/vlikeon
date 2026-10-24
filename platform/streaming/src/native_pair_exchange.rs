//! Single-use native P-256/HKDF exchange matching the browser signal-key KDF.
//! Local peer/comparison-code consent and relay invitation-secret proof remain
//! responsibilities of the native pairing controller; this is not a relay API.
use crate::{decode,document_hash,key,validate_audience,verify,verify_grant,invalid,unauthorized,
    SignedDocument,Invite,ClientHello,INVITE_DOMAIN,HELLO_DOMAIN,native_credentials::NativePairKey};
use base64::{Engine,engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{agreement,hkdf,rand::SystemRandom};
use sha2::{Digest,Sha256};
use std::time::{Duration,Instant};
use zeroize::Zeroizing;

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
