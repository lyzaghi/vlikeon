#![forbid(unsafe_code)]
mod store;
mod turn;
use axum::{Router,Json,extract::{State,DefaultBodyLimit},routing::{get,post},http::{HeaderMap,HeaderValue,StatusCode,header},response::{IntoResponse,Response}};
use omniplay_streaming::{RelayEnvelope,RelayOperation,verify_request,validate_audience,key_id};
use serde::Serialize;
use std::{collections::HashMap,net::SocketAddr,path::PathBuf,sync::Arc,time::Instant};
use tokio::sync::Mutex;

type ApiResult<T>=std::result::Result<T,ApiError>;
#[derive(Serialize)]struct ApiError{#[serde(skip)]status:StatusCode,code:&'static str,message:&'static str}
impl ApiError{
    fn invalid()->Self{Self{status:StatusCode::BAD_REQUEST,code:"invalid_request",message:"Streaming request is invalid"}}
    fn unauthorized()->Self{Self{status:StatusCode::UNAUTHORIZED,code:"unauthorized",message:"Streaming authorization is invalid, expired or revoked"}}
    fn quota()->Self{Self{status:StatusCode::TOO_MANY_REQUESTS,code:"resource_limit",message:"Streaming service limit was reached"}}
    fn not_found()->Self{Self{status:StatusCode::NOT_FOUND,code:"not_found",message:"Pairing invitation or request is unavailable"}}
    fn conflict()->Self{Self{status:StatusCode::CONFLICT,code:"conflict",message:"Streaming identity or sequence conflicts with existing state"}}
    fn storage()->Self{Self{status:StatusCode::INTERNAL_SERVER_ERROR,code:"storage_failure",message:"Streaming service storage is unavailable"}}
}
impl IntoResponse for ApiError{fn into_response(self)->Response{(self.status,Json(self)).into_response()}}
struct Rates{start:Instant,total:u32,identities:HashMap<String,u32>}
impl Rates{
    fn tick(&mut self){if self.start.elapsed().as_secs()>=60{self.start=Instant::now();self.total=0;self.identities.clear();}}
    fn global(&mut self)->ApiResult<()>{self.tick();if self.total>=12000{return Err(ApiError::quota());}self.total+=1;Ok(())}
    fn identity(&mut self,key:String)->ApiResult<()>{self.tick();if self.identities.len()>=1024&&!self.identities.contains_key(&key){return Err(ApiError::quota());}let count=self.identities.entry(key).or_default();if *count>=1200{return Err(ApiError::quota());}*count+=1;Ok(())}
}
struct Service{store:Mutex<store::Store>,audience:String,origins:Vec<String>,rates:Mutex<Rates>,turn:Option<turn::Turn>,allow_loopback_web:bool}
type Shared=Arc<Service>;
fn now_ms()->u64{std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|v|v.as_millis().min(u64::MAX as u128)as u64).unwrap_or(0)}
#[tokio::main]async fn main(){if let Err(error)=run().await{eprintln!("OmniPlay signaling stopped: {}",error.code);std::process::exit(1);}}
async fn run()->ApiResult<()>{
    let audience=std::env::var("OMNIPLAY_RELAY_AUDIENCE").map_err(|_|ApiError::invalid())?;validate_audience(&audience).map_err(|_|ApiError::invalid())?;
    let origins:Vec<_>=std::env::var("OMNIPLAY_RELAY_ORIGINS").map_err(|_|ApiError::invalid())?.split(',').map(str::trim).map(str::to_owned).collect();
    if origins.is_empty()||origins.len()>16{return Err(ApiError::invalid());}for origin in &origins{if validate_audience(origin).is_err()&&!origin.strip_prefix("chrome-extension://").is_some_and(|id|id.len()==32&&id.bytes().all(|b|(b'a'..=b'p').contains(&b))){return Err(ApiError::invalid());}}
    let root=PathBuf::from(std::env::var_os("OMNIPLAY_RELAY_DATA").ok_or_else(ApiError::invalid)?);
    if !root.is_absolute()||std::fs::symlink_metadata(&root).is_ok_and(|m|m.file_type().is_symlink()){return Err(ApiError::invalid());}std::fs::create_dir_all(&root).map_err(|_|ApiError::storage())?;
    #[cfg(unix)]{use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(&root,std::fs::Permissions::from_mode(0o700)).map_err(|_|ApiError::storage())?;}
    let root=root.canonicalize().map_err(|_|ApiError::storage())?;
    let listen:SocketAddr=std::env::var("OMNIPLAY_RELAY_LISTEN").unwrap_or_else(|_|"127.0.0.1:8793".into()).parse().map_err(|_|ApiError::invalid())?;
    if !listen.ip().is_loopback(){return Err(ApiError::invalid());}
    let shared=Arc::new(Service{store:Mutex::new(store::Store::open(&root)?),audience,origins,rates:Mutex::new(Rates{start:Instant::now(),total:0,identities:HashMap::new()}),turn:turn::Turn::configured()?,allow_loopback_web:std::env::var("OMNIPLAY_RELAY_ALLOW_LOOPBACK_WEB").as_deref()==Ok("1")});
    let app=Router::new().route("/health",get(||async{Json(serde_json::json!({"service":"omniplay-signaling","protocol":1}))})).route("/v1/relay",post(relay).options(preflight)).layer(DefaultBodyLimit::max(192*1024)).layer(tower::limit::ConcurrencyLimitLayer::new(16)).with_state(shared);
    let listener=tokio::net::TcpListener::bind(listen).await.map_err(|_|ApiError::storage())?;
    axum::serve(listener,app).with_graceful_shutdown(async{let _=tokio::signal::ctrl_c().await;}).await.map_err(|_|ApiError::storage())
}
fn origin<'a>(service:&Service,headers:&'a HeaderMap)->ApiResult<Option<&'a str>>{
    let value=headers.get(header::ORIGIN).map(|v|v.to_str().map_err(|_|ApiError::unauthorized())).transpose()?;
    if value.is_some_and(|v|!service.origins.iter().any(|known|known==v)&&!(service.allow_loopback_web&&v.strip_prefix("http://127.0.0.1:").is_some_and(|p|p.parse::<u16>().is_ok_and(|n|n>0)&&p.bytes().all(|b|b.is_ascii_digit())))){return Err(ApiError::unauthorized());}Ok(value)
}
fn cors(mut response:Response,origin:Option<&str>)->Response{
    let headers=response.headers_mut();headers.insert(header::CACHE_CONTROL,HeaderValue::from_static("no-store"));headers.insert(header::X_CONTENT_TYPE_OPTIONS,HeaderValue::from_static("nosniff"));headers.insert(header::REFERRER_POLICY,HeaderValue::from_static("no-referrer"));
    if let Some(origin)=origin{if let Ok(value)=HeaderValue::from_str(origin){headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN,value);headers.insert(header::VARY,HeaderValue::from_static("Origin"));}}response
}
async fn preflight(State(service):State<Shared>,headers:HeaderMap)->Response{
    let allowed=match origin(&service,&headers){Ok(Some(value))=>value,_=>return ApiError::unauthorized().into_response()};
    if headers.get(header::ACCESS_CONTROL_REQUEST_METHOD).and_then(|v|v.to_str().ok())!=Some("POST"){return ApiError::unauthorized().into_response();}
    if headers.get(header::ACCESS_CONTROL_REQUEST_HEADERS).and_then(|v|v.to_str().ok()).is_some_and(|v|v.split(',').any(|part|!part.trim().eq_ignore_ascii_case("content-type"))){return ApiError::unauthorized().into_response();}
    let mut response=cors(StatusCode::NO_CONTENT.into_response(),Some(allowed));response.headers_mut().insert(header::ACCESS_CONTROL_ALLOW_METHODS,HeaderValue::from_static("POST"));response.headers_mut().insert(header::ACCESS_CONTROL_ALLOW_HEADERS,HeaderValue::from_static("Content-Type"));response.headers_mut().insert(header::ACCESS_CONTROL_MAX_AGE,HeaderValue::from_static("600"));response
}
async fn relay(State(service):State<Shared>,headers:HeaderMap,body:Json<RelayEnvelope>)->Response{
    let allowed=match origin(&service,&headers){Ok(value)=>value,Err(error)=>return error.into_response()};
    let result=async{
        service.rates.lock().await.global()?;let signer=key_id(&body.public_key).map_err(|_|ApiError::unauthorized())?;
        let request=verify_request(&body,&service.audience,now_ms()).map_err(|_|ApiError::unauthorized())?;
        service.rates.lock().await.identity(signer.clone())?;
        let needs_ice=matches!(&request.operation,RelayOperation::IceCredentials{..});
        let value=service.store.lock().await.execute(&body.public_key,request,&service.audience,now_ms())?;
        if needs_ice{let expires=value["expires_at_ms"].as_u64().ok_or_else(ApiError::invalid)?;return Ok(Json(match &service.turn{Some(turn)=>turn.credentials(&signer,expires)?,None=>serde_json::json!({"ice_servers":[],"expires_at_ms":expires,"relay_available":false})}).into_response());}
        Ok::<_,ApiError>(Json(value).into_response())
    }.await;
    cors(result.unwrap_or_else(IntoResponse::into_response),allowed)
}
