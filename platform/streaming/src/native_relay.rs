//! Bounded native HTTP transport for paired operations and admitted client join. Product
//! generation/session authority must be checked by its native caller. Creating
//! this client or authoring this source does not deploy or contact a relay.
use crate::{validate_audience,verify_grant,verify_grant_for_retirement,verify_request,RelayEnvelope,RelayOperation,RelayRequest,
    native_credentials::{NativeSigningIdentity,SigningDomain}};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use reqwest::{blocking::Client,redirect::Policy,header};
use serde_json::Value;
use std::{io::Read,sync::atomic::{AtomicBool,Ordering},time::{Duration,Instant}};
use uuid::Uuid;
use zeroize::Zeroizing;
use zeroize::Zeroize;
use serde::Serialize;

struct SensitiveEnvelope(RelayEnvelope);
impl Drop for SensitiveEnvelope{fn drop(&mut self){self.0.document.payload.zeroize();}}
#[derive(Serialize)]
struct JoinProof<'a>{#[serde(rename="type")]kind:&'static str,invite_id:Uuid,secret:&'a str,hello:&'a crate::SignedDocument}
#[derive(Serialize)]
struct JoinRequest<'a>{schema_version:u32,audience:&'a str,issued_at_ms:u64,nonce:Uuid,operation:JoinProof<'a>}

const MAX_RESPONSE:usize=1024*1024;
pub(crate) struct NativeJoinStatus{pub state:String,pub grant:Option<crate::SignedDocument>}
#[derive(serde::Deserialize)]#[serde(deny_unknown_fields)]
struct PairingStatusReceipt{pairing_status_protocol:u32,invite_id:Uuid,client_public_key:String,hello_hash:String,state:String,grant:Option<crate::SignedDocument>}
pub struct NativePairRelay{client:Client,allowed:Vec<String>}
impl NativePairRelay{
    /// Allowlist must come from admitted installed/native policy, never an
    /// invitation, renderer URL, remote message or mutable environment override.
    pub fn new(allowed:Vec<String>)->Result<Self>{
        if allowed.is_empty()||allowed.len()>8{return Err(denied());}
        for origin in &allowed{validate_audience(origin)?;}
        if allowed.iter().enumerate().any(|(index,origin)|allowed[..index].contains(origin)){return Err(denied());}
        let client=Client::builder().https_only(true).redirect(Policy::none()).no_proxy()
            .pool_max_idle_per_host(2)
            .connect_timeout(Duration::from_secs(5)).timeout(Duration::from_secs(15))
            .user_agent("Vlikeon-Native-Remote-Play/1").build().map_err(|_|network())?;
        Ok(Self{client,allowed})
    }
    /// No retry/queue. A send retry must reuse the caller's original encrypted
    /// packet, with a fresh signed request nonce, while authority stays live.
    /// Ambiguous network failure is not approval/revocation acknowledgment.
    pub fn request(&self,identity:&NativeSigningIdentity,audience:&str,operation:RelayOperation,
        now:u64,deadline:Instant,cancel:&AtomicBool)->Result<Value>{
        let started=Instant::now();check(deadline,cancel)?;
        if deadline.saturating_duration_since(started)>Duration::from_secs(30)||!self.allowed.iter().any(|origin|origin==audience){return Err(denied());}
        validate_audience(audience)?;
        let document=match &operation{
            RelayOperation::Send{grant,..}|RelayOperation::Receive{grant,..}|RelayOperation::IceCredentials{grant}|RelayOperation::ConfirmPair{grant}|RelayOperation::RevokePair{grant}=>grant,
            _=>return Err(PlatformError::new(ErrorCode::UnsupportedCapability,"The native paired transport does not implement invitation or approval requests")),
        };
        let retiring=matches!(&operation,RelayOperation::RevokePair{..});
        let public=identity.public_key();let grant=if retiring{verify_grant_for_retirement(document,audience,now)?}else{verify_grant(document,audience,now)?};
        if grant.id.is_nil()||(grant.host_public_key!=public&&grant.client_public_key!=public){return Err(denied());}
        let body=RelayRequest{schema_version:1,audience:audience.into(),issued_at_ms:now,nonce:Uuid::new_v4(),operation};
        let bytes=Zeroizing::new(serde_json::to_vec(&body).map_err(|_|network())?);
        let document=identity.sign(SigningDomain::Request,&bytes)?;
        let envelope=RelayEnvelope{public_key:public,document};
        let current=elapsed_now(now,started)?;
        let _=verify_request(&envelope,audience,current)?;
        if !retiring{grant.validate(audience,current)?;}
        let bytes=Zeroizing::new(serde_json::to_vec(&envelope).map_err(|_|network())?);
        let value=self.post(audience,&bytes,deadline,cancel)?;
        if !retiring{grant.validate(audience,elapsed_now(now,started)?)?;}check(deadline,cancel)?;Ok(value)
    }
    /// Only the admitted pending transcript supplies its proof. No generic
    /// caller-selected JoinInvite/signing endpoint or secret result exists.
    pub(crate) fn join_invitation(&self,identity:&NativeSigningIdentity,pending:&crate::native_pair_exchange::NativeClientPairing,
        now:u64,deadline:Instant,cancel:&AtomicBool)->Result<()>{
        let started=Instant::now();check(deadline,cancel)?;
        let(audience,invite_id,secret,hello)=pending.join_material(identity,now)?;
        let body=JoinRequest{schema_version:1,audience,issued_at_ms:now,nonce:Uuid::new_v4(),operation:JoinProof{kind:"join_invite",invite_id,secret,hello}};
        let bytes=Zeroizing::new(serde_json::to_vec(&body).map_err(|_|network())?);
        let document=identity.sign(SigningDomain::Request,&bytes)?;
        let envelope=SensitiveEnvelope(RelayEnvelope{public_key:identity.public_key(),document});
        let bytes=Zeroizing::new(serde_json::to_vec(&envelope.0).map_err(|_|network())?);
        let response=self.post(audience,&bytes,deadline,cancel)?;
        #[derive(serde::Deserialize)]#[serde(deny_unknown_fields)]
        struct Receipt{join_protocol:u32,pending:bool,invite_id:Uuid,client_public_key:String,hello_hash:String}
        let receipt:Receipt=serde_json::from_value(response).map_err(|_|network())?;
        if receipt.join_protocol!=1||!receipt.pending||receipt.invite_id!=invite_id||receipt.client_public_key!=identity.public_key()||receipt.hello_hash!=crate::document_hash(hello)?{return Err(denied());}
        let _=pending.join_material(identity,elapsed_now(now,started)?)?;check(deadline,cancel)?;Ok(())
    }
    pub(crate) fn pairing_status(&self,identity:&NativeSigningIdentity,pending:&crate::native_pair_exchange::NativeClientPairing,
        now:u64,deadline:Instant,cancel:&AtomicBool)->Result<NativeJoinStatus>{
        let started=Instant::now();check(deadline,cancel)?;
        let(audience,invite_id,_,hello)=pending.join_material(identity,now)?;
        let receipt=self.signed_pairing_status(identity,audience,invite_id,now,deadline,cancel)?;
        if receipt.hello_hash!=crate::document_hash(hello)?{return Err(denied());}
        let _=pending.join_material(identity,elapsed_now(now,started)?)?;check(deadline,cancel)?;
        Ok(NativeJoinStatus{state:receipt.state,grant:receipt.grant})
    }
    /// Current relay approval for this exact persisted client grant, not local
    /// consent or a journal activation. May be used after invite UI has closed.
    pub fn confirm_client_grant(&self,identity:&NativeSigningIdentity,audience:&str,document:&crate::SignedDocument,
        now:u64,deadline:Instant,cancel:&AtomicBool)->Result<()>{
        let started=Instant::now();check(deadline,cancel)?;
        let grant=verify_grant(document,audience,now)?;
        if grant.id.is_nil()||grant.invite_id.is_nil()||grant.client_public_key!=identity.public_key(){return Err(denied());}
        let response=self.request(identity,audience,RelayOperation::ConfirmPair{grant:document.clone()},now,deadline,cancel)?;
        #[derive(serde::Deserialize)]#[serde(deny_unknown_fields)]
        struct Receipt{pair_confirmation_protocol:u32,approved:bool,pair_id:Uuid,grant_hash:String,expires_at_ms:u64}
        let receipt:Receipt=serde_json::from_value(response).map_err(|_|network())?;
        if receipt.pair_confirmation_protocol!=1||!receipt.approved||receipt.pair_id!=grant.id||
            receipt.grant_hash!=crate::document_hash(document)?||receipt.expires_at_ms!=grant.expires_at_ms{return Err(denied());}
        grant.validate(audience,elapsed_now(now,started)?)?;check(deadline,cancel)?;Ok(())
    }
    fn signed_pairing_status(&self,identity:&NativeSigningIdentity,audience:&str,invite_id:Uuid,
        now:u64,deadline:Instant,cancel:&AtomicBool)->Result<PairingStatusReceipt>{
        let started=Instant::now();check(deadline,cancel)?;
        let body=RelayRequest{schema_version:1,audience:audience.into(),issued_at_ms:now,nonce:Uuid::new_v4(),operation:RelayOperation::PairingStatus{invite_id}};
        let document=identity.sign(SigningDomain::Request,&serde_json::to_vec(&body).map_err(|_|network())?)?;
        let envelope=RelayEnvelope{public_key:identity.public_key(),document};
        let _=verify_request(&envelope,audience,elapsed_now(now,started)?)?;
        let response=self.post(audience,&serde_json::to_vec(&envelope).map_err(|_|network())?,deadline,cancel)?;
        let receipt:PairingStatusReceipt=serde_json::from_value(response).map_err(|_|network())?;
        if receipt.pairing_status_protocol!=1||receipt.invite_id!=invite_id||receipt.client_public_key!=identity.public_key()||
            !matches!(receipt.state.as_str(),"pending"|"declined"|"approved")||
            (receipt.state=="approved")!=receipt.grant.is_some(){return Err(denied());}
        check(deadline,cancel)?;Ok(receipt)
    }
    fn post(&self,audience:&str,bytes:&[u8],deadline:Instant,cancel:&AtomicBool)->Result<Value>{
        let started=Instant::now();check(deadline,cancel)?;validate_audience(audience)?;
        if deadline.saturating_duration_since(started)>Duration::from_secs(30)||!self.allowed.iter().any(|origin|origin==audience){return Err(denied());}
        if bytes.len()>192*1024{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Native relay request exceeds its fixed bound"));}
        check(deadline,cancel)?;
        let remaining=deadline.saturating_duration_since(Instant::now()).min(Duration::from_secs(15));
        if remaining.is_zero(){return Err(network());}
        // No cookies, ambient proxy, authorization header, redirects, arbitrary
        // path or caller headers. Standard TLS certificate verification stays on.
        let mut response=self.client.post(format!("{audience}/v1/relay"))
            .header(header::CONTENT_TYPE,"application/json").header(header::ACCEPT,"application/json")
            .timeout(remaining).body(bytes.to_vec()).send().map_err(|_|network())?;
        check(deadline,cancel)?;
        if response.content_length().is_some_and(|length|length>MAX_RESPONSE as u64){return Err(too_large());}
        let content_type=response.headers().get(header::CONTENT_TYPE).and_then(|value|value.to_str().ok()).ok_or_else(network)?;
        if !content_type.split(';').next().is_some_and(|value|value.trim().eq_ignore_ascii_case("application/json")){return Err(network());}
        if response.headers().get(header::CONTENT_ENCODING).is_some_and(|value|value.as_bytes()!=b"identity"){return Err(network());}
        let status=response.status();let mut bytes=Zeroizing::new(Vec::new());let mut chunk=[0u8;8192];
        loop{
            check(deadline,cancel)?;let count=response.read(&mut chunk).map_err(|_|network())?;
            if count==0{break;}if bytes.len().saturating_add(count)>MAX_RESPONSE{return Err(too_large());}
            bytes.extend_from_slice(&chunk[..count]);
        }
        check(deadline,cancel)?;
        let value:Value=serde_json::from_slice(&bytes).map_err(|_|network())?;
        if !value.is_object(){return Err(network());}
        if !status.is_success(){
            let code=match status.as_u16(){400=>ErrorCode::InvalidRequest,401|403=>ErrorCode::PermissionDenied,
                404=>ErrorCode::NotFound,409=>ErrorCode::Conflict,413|429=>ErrorCode::ResourceLimit,_=>ErrorCode::RuntimeUnavailable};
            return Err(PlatformError::new(code,"The configured native relay rejected the operation"));
        }
        // No positive HTTP result after cancellation/deadline. Callers also
        // recheck their own grant/invitation lifetime before publishing it.
        check(deadline,cancel)?;Ok(value)
    }
}
fn check(deadline:Instant,cancel:&AtomicBool)->Result<()>{
    if cancel.load(Ordering::Acquire){return Err(PlatformError::new(ErrorCode::Cancelled,"Native relay operation was cancelled"));}
    if Instant::now()>=deadline{return Err(PlatformError::new(ErrorCode::DeadlineExceeded,"Native relay exceeded its original operation deadline"));}Ok(())
}
fn elapsed_now(now:u64,started:Instant)->Result<u64>{now.checked_add(u64::try_from(started.elapsed().as_millis()).map_err(|_|network())?).ok_or_else(network)}
fn denied()->PlatformError{PlatformError::new(ErrorCode::PermissionDenied,"This relay or native identity is not authorized for the paired operation")}
fn network()->PlatformError{PlatformError::new(ErrorCode::RuntimeUnavailable,"The configured native relay could not complete the operation")}
fn too_large()->PlatformError{PlatformError::new(ErrorCode::ResourceLimit,"Native relay response exceeds its fixed bound")}
