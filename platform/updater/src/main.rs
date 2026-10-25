#![forbid(unsafe_code)]
use omniplay_contracts::Result;
use omniplay_updater::{channel::Channel,store::Store,rejected};
use std::path::Path;

fn encode(value:impl serde::Serialize)->Result<serde_json::Value>{serde_json::to_value(value).map_err(|_|rejected("Update response encoding failed"))}

fn run()->Result<serde_json::Value> {
    let args=std::env::args().skip(1).collect::<Vec<_>>();
    let channel=Channel::embedded()?;let mut store=Store::open(&omniplay_updater::install_root()?,channel)?;
    match args.as_slice() {
        [command]if command=="status"=>encode(store.status()?),
        [command,id]if command=="verify"=>{let release=store.verify_release_files(uuid::Uuid::parse_str(id).map_err(|_|rejected("Release ID is invalid"))?)?;Ok(serde_json::json!({"release_id":release.id,"inventory_matches":true,"files":release.manifest.files.len()}))},
        [command]if command=="download"=>{store.refresh_roots(&std::sync::atomic::AtomicBool::new(false))?;let root=store.trusted_root()?;let envelope=omniplay_updater::transport::metadata(&store.channel.metadata_url,&root)?;encode(store.stage_download(&envelope)?)},
        [command,metadata,package]if command=="stage"=>{let envelope=Store::envelope_from_file(Path::new(metadata))?;encode(store.stage_file(&envelope,Path::new(package))?)},
        [command,id]if command=="activate"=>encode(store.activate(uuid::Uuid::parse_str(id).map_err(|_|rejected("Release ID is invalid"))?)?),
        [command]if command=="rollback"=>encode(store.rollback()?),
        [command,id,package]if command=="repair"=>{let release_id=uuid::Uuid::parse_str(id).map_err(|_|rejected("Release ID is invalid"))?;encode(omniplay_updater::repair::repair_release(&mut store,release_id,Path::new(package),&std::sync::atomic::AtomicBool::new(false))?)},
        [command]if command=="sweep"=>encode(omniplay_updater::cleanup::sweep_staging(&store,86400)?),
        [command,seconds]if command=="sweep"=>{let secs=seconds.parse::<u64>().map_err(|_|rejected("Invalid seconds parameter"))?;encode(omniplay_updater::cleanup::sweep_staging(&store,secs)?)},
        [command]if command=="prune"=>encode(omniplay_updater::cleanup::prune_old_releases(&mut store,2)?),
        [command,count]if command=="prune"=>{let cnt=count.parse::<usize>().map_err(|_|rejected("Invalid keep count"))?;encode(omniplay_updater::cleanup::prune_old_releases(&mut store,cnt)?)},
        [command,old,new]if command=="rotate-root"=>{let version=store.rotate_root(&Store::envelope_from_file(Path::new(old))?,&Store::envelope_from_file(Path::new(new))?)?;Ok(serde_json::json!({"root_version":version}))},
        _=>Err(rejected("Use status, verify <id>, repair <id> <pkg>, sweep [secs], prune [count], download, stage <meta> <pkg>, activate <id>, rollback, or rotate-root <old> <new>")),
    }
}
fn main(){match run(){Ok(value)=>println!("{value}"),Err(error)=>{eprintln!("{error}");std::process::exit(1);}}}
