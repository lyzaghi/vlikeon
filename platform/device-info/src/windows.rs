use super::{DeviceReport,GraphicsAdapter,clean};
use ::windows::core::Interface;
use ::windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1,IDXGIFactory1,IDXGIDevice,DXGI_ADAPTER_FLAG_SOFTWARE};
use windows_sys::Win32::System::{SystemInformation::{GlobalMemoryStatusEx,MEMORYSTATUSEX},Power::{GetSystemPowerStatus,SYSTEM_POWER_STATUS}};

pub fn collect(report:&mut DeviceReport){
    report.source="windows_api_dxgi".into();
    let mut memory=MEMORYSTATUSEX::default();memory.dwLength=std::mem::size_of::<MEMORYSTATUSEX>()as u32;
    if unsafe{GlobalMemoryStatusEx(&mut memory)}!=0{report.total_memory_bytes=Some(memory.ullTotalPhys);report.available_memory_bytes=Some(memory.ullAvailPhys);}else{report.notes.push("memory_unavailable".into());}
    let mut power=SYSTEM_POWER_STATUS::default();
    if unsafe{GetSystemPowerStatus(&mut power)}!=0{
        report.power.ac_connected=match power.ACLineStatus{0=>Some(false),1=>Some(true),_=>None};
        if power.BatteryFlag!=255{let present=power.BatteryFlag&128==0;report.power.battery_present=Some(present);if present{report.power.charging=Some(power.BatteryFlag&8!=0);report.power.battery_percent=(power.BatteryLifePercent<=100).then_some(power.BatteryLifePercent);}}
        report.power.battery_saver=match power.SystemStatusFlag{0=>Some(false),1=>Some(true),_=>None};
    }else{report.notes.push("power_unavailable".into());}
    // Enumeration describes available OS adapters, not the adapter actually
    // selected by Chromium/SDL or proof of a graphics API's usable performance.
    let factory:IDXGIFactory1=match unsafe{CreateDXGIFactory1()}{Ok(value)=>value,Err(_)=>{report.notes.push("graphics_inventory_unavailable".into());return;}};
    for index in 0..8u32{
        let adapter=match unsafe{factory.EnumAdapters1(index)}{Ok(value)=>value,Err(_)=>break};
        let description=match unsafe{adapter.GetDesc1()}{Ok(value)=>value,Err(_)=>{report.notes.push("graphics_adapter_unavailable".into());continue;}};
        let end=description.Description.iter().position(|c|*c==0).unwrap_or(description.Description.len());let name=clean(&String::from_utf16_lossy(&description.Description[..end]),192);
        let driver=unsafe{adapter.CheckInterfaceSupport(&IDXGIDevice::IID)}.ok().map(|version|format!("{:016x}",version as u64));
        report.graphics_adapters.push(GraphicsAdapter{name:(!name.is_empty()).then_some(name),vendor_id:Some(description.VendorId),device_id:Some(description.DeviceId),dedicated_memory_bytes:Some(description.DedicatedVideoMemory as u64),shared_memory_bytes:Some(description.SharedSystemMemory as u64),software:Some(description.Flags&DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32!=0),driver_version_source:driver.as_ref().map(|_|"dxgi_umd_version_raw_hex".into()),driver_version:driver});
    }
    if report.graphics_adapters.len()==8{report.notes.push("graphics_inventory_may_be_truncated".into());}
}
