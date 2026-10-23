//! Inspect the closed newc image without extracting or executing guest code.
//! Actual embedded worker/core/assets must match the authenticated outer files.
use crate::{registry::{RuntimePackage, RuntimeFile}, unavailable};
use omniplay_contracts::*;
use omniplay_core::util::storage_error;
use sha2::{Digest, Sha256};
use std::{collections::{BTreeMap, BTreeSet}, fs::File, io::Read};

fn invalid() -> PlatformError { unavailable("The diskless guest image differs from its runtime inventory") }
fn exact(file: &mut File, bytes: &mut [u8]) -> Result<()> { file.read_exact(bytes).map_err(storage_error) }
fn number(bytes: &[u8]) -> Result<u64> {
    if bytes.len() != 8 || !bytes.iter().all(|b| b.is_ascii_hexdigit()) { return Err(invalid()); }
    u64::from_str_radix(std::str::from_utf8(bytes).map_err(|_| invalid())?, 16).map_err(|_| invalid())
}
fn padding(file: &mut File, length: u64) -> Result<()> {
    let mut bytes = [0; 3]; let count = ((4 - length % 4) % 4) as usize;
    exact(file, &mut bytes[..count])?;
    if bytes[..count].iter().any(|b| *b != 0) { return Err(invalid()); } Ok(())
}
pub(crate) fn admit(file: &mut File, package: &RuntimePackage) -> Result<()> {
    let mac = package.macos.as_ref().ok_or_else(invalid)?;
    let helper = package.presentation.as_ref().ok_or_else(invalid)?;
    let prefix = format!("native-runtimes/{}/", package.manifest.id);
    let mut expected: BTreeMap<String, &RuntimeFile> = BTreeMap::new();
    for item in &package.files {
        if item.path == helper.executable || item.path == mac.guest_kernel || item.path == mac.guest_initrd { continue; }
        let relative = item.path.strip_prefix(&prefix).ok_or_else(invalid)?;
        let relative = if item.path == package.worker { "omniplay-native-worker" } else { relative };
        if expected.insert(format!("opt/vlike/runtime/{relative}"), item).is_some() { return Err(invalid()); }
    }
    let mut seen = BTreeSet::new();
    let mut embedded = BTreeMap::new();
    let mut image_receipt = None;
    let mut total = 0u64;
    let mut ended = false;
    for _ in 0..512 {
        let mut header = [0; 110]; exact(file, &mut header)?;
        if &header[..6] != b"070701" { return Err(invalid()); }
        let mode = number(&header[14..22])?;
        let bytes = number(&header[54..62])?;
        let name_bytes = number(&header[94..102])?;
        if !(2..=201).contains(&name_bytes) || bytes > 256 * 1024 * 1024 ||
            number(&header[22..30])? != 0 || number(&header[30..38])? != 0 ||
            number(&header[38..46])? != 1 || number(&header[46..54])? != 0 ||
            [62, 70, 78, 86, 102].into_iter().any(|at| number(&header[at..at+8]).ok() != Some(0)) {
            return Err(invalid());
        }
        total = total.checked_add(110 + name_bytes + bytes + 6).ok_or_else(invalid)?;
        if total > 256 * 1024 * 1024 { return Err(invalid()); }
        let mut name = vec![0; name_bytes as usize]; exact(file, &mut name)?;
        if name.pop() != Some(0) || name.contains(&0) { return Err(invalid()); }
        let name = String::from_utf8(name).map_err(|_| invalid())?;
        padding(file, 110 + name_bytes)?;
        if name == "TRAILER!!!" {
            if bytes != 0 || mode != 0 { return Err(invalid()); }
            ended = true; break;
        }
        if !name.is_ascii() || name.split('/').any(|p| p.is_empty() || p == "." || p == ".." ||
            !p.bytes().all(|b| b.is_ascii_alphanumeric() || b"_+.-".contains(&b))) || !seen.insert(name.clone()) {
            return Err(invalid());
        }
        if mode == 0o040755 {
            if bytes != 0 { return Err(invalid()); }
            continue;
        }
        if mode != 0o100644 && mode != 0o100755 { return Err(invalid()); }
        if (name == "init" || name == "opt/vlike/runtime/omniplay-native-worker") && mode != 0o100755 {
            return Err(invalid());
        }
        if !expected.contains_key(&name) && name != "init" && name != "vlike-image.json" && !name.starts_with("lib/") {
            return Err(invalid());
        }
        if name == "vlike-image.json" && bytes > 1024 * 1024 { return Err(invalid()); }
        let mut hash = Sha256::new(); let mut remaining = bytes;
        let mut buffer = [0; 65536]; let mut receipt = Vec::new();
        while remaining > 0 {
            let count = remaining.min(buffer.len() as u64) as usize;
            exact(file, &mut buffer[..count])?; hash.update(&buffer[..count]);
            if name == "vlike-image.json" { receipt.extend_from_slice(&buffer[..count]); }
            remaining -= count as u64;
        }
        padding(file, bytes)?;
        let hash = format!("{:x}", hash.finalize());
        if let Some(item) = expected.remove(&name) {
            if item.bytes != bytes || item.sha256.0 != hash { return Err(invalid()); }
        }
        if name == "vlike-image.json" { image_receipt = Some(receipt); }
        else { embedded.insert(name, (bytes, hash)); }
    }
    if !ended || !expected.is_empty() { return Err(invalid()); }
    let mut trailing = [0; 512];
    let count = file.read(&mut trailing).map_err(storage_error)?;
    if count > 511 || trailing[..count].iter().any(|b| *b != 0) || file.read(&mut trailing[..1]).map_err(storage_error)? != 0 {
        return Err(invalid());
    }
    let value: serde_json::Value = serde_json::from_slice(&image_receipt.ok_or_else(invalid)?).map_err(|_| invalid())?;
    if value["schema_version"].as_u64() != Some(1) || value["guest_protocol"].as_u64() != Some(1) ||
        value["runtime"].as_str() != Some(package.manifest.id.as_str()) { return Err(invalid()); }
    let init = embedded.get("init").ok_or_else(invalid)?;
    let kernel = package.files.iter().find(|f| f.path == mac.guest_kernel).ok_or_else(invalid)?;
    if value["init_sha256"].as_str() != Some(init.1.as_str()) || value["kernel_sha256"].as_str() != Some(kernel.sha256.0.as_str()) {
        return Err(invalid());
    }
    let dependencies = value["dependencies"].as_array().filter(|v| v.len() <= 128).ok_or_else(invalid)?;
    let mut libraries: BTreeSet<_> = embedded.keys().filter(|p| p.starts_with("lib/")).cloned().collect();
    for dependency in dependencies {
        let name = dependency["path"].as_str().ok_or_else(invalid)?;
        let (bytes, hash) = embedded.get(name).ok_or_else(invalid)?;
        if !libraries.remove(name) || dependency["bytes"].as_u64() != Some(*bytes) || dependency["sha256"].as_str() != Some(hash.as_str()) {
            return Err(invalid());
        }
    }
    if !libraries.is_empty() { return Err(invalid()); }
    Ok(())
}
