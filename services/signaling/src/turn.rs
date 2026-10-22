use crate::{ApiError,ApiResult};
use base64::{Engine,engine::general_purpose::STANDARD};
use hmac::{Hmac,Mac};
use sha1::Sha1;
use zeroize::Zeroizing;
use serde_json::{Value,json};

pub struct Turn{urls:Vec<String>,secret:Zeroizing<String>}
impl Turn{
    pub fn configured()->ApiResult<Option<Self>>{
        let urls=std::env::var("OMNIPLAY_RELAY_TURN_URLS").ok();let secret=std::env::var("OMNIPLAY_RELAY_TURN_SECRET").ok();
        match(urls,secret){
            (None,None)=>Ok(None),
            (Some(urls),Some(secret))=>{let urls:Vec<_>=urls.split(',').map(str::trim).map(str::to_owned).collect();if urls.is_empty()||urls.len()>8||secret.len()<32||secret.len()>256||urls.iter().any(|v|!valid_url(v)){return Err(ApiError::invalid());}Ok(Some(Self{urls,secret:Zeroizing::new(secret)}))},
            _=>Err(ApiError::invalid()),
        }
    }
    pub fn credentials(&self,identity:&str,expires:u64)->ApiResult<Value>{
        let username=format!("{}:{}",expires/1000,identity);
        let mut mac=Hmac::<Sha1>::new_from_slice(self.secret.as_bytes()).map_err(|_|ApiError::invalid())?;mac.update(username.as_bytes());
        let credential=STANDARD.encode(mac.finalize().into_bytes());Ok(json!({"ice_servers":[{"urls":self.urls,"username":username,"credential":credential,"credentialType":"password"}],"expires_at_ms":expires,"relay_available":true}))
    }
}
fn valid_url(value:&str)->bool{
    if value.len()>256||!value.is_ascii()||value.bytes().any(|b|b.is_ascii_control()||b.is_ascii_whitespace()||[b'@',b'#',b'/',b'\\'].contains(&b)){return false;}
    let Some((scheme,rest))=value.split_once(':')else{return false;};if !["turn","turns"].contains(&scheme)||rest.is_empty(){return false;}
    let(host,query)=rest.split_once('?').unwrap_or((rest,""));if !query.is_empty()&&query!="transport=udp"&&query!="transport=tcp"{return false;}
    !host.is_empty()&&host.bytes().all(|b|b.is_ascii_alphanumeric()||b".-:[]".contains(&b))
}
