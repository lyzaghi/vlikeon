#![forbid(unsafe_code)]
// Offline operator utility. It is not included in the browser or connected to IPC.
use base64::{Engine,engine::general_purpose::STANDARD};
use ed25519_dalek::{Signer,SigningKey};
use omniplay_contracts::Result;
use omniplay_core::{updates::{ArtifactSignature,SignedEnvelope},util::{bounded_read,storage_error}};
use omniplay_updater::rejected;
use std::{fs::{File,OpenOptions},io::Write,path::Path};
use zeroize::Zeroizing;

fn create_private(path:&Path,bytes:&[u8])->Result<()> {
    let mut options=OpenOptions::new();options.create_new(true).write(true);
    #[cfg(unix)] {use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
    let mut file=options.open(path).map_err(storage_error)?;file.write_all(bytes).map_err(storage_error)?;file.sync_all().map_err(storage_error)
}
fn read_seed(path:&Path)->Result<Zeroizing<[u8;32]>> {
    let mut file=File::open(path).map_err(storage_error)?;
    #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;if file.metadata().map_err(storage_error)?.permissions().mode()&0o077!=0{return Err(rejected("Signing seed permissions must restrict access to its owner"));}}
    let raw=Zeroizing::new(bounded_read(&mut file,128)?);
    let decoded=Zeroizing::new(STANDARD.decode(raw.trim_ascii()).map_err(|_|rejected("Signing seed encoding is invalid"))?);
    if decoded.len()!=32{return Err(rejected("Signing seed must contain exactly 32 bytes"));}
    let mut seed=Zeroizing::new([0u8;32]);seed.copy_from_slice(&decoded);Ok(seed)
}

fn run()->Result<()> {
    let args=std::env::args().skip(1).collect::<Vec<_>>();
    match args.as_slice() {
        [command,key_id,seed_path,public_path] if command=="generate-key"=>{
            if !omniplay_contracts::valid_identifier(key_id){return Err(rejected("Signing key ID is invalid"));}
            let mut seed=Zeroizing::new([0u8;32]);getrandom::getrandom(&mut *seed).map_err(|_|rejected("Secure random generation failed"))?;
            let key=SigningKey::from_bytes(&seed);let encoded=Zeroizing::new(STANDARD.encode(*seed));
            create_private(Path::new(seed_path),encoded.as_bytes())?;
            let public=serde_json::to_vec_pretty(&serde_json::json!({"key_id":key_id,"public_key_base64":STANDARD.encode(key.verifying_key().as_bytes())})).map_err(storage_error)?;
            create_private(Path::new(public_path),&public)?;
        }
        [command,key_id,seed_path,payload_path,output] if command=="sign"=>{
            if !omniplay_contracts::valid_identifier(key_id){return Err(rejected("Signing key ID is invalid"));}
            let payload=bounded_read(&mut File::open(payload_path).map_err(storage_error)?,256*1024)?;
            if payload.is_empty(){return Err(rejected("There is no metadata payload to sign"));}
            let seed=read_seed(Path::new(seed_path))?;let key=SigningKey::from_bytes(&seed);
            let envelope=SignedEnvelope{payload_base64:STANDARD.encode(&payload),signatures:vec![ArtifactSignature{key_id:key_id.clone(),signature_base64:STANDARD.encode(key.sign(&payload).to_bytes())}]};
            create_private(Path::new(output),&serde_json::to_vec_pretty(&envelope).map_err(storage_error)?)?;
        }
        [command,output,inputs @ ..] if command=="combine"&&!inputs.is_empty()=>{
            let mut combined:Option<SignedEnvelope>=None;let mut ids=std::collections::BTreeSet::new();
            for input in inputs {
                let next:SignedEnvelope=serde_json::from_slice(&bounded_read(&mut File::open(input).map_err(storage_error)?,1024*1024)?).map_err(|_|rejected("Signature envelope is invalid"))?;
                if let Some(current)=&combined {if current.payload_base64!=next.payload_base64{return Err(rejected("Signers did not sign the exact same payload"));}}
                let target=combined.get_or_insert_with(||SignedEnvelope{payload_base64:next.payload_base64.clone(),signatures:Vec::new()});
                for signature in next.signatures {if !ids.insert(signature.key_id.clone()){return Err(rejected("Duplicate signer IDs cannot count toward a threshold"));}target.signatures.push(signature);if target.signatures.len()>32{return Err(rejected("Too many signers"));}}
            }
            create_private(Path::new(output),&serde_json::to_vec_pretty(&combined.expect("nonempty inputs")).map_err(storage_error)?)?;
        }
        _=>return Err(rejected("Use generate-key <key ID> <private seed file> <public file>, sign <key ID> <private seed file> <payload> <new envelope>, or combine <new envelope> <signed envelope>...")),
    }
    Ok(())
}
fn main(){if let Err(error)=run(){eprintln!("{error}");std::process::exit(1);}}
