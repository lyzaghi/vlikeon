//! This requires an explicitly compiled final-pass fixture and real Windows.
//! It is intentionally ignored by ordinary unit runs, never a silent mock pass.
use super::*;
use std::{fs,io::Read,path::PathBuf,sync::mpsc,time::{Duration,SystemTime,UNIX_EPOCH}};

struct Fixture{root:PathBuf,retired:bool}
impl Drop for Fixture{fn drop(&mut self){if self.retired{let _=fs::remove_dir_all(&self.root);}else{eprintln!("unconfirmed storage fixture retained: {}",self.root.display());}}}

fn assert_plain_test_thread(){
    let mut token=std::ptr::null_mut();
    if unsafe{OpenThreadToken(GetCurrentThread(),TOKEN_QUERY,1,&mut token)}!=0{
        drop(handle(token).unwrap());panic!("the final-pass caller must not have an impersonation token");
    }
    assert_eq!(unsafe{GetLastError()},ERROR_NO_TOKEN);
}

fn run_probe(variable:&str,writable_scratch:bool,expected:&str){
    assert_plain_test_thread();
    let probe=PathBuf::from(std::env::var_os(variable).expect("the exact assigned storage probe is required"));
    assert!(probe.is_absolute());let source=fs::symlink_metadata(&probe).unwrap();
    assert!(source.is_file()&&source.len()>0&&source.len()<=8*1024*1024);
    use std::os::windows::fs::MetadataExt;
    assert_eq!(source.file_attributes()&0x400,0);
    let nonce=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()^((std::process::id()as u128)<<64);
    let moniker=format!("OmniPlay.Session.{nonce:032x}");
    let path=std::env::temp_dir().join(format!("omniplay-storage-probe-{nonce:032x}"));fs::create_dir(&path).unwrap();
    let mut fixture=Fixture{root:path.canonicalize().unwrap(),retired:false};
    let runtime=fixture.root.join("runtime");let work=fixture.root.join("work");let scratch=work.join("scratch");
    fs::create_dir(&runtime).unwrap();fs::create_dir(&work).unwrap();fs::create_dir(&scratch).unwrap();
    let executable=runtime.join("storage-probe.exe");fs::copy(&probe,&executable).unwrap();
    let identity=ContainerIdentity::create_for_session(&moniker).unwrap();
    identity.grant_game_read(&work).unwrap();if writable_scratch{identity.grant_private_write(&scratch).unwrap();}
    grant_public_runtime_read(&runtime).unwrap();
    let Launched{process,input,output}=identity.launch(&executable,&work,&Limits{memory_bytes:256*1024*1024},|_|Ok(())).unwrap();
    assert_plain_test_thread();
    drop(input);let(sender,receiver)=mpsc::channel();
    let reader=std::thread::spawn(move||{let mut bytes=Vec::new();let result=output.take(4097).read_to_end(&mut bytes).map(|_|bytes);let _=sender.send(result);});
    let result=receiver.recv_timeout(Duration::from_secs(15));
    let exit_deadline=std::time::Instant::now()+Duration::from_secs(1);
    while !process.has_exited()&&std::time::Instant::now()<exit_deadline{std::thread::sleep(Duration::from_millis(10));}
    let mut exit_code=0u32;
    if unsafe{GetExitCodeProcess(process.process.as_raw_handle(),&mut exit_code)}!=0{
        eprintln!("storage probe observed exit code: {exit_code:#010x}");
    }
    process.terminate();let ended=process.has_exited();drop(process);
    if crate::owned_thread_ended(&reader){reader.join().unwrap();}
    assert!(ended,"the real probe process must be confirmed ended");
    let bytes=result.expect("storage probe response deadline").expect("storage probe read");
    assert!(bytes.len()<=4096);
    assert_eq!(std::str::from_utf8(&bytes).unwrap().trim(),expected);
    identity.retire().unwrap();
    fixture.retired=true;
}

#[test]
#[ignore="explicit Windows OS batch: compile and set OMNIPLAY_WINDOWS_STORAGE_PROBE"]
fn actual_actor_cannot_write_its_default_storage_but_can_use_private_scratch(){
    run_probe("OMNIPLAY_WINDOWS_STORAGE_PROBE",true,"storage-probe-v2 1 1 1 1 1 1 1");
}

#[test]
#[ignore="explicit Windows OS batch: compile readonly probe and set OMNIPLAY_WINDOWS_READONLY_STORAGE_PROBE"]
fn actual_actor_without_scratch_grant_cannot_write_private_scratch(){
    run_probe("OMNIPLAY_WINDOWS_READONLY_STORAGE_PROBE",false,"storage-probe-readonly-v1 1 1 1 1 1 1 1");
}
