//! Native TURN credential admission only: no DNS, allocation or ICE agent.
use crate::{invalid,unauthorized,key_id,document_hash,verify_grant,SignedDocument};
use base64::{Engine,engine::general_purpose::STANDARD};
use omniplay_contracts::Result;
use serde::Deserialize;
use std::{collections::BTreeSet,net::{Ipv4Addr,Ipv6Addr}};
use uuid::Uuid;
use zeroize::{Zeroizing,Zeroize,ZeroizeOnDrop};

/// Bounded canonical RFC 7065 subset. No hierarchical URI parser, userinfo,
/// paths, fragments, percent escapes or arbitrary transport extensions.
pub fn validate_turn_uri(value:&str)->Result<()>{
    if value.len()>256||!value.is_ascii()||value.bytes().any(|byte|byte.is_ascii_control()||byte.is_ascii_whitespace()||b"@#/\\%".contains(&byte)){return Err(invalid());}
    let(scheme,rest)=value.split_once(':').ok_or_else(invalid)?;
    if !matches!(scheme,"turn"|"turns"){return Err(invalid());}
    let host_port=if let Some((host,query))=rest.split_once('?'){
        if !matches!(query,"transport=udp"|"transport=tcp"){return Err(invalid());}host
    }else{rest};
    let(host,port)=if let Some(v6)=host_port.strip_prefix('['){
        let(end,suffix)=v6.split_once(']').ok_or_else(invalid)?;
        let address:Ipv6Addr=end.parse().map_err(|_|invalid())?;
        if address.to_string()!=end{return Err(invalid());}
        let port=if suffix.is_empty(){None}else{Some(suffix.strip_prefix(':').ok_or_else(invalid)?)};
        (None,port)
    }else{
        let(host,port)=host_port.split_once(':').map_or((host_port,None),|(host,port)|(host,Some(port)));
        (Some(host),port)
    };
    if let Some(host)=host{
        if host.is_empty()||host.len()>253{return Err(invalid());}
        if host.bytes().all(|byte|byte.is_ascii_digit()||byte==b'.'){
            let address:Ipv4Addr=host.parse().map_err(|_|invalid())?;
            if address.to_string()!=host{return Err(invalid());}
        }else{
            for label in host.split('.'){
                if label.is_empty()||label.len()>63||label.starts_with('-')||label.ends_with('-')||
                    !label.bytes().all(|byte|byte.is_ascii_lowercase()||byte.is_ascii_digit()||byte==b'-'){return Err(invalid());}
            }
        }
    }
    if let Some(port)=port{
        if port.is_empty()||port.len()>5||!port.bytes().all(|byte|byte.is_ascii_digit())||port.starts_with('0')||port.parse::<u16>().is_err(){return Err(invalid());}
    }Ok(())
}
#[derive(Deserialize,Zeroize,ZeroizeOnDrop)]#[serde(deny_unknown_fields)]
struct WireServer{urls:Vec<String>,username:String,credential:String,#[serde(rename="credentialType")]credential_type:String}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]
struct WireIce{ice_protocol:u32,pair_id:Uuid,grant_hash:String,ice_servers:Vec<WireServer>,expires_at_ms:u64,relay_available:bool}

pub struct NativeIceServer{urls:Vec<String>,username:Zeroizing<String>,credential:Zeroizing<String>}
impl NativeIceServer{
    pub fn urls(&self)->&[String]{&self.urls}
    /// Native ICE adapter only, never public diagnostics or export UI.
    pub fn username(&self)->&str{&self.username}
    pub fn credential(&self)->&str{&self.credential}
}
pub struct NativeIceConfiguration{servers:Vec<NativeIceServer>,expires_at_ms:u64,relay_available:bool}
impl NativeIceConfiguration{
    pub fn servers(&self)->&[NativeIceServer]{&self.servers}
    pub fn expires_at_ms(&self)->u64{self.expires_at_ms}
    pub fn relay_available(&self)->bool{self.relay_available}
    pub fn require_live(&self,now:u64)->Result<()>{if now>=self.expires_at_ms{return Err(unauthorized());}Ok(())}
}
pub fn admit(value:serde_json::Value,document:&SignedDocument,audience:&str,local_public:&str,allowed_urls:&[String],now:u64)->Result<NativeIceConfiguration>{
    crate::validate_audience(audience)?;
    if allowed_urls.len()>32{return Err(invalid());}
    let mut allowed=BTreeSet::new();for url in allowed_urls{validate_turn_uri(url)?;if !allowed.insert(url.as_str()){return Err(invalid());}}
    let grant=verify_grant(document,audience,now)?;
    if grant.id.is_nil()||(local_public!=grant.host_public_key&&local_public!=grant.client_public_key){return Err(unauthorized());}
    let wire:WireIce=serde_json::from_value(value).map_err(|_|invalid())?;
    if wire.ice_protocol!=1||wire.pair_id!=grant.id||wire.grant_hash!=document_hash(document)?||wire.ice_servers.len()>8||
        wire.expires_at_ms<=now||wire.expires_at_ms>now.saturating_add(600_000)||wire.expires_at_ms>grant.expires_at_ms||
        wire.expires_at_ms>9_007_199_254_740_991||wire.relay_available==wire.ice_servers.is_empty(){return Err(invalid());}
    // Current TURN REST usernames use integral seconds: never round up expiry.
    let expires=(wire.expires_at_ms/1000)*1000;if expires<=now{return Err(unauthorized());}
    let expected=format!("{}:{}",wire.expires_at_ms/1000,key_id(local_public)?);
    let mut servers=Vec::with_capacity(wire.ice_servers.len());let mut urls=BTreeSet::new();
    for mut server in wire.ice_servers{
        if server.credential_type!="password"||server.urls.is_empty()||server.urls.len()>8||server.username!=expected||server.credential.len()!=28{return Err(invalid());}
        let credential=Zeroizing::new(STANDARD.decode(&server.credential).map_err(|_|invalid())?);
        if credential.len()!=20||STANDARD.encode(credential.as_slice())!=server.credential{return Err(invalid());}
        for url in &server.urls{
            validate_turn_uri(url)?;
            if !allowed.contains(url.as_str())||!urls.insert(url.clone())||urls.len()>32{return Err(unauthorized());}
        }
        servers.push(NativeIceServer{urls:std::mem::take(&mut server.urls),username:Zeroizing::new(std::mem::take(&mut server.username)),credential:Zeroizing::new(std::mem::take(&mut server.credential))});
    }
    Ok(NativeIceConfiguration{servers,expires_at_ms:expires,relay_available:wire.relay_available})
}
