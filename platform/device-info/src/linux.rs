use super::{DeviceReport,GraphicsAdapter,clean};
use std::{fs::{self,File},io::Read,path::Path};

fn value(path:&Path,limit:usize)->Option<String>{let file=File::open(path).ok()?;let mut bytes=Vec::new();file.take(limit as u64+1).read_to_end(&mut bytes).ok()?;if bytes.len()>limit{return None;}Some(std::str::from_utf8(&bytes).ok()?.trim().into())}
fn integer(path:&Path)->Option<u64>{let raw=value(path,64)?;if raw.is_empty()||!raw.bytes().all(|v|v.is_ascii_digit()){return None;}raw.parse().ok()}
fn hex(path:&Path)->Option<u32>{let raw=value(path,32)?;let raw=raw.strip_prefix("0x").unwrap_or(&raw);if raw.len()>8||raw.is_empty(){return None;}u32::from_str_radix(raw,16).ok()}
pub fn collect(report:&mut DeviceReport){
    report.source="linux_proc_sysfs".into();
    if let Some(raw)=value(Path::new("/proc/meminfo"),64*1024){
        for line in raw.lines(){let mut parts=line.split_whitespace();let field=parts.next();if !matches!(field,Some("MemTotal:"|"MemAvailable:")){continue;}
            let amount=parts.next().and_then(|v|v.parse::<u64>().ok());let unit=parts.next();if unit!=Some("kB")||parts.next().is_some(){continue;}let bytes=amount.and_then(|v|v.checked_mul(1024));
            if field==Some("MemTotal:"){report.total_memory_bytes=bytes;}else{report.available_memory_bytes=bytes;}
        }
    }else{report.notes.push("memory_unavailable".into());}
    report.kernel_version=value(Path::new("/proc/sys/kernel/osrelease"),128).map(|v|clean(&v,96)).filter(|v|!v.is_empty());
    power(report);graphics(report);
}
fn power(report:&mut DeviceReport){
    let entries=match fs::read_dir("/sys/class/power_supply"){Ok(value)=>value,Err(_)=>{report.notes.push("power_inventory_unavailable".into());return;}};
    let mut batteries=Vec::new();let mut external=Vec::new();let mut complete=true;
    // These read-only sysfs class links are kernel-owned. No application path,
    // serial/model identifiers, writable control or shell command is accepted.
    for(index,entry)in entries.enumerate(){if index>=64{complete=false;break;}let entry=match entry{Ok(value)=>value,Err(_)=>{complete=false;continue;}};let path=entry.path();let kind=match value(&path.join("type"),64){Some(value)=>value,None=>{complete=false;continue;}};
        if kind=="Battery"{
            if value(&path.join("scope"),32).as_deref()==Some("Device"){continue;}
            let present=integer(&path.join("present"));if present==Some(0){continue;}
            let capacity=integer(&path.join("capacity")).filter(|v|*v<=100).map(|v|v as u8);
            let charging=match value(&path.join("status"),32).as_deref(){Some("Charging")=>Some(true),Some("Discharging"|"Full"|"Not charging")=>Some(false),_=>None};batteries.push((present==Some(1),capacity,charging));
        }else if kind=="Mains"||kind=="USB"||kind.starts_with("USB_"){
            external.push(match integer(&path.join("online")){Some(0)=>Some(false),Some(1)=>Some(true),_=>None});
        }
    }
    if !complete{report.notes.push("power_inventory_incomplete".into());}
    report.power.battery_present=if batteries.iter().any(|value|value.0){Some(true)}else if complete&&batteries.is_empty(){Some(false)}else{None};
    if batteries.len()==1{report.power.battery_percent=batteries[0].1;report.power.charging=batteries[0].2;}
    else if batteries.len()>1{report.notes.push("multiple_batteries_not_aggregated".into());}
    report.power.ac_connected=if external.iter().any(|v|*v==Some(true)){Some(true)}else if complete&&!external.is_empty()&&external.iter().all(|v|*v==Some(false)){Some(false)}else{None};
}
fn graphics(report:&mut DeviceReport){
    let entries=match fs::read_dir("/sys/class/drm"){Ok(value)=>value,Err(_)=>{report.notes.push("graphics_inventory_unavailable".into());return;}};
    for(index,entry)in entries.enumerate(){if index>=128{report.notes.push("graphics_inventory_incomplete".into());break;}let entry=match entry{Ok(value)=>value,Err(_)=>continue};let name=entry.file_name();let Some(name)=name.to_str()else{continue;};let Some(number)=name.strip_prefix("card")else{continue;};if number.is_empty()||!number.bytes().all(|c|c.is_ascii_digit()){continue;}
        let path=entry.path().join("device");let vendor=hex(&path.join("vendor"));let device=hex(&path.join("device"));if vendor.is_none()||device.is_none(){continue;}
        report.graphics_adapters.push(GraphicsAdapter{name:None,vendor_id:vendor,device_id:device,dedicated_memory_bytes:None,shared_memory_bytes:None,software:None,driver_version:None,driver_version_source:None});
        if report.graphics_adapters.len()>=8{report.notes.push("graphics_inventory_may_be_truncated".into());break;}
    }
}
