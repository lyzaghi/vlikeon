#include "namespaces.h"
#include "policy.h"
#include "cgroup.h"
#if !defined(__linux__) || !defined(__x86_64__) || defined(__ILP32__)
#error This supervisor supports only native Linux x86-64
#endif
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <linux/capability.h>
#include <linux/magic.h>
#include <linux/securebits.h>
#include <poll.h>
#include <sched.h>
#include <signal.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/mount.h>
#include <sys/prctl.h>
#include <sys/stat.h>
#include <sys/statfs.h>
#include <sys/statvfs.h>
#include <sys/socket.h>
#include <sys/syscall.h>
#include <sys/sysmacros.h>
#include <sys/types.h>
#include <unistd.h>
#include <time.h>

static int control(int operation,unsigned long a,unsigned long b,unsigned long c,unsigned long d){return prctl(operation,a,b,c,d);}
#define prctl(operation,a,b,c,d) control(operation,a,b,c,d)
static _Noreturn void refused(void){_exit(125);}
static void require(bool condition){if(!condition)refused();}
static bool session(const char value[37]){
  if(value[36]!=0)return false;bool nonzero=false;
  for(size_t i=0;i<36;i++){const unsigned char byte=(unsigned char)value[i];if(i==8||i==13||i==18||i==23){if(byte!='-')return false;}else{if(!((byte>='0'&&byte<='9')||(byte>='a'&&byte<='f')))return false;if(byte!='0')nonzero=true;}}
  return nonzero;
}
static void one_thread(void){
  DIR* directory=opendir("/proc/self/task");require(directory!=NULL);size_t count=0;struct dirent* entry;errno=0;
  while((entry=readdir(directory))!=NULL){if(entry->d_name[0]=='.')continue;for(const char* at=entry->d_name;*at;at++)require(*at>='0'&&*at<='9');require(++count==1);}
  const int error=errno;require(closedir(directory)==0&&error==0&&count==1);
}
static void empty_starting_capabilities(void){
  struct __user_cap_header_struct header={.version=_LINUX_CAPABILITY_VERSION_3,.pid=0};struct __user_cap_data_struct values[2];memset(values,0,sizeof(values));require(syscall(SYS_capget,&header,values)==0);
  for(size_t i=0;i<2;i++)require(values[i].effective==0&&values[i].permitted==0&&values[i].inheritable==0);
}
static void lock_capabilities(void){
  const int bits=SECBIT_NOROOT|SECBIT_NOROOT_LOCKED|SECBIT_NO_SETUID_FIXUP|SECBIT_NO_SETUID_FIXUP_LOCKED|SECBIT_NO_CAP_AMBIENT_RAISE|SECBIT_NO_CAP_AMBIENT_RAISE_LOCKED;
  require(prctl(PR_SET_SECUREBITS,bits,0,0,0)==0&&prctl(PR_CAP_AMBIENT,PR_CAP_AMBIENT_CLEAR_ALL,0,0,0)==0);
  bool bounded=false;
  for(int capability=0;capability<64;capability++){
    errno=0;const int present=prctl(PR_CAPBSET_READ,capability,0,0,0);
    if(present<0){require(errno==EINVAL&&capability>0);bounded=true;break;}
    require((present==0||present==1)&&prctl(PR_CAPBSET_DROP,capability,0,0,0)==0);
  }
  require(bounded);struct __user_cap_header_struct header={.version=_LINUX_CAPABILITY_VERSION_3,.pid=0};struct __user_cap_data_struct values[2];memset(values,0,sizeof(values));require(syscall(SYS_capset,&header,values)==0);empty_starting_capabilities();
  require(prctl(PR_GET_SECUREBITS,0,0,0,0)==bits&&prctl(PR_SET_DUMPABLE,0,0,0,0)==0);
}
static int pidfd(pid_t process){const long fd=syscall(SYS_pidfd_open,process,0);require(fd>=0&&fd<=INT_MAX);return(int)fd;}
static void alive(int fd){struct pollfd status={.fd=fd,.events=POLLIN};require(poll(&status,1,0)==0);}
static void parent_death(int parent){
  alive(parent);require(prctl(PR_SET_PDEATHSIG,SIGKILL,0,0,0)==0);alive(parent);
}
static int64_t monotonic_ms(void){struct timespec value;require(clock_gettime(CLOCK_MONOTONIC,&value)==0&&value.tv_sec>=0&&value.tv_sec<INT64_MAX/1000);return(int64_t)value.tv_sec*1000+value.tv_nsec/1000000;}
static void start_gate(pid_t parent){
  int kind=0,domain=0;socklen_t size=sizeof(kind);require(getsockopt(6,SOL_SOCKET,SO_TYPE,&kind,&size)==0&&size==sizeof(kind)&&kind==SOCK_SEQPACKET);size=sizeof(domain);require(getsockopt(6,SOL_SOCKET,SO_DOMAIN,&domain,&size)==0&&size==sizeof(domain)&&domain==AF_UNIX);
  struct ucred peer;size=sizeof(peer);require(getsockopt(6,SOL_SOCKET,SO_PEERCRED,&peer,&size)==0&&size==sizeof(peer)&&peer.pid==parent&&peer.uid==getuid()&&fcntl(6,F_SETFD,FD_CLOEXEC)==0);
  const int64_t deadline=monotonic_ms()+10000;bool authorized=false;
  for(;;){const int64_t remaining=deadline-monotonic_ms();require(remaining>0&&remaining<=10000);struct pollfd event={.fd=6,.events=POLLIN};const int ready=poll(&event,1,(int)remaining);if(ready<0&&errno==EINTR)continue;require(ready==1&&(event.revents&(POLLERR|POLLNVAL))==0&&(event.revents&(POLLIN|POLLHUP))!=0);
    unsigned char value[2];const ssize_t received=recv(6,value,sizeof(value),MSG_DONTWAIT);if(received<0&&(errno==EINTR||errno==EAGAIN))continue;
    if(!authorized){require(received==1&&value[0]==1);authorized=true;}else{require(received==0&&(event.revents&POLLHUP)!=0);break;}
  }
  require(close(6)==0);
}
static void close_except(int keep){
  require(keep>=0);if(keep>0)require(syscall(SYS_close_range,0u,(unsigned)keep-1,0)==0);
  require(syscall(SYS_close_range,(unsigned)keep+1,UINT_MAX,0)==0);
}
static int reap(int process){
  siginfo_t status;memset(&status,0,sizeof(status));long result;
  do{result=syscall(SYS_waitid,3,process,&status,WEXITED,NULL);}while(result<0&&errno==EINTR);
  require(result==0&&status.si_pid>0&&status.si_signo==SIGCHLD);
  if(status.si_code==CLD_EXITED){require(status.si_status>=0&&status.si_status<=255);return status.si_status;}
  require((status.si_code==CLD_KILLED||status.si_code==CLD_DUMPED)&&status.si_status>0&&status.si_status<128);return 128+status.si_status;
}
static void map_file(const char* path,const char* bytes,size_t count){
  const int fd=open(path,O_WRONLY|O_CLOEXEC|O_NOFOLLOW);require(fd>=0);size_t at=0;
  while(at<count){const ssize_t written=write(fd,bytes+at,count-at);if(written<0&&errno==EINTR)continue;require(written>0);at+=(size_t)written;}require(close(fd)==0);
}
static void identity_map(uid_t user,gid_t group){
  require(unshare(CLONE_NEWUSER)==0);char value[64];int count=snprintf(value,sizeof(value),"0 %lu 1\n",(unsigned long)user);require(count>0&&(size_t)count<sizeof(value));map_file("/proc/self/uid_map",value,(size_t)count);
  map_file("/proc/self/setgroups","deny\n",5);count=snprintf(value,sizeof(value),"0 %lu 1\n",(unsigned long)group);require(count>0&&(size_t)count<sizeof(value));map_file("/proc/self/gid_map",value,(size_t)count);
  require(getuid()==0&&geteuid()==0&&getgid()==0&&getegid()==0);
}
static void pipe_end(int fd,int mode){struct stat value;const int flags=fcntl(fd,F_GETFL);require(fstat(fd,&value)==0&&S_ISFIFO(value.st_mode)&&flags>=0&&(flags&O_ACCMODE)==mode&&(flags&O_NONBLOCK)==0);}
static void endpoints(void){
  pipe_end(0,O_RDONLY);pipe_end(1,O_WRONLY);pipe_end(3,O_RDONLY);pipe_end(4,O_WRONLY);
  const int pipes[]={0,1,3,4};struct stat identities[4];
  for(size_t i=0;i<4;i++){require(fstat(pipes[i],&identities[i])==0);for(size_t before=0;before<i;before++)require(identities[i].st_dev!=identities[before].st_dev||identities[i].st_ino!=identities[before].st_ino);}
  struct stat value;const int flags=fcntl(2,F_GETFL);require(fstat(2,&value)==0&&S_ISCHR(value.st_mode)&&major(value.st_rdev)==1&&minor(value.st_rdev)==3&&flags>=0&&(flags&O_ACCMODE)==O_WRONLY);
}
static void filesystem(const char* path,long type,unsigned long required,unsigned long forbidden,uint64_t maximum){
  struct statfs shape;struct statvfs capacity;require(statfs(path,&shape)==0&&shape.f_type==type&&statvfs(path,&capacity)==0&&(capacity.f_flag&required)==required&&(capacity.f_flag&forbidden)==0);
  if(maximum){require(capacity.f_frsize>0&&capacity.f_blocks<=maximum/capacity.f_frsize);}
}
static void root_admission(const struct omni_linux_namespace_plan* plan,const struct stat* before){
  struct stat root;require(stat("/",&root)==0&&(root.st_dev!=before->st_dev||root.st_ino!=before->st_ino));
  const unsigned long immutable=ST_RDONLY|ST_NOSUID|ST_NODEV,private_write=ST_NOSUID|ST_NODEV|ST_NOEXEC;
  filesystem("/",TMPFS_MAGIC,immutable,0,plan->root_bytes);filesystem("/runtime",TMPFS_MAGIC,immutable,0,plan->root_bytes);
  filesystem("/work",TMPFS_MAGIC,immutable|ST_NOEXEC,0,plan->root_bytes);
  filesystem("/work/scratch",TMPFS_MAGIC,private_write,ST_RDONLY,plan->scratch_bytes);filesystem("/work/save",TMPFS_MAGIC,private_write,ST_RDONLY,plan->save_bytes);
  filesystem("/proc",PROC_SUPER_MAGIC,immutable|ST_NOEXEC,0,0);
  char self[32];const ssize_t length=readlink("/proc/self",self,sizeof(self));require(length==1&&self[0]=='1');
  struct stat device;require(lstat("/dev/null",&device)==0&&S_ISCHR(device.st_mode)&&major(device.st_rdev)==1&&minor(device.st_rdev)==3);
  errno=0;require(lstat("/.old-root",&device)<0&&errno==ENOENT);
}
static void path(struct omni_linux_policy* policy,const char* name,enum omni_linux_access access){
  require(policy->path_count<OMNI_LINUX_PATHS);const int fd=open(name,O_PATH|O_CLOEXEC|(access==OMNI_LINUX_READ_DIRECTORY||access==OMNI_LINUX_WRITE_DIRECTORY?O_DIRECTORY:0));require(fd>=5&&fd<128);
  policy->paths[policy->path_count++]=(struct omni_linux_path){.fd=fd,.access=access};if(strcmp(name,"/runtime/omniplay-native-worker")==0)policy->executable_fd=fd;
}
static _Noreturn void actor(const struct omni_linux_namespace_plan* plan,int parent){
  parent_death(parent);require(syscall(SYS_close_range,5u,UINT_MAX,0)==0&&chdir("/work")==0);endpoints();
  for(int fd=0;fd<5;fd++)require(fcntl(fd,F_SETFD,0)==0);
  struct omni_linux_policy policy;memset(&policy,0,sizeof(policy));policy.memory_bytes=plan->memory_bytes;policy.maximum_file_bytes=plan->maximum_file_bytes;policy.executable_fd=-1;
  path(&policy,"/runtime/omniplay-native-worker",OMNI_LINUX_EXECUTABLE);path(&policy,"/lib64/ld-linux-x86-64.so.2",OMNI_LINUX_EXECUTABLE);
  path(&policy,"/runtime",OMNI_LINUX_READ_DIRECTORY);path(&policy,"/work",OMNI_LINUX_READ_DIRECTORY);path(&policy,"/work/scratch",OMNI_LINUX_WRITE_DIRECTORY);path(&policy,"/work/save",OMNI_LINUX_WRITE_DIRECTORY);
  path(&policy,"/proc/self",OMNI_LINUX_READ_DIRECTORY);path(&policy,"/proc/cpuinfo",OMNI_LINUX_READ_FILE);path(&policy,"/proc/meminfo",OMNI_LINUX_READ_FILE);path(&policy,"/dev/null",OMNI_LINUX_NULL_DEVICE);
  char*const arguments[]={"/runtime/omniplay-native-worker","--stdio-runtime-v1","--presentation-client-v2",(char*)plan->session,"3","4",NULL};
  char*const environment[]={"PATH=/runtime","LD_LIBRARY_PATH=/runtime","HOME=/work/scratch","TMPDIR=/work/scratch","TEMP=/work/scratch","TMP=/work/scratch","LANG=C.UTF-8","TZ=UTC",NULL};
  omni_linux_policy_exec(&policy,arguments,environment);
}
static _Noreturn void init(const struct omni_linux_namespace_plan* plan,int parent){
  require(getpid()==1);parent_death(parent);struct stat old_root;require(stat("/",&old_root)==0);
  omni_linux_construct_root(plan->root_plan,plan->root_bytes,plan->scratch_bytes,plan->save_bytes);root_admission(plan,&old_root);lock_capabilities();parent_death(parent);
  const int own=pidfd(getpid());const pid_t child=fork();require(child>=0);if(child==0)actor(plan,own);
  const int process=pidfd(child);close_except(process);const int status=reap(process);require(close(process)==0);_exit(status);
}
_Noreturn void omni_linux_namespace_supervise(const struct omni_linux_namespace_plan* plan){
  require(plan!=NULL&&plan->expected_parent>1&&plan->root_plan!=NULL&&plan->cgroup_fd>=6&&plan->cgroup_fd<1024&&plan->cpu_cores>=1&&plan->cpu_cores<=8&&session(plan->session));
  require(plan->memory_bytes>=256ull*1024*1024&&plan->memory_bytes<=8ull*1024*1024*1024&&plan->maximum_file_bytes>0&&plan->maximum_file_bytes<=256ull*1024*1024);
  require(plan->root_bytes>=16ull*1024*1024&&plan->root_bytes<=1024ull*1024*1024&&plan->scratch_bytes>=4ull*1024*1024&&plan->scratch_bytes<=512ull*1024*1024&&plan->save_bytes>=4ull*1024*1024&&plan->save_bytes<=512ull*1024*1024&&(plan->root_bytes|plan->scratch_bytes|plan->save_bytes)%4096==0);
  uid_t real,effective,saved;gid_t group_real,group_effective,group_saved;require(getresuid(&real,&effective,&saved)==0&&getresgid(&group_real,&group_effective,&group_saved)==0&&real!=0&&real==effective&&real==saved&&group_real==group_effective&&group_real==group_saved);
  one_thread();endpoints();empty_starting_capabilities();require(getppid()==plan->expected_parent);const int broker=pidfd(plan->expected_parent);parent_death(broker);require(getppid()==plan->expected_parent&&setpgid(0,0)==0&&getpgrp()==getpid());
  start_gate(plan->expected_parent);alive(broker);
  omni_linux_cgroup_enter(plan->cgroup_fd,plan->session,plan->memory_bytes,plan->root_bytes,plan->scratch_bytes,plan->save_bytes,plan->cpu_cores);
  sigset_t mask;require(sigemptyset(&mask)==0&&sigprocmask(SIG_SETMASK,&mask,NULL)==0);struct sigaction action;memset(&action,0,sizeof(action));action.sa_handler=SIG_DFL;require(sigemptyset(&action.sa_mask)==0&&sigaction(SIGCHLD,&action,NULL)==0);
  require(prctl(PR_SET_NO_NEW_PRIVS,1,0,0,0)==0&&prctl(PR_SET_DUMPABLE,0,0,0,0)==0);identity_map(real,group_real);
  require(unshare(CLONE_NEWNS|CLONE_NEWPID|CLONE_NEWNET|CLONE_NEWIPC|CLONE_NEWUTS)==0&&mount(NULL,"/",NULL,MS_REC|MS_PRIVATE,NULL)==0&&sethostname("omniplay",8)==0&&setdomainname("",0)==0);
  parent_death(broker);const int own=pidfd(getpid());const pid_t child=fork();require(child>=0);if(child==0)init(plan,own);
  const int process=pidfd(child);lock_capabilities();parent_death(broker);close_except(process);const int status=reap(process);require(close(process)==0);_exit(status);
}
