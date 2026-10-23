//! Synthetic package and signature boundary cases. No real release keys or I/O
//! outside an owned temporary directory; no network or executable is launched.
use base64::{engine::general_purpose::STANDARD,Engine};
use ed25519_dalek::{Signer,SigningKey};
use omniplay_core::updates::{ArtifactSignature,ReleaseConstraints,ReleaseManifest,SignedEnvelope,TrustedRoot};
use omniplay_updater::package::{PackageFile,PackageManifest,unpack};
use omniplay_contracts::ContentHash;
use sha2::{Digest,Sha256};
use std::{collections::{BTreeMap,BTreeSet},fs,path::PathBuf};

fn digest(bytes:&[u8])->ContentHash{ContentHash(format!("{:x}",Sha256::digest(bytes)))}
fn release()->ReleaseManifest{
    serde_json::from_value(serde_json::json!({"schema_version":1,"metadata_version":2,
        "channel":"stable","platform":"windows-x64","version":"1.1.0","security_epoch":1,
        "root_version":1,"issued_at_ms":1000,"expires_at_ms":20000,
        "artifact_url":"https://example.invalid/release.oppk","artifact_sha256":"0".repeat(64),
        "artifact_bytes":100,"chromium_revision":"a".repeat(40),"minimum_data_schema":1,"maximum_data_schema":2})).unwrap()
}
fn fixture()->(PackageManifest,Vec<u8>){
    let payload=[("app.bin",b"synthetic browser".as_slice(),true),("broker.bin",b"synthetic broker".as_slice(),true),
        ("LICENSE",b"fixture license".as_slice(),false),("NOTICE",b"fixture notice".as_slice(),false),
        ("licenses/third_party_notices.txt",b"fixture notices".as_slice(),false),
        ("licenses/source_offer.json",b"{}".as_slice(),false)];
    let files=payload.iter().map(|(name,bytes,executable)|PackageFile{path:name.to_string(),bytes:bytes.len()as u64,sha256:digest(bytes),executable:*executable}).collect();
    let manifest=PackageManifest{schema_version:1,channel:"stable".into(),platform:"windows-x64".into(),version:"1.1.0".parse().unwrap(),
        chromium_revision:"a".repeat(40),minimum_data_schema:1,maximum_data_schema:2,browser_entry:"app.bin".into(),broker_entry:"broker.bin".into(),files};
    let body=payload.iter().flat_map(|(_,bytes,_)|bytes.iter().copied()).collect();(manifest,body)
}
struct Scratch(PathBuf);
impl Scratch{fn new()->Self{let path=std::env::temp_dir().join(format!("vlikeon-release-security-{}",uuid::Uuid::new_v4()));fs::create_dir(&path).unwrap();Self(path)}}
impl Drop for Scratch{fn drop(&mut self){let _=fs::remove_dir_all(&self.0);}}
fn attempt(manifest:&PackageManifest,body:&[u8],existing:bool)->bool{
    let scratch=Scratch::new();let path=scratch.0.join("fixture.oppk");let target=scratch.0.join("staging");fs::create_dir(&target).unwrap();
    if existing{fs::write(target.join("keep.txt"),b"preserve").unwrap();}
    let raw=serde_json::to_vec(manifest).unwrap();let mut bytes=b"OPPK0001".to_vec();bytes.extend_from_slice(&(raw.len()as u32).to_le_bytes());bytes.extend_from_slice(&raw);bytes.extend_from_slice(body);fs::write(&path,bytes).unwrap();
    let accepted=unpack(fs::File::open(path).unwrap(),&target,&release()).is_ok();
    if existing{assert_eq!(fs::read(target.join("keep.txt")).unwrap(),b"preserve");}
    if accepted{assert_eq!(fs::read(target.join("app.bin")).unwrap(),b"synthetic browser");}
    accepted
}
#[test]fn exact_fixture_unpacks(){let(manifest,body)=fixture();assert!(attempt(&manifest,&body,false));}
#[test]fn truncated_payload_is_refused(){let(manifest,mut body)=fixture();body.pop();assert!(!attempt(&manifest,&body,false));}
#[test]fn changed_payload_is_refused(){let(manifest,mut body)=fixture();body[0]^=1;assert!(!attempt(&manifest,&body,false));}
#[test]fn unlisted_trailing_data_is_refused(){let(manifest,mut body)=fixture();body.push(0);assert!(!attempt(&manifest,&body,false));}
#[test]fn extraction_never_merges_an_existing_tree(){let(manifest,body)=fixture();assert!(!attempt(&manifest,&body,true));}
#[test]fn case_collision_and_parent_file_are_refused(){
    let(mut manifest,_)=fixture();let mut alias=manifest.files[0].clone();alias.path="APP.BIN".into();manifest.files.push(alias);assert!(manifest.validate(&release()).is_err());
    let(mut manifest,_)=fixture();let mut child=manifest.files[0].clone();child.path="app.bin/child".into();manifest.files.insert(0,child);assert!(manifest.validate(&release()).is_err());
}
#[test]fn signed_identity_mismatch_is_refused(){let(mut manifest,_)=fixture();manifest.version="1.2.0".parse().unwrap();assert!(manifest.validate(&release()).is_err());}

fn trust()->(TrustedRoot,SigningKey,SigningKey){
    // Fixed synthetic test-only seeds, never accepted by a configured product root.
    let first=SigningKey::from_bytes(&[7;32]);let second=SigningKey::from_bytes(&[8;32]);
    let keys=BTreeMap::from([("first".into(),STANDARD.encode(first.verifying_key().to_bytes())),("second".into(),STANDARD.encode(second.verifying_key().to_bytes()))]);
    (TrustedRoot{version:1,expires_at_ms:30000,threshold:2,keys,release_hosts:BTreeSet::from(["example.invalid".into()])},first,second)
}
fn signed(payload:&[u8],first:&SigningKey,second:&SigningKey)->SignedEnvelope{
    SignedEnvelope{payload_base64:STANDARD.encode(payload),signatures:vec![ArtifactSignature{key_id:"first".into(),signature_base64:STANDARD.encode(first.sign(payload).to_bytes())},ArtifactSignature{key_id:"second".into(),signature_base64:STANDARD.encode(second.sign(payload).to_bytes())}]}
}
#[test]fn threshold_requires_distinct_real_signers(){
    let(root,first,second)=trust();let payload=b"synthetic payload";let mut envelope=signed(payload,&first,&second);
    assert_eq!(root.verify_payload(&envelope,2000).unwrap(),payload);
    envelope.signatures[1]=envelope.signatures[0].clone();assert!(root.verify_payload(&envelope,2000).is_err());
}
#[test]fn duplicate_public_keys_and_expired_roots_are_refused(){
    let(mut root,first,second)=trust();assert!(root.verify_payload(&signed(b"fixture",&first,&second),30000).is_err());
    root.keys.insert("second".into(),root.keys["first"].clone());assert!(root.validate(2000).is_err());
}
#[test]fn authentic_signatures_do_not_authorize_replayed_metadata(){
    let(root,first,second)=trust();let envelope=signed(&serde_json::to_vec(&release()).unwrap(),&first,&second);let current="1.0.0".parse().unwrap();
    let mut policy=ReleaseConstraints{channel:"stable",platform:"windows-x64",current_version:&current,minimum_security_epoch:1,highest_metadata_version:1,data_schema:1,now_ms:2000};
    assert!(root.verify_release(&envelope,&policy).is_ok());policy.highest_metadata_version=2;assert!(root.verify_release(&envelope,&policy).is_err());
}
#[test]fn signed_release_cannot_change_host_or_schema_admission(){
    let(root,first,second)=trust();let mut value=release();let current="1.0.0".parse().unwrap();
    let policy=ReleaseConstraints{channel:"stable",platform:"windows-x64",current_version:&current,minimum_security_epoch:1,highest_metadata_version:1,data_schema:1,now_ms:2000};
    value.artifact_url="https://foreign.invalid/payload".into();assert!(root.verify_release(&signed(&serde_json::to_vec(&value).unwrap(),&first,&second),&policy).is_err());
    value=release();value.minimum_data_schema=2;assert!(root.verify_release(&signed(&serde_json::to_vec(&value).unwrap(),&first,&second),&policy).is_err());
}
