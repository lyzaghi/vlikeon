use crate::{AccountIdentity,error,random};
use base64::{Engine,engine::general_purpose::URL_SAFE_NO_PAD};
use chacha20poly1305::{XChaCha20Poly1305,XNonce,KeyInit,AeadInPlace};
use omniplay_contracts::{ErrorCode,PlatformError,Result,SaveMetadata};
use serde::{Deserialize,Serialize};
use std::io::{Cursor,Read,Write};
use uuid::Uuid;
use zeroize::{Zeroize,Zeroizing};

/// A delegated device receives only this data key. It never retains the account
/// root signing seed, so revoking its device certificate remains meaningful.
pub struct SaveCipher { account_id:String,pub(crate) encryption_key:Zeroizing<[u8;32]> }

impl SaveCipher {
    pub fn from_secret(account_id:String,key:[u8;32])->Result<Self> {
        if !crate::valid_account_id(&account_id){return Err(error("Save encryption account identity is invalid"));}
        Ok(Self{account_id,encryption_key:Zeroizing::new(key)})
    }
    pub fn account_id(&self)->&str {&self.account_id}
    pub fn secret_bytes(&self)->Zeroizing<[u8;32]> {Zeroizing::new(*self.encryption_key)}
}

const MAX_METADATA:usize=64*1024;
pub const MAX_CIPHERTEXT_BYTES:usize=omniplay_contracts::MAX_SAVE_BYTES as usize+MAX_METADATA+32;
const IO_CHUNK:usize=64*1024;

fn allocation_error()->PlatformError{PlatformError::new(ErrorCode::ResourceLimit,"The encrypted save buffer exceeds available memory")}
fn input_error()->PlatformError{PlatformError::new(ErrorCode::StorageFailure,"Encrypted save input or private staging I/O failed")}
fn owned_buffer(bytes:usize)->Result<Zeroizing<Vec<u8>>>{
    if bytes>MAX_CIPHERTEXT_BYTES{return Err(allocation_error());}
    let mut value=Zeroizing::new(Vec::new());value.try_reserve_exact(bytes).map_err(|_|allocation_error())?;Ok(value)
}
fn hash_checked(bytes:&[u8],check:&dyn Fn()->Result<()>)->Result<String>{
    use sha2::{Digest,Sha256};let mut hash=Sha256::new();
    for part in bytes.chunks(IO_CHUNK){check()?;hash.update(part);}check()?;Ok(format!("{:x}",hash.finalize()))
}
fn read_checked(source:&mut impl Read,buffer:&mut[u8],check:&dyn Fn()->Result<()>)->Result<()>{
    for part in buffer.chunks_mut(IO_CHUNK){check()?;source.read_exact(part).map_err(|_|input_error())?;}check()?;Ok(())
}
fn require_end(source:&mut impl Read,check:&dyn Fn()->Result<()>)->Result<()>{
    check()?;let mut tail=Zeroizing::new([0u8;1]);if source.read(tail.as_mut()).map_err(|_|input_error())?!=0{return Err(input_error());}check()
}

#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
#[serde(deny_unknown_fields)]
pub struct SaveBinding {pub schema_version:u32,pub account_id:String,pub object_id:Uuid,pub key_epoch:u32}

#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncryptedSave {pub binding:SaveBinding,pub nonce_base64:String,pub ciphertext_base64:String}

#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
#[serde(deny_unknown_fields)]
pub struct BlobHeader {pub binding:SaveBinding,pub nonce_base64:String,pub ciphertext_sha256:String,pub ciphertext_bytes:u64}

impl BlobHeader {
    pub fn validate(&self)->Result<()> {
        if self.binding.schema_version!=1||self.binding.key_epoch!=1||!crate::valid_account_id(&self.binding.account_id)||self.nonce_base64.len()!=32||self.ciphertext_bytes<16||self.ciphertext_bytes>MAX_CIPHERTEXT_BYTES as u64 {
            return Err(error("Encrypted object header exceeds policy"));
        }
        omniplay_contracts::ContentHash::parse(self.ciphertext_sha256.clone())?;
        let nonce=URL_SAFE_NO_PAD.decode(&self.nonce_base64).map_err(|_|error("Encrypted object nonce is invalid"))?;
        if nonce.len()!=24{return Err(error("Encrypted object nonce length is invalid"));}
        Ok(())
    }
    /// Copies only encrypted bytes, with bounded scratch space. Publication is
    /// the caller's responsibility and must follow successful final verification.
    pub fn copy_blob_checked(&self,source:&mut impl Read,target:&mut impl Write,check:&dyn Fn()->Result<()>)->Result<()>{
        use sha2::{Digest,Sha256};self.validate()?;check()?;
        let mut left=self.ciphertext_bytes;let mut hash=Sha256::new();let mut buffer=[0u8;IO_CHUNK];
        while left>0{
            check()?;let count=left.min(buffer.len() as u64) as usize;
            source.read_exact(&mut buffer[..count]).map_err(|_|input_error())?;hash.update(&buffer[..count]);
            check()?;target.write_all(&buffer[..count]).map_err(|_|input_error())?;left-=count as u64;
        }
        require_end(source,check)?;
        if format!("{:x}",hash.finalize())!=self.ciphertext_sha256{return Err(error("Encrypted object transport integrity check failed"));}check()
    }
}

impl EncryptedSave {
    pub fn to_blob(&self)->Result<(BlobHeader,Vec<u8>)> {
        self.validate_public()?;
        let bytes=URL_SAFE_NO_PAD.decode(&self.ciphertext_base64).map_err(|_|error("Encrypted payload encoding is invalid"))?;
        let header=BlobHeader{binding:self.binding.clone(),nonce_base64:self.nonce_base64.clone(),ciphertext_sha256:sha256(&bytes),ciphertext_bytes:bytes.len() as u64};
        Ok((header,bytes))
    }

    pub fn from_blob(header:BlobHeader,bytes:&[u8])->Result<Self> {
        header.validate()?;
        if bytes.len() as u64!=header.ciphertext_bytes||sha256(bytes)!=header.ciphertext_sha256{return Err(error("Encrypted object transport integrity check failed"));}
        Ok(Self{binding:header.binding,nonce_base64:header.nonce_base64,ciphertext_base64:URL_SAFE_NO_PAD.encode(bytes)})
    }
    pub fn validate_public(&self)->Result<()> {
        if self.binding.schema_version!=1||self.binding.key_epoch!=1||!crate::valid_account_id(&self.binding.account_id)||self.nonce_base64.len()!=32||self.ciphertext_base64.len()>MAX_CIPHERTEXT_BYTES*4/3+8 {
            return Err(error("Encrypted save envelope exceeds policy"));
        }
        let nonce=URL_SAFE_NO_PAD.decode(&self.nonce_base64).map_err(|_|error("Encrypted save nonce is invalid"))?;
        if nonce.len()!=24{return Err(error("Encrypted save nonce length is invalid"));}
        let ciphertext=URL_SAFE_NO_PAD.decode(&self.ciphertext_base64).map_err(|_|error("Encrypted save payload encoding is invalid"))?;
        if ciphertext.len()<16||ciphertext.len()>MAX_CIPHERTEXT_BYTES{return Err(error("Encrypted save payload length is invalid"));}
        Ok(())
    }
}

impl SaveCipher {
    pub fn encrypt_save(&self,metadata:&SaveMetadata,payload:&[u8])->Result<EncryptedSave> {
        let (header,ciphertext)=self.encrypt_blob(metadata,payload)?;
        Ok(EncryptedSave{binding:header.binding,nonce_base64:header.nonce_base64,ciphertext_base64:URL_SAFE_NO_PAD.encode(ciphertext)})
    }
    /// The version-one wire format is unchanged. The reader form allocates one
    /// fallibly reserved AEAD buffer instead of a payload plus plaintext/ciphertext
    /// copies; authenticated encryption itself still operates on the whole blob.
    pub fn encrypt_blob(&self,metadata:&SaveMetadata,payload:&[u8])->Result<(BlobHeader,Vec<u8>)> {
        self.encrypt_blob_reader_checked(metadata,&mut Cursor::new(payload),&||Ok(()))
    }
    pub fn encrypt_blob_reader_checked(&self,metadata:&SaveMetadata,source:&mut impl Read,check:&dyn Fn()->Result<()>)->Result<(BlobHeader,Vec<u8>)> {
        check()?;
        metadata.validate()?;
        let metadata_json=serde_json::to_vec(metadata).map_err(|_|error("Save metadata encoding failed"))?;
        if metadata_json.len()>MAX_METADATA{return Err(error("Save metadata exceeds the encryption limit"));}
        let prefix=4usize.checked_add(metadata_json.len()).ok_or_else(allocation_error)?;
        let bytes=usize::try_from(metadata.payload_bytes).map_err(|_|allocation_error())?;
        let length=prefix.checked_add(bytes).ok_or_else(allocation_error)?;
        let mut plaintext=owned_buffer(length.checked_add(16).ok_or_else(allocation_error)?)?;
        plaintext.extend_from_slice(&(metadata_json.len() as u32).to_le_bytes());plaintext.extend_from_slice(&metadata_json);plaintext.resize(length,0);
        read_checked(source,&mut plaintext[prefix..],check)?;require_end(source,check)?;
        if hash_checked(&plaintext[prefix..],check)?!=metadata.payload_hash.0{return Err(error("Save payload integrity check failed before encryption"));}
        let binding=SaveBinding{schema_version:1,account_id:self.account_id().into(),object_id:metadata.id,key_epoch:1};
        let mut aad=b"OmniPlay encrypted save v1\0".to_vec();aad.extend(serde_json::to_vec(&binding).map_err(|_|error("Save binding encoding failed"))?);
        let nonce:[u8;24]=random()?;
        let cipher=XChaCha20Poly1305::new_from_slice(self.encryption_key.as_ref()).map_err(|_|error("Save encryption key is invalid"))?;
        check()?;cipher.encrypt_in_place(XNonce::from_slice(&nonce),&aad,&mut *plaintext).map_err(|_|error("Save encryption failed"))?;check()?;
        let header=BlobHeader{binding,nonce_base64:URL_SAFE_NO_PAD.encode(nonce),ciphertext_sha256:hash_checked(&plaintext,check)?,ciphertext_bytes:plaintext.len() as u64};header.validate()?;
        let ciphertext=std::mem::take(&mut *plaintext);
        Ok((header,ciphertext))
    }

    pub fn decrypt_save(&self,envelope:&EncryptedSave)->Result<(SaveMetadata,Zeroizing<Vec<u8>>)> {
        let (header,ciphertext)=envelope.to_blob()?;self.decrypt_owned_blob_checked(&header,Zeroizing::new(ciphertext),&||Ok(()))
    }
    pub fn decrypt_blob(&self,header:&BlobHeader,ciphertext:&[u8])->Result<(SaveMetadata,Zeroizing<Vec<u8>>)> {
        self.decrypt_blob_reader_checked(header,&mut Cursor::new(ciphertext),&||Ok(()))
    }
    pub fn decrypt_blob_reader_checked(&self,header:&BlobHeader,source:&mut impl Read,check:&dyn Fn()->Result<()>)->Result<(SaveMetadata,Zeroizing<Vec<u8>>)> {
        header.validate()?;check()?;
        if header.binding.account_id!=self.account_id(){return Err(error("Encrypted object belongs to another account"));}
        let mut buffer=owned_buffer(usize::try_from(header.ciphertext_bytes).map_err(|_|allocation_error())?)?;
        buffer.resize(header.ciphertext_bytes as usize,0);read_checked(source,&mut buffer,check)?;require_end(source,check)?;
        self.decrypt_owned_blob_checked(header,buffer,check)
    }
    /// Ownership permits authentication/decryption and metadata removal in the
    /// same zeroizing allocation. No plaintext escapes on any rejected outcome.
    pub fn decrypt_owned_blob_checked(&self,header:&BlobHeader,mut plaintext:Zeroizing<Vec<u8>>,check:&dyn Fn()->Result<()>)->Result<(SaveMetadata,Zeroizing<Vec<u8>>)> {
        header.validate()?;
        if header.binding.account_id!=self.account_id()||plaintext.len() as u64!=header.ciphertext_bytes||hash_checked(&plaintext,check)?!=header.ciphertext_sha256{return Err(error("Encrypted object transport integrity check failed"));}
        let nonce=URL_SAFE_NO_PAD.decode(&header.nonce_base64).map_err(|_|error("Save nonce is invalid"))?;
        let mut aad=b"OmniPlay encrypted save v1\0".to_vec();aad.extend(serde_json::to_vec(&header.binding).map_err(|_|error("Save binding encoding failed"))?);
        let cipher=XChaCha20Poly1305::new_from_slice(self.encryption_key.as_ref()).map_err(|_|error("Save encryption key is invalid"))?;
        check()?;cipher.decrypt_in_place(XNonce::from_slice(&nonce),&aad,&mut *plaintext).map_err(|_|error("Save authentication failed; the encrypted content was not imported"))?;check()?;
        if plaintext.len()<4{return Err(error("Decrypted save is truncated"));}
        let length=u32::from_le_bytes(plaintext[..4].try_into().map_err(|_|error("Save metadata header is truncated"))?) as usize;
        if length==0||length>MAX_METADATA||4+length>=plaintext.len(){return Err(error("Decrypted save metadata length is invalid"));}
        let metadata:SaveMetadata=serde_json::from_slice(&plaintext[4..4+length]).map_err(|_|error("Decrypted save metadata is invalid"))?;
        metadata.validate()?;
        let prefix=4+length;let payload_bytes=plaintext.len()-prefix;
        if metadata.id!=header.binding.object_id||payload_bytes as u64!=metadata.payload_bytes||hash_checked(&plaintext[prefix..],check)?!=metadata.payload_hash.0{return Err(error("Decrypted save content does not match its authenticated metadata"));}
        // copy_within retains one allocation. Wipe the vacated tail before
        // truncate so Zeroizing's eventual logical-length wipe loses no plaintext.
        check()?;plaintext.copy_within(prefix..,0);plaintext[payload_bytes..].zeroize();plaintext.truncate(payload_bytes);check()?;
        Ok((metadata,plaintext))
    }
}

impl AccountIdentity {
    pub fn encrypt_save(&self,metadata:&SaveMetadata,payload:&[u8])->Result<EncryptedSave> {self.save_cipher()?.encrypt_save(metadata,payload)}
    pub fn decrypt_save(&self,envelope:&EncryptedSave)->Result<(SaveMetadata,Zeroizing<Vec<u8>>)> {self.save_cipher()?.decrypt_save(envelope)}
}

fn sha256(payload:&[u8])->String {use sha2::{Digest,Sha256};format!("{:x}",Sha256::digest(payload))}
