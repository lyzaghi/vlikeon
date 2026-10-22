//! Actual actor/helper/core/pipes under production AppContainer and job policy.
//! Explicit ignored batch only; a test peer observes media, never physical devices.
use super::*;
use serde_json::{Value,json};
use std::{fs,io::{Read,Write},path::PathBuf,sync::mpsc,time::{Duration,Instant,SystemTime,UNIX_EPOCH}};

struct Fixture{root:PathBuf,retired:bool}
impl Drop for Fixture{fn drop(&mut self){if self.retired{let _=fs::remove_dir_all(&self.root);}else{eprintln!("unconfirmed device-free fixture retained: {}",self.root.display());}}}
fn explicit(variable:&str)->PathBuf{
    use std::os::windows::fs::MetadataExt;
    let path=PathBuf::from(std::env::var_os(variable).expect("exact assigned qualification artifact required"));
    let metadata=fs::symlink_metadata(&path).unwrap();
    assert!(path.is_absolute()&&metadata.is_file()&&metadata.len()>0&&metadata.len()<=16*1024*1024&&metadata.file_attributes()&0x400==0);path
}
fn wait_ended(check:impl Fn()->bool){let deadline=Instant::now()+Duration::from_secs(5);while !check()&&Instant::now()<deadline{std::thread::sleep(Duration::from_millis(10));}assert!(check(),"owned qualification process did not exit");}
fn exit_zero(process:&OwnedHandle){let mut code=0;assert_ne!(unsafe{GetExitCodeProcess(process.as_raw_handle(),&mut code)},0);assert_eq!(code,0,"actual process exit; 87=input/media mismatch, 88=missing Win32k policy, 86=lifetime witness");}

#[test]
#[ignore="explicit Windows batch: exact device-free worker/core and private peer required"]
fn actual_device_free_actor_exchanges_media_under_win32k_lockdown(){
    let worker=explicit("OMNIPLAY_WINDOWS_DEVICE_FREE_WORKER");
    let core=explicit("OMNIPLAY_WINDOWS_DEVICE_FREE_CORE");
    let peer=explicit("OMNIPLAY_WINDOWS_PRESENTATION_FIXTURE");
    let nonce=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()^((std::process::id()as u128)<<64);
    let moniker=format!("OmniPlay.Session.{nonce:032x}");let session=format!("{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",(nonce>>96)as u32,((nonce>>80)&65535)as u16,((nonce>>64)&65535)as u16,((nonce>>48)&65535)as u16,nonce&0xffffffffffff);
    let root=std::env::temp_dir().join(format!("omniplay-device-free-{nonce:032x}"));fs::create_dir(&root).unwrap();
    let mut fixture=Fixture{root:root.canonicalize().unwrap(),retired:false};
    let runtime=fixture.root.join("runtime");let work=fixture.root.join("work");let host_work=fixture.root.join("host-work");let host_temp=fixture.root.join("host-temp");
    for path in [&runtime,&work,&host_work,&host_temp]{fs::create_dir(path).unwrap();}fs::create_dir(work.join("scratch")).unwrap();
    let executable=runtime.join("omniplay-native-worker.exe");let helper=runtime.join("omniplay-presentation-fixture.exe");
    fs::copy(worker,&executable).unwrap();fs::copy(core,runtime.join("omniplay-lifecycle-fixture.dll")).unwrap();fs::copy(peer,&helper).unwrap();
    let mut game=b"LIFECYCLE".to_vec();game.resize(64,0);fs::write(work.join("game.bin"),game).unwrap();
    let identity=ContainerIdentity::create_for_session(&moniker).unwrap();identity.grant_game_read(&work).unwrap();grant_public_runtime_read(&runtime).unwrap();
    let spec=PresentationLaunch{executable:&helper,working_directory:&host_work,temporary_directory:&host_temp,session:&session,maximum_width:64,maximum_height:64,volume_percent:50,haptics:false,memory_bytes:256*1024*1024};
    let pair=identity.launch_presented(&executable,&work,&Limits{memory_bytes:256*1024*1024},&spec,|id|{assert!(id.process_id>0&&id.creation_time>0);Ok(())},|id|{assert!(id.process_id>0&&id.creation_time>0);Ok(())}).unwrap();
    let PresentedLaunched{actor:Launched{process,mut input,mut output},presentation}=pair;
    let(sender,receiver)=mpsc::sync_channel(2);
    let reader=std::thread::spawn(move||{
        let result=(||->std::io::Result<()>{for _ in 0..32{
            let mut prefix=[0;4];let first=output.read(&mut prefix[..1])?;if first==0{return Ok(());}output.read_exact(&mut prefix[1..])?;
            let size=u32::from_le_bytes(prefix)as usize;if size>65536{return Err(std::io::ErrorKind::InvalidData.into());}
            let mut header=vec![0;size];output.read_exact(&mut header)?;
            output.read_exact(&mut prefix)?;let size=u32::from_le_bytes(prefix)as usize;if size>4096{return Err(std::io::ErrorKind::InvalidData.into());}
            let mut payload=vec![0;size];output.read_exact(&mut payload)?;
            if sender.send(Ok((header,payload))).is_err(){return Ok(());}
        }Err(std::io::ErrorKind::InvalidData.into())})();
        if let Err(error)=result{let _=sender.send(Err(error.kind()));}
    });
    let mut sequence=0u64;
    let mut request=|method:&str,params:Value|{
        sequence+=1;let header=serde_json::to_vec(&json!({"protocol":1,"id":sequence,"method":method,"params":params})).unwrap();assert!(header.len()<=65536);
        input.write_all(&(header.len()as u32).to_le_bytes()).unwrap();input.write_all(&header).unwrap();input.write_all(&0u32.to_le_bytes()).unwrap();input.flush().unwrap();
        let(header,payload)=receiver.recv_timeout(Duration::from_secs(5)).expect("actual worker reply deadline").expect("actual worker reply framing");let value:Value=serde_json::from_slice(&header).unwrap();
        assert_eq!(value["protocol"],1);assert_eq!(value["id"],sequence);assert_eq!(value["status"],"ok","actual worker refusal: {value}");(value,payload)
    };
    let(hello,_)=request("hello",json!({}));assert_eq!(hello["result"]["presentation_mode"],"brokered_rgba");assert_eq!(hello["result"]["library_name"],"OmniPlay lifecycle qualification");
    let mut keyboard=vec![Value::Null;16];keyboard[0]=json!("KeyZ");
    request("prepare",json!({"filename":"game.bin","title":"Device-free qualification","bytes":64,"core_options":{"fixture_scenario":"brokered-media"},"config":{"audio_volume":0.5,"latency_frames":2,"preset":"accurate"},"input_profile":{"schema_version":1,"keyboard":keyboard,"analog_keyboard":[null,null,null,null],"button_sources":[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15]},"pause_barrier":true,"pause_barrier_protocol":1}));
    request("start",json!({}));std::thread::sleep(Duration::from_millis(250));
    let(status,_)=request("status",json!({}));assert!(status["result"]["active_playtime_ms"].as_u64().unwrap()>0);
    request("pause",json!({}));let(_,memory)=request("save",json!({"kind":"in_game"}));assert_eq!(memory.len(),32);assert_eq!(&memory[..8],b"OPRM0001");
    request("stop",json!({}));drop(request);drop(input);
    wait_ended(||process.has_exited());exit_zero(&process.process);drop(process);
    wait_ended(||presentation.has_exited());assert_eq!(presentation.observed_exit_code(),Some(0));drop(presentation);
    wait_ended(||crate::owned_thread_ended(&reader));reader.join().unwrap();
    let receipt=host_work.join("presentation-receipt.json");assert!(fs::metadata(&receipt).unwrap().len()<=1024);
    let receipt:Value=serde_json::from_slice(&fs::read(receipt).unwrap()).unwrap();assert_eq!(receipt["valid"],true);assert!(receipt["video"].as_u64().unwrap()>0&&receipt["audio"].as_u64().unwrap()>0);
    identity.retire().unwrap();fixture.retired=true;
}
