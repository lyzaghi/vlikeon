//! Explicit browser-origin transport policy. CORS never replaces root/device
//! signatures, nonce admission, revocation or the service's TLS ingress boundary.
use crate::{ApiError,ApiResult};
use axum::{extract::{Request,State},http::{HeaderMap,HeaderValue,Method,StatusCode,header},middleware::Next,response::{IntoResponse,Response}};
use std::sync::Arc;

pub(crate) struct Policy{origins:Vec<String>}
impl Policy{
    pub(crate) fn from_environment()->ApiResult<Self>{
        let raw=match std::env::var("OMNIPLAY_SYNC_WEB_ORIGINS"){Ok(value)=>value,Err(std::env::VarError::NotPresent)=>"[]".into(),Err(_)=>return Err(ApiError::invalid())};if raw.len()>4096{return Err(ApiError::invalid());}
        let origins:Vec<String>=serde_json::from_str(&raw).map_err(|_|ApiError::invalid())?;if origins.len()>8{return Err(ApiError::invalid());}
        let mut seen=std::collections::BTreeSet::new();
        for value in &origins{
            let url=url::Url::parse(value).map_err(|_|ApiError::invalid())?;
            if value.len()>256||!value.is_ascii()||url.scheme()!="https"||url.host_str().is_none()||!url.username().is_empty()||url.password().is_some()||url.query().is_some()||url.fragment().is_some()||url.path()!="/"||url.origin().ascii_serialization()!=*value||!seen.insert(value.clone()){return Err(ApiError::invalid());}
        }
        Ok(Self{origins})
    }
    fn origin(&self,headers:&HeaderMap)->ApiResult<Option<String>>{
        if headers.get_all(header::ORIGIN).iter().nth(1).is_some(){return Err(ApiError::unauthorized());}
        let Some(value)=headers.get(header::ORIGIN)else{return Ok(None)};let value=value.to_str().map_err(|_|ApiError::unauthorized())?;
        if value.len()>256||!self.origins.iter().any(|known|known==value){return Err(ApiError::unauthorized());}Ok(Some(value.into()))
    }
}
fn secured(mut response:Response,origin:Option<&str>)->Response{
    let headers=response.headers_mut();headers.insert(header::CACHE_CONTROL,HeaderValue::from_static("no-store"));headers.insert(header::X_CONTENT_TYPE_OPTIONS,HeaderValue::from_static("nosniff"));headers.insert(header::REFERRER_POLICY,HeaderValue::from_static("no-referrer"));
    // These endpoints never use cookies or ambient browser HTTP credentials.
    headers.remove(header::ACCESS_CONTROL_ALLOW_CREDENTIALS);headers.remove(header::ACCESS_CONTROL_ALLOW_ORIGIN);
    if let Some(origin)=origin{if let Ok(value)=HeaderValue::from_str(origin){headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN,value);headers.append(header::VARY,HeaderValue::from_static("Origin"));headers.insert(header::ACCESS_CONTROL_EXPOSE_HEADERS,HeaderValue::from_static("Content-Length, Content-Type, X-OmniPlay-Object, X-OmniPlay-Chunk-SHA256"));}}
    response
}
fn preflight(headers:&HeaderMap,path:&str,origin:&str)->Response{
    if headers.get_all(header::ACCESS_CONTROL_REQUEST_METHOD).iter().nth(1).is_some()||headers.get(header::ACCESS_CONTROL_REQUEST_METHOD).and_then(|v|v.to_str().ok())!=Some("POST")||headers.get_all(header::ACCESS_CONTROL_REQUEST_HEADERS).iter().nth(1).is_some()||headers.contains_key("access-control-request-private-network"){return secured(ApiError::unauthorized().into_response(),None);}
    let permitted=if path=="/v1/account"{["content-type","x-omniplay-registration"]}else{["content-type","x-omniplay-authorization"]};
    if let Some(value)=headers.get(header::ACCESS_CONTROL_REQUEST_HEADERS){
        let Ok(value)=value.to_str()else{return secured(ApiError::unauthorized().into_response(),None)};let mut seen=std::collections::BTreeSet::new();
        if value.len()>256||value.split(',').any(|part|{let name=part.trim().to_ascii_lowercase();!permitted.contains(&name.as_str())||!seen.insert(name)}){return secured(ApiError::unauthorized().into_response(),None);}
    }
    let mut response=secured(StatusCode::NO_CONTENT.into_response(),Some(origin));let headers=response.headers_mut();headers.insert(header::ACCESS_CONTROL_ALLOW_METHODS,HeaderValue::from_static("POST"));headers.insert(header::ACCESS_CONTROL_ALLOW_HEADERS,if path=="/v1/account"{HeaderValue::from_static("Content-Type, X-OmniPlay-Registration")}else{HeaderValue::from_static("Content-Type, X-OmniPlay-Authorization")});headers.insert(header::ACCESS_CONTROL_MAX_AGE,HeaderValue::from_static("600"));headers.append(header::VARY,HeaderValue::from_static("Access-Control-Request-Method, Access-Control-Request-Headers"));response
}
pub(crate) async fn handle(State(policy):State<Arc<Policy>>,request:Request,next:Next)->Response{
    let origin=match policy.origin(request.headers()){Ok(value)=>value,Err(error)=>return secured(error.into_response(),None)};
    let path=request.uri().path();let api=matches!(path,"/v1/account"|"/v1/sync");
    if api&&request.uri().query().is_some(){return secured(ApiError::invalid().into_response(),origin.as_deref());}
    if request.method()==Method::OPTIONS{
        return if api{match origin{Some(value)=>preflight(request.headers(),path,&value),None=>secured(ApiError::unauthorized().into_response(),None)}}else{secured(ApiError::invalid().into_response(),None)};
    }
    if api&&request.method()!=Method::POST{return secured(ApiError::invalid().into_response(),origin.as_deref());}
    secured(next.run(request).await,origin.as_deref())
}
