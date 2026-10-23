//! Linux userland socket tests; no cgroup mutation, service or desktop required.
use super::*;
use std::os::unix::fs::OpenOptionsExt;

fn pipe()->(File,File){
    let mut descriptors=[-1;2];
    assert_eq!(unsafe{libc::pipe2(descriptors.as_mut_ptr(),libc::O_CLOEXEC|libc::O_NONBLOCK)},0);
    unsafe{(File::from_raw_fd(descriptors[0]),File::from_raw_fd(descriptors[1]))}
}
fn packet(stream:&UnixStream,bytes:&[u8],descriptors:&[RawFd]){
    assert!(descriptors.len()<=4);
    let mut io=libc::iovec{iov_base:bytes.as_ptr().cast_mut().cast(),iov_len:bytes.len()};
    let mut control=[0usize;8];let mut message:libc::msghdr=unsafe{zeroed()};
    message.msg_iov=&mut io;message.msg_iovlen=1;
    if !descriptors.is_empty(){unsafe{
        message.msg_control=control.as_mut_ptr().cast();message.msg_controllen=libc::CMSG_SPACE(std::mem::size_of_val(descriptors)as u32)as usize;
        let header=libc::CMSG_FIRSTHDR(&message);assert!(!header.is_null());
        (*header).cmsg_level=libc::SOL_SOCKET;(*header).cmsg_type=libc::SCM_RIGHTS;(*header).cmsg_len=libc::CMSG_LEN(std::mem::size_of_val(descriptors)as u32)as usize;
        for(index,descriptor)in descriptors.iter().enumerate(){std::ptr::write_unaligned(libc::CMSG_DATA(header).cast::<RawFd>().add(index),*descriptor);}
    }}
    assert_eq!(unsafe{libc::sendmsg(stream.as_raw_fd(),&message,libc::MSG_NOSIGNAL)},bytes.len()as isize);
}
fn writer_gone(mut reader:File){
    let mut byte=[0];assert_eq!(reader.read(&mut byte).expect("all received write descriptors must close on rejection"),0);
}

#[test]
fn fragmented_response_preserves_one_cloexec_descriptor(){
    let(client,mut server)=UnixStream::pair().unwrap();let(reader,writer)=pipe();
    let thread=std::thread::spawn(move||{let raw=response(0);packet(&server,&raw[..4],&[writer.as_raw_fd()]);drop(writer);std::thread::sleep(Duration::from_millis(20));server.write_all(&raw[4..]).unwrap();});
    let received=receive_descriptor(&client).unwrap();
    assert_ne!(unsafe{libc::fcntl(received.as_raw_fd(),libc::F_GETFD)}&libc::FD_CLOEXEC,0);
    drop(received);thread.join().unwrap();writer_gone(reader);
}

#[test]
fn extra_descriptors_are_all_closed(){
    let(client,server)=UnixStream::pair().unwrap();let(reader,writer)=pipe();
    packet(&server,&response(0),&[writer.as_raw_fd();3]);drop(writer);
    assert!(receive_descriptor(&client).is_err());writer_gone(reader);
}

#[test]
fn refused_or_malformed_response_closes_transferred_descriptor(){
    for variant in 0..3{
        let(client,server)=UnixStream::pair().unwrap();let(reader,writer)=pipe();let mut raw=response(if variant==0{1}else{0});
        if variant==1{raw[0]=b'?';}if variant==2{raw[15]=1;}
        packet(&server,&raw,&[writer.as_raw_fd()]);drop(writer);
        assert!(receive_descriptor(&client).is_err());writer_gone(reader);
    }
}

#[test]
fn second_fragment_descriptor_is_rejected_and_both_close(){
    let(client,server)=UnixStream::pair().unwrap();let(reader,writer)=pipe();let raw=response(0);
    packet(&server,&raw[..4],&[writer.as_raw_fd()]);packet(&server,&raw[4..],&[writer.as_raw_fd()]);drop(writer);
    assert!(receive_descriptor(&client).is_err());writer_gone(reader);
}

#[test]
fn early_eof_closes_descriptor_already_received(){
    let(client,server)=UnixStream::pair().unwrap();let(reader,writer)=pipe();
    packet(&server,&response(0)[..4],&[writer.as_raw_fd()]);drop(writer);drop(server);
    assert!(receive_descriptor(&client).is_err());writer_gone(reader);
}

#[test]
fn ordinary_fd_does_not_gain_cgroup_authority(){
    let(client,server)=UnixStream::pair().unwrap();let(reader,writer)=pipe();
    send(&server,0,Some(&writer)).unwrap();drop(writer);
    assert!(receive(&client).is_err());writer_gone(reader);
}

#[test]
fn bad_client_does_not_end_listener_or_invoke_provisioning(){
    assert_ne!(unsafe{libc::geteuid()},0,"request peer tests require the supported non-root host identity");
    let name=format!("op-host-{}-{}.sock",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
    let path=std::env::temp_dir().join(name);let listener=UnixListener::bind(&path).unwrap();
    let task=std::thread::spawn(move||{
        serve_client(&listener,|_|panic!("malformed request reached provisioner")).unwrap();
        serve_client(&listener,|profile|{assert_eq!(profile,"11111111-2222-4333-8444-555555555555");OpenOptions::new().read(true).custom_flags(libc::O_CLOEXEC).open("/dev/null").map_err(|_|unavailable())}).unwrap();
    });
    let mut bad=UnixStream::connect(&path).unwrap();bad.write_all(&[0;REQUEST_BYTES]).unwrap();let mut refused=[0;RESPONSE_BYTES];bad.read_exact(&mut refused).unwrap();assert_eq!(&refused[..8],RESPONSE_MAGIC);assert_ne!(u32::from_le_bytes(refused[8..12].try_into().unwrap()),0);drop(bad);
    let mut good=UnixStream::connect(&path).unwrap();let mut raw=[0;REQUEST_BYTES];raw[..8].copy_from_slice(REQUEST_MAGIC);raw[8..44].copy_from_slice(b"11111111-2222-4333-8444-555555555555");good.write_all(&raw).unwrap();drop(receive_descriptor(&good).unwrap());
    task.join().unwrap();std::fs::remove_file(path).unwrap();
}
