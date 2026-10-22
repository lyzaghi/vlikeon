#include "owner.hpp"
#include <algorithm>
#include <atomic>
#include <chrono>
#include <thread>
#if defined(__linux__) && !defined(__ANDROID__)
#include <cerrno>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <sys/prctl.h>
#include <sys/resource.h>
#include <sys/socket.h>
#include <sys/syscall.h>
#include <unistd.h>
#endif

namespace omni::presentation {
struct Owner::State{
  std::atomic_bool stop=false,focus=false;
#if defined(__linux__) && !defined(__ANDROID__)
  int descriptor=-1;Session session;std::uint64_t sequence=0;
  explicit State(Session value):session(value){}
  ~State(){if(descriptor>=0)::close(descriptor);}
  void receive(){try{for(;;){std::array<std::uint8_t,40> raw{};const auto size=recv(descriptor,raw.data(),raw.size(),MSG_TRUNC);if(size<0&&errno==EINTR)continue;check(size==static_cast<ssize_t>(raw.size()));const std::array<std::uint8_t,8> magic{'O','P','O','W','N','E','R','1'};check(std::equal(magic.begin(),magic.end(),raw.begin())&&std::equal(session.begin(),session.end(),raw.begin()+8));const auto next=u64(raw,24);const auto action=u32(raw,32);check(next>sequence&&next<=kMaxSequence&&(action==1||action==2)&&u32(raw,36)==0);sequence=next;if(action==2){stop.store(true);return;}focus.store(true);}}catch(...){}stop.store(true);}
#endif
};
#if defined(__linux__) && !defined(__ANDROID__)
static void alive(int fd){pollfd value{fd,POLLIN,0};check(poll(&value,1,0)==0);}
static void peer(int descriptor,pid_t parent){int value=0;socklen_t size=sizeof(value);check(getsockopt(descriptor,SOL_SOCKET,SO_TYPE,&value,&size)==0&&size==sizeof(value)&&value==SOCK_SEQPACKET);size=sizeof(value);check(getsockopt(descriptor,SOL_SOCKET,SO_DOMAIN,&value,&size)==0&&size==sizeof(value)&&value==AF_UNIX);ucred credentials{};size=sizeof(credentials);check(getsockopt(descriptor,SOL_SOCKET,SO_PEERCRED,&credentials,&size)==0&&size==sizeof(credentials)&&credentials.pid==parent&&credentials.uid==getuid());}
static void authorize(){
  const auto until=std::chrono::steady_clock::now()+std::chrono::seconds(10);bool authorized=false;
  for(;;){const auto left=std::chrono::duration_cast<std::chrono::milliseconds>(until-std::chrono::steady_clock::now()).count();check(left>0&&left<=10000);pollfd value{3,POLLIN,0};const auto result=poll(&value,1,static_cast<int>(left));if(result<0&&errno==EINTR)continue;check(result==1&&(value.revents&(POLLERR|POLLNVAL))==0&&(value.revents&(POLLIN|POLLHUP))!=0);std::array<std::uint8_t,2> raw{};const auto received=recv(3,raw.data(),raw.size(),MSG_DONTWAIT|MSG_TRUNC);if(received<0&&(errno==EINTR||errno==EAGAIN))continue;if(!authorized){check(received==1&&raw[0]==1);authorized=true;}else{check(received==0&&(value.revents&POLLHUP)!=0);return;}}
}
#endif
Owner Owner::gated(Session session,std::uint32_t expected_parent){
#if defined(__linux__) && !defined(__ANDROID__)
  check(expected_parent>1&&expected_parent<=0x7fffffffu&&getppid()==static_cast<pid_t>(expected_parent)&&prctl(PR_GET_NO_NEW_PRIVS,0UL,0UL,0UL,0UL)==1);uid_t real,effective,saved;gid_t group_real,group_effective,group_saved;check(getresuid(&real,&effective,&saved)==0&&getresgid(&group_real,&group_effective,&group_saved)==0&&real!=0&&real==effective&&real==saved&&group_real==group_effective&&group_real==group_saved);
  peer(3,static_cast<pid_t>(expected_parent));peer(5,static_cast<pid_t>(expected_parent));const int parent=static_cast<int>(syscall(SYS_pidfd_open,expected_parent,0));check(parent>=0);alive(parent);check(prctl(PR_SET_PDEATHSIG,static_cast<unsigned long>(SIGKILL),0UL,0UL,0UL)==0&&getppid()==static_cast<pid_t>(expected_parent));alive(parent);check(::close(4)==0);authorize();alive(parent);check(::close(3)==0&&::close(parent)==0);
  const int adjustment=open("/proc/self/oom_score_adj",O_WRONLY|O_CLOEXEC|O_NOFOLLOW);check(adjustment>=0);check(write(adjustment,"0",1)==1&&::close(adjustment)==0);const rlimit zero{0,0};check(setrlimit(RLIMIT_CORE,&zero)==0&&setrlimit(RLIMIT_MEMLOCK,&zero)==0&&setrlimit(RLIMIT_RTPRIO,&zero)==0);
  Owner result;result.state_=std::make_shared<State>(session);result.state_->descriptor=fcntl(5,F_DUPFD_CLOEXEC,6);check(result.state_->descriptor>=6&&::close(5)==0);const auto state=result.state_;std::thread([state]{state->receive();}).detach();return result;
#else
  (void)session;(void)expected_parent;throw Failure();
#endif
}
bool Owner::stop_requested()const{return state_&&state_->stop.load();}
bool Owner::take_focus(){return state_&&state_->focus.exchange(false);}
}
