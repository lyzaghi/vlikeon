//! Native signal AEAD primitive. Sequence reservation and inbox acceptance must
//! be durably journaled by the native transport before send/delivery respectively.
//! Authentication alone is not replay protection or live session authority.
use crate::{invalid,unauthorized,key,verify_grant,decode,MAX_PACKET_BYTES,SignedDocument,
    native_credentials::{NativePairKey,validate_object}};
use base64::{Engine,engine::general_purpose::URL_SAFE_NO_PAD};
use omniplay_contracts::Result;
use ring::aead;
use serde::{Deserialize,Serialize};
use std::time::Instant;
use uuid::Uuid;
use zeroize::Zeroizing;

#[derive(Clone,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct SignalMetadata{pub message_id:Uuid,pub session_id:Uuid,pub sequence:u64,pub expires_at_ms:u64}
impl SignalMetadata{
    fn validate(&self)->Result<()>{
        if self.message_id.is_nil()||self.session_id.is_nil()||!(1..=1_000_000).contains(&self.sequence)||
            self.expires_at_ms==0||self.expires_at_ms>9_007_199_254_740_991{return Err(invalid());}Ok(())
    }
}
#[derive(Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct EncryptedSignal{pub iv:String,pub ciphertext:String}

/// Holds no raw-key export. The inner key is dropped with this native owner.
/// Caller must retire it on journal revoke/epoch change, not merely rely on the
/// grant's certificate expiry or retain this object as indefinite authority.
pub struct NativeSignalCipher{key:aead::LessSafeKey,pair:Uuid,local:String,peer:String,expires:u64}
impl NativeSignalCipher{
    pub fn new(material:NativePairKey,document:&SignedDocument,audience:&str,local:&str,now:u64)->Result<Self>{
        crate::validate_audience(audience)?;key(local)?;
        let grant=verify_grant(document,audience,now)?;
        if grant.id.is_nil(){return Err(invalid());}
        let peer=if local==grant.host_public_key{grant.client_public_key}else if local==grant.client_public_key{grant.host_public_key}else{return Err(unauthorized());};
        Ok(Self{key:material.into_signal_key()?,pair:grant.id,local:local.into(),peer,expires:grant.expires_at_ms})
    }
    pub fn pair(&self)->Uuid{self.pair}
    pub fn peer(&self)->&str{&self.peer}
    /// Metadata sequence must have been durably reserved BEFORE calling this.
    /// Retries reuse the original encrypted packet; do not reseal a send retry.
    pub fn seal_reserved(&self,metadata:&SignalMetadata,body:&[u8],now:u64)->Result<EncryptedSignal>{
        let started=Instant::now();metadata.validate()?;
        if now>=self.expires||metadata.expires_at_ms<=now||metadata.expires_at_ms>now.saturating_add(60_000)||metadata.expires_at_ms>self.expires{return Err(unauthorized());}
        if body.is_empty()||body.len()>MAX_PACKET_BYTES-16{return Err(invalid());}validate_object(body)?;
        let aad=aad(self.pair,&self.local,metadata)?;
        let mut nonce=[0u8;12];getrandom::getrandom(&mut nonce).map_err(|_|invalid())?;
        let mut buffer=Zeroizing::new(Vec::with_capacity(body.len()+16));buffer.extend_from_slice(body);
        self.key.seal_in_place_append_tag(aead::Nonce::assume_unique_for_key(nonce),aead::Aad::from(aad.as_slice()),&mut *buffer).map_err(|_|unauthorized())?;
        let current=after(now,started)?;
        if current>=self.expires||current>=metadata.expires_at_ms{return Err(unauthorized());}
        Ok(EncryptedSignal{iv:URL_SAFE_NO_PAD.encode(nonce),ciphertext:URL_SAFE_NO_PAD.encode(buffer.as_slice())})
    }
    /// Returns authenticated JSON object bytes. Native transport must commit
    /// sender sequence/cursor acceptance and recheck current authority before
    /// delivering these bytes; this function does not update replay state.
    pub fn open_uncommitted(&self,sender:&str,metadata:&SignalMetadata,encrypted:&EncryptedSignal,now:u64)->Result<Zeroizing<Vec<u8>>>{
        let started=Instant::now();metadata.validate()?;
        if sender!=self.peer||now>=self.expires||metadata.expires_at_ms<=now.saturating_sub(30_000)||metadata.expires_at_ms>now.saturating_add(90_000)||metadata.expires_at_ms>self.expires{return Err(unauthorized());}
        let nonce: [u8;12]=decode(&encrypted.iv,12)?.try_into().map_err(|_|invalid())?;
        let mut buffer=Zeroizing::new(decode(&encrypted.ciphertext,MAX_PACKET_BYTES)?);
        if buffer.len()<16{return Err(invalid());}
        let aad=aad(self.pair,sender,metadata)?;
        let length=self.key.open_in_place(aead::Nonce::assume_unique_for_key(nonce),aead::Aad::from(aad.as_slice()),&mut buffer[..]).map_err(|_|unauthorized())?.len();
        // Clear the removed tag/tail as well as the surviving plaintext on Drop.
        use zeroize::Zeroize;buffer[length..].zeroize();buffer.truncate(length);
        validate_object(&buffer)?;
        let current=after(now,started)?;
        if current>=self.expires||metadata.expires_at_ms<=current.saturating_sub(30_000){return Err(unauthorized());}
        Ok(buffer)
    }
}
fn aad(pair:Uuid,sender:&str,metadata:&SignalMetadata)->Result<Vec<u8>>{
    // Same JSON array/order and integer layout as browser signalAad. All values
    // are canonical UUID/base64url/JS-safe integers; no alternate JSON tree.
    serde_json::to_vec(&("omniplay.streaming.signal.v1",pair,sender,metadata.message_id,
        metadata.session_id,metadata.sequence,metadata.expires_at_ms)).map_err(|_|invalid())
}
fn after(now:u64,started:Instant)->Result<u64>{
    now.checked_add(u64::try_from(started.elapsed().as_millis()).map_err(|_|invalid())?).ok_or_else(invalid)
}
