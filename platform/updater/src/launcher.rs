#![forbid(unsafe_code)]
#![cfg_attr(all(windows,not(debug_assertions)),windows_subsystem="windows")]
use omniplay_contracts::Result;
use omniplay_updater::{channel::Channel,package,store::Store,rejected};
use std::process::{Command,Stdio};

const ORIGIN:&str=include_str!("../../../security/game_hub_origin.txt");

fn run()->Result<i32> {
    let args=std::env::args_os().skip(1).collect::<Vec<_>>();
    let service=args.len()==1&&args[0].to_str()==Some("--linux-native-host-service");
    let native=args.first().and_then(|v|v.to_str()).is_some_and(|v|v.starts_with("chrome-extension://"));
    if native&&args.first().and_then(|v|v.to_str())!=Some(ORIGIN.trim()){return Err(rejected("Native messaging caller is not the OmniPlay component"));}
    let mut store=Store::open(&omniplay_updater::install_root()?,Channel::embedded()?)?;
    let mut release=store.active()?;
    if !native&&!service&&!release.healthy&&release.attempts>=3 {
        // Roll back only inside the security epoch. If no safe prior version exists,
        // retain the current one so a signed repair can be installed explicitly.
        if let Ok(previous)=store.rollback(){release=previous;}
    }
    store.verify_installed(&release,false)?;
    let directory=store.version_directory(release.id);
    if service&&release.manifest.platform!="linux-x86_64"{return Err(rejected("The delegated native host is available only in its Linux release"));}
    let entry=if service{omniplay_updater::package::LINUX_NATIVE_HOST_ENTRY}else if native{&release.manifest.broker_entry}else{&release.manifest.browser_entry};
    let executable=package::verify_entry(&directory,&release.manifest,entry)?;
    let mut process=Command::new(executable);
    process.current_dir(&directory).env("OMNIPLAY_ACTIVE_RELEASE",release.id.to_string());if !service{process.args(&args);}
    #[cfg(unix)]
    if service{use std::os::unix::process::CommandExt;let error=process.exec();return Err(rejected(&format!("The installed delegated native host could not replace its launcher: {error}")));}
    #[cfg(not(unix))]
    if service{return Err(rejected("The delegated native host cannot run on this platform"));}
    // A stable launcher is registered as the native host. Its dispatch path follows
    // the activated version atomically; registration never points at a deleted update.
    if native {
        process.stdin(Stdio::inherit()).stdout(Stdio::inherit()).stderr(Stdio::inherit());
        let mut child=process.spawn().map_err(|_|rejected("The installed native host could not start"))?;
        Ok(child.wait().map_err(|_|rejected("The installed native host did not exit cleanly"))?.code().unwrap_or(1))
    }else{
        store.note_launch(release.id)?;
        process.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        process.spawn().map_err(|_|rejected("The installed browser could not start"))?;
        Ok(0)
    }
}
fn main(){match run(){Ok(code)=>std::process::exit(code),Err(error)=>{eprintln!("{error}");std::process::exit(1);}}}
