//! Fixed local channel between the browser-spawned broker and the trusted user
//! service. The only transferred authority is one exact profile cgroup O_PATH fd.
use crate::linux_host::NativeHostRoot;
use omniplay_contracts::{ErrorCode,PlatformError,Result};
use std::{fs::{File,OpenOptions},io::{Read,Write},mem::{size_of,zeroed},os::{fd::{AsRawFd,FromRawFd,RawFd},unix::{fs::{FileTypeExt,MetadataExt,OpenOptionsExt},net::{UnixListener,UnixStream},ffi::OsStrExt}},path::Path,time::{Duration,Instant}};

const REQUEST_MAGIC:&[u8;8]=b"OPHREQ01";const RESPONSE_MAGIC:&[u8;8]=b"OPHRES01";const REQUEST_BYTES:usize=48;const RESPONSE_BYTES:usize=16;
fn unavailable()->PlatformError{PlatformError::new(ErrorCode::RuntimeUnavailable,"The trusted Linux native-host channel is unavailable")}
fn private_parent(path:&Path)->Result<()> {let parent=path.parent().ok_or_else(unavailable)?;let directory=OpenOptions::new().read(true).custom_flags(libc::O_PATH|libc::O_DIRECTORY|libc::O_NOFOLLOW|libc::O_CLOEXEC).open(parent).map_err(|_|unavailable())?;let value=directory.metadata().map_err(|_|unavailable())?;if unsafe{libc::geteuid()}==0||!value.is_dir()||value.uid()!=unsafe{libc::geteuid()}||value.mode()&0o7777!=0o700{return Err(unavailable());}Ok(())}
fn peer(descriptor:RawFd)->Result<libc::ucred>{let mut value:libc::ucred=unsafe{zeroed()};let mut bytes=size_of::<libc::ucred>()as libc::socklen_t;if unsafe{libc::getsockopt(descriptor,libc::SOL_SOCKET,libc::SO_PEERCRED,(&mut value as*mut libc::ucred).cast(),&mut bytes)}!=0||bytes as usize!=size_of::<libc::ucred>()||value.uid!=unsafe{libc::geteuid()}||value.uid==0||value.pid<=1{return Err(unavailable());}Ok(value)}
fn response(status:u32)->[u8;RESPONSE_BYTES]{let mut raw=[0u8;RESPONSE_BYTES];raw[..8].copy_from_slice(RESPONSE_MAGIC);raw[8..12].copy_from_slice(&status.to_le_bytes());raw}
fn send(stream:&UnixStream,status:u32,file:Option<&File>)->Result<()> {
    let raw=response(status);
    let mut io=libc::iovec{iov_base:raw.as_ptr().cast_mut().cast(),iov_len:raw.len()};
    // cmsghdr and its descriptor payload require native-word alignment.
    let mut control=[0usize;8];let mut message:libc::msghdr=unsafe{zeroed()};message.msg_iov=&mut io;message.msg_iovlen=1;
    if let Some(file)=file{message.msg_control=control.as_mut_ptr().cast();message.msg_controllen=unsafe{libc::CMSG_SPACE(size_of::<RawFd>()as u32)}as usize;let header=unsafe{libc::CMSG_FIRSTHDR(&message)};if header.is_null(){return Err(unavailable());}unsafe{(*header).cmsg_level=libc::SOL_SOCKET;(*header).cmsg_type=libc::SCM_RIGHTS;(*header).cmsg_len=libc::CMSG_LEN(size_of::<RawFd>()as u32)as usize;std::ptr::write_unaligned(libc::CMSG_DATA(header).cast::<RawFd>(),file.as_raw_fd());}}
    let count=loop{let count=unsafe{libc::sendmsg(stream.as_raw_fd(),&message,libc::MSG_NOSIGNAL)};if count<0&&std::io::Error::last_os_error().kind()==std::io::ErrorKind::Interrupted{continue;}break count;};
    if count<=0||count as usize>raw.len(){return Err(unavailable());}
    // SCM_RIGHTS accompanies the first bytes exactly once. A short stream write
    // must finish only the remaining ordinary payload, never resend the fd.
    let mut writer=stream;writer.write_all(&raw[count as usize..]).map_err(|_|unavailable())
}
fn receive_descriptor(stream:&UnixStream)->Result<File>{
    let deadline=Instant::now()+Duration::from_secs(5);
    let mut raw=[0u8;RESPONSE_BYTES];let mut used=0;let mut files=Vec::new();let mut valid=true;let mut headers=0;
    while used<raw.len(){
        let remaining=deadline.checked_duration_since(Instant::now()).filter(|left|!left.is_zero()).ok_or_else(unavailable)?;
        stream.set_read_timeout(Some(remaining)).map_err(|_|unavailable())?;
        let mut io=libc::iovec{iov_base:raw[used..].as_mut_ptr().cast(),iov_len:raw.len()-used};
        let mut control=[0usize;8];let mut message:libc::msghdr=unsafe{zeroed()};message.msg_iov=&mut io;message.msg_iovlen=1;message.msg_control=control.as_mut_ptr().cast();message.msg_controllen=size_of::<[usize;8]>();
        let count=unsafe{libc::recvmsg(stream.as_raw_fd(),&mut message,libc::MSG_CMSG_CLOEXEC)};
        if count<0&&std::io::Error::last_os_error().kind()==std::io::ErrorKind::Interrupted{continue;}
        if count<=0||count as usize>raw.len()-used{return Err(unavailable());}
        // Adopt EVERY received fd before rejecting a packet, including extra
        // descriptors in later fragments or a truncated control message. The
        // kernel closes descriptors not installed due to ancillary truncation.
        let mut header=unsafe{libc::CMSG_FIRSTHDR(&message)};
        while !header.is_null(){
            headers+=1;
            let size=unsafe{(*header).cmsg_len};let minimum=unsafe{libc::CMSG_LEN(0)}as usize;
            if size<minimum{return Err(unavailable());}
            if unsafe{(*header).cmsg_level}==libc::SOL_SOCKET&&unsafe{(*header).cmsg_type}==libc::SCM_RIGHTS{
                let bytes=size-minimum;valid&=bytes%size_of::<RawFd>()==0;
                for index in 0..bytes/size_of::<RawFd>(){
                    let descriptor=unsafe{std::ptr::read_unaligned(libc::CMSG_DATA(header).cast::<RawFd>().add(index))};
                    if descriptor<0{valid=false;}else{files.push(unsafe{File::from_raw_fd(descriptor)});}
                }
            }else{valid=false;}
            header=unsafe{libc::CMSG_NXTHDR(&message,header)};
        }
        valid&=message.msg_flags&(libc::MSG_TRUNC|libc::MSG_CTRUNC)==0;
        if !valid||headers>1||files.len()>1{return Err(unavailable());}
        used+=count as usize;
    }
    if !valid||headers!=1||files.len()!=1||&raw[..8]!=RESPONSE_MAGIC||u32::from_le_bytes(raw[8..12].try_into().map_err(|_|unavailable())?)!=0||raw[12..].iter().any(|byte|*byte!=0){return Err(unavailable());}
    files.pop().ok_or_else(unavailable)
}
fn receive(stream:&UnixStream)->Result<File>{
    let file=receive_descriptor(stream)?;let value=file.metadata().map_err(|_|unavailable())?;let mut shape=std::mem::MaybeUninit::<libc::statfs>::uninit();let flags=unsafe{libc::fcntl(file.as_raw_fd(),libc::F_GETFL)};
    if !value.is_dir()||value.uid()!=unsafe{libc::geteuid()}||value.mode()&0o7777!=0o700||flags<0||flags&libc::O_PATH!=libc::O_PATH||unsafe{libc::fstatfs(file.as_raw_fd(),shape.as_mut_ptr())}!=0||unsafe{shape.assume_init()}.f_type!=libc::CGROUP2_SUPER_MAGIC{return Err(unavailable());}Ok(file)
}
pub fn bind(path:&Path)->Result<UnixListener>{
    private_parent(path)?;if path.as_os_str().as_bytes().len()>100{return Err(unavailable());}let listener=UnixListener::bind(path).map_err(|_|unavailable())?;std::fs::set_permissions(path,std::os::unix::fs::PermissionsExt::from_mode(0o600)).map_err(|_|unavailable())?;let value=std::fs::symlink_metadata(path).map_err(|_|unavailable())?;if !value.file_type().is_socket()||value.uid()!=unsafe{libc::geteuid()}||value.mode()&0o7777!=0o600{return Err(unavailable());}Ok(listener)
}
pub fn systemd_listener(expected:&Path)->Result<UnixListener>{
    private_parent(expected)?;let pid=std::env::var("LISTEN_PID").ok().and_then(|value|value.parse::<u32>().ok());let fds=std::env::var("LISTEN_FDS").ok().and_then(|value|value.parse::<u32>().ok());if pid!=Some(std::process::id())||fds!=Some(1){return Err(unavailable());}
    let descriptor=3;if unsafe{libc::fcntl(descriptor,libc::F_SETFD,libc::FD_CLOEXEC)}!=0{return Err(unavailable());}let mut kind=0;let mut bytes=size_of::<libc::c_int>()as libc::socklen_t;if unsafe{libc::getsockopt(descriptor,libc::SOL_SOCKET,libc::SO_TYPE,(&mut kind as*mut libc::c_int).cast(),&mut bytes)}!=0||bytes as usize!=size_of::<libc::c_int>()||kind!=libc::SOCK_STREAM{return Err(unavailable());}let mut accepting=0;bytes=size_of::<libc::c_int>()as libc::socklen_t;if unsafe{libc::getsockopt(descriptor,libc::SOL_SOCKET,libc::SO_ACCEPTCONN,(&mut accepting as*mut libc::c_int).cast(),&mut bytes)}!=0||accepting!=1{return Err(unavailable());}
    let listener=unsafe{UnixListener::from_raw_fd(descriptor)};if listener.local_addr().ok().and_then(|value|value.as_pathname().map(Path::to_owned)).as_deref()!=Some(expected){return Err(unavailable());}let value=std::fs::symlink_metadata(expected).map_err(|_|unavailable())?;if !value.file_type().is_socket()||value.uid()!=unsafe{libc::geteuid()}||value.mode()&0o7777!=0o600{return Err(unavailable());}std::env::remove_var("LISTEN_PID");std::env::remove_var("LISTEN_FDS");Ok(listener)
}
pub fn serve_one(listener:&UnixListener,host:&NativeHostRoot)->Result<()> {
    serve_client(listener,|profile|host.provision_profile(profile))
}
fn serve_client(listener:&UnixListener,provision:impl FnOnce(&str)->Result<File>)->Result<()> {
    let(stream,_)=listener.accept().map_err(|_|unavailable())?;
    // Client errors end this connection, not the shared host and its active
    // delegated session tree. Listener/admission/quiescence failures stay fatal.
    let _=serve_request(stream,provision);Ok(())
}
fn serve_request(mut stream:UnixStream,provision:impl FnOnce(&str)->Result<File>)->Result<()>{
    peer(stream.as_raw_fd())?;stream.set_write_timeout(Some(Duration::from_secs(5))).map_err(|_|unavailable())?;
    let deadline=Instant::now()+Duration::from_secs(5);let mut raw=[0u8;REQUEST_BYTES];let mut used=0;
    while used<raw.len(){let remaining=deadline.checked_duration_since(Instant::now()).filter(|left|!left.is_zero()).ok_or_else(unavailable)?;stream.set_read_timeout(Some(remaining)).map_err(|_|unavailable())?;match stream.read(&mut raw[used..]){Ok(0)=>return Err(unavailable()),Ok(count)=>used+=count,Err(error)if error.kind()==std::io::ErrorKind::Interrupted=>continue,Err(_)=>return Err(unavailable())}}
    if &raw[..8]!=REQUEST_MAGIC||raw[44..].iter().any(|byte|*byte!=0){let _=send(&stream,1,None);return Err(unavailable());}
    let profile=std::str::from_utf8(&raw[8..44]).map_err(|_|unavailable())?;crate::linux_plan::session(profile)?;
    match provision(profile){Ok(file)=>send(&stream,0,Some(&file)),Err(error)=>{let _=send(&stream,1,None);Err(error)}}
}
pub fn wait(listener:&UnixListener,timeout:std::time::Duration)->Result<bool>{
    let deadline=Instant::now().checked_add(timeout).ok_or_else(unavailable)?;
    let mut descriptor=libc::pollfd{fd:listener.as_raw_fd(),events:libc::POLLIN,revents:0};
    loop{
        let milliseconds=deadline.saturating_duration_since(Instant::now()).as_millis().min(i32::MAX as u128)as i32;
        let value=unsafe{libc::poll(&mut descriptor,1,milliseconds)};
        if value>0{if descriptor.revents&libc::POLLIN!=0&&descriptor.revents&(libc::POLLERR|libc::POLLHUP|libc::POLLNVAL)==0{return Ok(true);}return Err(unavailable());}
        if value==0{return Ok(false);}
        if std::io::Error::last_os_error().kind()!=std::io::ErrorKind::Interrupted{return Err(unavailable());}
        if Instant::now()>=deadline{return Ok(false);}
    }
}
fn connect(path:&Path,timeout:Duration)->Result<UnixStream>{
    let name=path.as_os_str().as_bytes();let mut address:libc::sockaddr_un=unsafe{zeroed()};
    if !path.is_absolute()||name.is_empty()||name.len()>=address.sun_path.len()||name.contains(&0)||timeout.is_zero(){return Err(unavailable());}
    address.sun_family=libc::AF_UNIX as libc::sa_family_t;
    for(target,byte)in address.sun_path.iter_mut().zip(name){*target=*byte as libc::c_char;}
    let raw=unsafe{libc::socket(libc::AF_UNIX,libc::SOCK_STREAM|libc::SOCK_CLOEXEC|libc::SOCK_NONBLOCK,0)};
    if raw<0{return Err(unavailable());}let stream=unsafe{UnixStream::from_raw_fd(raw)};
    let length=(size_of::<libc::sa_family_t>()+name.len()+1)as libc::socklen_t;
    let result=unsafe{libc::connect(raw,(&address as*const libc::sockaddr_un).cast(),length)};
    if result<0{
        // AF_UNIX EAGAIN means a full listener backlog, not an in-flight TCP
        // connect. Refuse that busy host immediately instead of waiting forever
        // or treating a writable unconnected socket as successfully admitted.
        if std::io::Error::last_os_error().raw_os_error()!=Some(libc::EINPROGRESS){return Err(unavailable());}
        let deadline=Instant::now().checked_add(timeout).ok_or_else(unavailable)?;
        let mut event=libc::pollfd{fd:raw,events:libc::POLLOUT,revents:0};
        loop{
            let remaining=deadline.checked_duration_since(Instant::now()).filter(|left|!left.is_zero()).ok_or_else(unavailable)?;
            let milliseconds=remaining.as_millis().max(1).min(i32::MAX as u128)as i32;
            let count=unsafe{libc::poll(&mut event,1,milliseconds)};
            if count<0&&std::io::Error::last_os_error().kind()==std::io::ErrorKind::Interrupted{continue;}
            if count!=1||event.revents&libc::POLLOUT==0||event.revents&(libc::POLLERR|libc::POLLHUP|libc::POLLNVAL)!=0{return Err(unavailable());}
            let mut error=0;let mut size=size_of::<libc::c_int>()as libc::socklen_t;
            if unsafe{libc::getsockopt(raw,libc::SOL_SOCKET,libc::SO_ERROR,(&mut error as*mut libc::c_int).cast(),&mut size)}!=0||size as usize!=size_of::<libc::c_int>()||error!=0{return Err(unavailable());}
            break;
        }
    }
    stream.set_nonblocking(false).map_err(|_|unavailable())?;Ok(stream)
}
pub fn request(path:&Path,profile:&str)->Result<File>{
    crate::linux_plan::session(profile)?;private_parent(path)?;let value=std::fs::symlink_metadata(path).map_err(|_|unavailable())?;if !value.file_type().is_socket()||value.uid()!=unsafe{libc::geteuid()}||value.mode()&0o7777!=0o600{return Err(unavailable());}
    let mut stream=connect(path,Duration::from_secs(5))?;peer(stream.as_raw_fd())?;stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).map_err(|_|unavailable())?;stream.set_write_timeout(Some(std::time::Duration::from_secs(5))).map_err(|_|unavailable())?;let mut raw=[0u8;REQUEST_BYTES];raw[..8].copy_from_slice(REQUEST_MAGIC);raw[8..44].copy_from_slice(profile.as_bytes());stream.write_all(&raw).map_err(|_|unavailable())?;receive(&stream)
}

#[cfg(test)]
#[path="linux_host_channel_tests.rs"]
mod tests;
