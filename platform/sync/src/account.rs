use crate::{error,random};
use base64::{Engine,engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{SigningKey,Signer};
use hkdf::Hkdf;
use omniplay_contracts::Result;
use sha2::{Digest,Sha256};
use uuid::Uuid;
use zeroize::Zeroizing;

/// Root authority and encryption keys stay on the client. The server receives
/// only the public root and root-authorized device certificates.
pub struct AccountIdentity {
    seed:Zeroizing<[u8;32]>,
    root:SigningKey,
    pub(crate) encryption_key:Zeroizing<[u8;32]>,
    id:String,
}

impl AccountIdentity {
    pub fn generate()->Result<Self> { Self::from_seed(random()?) }

    pub fn from_seed(seed:[u8;32])->Result<Self> {
        let seed=Zeroizing::new(seed);
        let hkdf=Hkdf::<Sha256>::new(Some(b"OmniPlay account v1"),seed.as_ref());
        let mut signing=Zeroizing::new([0u8;32]);
        let mut encryption_key=Zeroizing::new([0u8;32]);
        hkdf.expand(b"root signing key",signing.as_mut()).map_err(|_|error("Account signing key derivation failed"))?;
        hkdf.expand(b"save encryption key",encryption_key.as_mut()).map_err(|_|error("Account encryption key derivation failed"))?;
        let root=SigningKey::from_bytes(&signing);
        let id=format!("{:x}",Sha256::digest(root.verifying_key().as_bytes()));
        Ok(Self {seed,root,encryption_key,id})
    }

    pub fn recover(recovery:&str)->Result<Self> {
        if recovery.len()!=47||!recovery.starts_with("OP1-"){return Err(error("Recovery key format is invalid"));}
        let bytes=Zeroizing::new(URL_SAFE_NO_PAD.decode(&recovery[4..]).map_err(|_|error("Recovery key encoding is invalid"))?);
        let seed:[u8;32]=bytes.as_slice().try_into().map_err(|_|error("Recovery key length is invalid"))?;
        Self::from_seed(seed)
    }

    /// Display only after an explicit local recovery-key request. Never log or
    /// send this string to the synchronization service or diagnostic pipeline.
    pub fn recovery_key(&self)->Zeroizing<String> {
        Zeroizing::new(format!("OP1-{}",URL_SAFE_NO_PAD.encode(self.seed.as_ref())))
    }

    pub fn account_id(&self)->&str {&self.id}
    pub fn save_cipher(&self)->Result<crate::SaveCipher> {crate::SaveCipher::from_secret(self.id.clone(),*self.encryption_key)}
    pub fn public_key_base64(&self)->String {URL_SAFE_NO_PAD.encode(self.root.verifying_key().as_bytes())}

    pub fn authorize_device(&self,device:&DeviceIdentity,audience:&str,now_ms:u64)->Result<crate::SignedDocument> {
        crate::validate_audience(audience)?;
        let certificate=crate::DeviceCertificate {
            schema_version:1,account_id:self.id.clone(),device_id:device.id,
            device_public_key:device.public_key_base64(),audience:audience.into(),
            issued_at_ms:now_ms,expires_at_ms:now_ms.checked_add(90*24*60*60*1000).ok_or_else(||error("Certificate time overflow"))?,
            serial:Uuid::new_v4(),
        };
        self.sign_root(crate::CERTIFICATE_DOMAIN,&certificate)
    }

    pub fn sign_root<T:serde::Serialize>(&self,domain:&[u8],value:&T)->Result<crate::SignedDocument> {
        let payload=serde_json::to_vec(value).map_err(|_|error("Account authorization encoding failed"))?;
        crate::sign_document(&self.root,domain,&payload)
    }
}

pub struct DeviceIdentity { seed:Zeroizing<[u8;32]>,key:SigningKey,pub id:Uuid }

impl DeviceIdentity {
    pub fn generate()->Result<Self> {Self::from_seed(Uuid::new_v4(),random()?)}
    pub fn from_seed(id:Uuid,seed:[u8;32])->Result<Self> {
        let seed=Zeroizing::new(seed);
        Ok(Self {key:SigningKey::from_bytes(&seed),seed,id})
    }
    pub fn public_key_base64(&self)->String {URL_SAFE_NO_PAD.encode(self.key.verifying_key().as_bytes())}
    /// Use only with an OS credential-vault adapter, never plaintext storage.
    pub fn secret_bytes(&self)->Zeroizing<[u8;32]> {Zeroizing::new(*self.seed)}
    pub fn sign_request(&self,request:&crate::SyncRequest,certificate:crate::SignedDocument)->Result<crate::AuthenticatedRequest> {
        if request.device_id!=self.id {return Err(error("Request device identity does not match its signer"));}
        let payload=serde_json::to_vec(request).map_err(|_|error("Sync request encoding failed"))?;
        Ok(crate::AuthenticatedRequest {certificate,request:crate::sign_document(&self.key,crate::REQUEST_DOMAIN,&payload)?})
    }
}
