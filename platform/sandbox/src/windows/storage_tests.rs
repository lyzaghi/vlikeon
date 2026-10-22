//! This requires an explicitly compiled final-pass fixture and real Windows.
//! It is intentionally ignored by ordinary unit runs, never a silent mock pass.
use super::*;
use std::{fs,io::Read,path::PathBuf,sync::mpsc,time::{Duration,SystemTime,UNIX_EPOCH}};

struct Fixture(PathBuf);
impl Drop for Fixture{fn drop(&mut self){let _=fs::remove_dir_all(&self.0);}}

fn assert_plain_test_thread(){
    let mut token=std::ptr::null_mut();
    if unsafe{OpenThreadToken(GetCurrentThread(),TOKEN_QUERY,1,&mut token)}!=0{
        drop(handle(token).unwrap());panic!("the final-pass caller must not have an impersonation token");
    }
    assert_eq!(unsafe{GetLastError()},ERROR_NO_TOKEN);
}

#[test]
#[ignore="item-16 Windows OS fixture: set OMNIPLAY_WINDOWS_STORAGE_PROBE to the exact compiled qa/windows/storage-probe.exe"]
fn actual_actor_cannot_write_its_default_storage_but_can_use_private_scratch(){
    assert_plain_test_thread();
    let probe=PathBuf::from(std::env::var_os("OMNIPLAY_WINDOWS_STORAGE_PROBE").expect("the exact final-pass storage probe is required"));
    assert!(probe.is_absolute());let source=fs::symlink_metadata(&probe).unwrap();
    assert!(source.is_file()&&source.len()>0&&source.len()<=8*1024*1024);
    use std::os::windows::fs::MetadataExt;
    assert_eq!(source.file_attributes()&0x400,0);
    let nonce=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()^((std::process::id()as u128)<<64);
    let moniker=format!("OmniPlay.Session.{nonce:032x}");
    let path=std::env::temp_dir().join(format!("omniplay-storage-probe-{nonce:032x}"));fs::create_dir(&path).unwrap();
    let fixture=Fixture(path.canonicalize().unwrap());
    let runtime=fixture.0.join("runtime");let work=fixture.0.join("work");let scratch=work.join("scratch");
    fs::create_dir(&runtime).unwrap();fs::create_dir(&work).unwrap();fs::create_dir(&scratch).unwrap();
    let executable=runtime.join("storage-probe.exe");fs::copy(&probe,&executable).unwrap();
    let identity=ContainerIdentity::create_for_session(&moniker).unwrap();
    identity.grant_game_read(&work).unwrap();identity.grant_private_write(&scratch).unwrap();
    grant_public_runtime_read(&runtime).unwrap();
    let Launched{process,input,output}=identity.launch(&executable,&work,&Limits{memory_bytes:256*1024*1024},|_|Ok(())).unwrap();
    assert_plain_test_thread();
    drop(input);let(sender,receiver)=mpsc::channel();
    let reader=std::thread::spawn(move||{let mut bytes=Vec::new();let result=output.take(4097).read_to_end(&mut bytes).map(|_|bytes);let _=sender.send(result);});
    let result=receiver.recv_timeout(Duration::from_secs(15));
    process.terminate();let ended=process.has_exited();drop(process);
    if reader.is_finished(){reader.join().unwrap();}
    assert!(ended,"the real probe process must be confirmed ended");
    let bytes=result.expect("storage probe response deadline").expect("storage probe read");
    assert!(bytes.len()<=4096);
    assert_eq!(std::str::from_utf8(&bytes).unwrap().trim(),"storage-probe-v2 1 1 1 1 1 1 1");
    identity.retire().unwrap();
}
