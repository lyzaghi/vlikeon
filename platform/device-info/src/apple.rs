use super::{DeviceReport,clean};
pub fn collect(report:&mut DeviceReport){
    report.source="apple_sysctl".into();let mut bytes=0u64;let mut length=std::mem::size_of::<u64>();
    if unsafe{libc::sysctlbyname(c"hw.memsize".as_ptr(),(&mut bytes as *mut u64).cast(),&mut length,std::ptr::null_mut(),0)}==0&&length==std::mem::size_of::<u64>(){report.total_memory_bytes=Some(bytes);}else{report.notes.push("memory_unavailable".into());}
    let mut kernel=[0u8;128];let mut length=kernel.len();
    if unsafe{libc::sysctlbyname(c"kern.osrelease".as_ptr(),kernel.as_mut_ptr().cast(),&mut length,std::ptr::null_mut(),0)}==0&&length>0&&length<=kernel.len(){if let Ok(value)=std::str::from_utf8(&kernel[..length.saturating_sub(1)]){let value=clean(value,96);if !value.is_empty(){report.kernel_version=Some(value);}}}
    report.notes.push("power_inventory_unavailable".into());report.notes.push("graphics_inventory_unavailable".into());
}
