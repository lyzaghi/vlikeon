#pragma once
#ifdef __APPLE__
#include <cerrno>
#include <charconv>
#include <cstdlib>
#include <cstring>
#include <fcntl.h>
#include <poll.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <thread>
#include <unistd.h>

namespace omni::macos {
// Descriptor 6 is a private read-only lifetime pipe. Only the signed XPC owner
// retains its write end; no PID lookup or parent-PID reuse can keep it alive.
// Run before loading a core or starting SDL/device/control worker threads.
inline bool establish_child(){
  if(getuid()==0||getuid()!=geteuid()||getgid()!=getegid())return false;
  struct stat shape{};const int flags=fcntl(6,F_GETFL);
  if(flags<0||(flags&O_ACCMODE)!=O_RDONLY||fstat(6,&shape)!=0||!S_ISFIFO(shape.st_mode)||fcntl(6,F_SETFD,FD_CLOEXEC)!=0)return false;
  pollfd lifetime{6,POLLIN,0};if(poll(&lifetime,1,0)!=0)return false;
  const char* raw=std::getenv("OMNIPLAY_CHILD_MEMORY_MB");
  if(!raw||!*raw||std::strlen(raw)>4)return false;
  unsigned memory_mb=0;const auto parsed=std::from_chars(raw,raw+std::strlen(raw),memory_mb);
  if(parsed.ec!=std::errc{}||parsed.ptr!=raw+std::strlen(raw)||memory_mb<256||memory_mb>8192)return false;
  const rlimit zero{0,0},files{64,64},memory{static_cast<rlim_t>(memory_mb)*1024*1024,static_cast<rlim_t>(memory_mb)*1024*1024};
  // Hard NPROC=0 on a non-root child prevents both fork and posix_spawn. Threads
  // remain usable. A core cannot raise the hard limit without elevated rights.
  // FSIZE is an individual-file limit, NOT an aggregate storage quota.
  const rlimit file_bytes{256ULL*1024*1024,256ULL*1024*1024};
  if(setrlimit(RLIMIT_CORE,&zero)!=0||setrlimit(RLIMIT_NPROC,&zero)!=0||setrlimit(RLIMIT_NOFILE,&files)!=0||setrlimit(RLIMIT_AS,&memory)!=0||setrlimit(RLIMIT_FSIZE,&file_bytes)!=0)return false;
  try{std::thread([]{unsigned char byte=0;for(;;){const auto count=read(6,&byte,1);if(count<0&&errno==EINTR)continue;std::_Exit(125);}}).detach();}catch(...){return false;}
  return true;
}
}
#endif
