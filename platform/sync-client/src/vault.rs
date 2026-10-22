use base64::{Engine,engine::general_purpose::URL_SAFE_NO_PAD};
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use omniplay_sync::{DeviceIdentity,SaveCipher};
use serde::{Serialize,Deserialize};
use uuid::Uuid;
use zeroize::{Zeroizing,Zeroize,ZeroizeOnDrop};

pub const MAX_CREDENTIAL_BYTES:usize=4096;
/// Native implementations must bind the namespace/profile, store durably before
/// returning success, and never fall back to a plaintext file or renderer store.
pub trait CredentialStore:Send+Sync{
    fn check_access(&self)->Result<()>{Ok(())}
    fn read(&self,profile:Uuid)->Result<Zeroizing<Vec<u8>>>;
    fn write(&self,profile:Uuid,secret:&[u8])->Result<()>;
    fn remove(&self,profile:Uuid)->Result<()>;
}
#[derive(Default)]pub struct OsCredentialStore;

#[derive(Serialize,Deserialize,Zeroize,ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
struct DeviceSecrets {#[zeroize(skip)]schema_version:u32,#[zeroize(skip)]device_id:Uuid,device_seed:String,data_key:String}

#[cfg(any(target_os="windows",target_os="macos",target_os="linux"))]
fn entry(profile_id:Uuid)->Result<keyring::Entry> {
    keyring::Entry::new("OmniPlay encrypted save synchronization",&profile_id.to_string()).map_err(vault_error)
}
fn vault_error(_:impl std::fmt::Display)->PlatformError {PlatformError::new(ErrorCode::PermissionDenied,"The operating-system credential vault is unavailable").recover("Unlock the credential vault before enabling synchronization")}

impl CredentialStore for OsCredentialStore{
    fn read(&self,profile:Uuid)->Result<Zeroizing<Vec<u8>>>{
        #[cfg(any(target_os="windows",target_os="macos",target_os="linux"))]{let value=Zeroizing::new(entry(profile)?.get_password().map_err(vault_error)?);if value.len()>MAX_CREDENTIAL_BYTES{return Err(vault_error("Credential record exceeds its bound"));}Ok(Zeroizing::new(value.as_bytes().to_vec()))}
        #[cfg(not(any(target_os="windows",target_os="macos",target_os="linux")))]{let _=profile;Err(native_provider_required())}
    }
    fn write(&self,profile:Uuid,secret:&[u8])->Result<()>{
        if secret.is_empty()||secret.len()>MAX_CREDENTIAL_BYTES{return Err(vault_error("Credential record exceeds its bound"));}
        #[cfg(any(target_os="windows",target_os="macos",target_os="linux"))]{entry(profile)?.set_password(std::str::from_utf8(secret).map_err(vault_error)?).map_err(vault_error)}
        #[cfg(not(any(target_os="windows",target_os="macos",target_os="linux")))]{let _=profile;Err(native_provider_required())}
    }
    fn remove(&self,profile:Uuid)->Result<()>{
        #[cfg(any(target_os="windows",target_os="macos",target_os="linux"))]{match entry(profile)?.delete_credential(){Ok(())|Err(keyring::Error::NoEntry)=>Ok(()),Err(error)=>Err(vault_error(error))}}
        #[cfg(not(any(target_os="windows",target_os="macos",target_os="linux")))]{let _=profile;Err(native_provider_required())}
    }
}
#[cfg(not(any(target_os="windows",target_os="macos",target_os="linux")))]
fn native_provider_required()->PlatformError{PlatformError::new(ErrorCode::UnsupportedCapability,"This platform requires its native application credential provider")}

pub fn store(provider:&dyn CredentialStore,profile:Uuid,device:&DeviceIdentity,cipher:&SaveCipher)->Result<()> {
    let secrets=DeviceSecrets{schema_version:1,device_id:device.id,device_seed:URL_SAFE_NO_PAD.encode(device.secret_bytes().as_ref()),data_key:URL_SAFE_NO_PAD.encode(cipher.secret_bytes().as_ref())};
    let json=Zeroizing::new(serde_json::to_vec(&secrets).map_err(vault_error)?);
    // There is deliberately no plaintext-file fallback and no root signing seed.
    if json.len()>MAX_CREDENTIAL_BYTES{return Err(vault_error("Credential record exceeds its bound"));}provider.write(profile,&json)
}

pub fn load(provider:&dyn CredentialStore,profile:Uuid,account:&str)->Result<(DeviceIdentity,SaveCipher)> {
    let json=provider.read(profile)?;if json.is_empty()||json.len()>MAX_CREDENTIAL_BYTES||json.iter().copied().find(|byte|!byte.is_ascii_whitespace())!=Some(b'{'){return Err(vault_error("Credential record needs a bounded object"));}
    let secrets:DeviceSecrets=serde_json::from_slice(&json).map_err(vault_error)?;
    if secrets.schema_version!=1||secrets.device_seed.len()!=43||secrets.data_key.len()!=43{return Err(vault_error("Unknown credential version or seed shape"));}
    let device_seed=Zeroizing::new(URL_SAFE_NO_PAD.decode(&secrets.device_seed).map_err(vault_error)?);
    let data_key=Zeroizing::new(URL_SAFE_NO_PAD.decode(&secrets.data_key).map_err(vault_error)?);
    let canonical_seed=Zeroizing::new(URL_SAFE_NO_PAD.encode(device_seed.as_slice()));
    let canonical_key=Zeroizing::new(URL_SAFE_NO_PAD.encode(data_key.as_slice()));
    if canonical_seed.as_str()!=secrets.device_seed.as_str()||canonical_key.as_str()!=secrets.data_key.as_str(){return Err(vault_error("Credential encoding is not canonical"));}
    let device=DeviceIdentity::from_seed(secrets.device_id,device_seed.as_slice().try_into().map_err(vault_error)?)?;
    let cipher=SaveCipher::from_secret(account.into(),data_key.as_slice().try_into().map_err(vault_error)?)?;
    Ok((device,cipher))
}

