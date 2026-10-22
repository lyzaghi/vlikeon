#![deny(unsafe_op_in_unsafe_fn)]
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use serde::{Serialize,Deserialize};
use std::{sync::mpsc,time::{Duration,Instant,SystemTime,UNIX_EPOCH}};
#[cfg(windows)]mod windows;
#[cfg(any(target_os="linux",target_os="android"))]mod linux;
#[cfg(any(target_os="macos",target_os="ios"))]mod apple;

#[derive(Clone,Debug,Default,Serialize,Deserialize)]
pub struct PowerInfo{pub battery_present:Option<bool>,pub ac_connected:Option<bool>,pub charging:Option<bool>,pub battery_percent:Option<u8>,pub battery_saver:Option<bool>}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct GraphicsAdapter{pub name:Option<String>,pub vendor_id:Option<u32>,pub device_id:Option<u32>,pub dedicated_memory_bytes:Option<u64>,pub shared_memory_bytes:Option<u64>,pub software:Option<bool>,pub driver_version:Option<String>,pub driver_version_source:Option<String>}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct DeviceReport{
    pub schema_version:u32,pub source:String,pub sampled_at_ms:u64,pub collection_duration_ms:u64,
    pub operating_system:String,pub process_architecture:String,pub kernel_version:Option<String>,
    pub platform_version:Option<String>,pub platform_source:Option<String>,pub platform_sampled_at_ms:Option<u64>,pub thermal_state:Option<String>,
    pub available_parallelism:Option<u32>,pub total_memory_bytes:Option<u64>,pub available_memory_bytes:Option<u64>,
    pub power:PowerInfo,pub graphics_adapters:Vec<GraphicsAdapter>,pub notes:Vec<String>,
    pub benchmark_evidence:bool,
}
pub(crate) fn clean(value:&str,limit:usize)->String{value.chars().filter(|c|!c.is_control()&&!matches!(*c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}')).take(limit).collect::<String>().trim().to_string()}
fn capture()->DeviceReport{
    let start=Instant::now();let mut report=DeviceReport{schema_version:1,source:"unsupported_platform".into(),sampled_at_ms:SystemTime::now().duration_since(UNIX_EPOCH).map(|value|value.as_millis().min(9_007_199_254_740_991)as u64).unwrap_or(0),collection_duration_ms:0,operating_system:std::env::consts::OS.into(),process_architecture:std::env::consts::ARCH.into(),kernel_version:None,platform_version:None,platform_source:None,platform_sampled_at_ms:None,thermal_state:None,available_parallelism:std::thread::available_parallelism().ok().and_then(|value|u32::try_from(value.get()).ok()).filter(|value|*value>0&&*value<=65536),total_memory_bytes:None,available_memory_bytes:None,power:PowerInfo::default(),graphics_adapters:Vec::new(),notes:Vec::new(),benchmark_evidence:false};
    #[cfg(windows)]windows::collect(&mut report);
    #[cfg(any(target_os="linux",target_os="android"))]linux::collect(&mut report);
    #[cfg(any(target_os="macos",target_os="ios"))]apple::collect(&mut report);
    if report.total_memory_bytes.is_some_and(|value|value==0||value>9_007_199_254_740_991){report.total_memory_bytes=None;}
    if report.available_memory_bytes.is_some_and(|value|value>9_007_199_254_740_991||report.total_memory_bytes.is_some_and(|total|value>total)){report.available_memory_bytes=None;}
    for adapter in &mut report.graphics_adapters{if adapter.dedicated_memory_bytes.is_some_and(|value|value>9_007_199_254_740_991){adapter.dedicated_memory_bytes=None;}if adapter.shared_memory_bytes.is_some_and(|value|value>9_007_199_254_740_991){adapter.shared_memory_bytes=None;}}
    report.graphics_adapters.truncate(8);report.notes.truncate(16);report.collection_duration_ms=start.elapsed().as_millis().min(u64::MAX as u128)as u64;report
}

#[derive(Default)]
pub struct DeviceProbe{pending:Option<mpsc::Receiver<DeviceReport>>,cached:Option<(Instant,DeviceReport)>}
impl DeviceProbe{
    pub fn sample(&mut self)->Result<DeviceReport>{
        if let Some((at,report))=&self.cached{if at.elapsed()<Duration::from_secs(15){return Ok(report.clone());}}
        if self.pending.is_none(){
            let(sender,receiver)=mpsc::sync_channel(1);
            std::thread::Builder::new().name("omniplay-device-probe".into()).spawn(move||{let _=sender.send(capture());}).map_err(|_|PlatformError::new(ErrorCode::RuntimeUnavailable,"The device probe could not start"))?;
            self.pending=Some(receiver);
        }
        let result=self.pending.as_ref().expect("probe receiver was installed").recv_timeout(Duration::from_secs(2));
        match result{
            Ok(report)=>{self.pending=None;self.cached=Some((Instant::now(),report.clone()));Ok(report)},
            Err(mpsc::RecvTimeoutError::Timeout)=>Err(PlatformError::new(ErrorCode::Busy,"The device probe is still collecting information").recover("Try refreshing the device panel later")),
            Err(mpsc::RecvTimeoutError::Disconnected)=>{self.pending=None;Err(PlatformError::new(ErrorCode::RuntimeUnavailable,"Device information is unavailable"))},
        }
    }
}
