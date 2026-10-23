//! Bind the broker's authenticated catalog to the exact nested XPC bundle.
use crate::{registry::{RuntimePackage,approved_file,parse_catalog},unavailable};
use omniplay_contracts::*;
use omniplay_core::util::bounded_read;
use omniplay_updater::package::PackageManifest;
use serde::Deserialize;
use std::path::Path;
#[derive(Deserialize)]#[serde(deny_unknown_fields)]struct Catalog{schema_version:u32,runtimes:Vec<Identity>}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]struct Identity{id:String,team_identifier:String,presentation:String,presentation_identifier:String,presentation_memory_mb:u32,kernel:String,kernel_sha256:String,kernel_bytes:u64,initrd:String,initrd_sha256:String,initrd_bytes:u64,guest_memory_mb:u32,guest_cpu_count:u32,scratch_mb:u32}
fn invalid()->PlatformError{unavailable("The macOS XPC bundle differs from the authenticated release")}
pub(crate) fn admit(root:&Path,package:&RuntimePackage,inventory:&PackageManifest)->Result<()>{
    if inventory.platform!="macos-aarch64"||inventory.broker_entry!="VlikeonBroker.app/Contents/MacOS/omniplay-broker"{return Err(invalid());}
    let mac=package.macos.as_ref().ok_or_else(invalid)?;mac.validate(package)?;
    let raw=bounded_read(&mut approved_file(root,"native-runtimes/catalog.json",Some(inventory),None)?,1024*1024)?;
    let selected=parse_catalog(&raw)?.into_iter().find(|p|p.manifest.id==package.manifest.id).ok_or_else(invalid)?;
    if serde_json::to_value(&selected).map_err(|_|invalid())?!=serde_json::to_value(package).map_err(|_|invalid())?{return Err(invalid());}
    let owner=format!("VlikeonBroker.app/Contents/XPCServices/{}.xpc/Contents",mac.owner_signing_identifier);
    for path in[format!("{owner}/Info.plist"),format!("{owner}/MacOS/omniplay-macos-owner"),format!("{owner}/_CodeSignature/CodeResources")]{approved_file(root,&path,Some(inventory),None)?;}
    let catalog_path=format!("{owner}/Resources/runtime-catalog.json");
    let raw=bounded_read(&mut approved_file(root,&catalog_path,Some(inventory),None)?,1024*1024)?;
    let catalog:Catalog=serde_json::from_slice(&raw).map_err(|_|invalid())?;
    if catalog.schema_version!=2||catalog.runtimes.is_empty()||catalog.runtimes.len()>32{return Err(invalid());}
    let mut ids=std::collections::BTreeSet::new();
    if catalog.runtimes.iter().any(|entry|!ids.insert(&entry.id)){return Err(invalid());}
    let identity=catalog.runtimes.iter().find(|entry|entry.id==package.manifest.id).ok_or_else(invalid)?;
    let helper=package.presentation.as_ref().ok_or_else(invalid)?;
    if identity.team_identifier!=mac.team_identifier||identity.presentation_identifier!=mac.presentation_signing_identifier||identity.guest_memory_mb!=package.memory_limit_mb||identity.presentation_memory_mb!=helper.memory_limit_mb||identity.guest_cpu_count!=mac.guest_cpu_count||identity.scratch_mb!=mac.scratch_memory_mb{return Err(invalid());}
    let directory=format!("{owner}/Resources/Runtimes/{}",package.manifest.id);
    for(path,basename)in[(&mac.guest_kernel,&identity.kernel),(&helper.executable,&identity.presentation),(&mac.guest_initrd,&identity.initrd)]{
        if Path::new(path).file_name().and_then(|v|v.to_str())!=Some(basename.as_str()){return Err(invalid());}
        let original=package.files.iter().find(|file|&file.path==path).ok_or_else(invalid)?;
        approved_file(root,path,Some(inventory),Some(original))?;
        let mirror=format!("{directory}/{basename}");
        let copied=inventory.files.iter().find(|file|file.path==mirror).ok_or_else(invalid)?;
        if copied.sha256!=original.sha256||copied.bytes!=original.bytes||path==&helper.executable&&!copied.executable{return Err(invalid());}
        approved_file(root,&mirror,Some(inventory),None)?;
    }
    for (path, hash, bytes) in [(&mac.guest_kernel, &identity.kernel_sha256, identity.kernel_bytes),
        (&mac.guest_initrd, &identity.initrd_sha256, identity.initrd_bytes)] {
        let file = package.files.iter().find(|file| &file.path == path).ok_or_else(invalid)?;
        if &file.sha256.0 != hash || file.bytes != bytes { return Err(invalid()); }
    }
    let prefix=format!("native-runtimes/{}/",package.manifest.id);
    for file in &package.files{
        approved_file(root,&file.path,Some(inventory),Some(file))?;
        if let Some(relative)=file.path.strip_prefix(&format!("{prefix}assets/")){
            let mirror=format!("{directory}/assets/{relative}");let copied=inventory.files.iter().find(|item|item.path==mirror).ok_or_else(invalid)?;
            if copied.bytes!=file.bytes||copied.sha256!=file.sha256{return Err(invalid());}
            approved_file(root,&mirror,Some(inventory),None)?;
        }
    }
    let image = package.files.iter().find(|file| file.path == mac.guest_initrd).ok_or_else(invalid)?;
    let mut image = approved_file(root, &image.path, Some(inventory), Some(image))?;
    crate::mac_guest::admit(&mut image, package)?;
    Ok(())
}
