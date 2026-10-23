#include "policy.h"
#if !defined(__linux__) || !defined(__x86_64__) || defined(__ILP32__)
#error This policy supports only the native Linux x86-64 syscall ABI
#endif
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <linux/audit.h>
#include <linux/capability.h>
#include <linux/filter.h>
#include "uapi/linux/landlock.h"
#include <linux/sched.h>
#include <linux/seccomp.h>
#include <linux/securebits.h>
#include <signal.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/prctl.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/sysmacros.h>
#include <unistd.h>

#if !defined(LANDLOCK_SCOPE_SIGNAL) || !defined(LANDLOCK_ACCESS_FS_IOCTL_DEV)
#error A qualified Landlock ABI-6-or-newer UAPI SDK is required
#endif
_Static_assert(sizeof(struct landlock_ruleset_attr)==24,"Pinned Landlock ABI-6 ruleset size");
_Static_assert(offsetof(struct landlock_ruleset_attr,scoped)==16,"Pinned Landlock scope offset");

static int control(int operation,unsigned long a,unsigned long b,unsigned long c,unsigned long d){return prctl(operation,a,b,c,d);}
#define prctl(operation,a,b,c,d) control(operation,a,b,c,d)
static _Noreturn void refused(void){_exit(125);}
static void require(bool condition){if(!condition)refused();}
static void limit(int resource,uint64_t value){struct rlimit bound={(rlim_t)value,(rlim_t)value};require((uint64_t)bound.rlim_cur==value&&setrlimit(resource,&bound)==0);}
static void single_threaded(void){
  /* This is called by reviewed bootstrap code in its private proc mount. No
   * untrusted initializer or thread producer has run. The count is an additional
   * admission check, not a way to stop a concurrent untrusted thread creator. */
  DIR* tasks=opendir("/proc/self/task");require(tasks!=NULL);size_t count=0;struct dirent* entry;
  errno=0;while((entry=readdir(tasks))!=NULL){if(entry->d_name[0]=='.')continue;const char* at=entry->d_name;for(;*at;at++)require(*at>='0'&&*at<='9');require(++count==1);}
  const int error=errno;require(closedir(tasks)==0&&error==0&&count==1);
}
static void empty_capabilities(void){
  struct __user_cap_header_struct header={.version=_LINUX_CAPABILITY_VERSION_3,.pid=0};
  struct __user_cap_data_struct values[2];memset(values,0,sizeof(values));require(syscall(SYS_capget,&header,values)==0);
  for(size_t i=0;i<2;i++)require(values[i].effective==0&&values[i].permitted==0&&values[i].inheritable==0);
  const int required=SECBIT_NOROOT|SECBIT_NOROOT_LOCKED|SECBIT_NO_SETUID_FIXUP|SECBIT_NO_SETUID_FIXUP_LOCKED|SECBIT_NO_CAP_AMBIENT_RAISE|SECBIT_NO_CAP_AMBIENT_RAISE_LOCKED;
  const int actual=prctl(PR_GET_SECUREBITS,0,0,0,0);require(actual>=0&&(actual&required)==required);
}
static uint64_t read_rights(bool directory){return LANDLOCK_ACCESS_FS_READ_FILE|(directory?LANDLOCK_ACCESS_FS_READ_DIR:0);}
static void paths(const struct omni_linux_policy* policy){
  const long abi=syscall(SYS_landlock_create_ruleset,NULL,0,LANDLOCK_CREATE_RULESET_VERSION);require(abi>=6);
  struct landlock_ruleset_attr attr;memset(&attr,0,sizeof(attr));
  attr.handled_access_fs=LANDLOCK_ACCESS_FS_EXECUTE|LANDLOCK_ACCESS_FS_WRITE_FILE|LANDLOCK_ACCESS_FS_READ_FILE|LANDLOCK_ACCESS_FS_READ_DIR|LANDLOCK_ACCESS_FS_REMOVE_DIR|LANDLOCK_ACCESS_FS_REMOVE_FILE|LANDLOCK_ACCESS_FS_MAKE_CHAR|LANDLOCK_ACCESS_FS_MAKE_DIR|LANDLOCK_ACCESS_FS_MAKE_REG|LANDLOCK_ACCESS_FS_MAKE_SOCK|LANDLOCK_ACCESS_FS_MAKE_FIFO|LANDLOCK_ACCESS_FS_MAKE_BLOCK|LANDLOCK_ACCESS_FS_MAKE_SYM|LANDLOCK_ACCESS_FS_REFER|LANDLOCK_ACCESS_FS_TRUNCATE|LANDLOCK_ACCESS_FS_IOCTL_DEV;
  attr.handled_access_net=LANDLOCK_ACCESS_NET_BIND_TCP|LANDLOCK_ACCESS_NET_CONNECT_TCP;
  attr.scoped=LANDLOCK_SCOPE_ABSTRACT_UNIX_SOCKET|LANDLOCK_SCOPE_SIGNAL;
  const size_t bytes=offsetof(struct landlock_ruleset_attr,scoped)+sizeof(attr.scoped);
  const int rules=(int)syscall(SYS_landlock_create_ruleset,&attr,bytes,0);require(rules>=0);
  bool executable=false;
  for(size_t i=0;i<policy->path_count;i++){
    const struct omni_linux_path* path=&policy->paths[i];require(path->fd>=3&&path->fd<128);
    for(size_t before=0;before<i;before++)require(policy->paths[before].fd!=path->fd);
    const int flags=fcntl(path->fd,F_GETFL),descriptor_flags=fcntl(path->fd,F_GETFD);struct stat value;
    require(flags>=0&&(flags&O_PATH)==O_PATH&&descriptor_flags>=0&&(descriptor_flags&FD_CLOEXEC)!=0&&fstat(path->fd,&value)==0);
    const bool directory=S_ISDIR(value.st_mode),null_device=S_ISCHR(value.st_mode)&&major(value.st_rdev)==1&&minor(value.st_rdev)==3;
    require(directory||S_ISREG(value.st_mode)||(null_device&&path->access==OMNI_LINUX_NULL_DEVICE));
    uint64_t allowed=0;
    switch(path->access){
      case OMNI_LINUX_READ_FILE:require(!directory);allowed=read_rights(false);break;
      case OMNI_LINUX_READ_DIRECTORY:require(directory);allowed=read_rights(true);break;
      case OMNI_LINUX_WRITE_DIRECTORY:
        require(directory);allowed=read_rights(true)|LANDLOCK_ACCESS_FS_WRITE_FILE|LANDLOCK_ACCESS_FS_REMOVE_DIR|LANDLOCK_ACCESS_FS_REMOVE_FILE|LANDLOCK_ACCESS_FS_MAKE_DIR|LANDLOCK_ACCESS_FS_MAKE_REG|LANDLOCK_ACCESS_FS_REFER|LANDLOCK_ACCESS_FS_TRUNCATE;break;
      case OMNI_LINUX_EXECUTABLE:
        require(!directory);allowed=read_rights(false)|LANDLOCK_ACCESS_FS_EXECUTE;if(path->fd==policy->executable_fd)executable=true;break;
      case OMNI_LINUX_NULL_DEVICE:
        require(null_device);allowed=LANDLOCK_ACCESS_FS_READ_FILE|LANDLOCK_ACCESS_FS_WRITE_FILE;break;
      default:refused();
    }
    const struct landlock_path_beneath_attr beneath={.allowed_access=allowed,.parent_fd=path->fd};
    require(syscall(SYS_landlock_add_rule,rules,LANDLOCK_RULE_PATH_BENEATH,&beneath,0)==0);
  }
  require(executable&&prctl(PR_SET_NO_NEW_PRIVS,1,0,0,0)==0&&syscall(SYS_landlock_restrict_self,rules,0)==0);
  require(close(rules)==0);
  for(size_t i=0;i<policy->path_count;i++)if(policy->paths[i].fd!=policy->executable_fd)require(close(policy->paths[i].fd)==0);
}

#define LOAD(field) BPF_STMT(BPF_LD|BPF_W|BPF_ABS,(uint32_t)offsetof(struct seccomp_data,field))
#define HIGH(field) BPF_STMT(BPF_LD|BPF_W|BPF_ABS,(uint32_t)offsetof(struct seccomp_data,field)+4)
#define ALLOW BPF_STMT(BPF_RET|BPF_K,SECCOMP_RET_ALLOW)
#define DENY BPF_STMT(BPF_RET|BPF_K,SECCOMP_RET_ERRNO|EPERM)
#define STOP BPF_STMT(BPF_RET|BPF_K,SECCOMP_RET_KILL_PROCESS)
#define MATCH(number,skip) BPF_JUMP(BPF_JMP|BPF_JEQ|BPF_K,(number),0,(skip))
#define COUNT(values) (sizeof(values)/sizeof((values)[0]))
struct filter_builder{struct sock_filter code[512];size_t count;};
static void push(struct filter_builder* filter,struct sock_filter instruction){require(filter->count<COUNT(filter->code));filter->code[filter->count++]=instruction;}
static void special(struct filter_builder* filter,uint32_t number,const struct sock_filter* body,size_t count){
  require(count>0&&count<=255);push(filter,(struct sock_filter)MATCH(number,(uint8_t)count));
  for(size_t i=0;i<count;i++)push(filter,body[i]);
}
static void argument_choices(struct filter_builder* filter,uint32_t number,uint32_t offset,const uint32_t* choices,size_t count){
  require(count>0&&count<=8);struct sock_filter body[21];size_t used=0;
  body[used++]=(struct sock_filter)BPF_STMT(BPF_LD|BPF_W|BPF_ABS,offset+4);
  body[used++]=(struct sock_filter)BPF_JUMP(BPF_JMP|BPF_JEQ|BPF_K,0,1,0);body[used++]=(struct sock_filter)DENY;
  body[used++]=(struct sock_filter)BPF_STMT(BPF_LD|BPF_W|BPF_ABS,offset);
  for(size_t i=0;i<count;i++){body[used++]=(struct sock_filter)BPF_JUMP(BPF_JMP|BPF_JEQ|BPF_K,choices[i],0,1);body[used++]=(struct sock_filter)ALLOW;}
  body[used++]=(struct sock_filter)DENY;special(filter,number,body,used);
}
#define BLOCK(name,...) do{const struct sock_filter body[]={__VA_ARGS__};special(&filter,SYS_##name,body,COUNT(body));}while(0)
#define SIMPLE(name) BLOCK(name,ALLOW)
static void syscalls(int executable){
  const uint32_t thread_required=CLONE_VM|CLONE_FS|CLONE_FILES|CLONE_SIGHAND|CLONE_THREAD|CLONE_SYSVSEM;
  const uint32_t thread_allowed=thread_required|CLONE_SETTLS|CLONE_PARENT_SETTID|CLONE_CHILD_CLEARTID|CLONE_CHILD_SETTID;
  const uint32_t self=(uint32_t)getpid();require(self>0);
  struct filter_builder filter={0};const struct sock_filter head[]={
    LOAD(arch),BPF_JUMP(BPF_JMP|BPF_JEQ|BPF_K,AUDIT_ARCH_X86_64,1,0),STOP,
    LOAD(nr),BPF_JUMP(BPF_JMP|BPF_JSET|BPF_K,0x40000000u,0,1),STOP,
  };
  for(size_t i=0;i<COUNT(head);i++)push(&filter,head[i]);
  /* The matcher skip uses each actual block's length, not hand-maintained
   * offsets into another syscall's argument accumulator. Every block returns. */
  BLOCK(clone3,BPF_STMT(BPF_RET|BPF_K,SECCOMP_RET_ERRNO|ENOSYS));
  BLOCK(clone,
    HIGH(args[0]),BPF_JUMP(BPF_JMP|BPF_JEQ|BPF_K,0,1,0),DENY,
    LOAD(args[0]),BPF_STMT(BPF_ALU|BPF_AND|BPF_K,~thread_allowed),BPF_JUMP(BPF_JMP|BPF_JEQ|BPF_K,0,1,0),DENY,
    LOAD(args[0]),BPF_STMT(BPF_ALU|BPF_AND|BPF_K,thread_required),BPF_JUMP(BPF_JMP|BPF_JEQ|BPF_K,thread_required,1,0),DENY,ALLOW);
    /* No future executable path is accepted: execveat is restricted to the
     * bootstrap FD and Landlock permits only inventoried executable files. */
  BLOCK(execveat,
      HIGH(args[0]),BPF_JUMP(BPF_JMP|BPF_JEQ|BPF_K,0,1,0),DENY,
      LOAD(args[0]),BPF_JUMP(BPF_JMP|BPF_JEQ|BPF_K,(uint32_t)executable,1,0),DENY,
      HIGH(args[4]),BPF_JUMP(BPF_JMP|BPF_JEQ|BPF_K,0,1,0),DENY,
      LOAD(args[4]),BPF_JUMP(BPF_JMP|BPF_JEQ|BPF_K,AT_EMPTY_PATH,1,0),DENY,
      ALLOW);
  const uint32_t own_group[]={self};argument_choices(&filter,SYS_tgkill,(uint32_t)offsetof(struct seccomp_data,args[0]),own_group,COUNT(own_group));
  const uint32_t controls[]={PR_SET_NAME,PR_GET_NAME,PR_GET_SECCOMP,PR_GET_NO_NEW_PRIVS};argument_choices(&filter,SYS_prctl,(uint32_t)offsetof(struct seccomp_data,args[0]),controls,COUNT(controls));
  const uint32_t descriptor_operations[]={F_GETFD,F_SETFD,F_GETFL,F_SETFL,F_DUPFD,F_DUPFD_CLOEXEC};argument_choices(&filter,SYS_fcntl,(uint32_t)offsetof(struct seccomp_data,args[1]),descriptor_operations,COUNT(descriptor_operations));
  BLOCK(prlimit64,
    HIGH(args[2]),BPF_JUMP(BPF_JMP|BPF_JEQ|BPF_K,0,1,0),DENY,
    LOAD(args[2]),BPF_JUMP(BPF_JMP|BPF_JEQ|BPF_K,0,1,0),DENY,
    HIGH(args[0]),BPF_JUMP(BPF_JMP|BPF_JEQ|BPF_K,0,1,0),DENY,
    LOAD(args[0]),BPF_JUMP(BPF_JMP|BPF_JEQ|BPF_K,0,0,1),ALLOW,
    BPF_JUMP(BPF_JMP|BPF_JEQ|BPF_K,self,0,1),ALLOW,DENY);
    /* File-backed RX mappings are needed by the approved ELF loader. Anonymous
     * executable mappings and W+X mappings are refused; executable protection
     * cannot be added later. This does not claim complete Linux ACG semantics. */
  BLOCK(mmap,LOAD(args[2]),BPF_JUMP(BPF_JMP|BPF_JSET|BPF_K,PROT_EXEC,0,5),
      BPF_JUMP(BPF_JMP|BPF_JSET|BPF_K,PROT_WRITE,0,1),DENY,
      LOAD(args[3]),BPF_JUMP(BPF_JMP|BPF_JSET|BPF_K,MAP_ANONYMOUS,0,1),DENY,ALLOW);
  BLOCK(mprotect,LOAD(args[2]),BPF_JUMP(BPF_JMP|BPF_JSET|BPF_K,PROT_EXEC,0,1),DENY,ALLOW);
  SIMPLE(read);SIMPLE(write);SIMPLE(readv);SIMPLE(writev);SIMPLE(pread64);SIMPLE(pwrite64);
  SIMPLE(close);SIMPLE(lseek);SIMPLE(fstat);SIMPLE(newfstatat);SIMPLE(statx);SIMPLE(stat);SIMPLE(lstat);
  SIMPLE(openat);SIMPLE(readlink);SIMPLE(readlinkat);SIMPLE(getdents64);SIMPLE(access);SIMPLE(faccessat);SIMPLE(faccessat2);
  SIMPLE(fsync);SIMPLE(fdatasync);SIMPLE(ftruncate);SIMPLE(truncate);SIMPLE(fadvise64);
  SIMPLE(unlink);SIMPLE(unlinkat);SIMPLE(rmdir);SIMPLE(rename);SIMPLE(renameat);SIMPLE(renameat2);SIMPLE(mkdir);SIMPLE(mkdirat);SIMPLE(umask);
  SIMPLE(chdir);SIMPLE(fchdir);SIMPLE(getcwd);SIMPLE(dup);SIMPLE(dup2);SIMPLE(dup3);
  SIMPLE(munmap);SIMPLE(mremap);SIMPLE(madvise);SIMPLE(brk);SIMPLE(arch_prctl);
  SIMPLE(rt_sigaction);SIMPLE(rt_sigprocmask);SIMPLE(rt_sigreturn);SIMPLE(sigaltstack);SIMPLE(rt_sigtimedwait);
  SIMPLE(set_tid_address);SIMPLE(set_robust_list);SIMPLE(get_robust_list);SIMPLE(futex);SIMPLE(rseq);
  SIMPLE(getpid);SIMPLE(gettid);SIMPLE(getppid);SIMPLE(getuid);SIMPLE(geteuid);SIMPLE(getgid);SIMPLE(getegid);
  SIMPLE(clock_gettime);SIMPLE(clock_getres);SIMPLE(clock_nanosleep);SIMPLE(nanosleep);SIMPLE(gettimeofday);SIMPLE(time);
  SIMPLE(getrandom);SIMPLE(uname);SIMPLE(sysinfo);SIMPLE(getrusage);SIMPLE(getrlimit);SIMPLE(times);SIMPLE(sched_yield);SIMPLE(sched_getaffinity);
  SIMPLE(poll);SIMPLE(ppoll);SIMPLE(select);SIMPLE(pselect6);SIMPLE(epoll_create1);SIMPLE(epoll_ctl);SIMPLE(epoll_wait);SIMPLE(epoll_pwait);
  SIMPLE(eventfd2);SIMPLE(pipe2);SIMPLE(restart_syscall);SIMPLE(exit);SIMPLE(exit_group);push(&filter,(struct sock_filter)DENY);
  const struct sock_fprog program={.len=(unsigned short)filter.count,.filter=filter.code};
  require(filter.count<512&&syscall(SYS_seccomp,SECCOMP_SET_MODE_FILTER,SECCOMP_FILTER_FLAG_TSYNC,&program)==0);
}
_Noreturn void omni_linux_policy_exec(const struct omni_linux_policy* policy,char*const argv[],char*const environment[]){
  require(policy!=NULL&&argv!=NULL&&argv[0]!=NULL&&environment!=NULL&&policy->path_count>0&&policy->path_count<=OMNI_LINUX_PATHS&&policy->executable_fd>=3&&policy->executable_fd<128);
  require(policy->memory_bytes>=256ull*1024*1024&&policy->memory_bytes<=8ull*1024*1024*1024&&policy->maximum_file_bytes>0&&policy->maximum_file_bytes<=256ull*1024*1024);
  single_threaded();empty_capabilities();limit(RLIMIT_AS,policy->memory_bytes);limit(RLIMIT_FSIZE,policy->maximum_file_bytes);limit(RLIMIT_CORE,0);limit(RLIMIT_MEMLOCK,0);limit(RLIMIT_NOFILE,128);limit(RLIMIT_RTPRIO,0);
  require(prctl(PR_SET_DUMPABLE,0,0,0,0)==0);paths(policy);syscalls(policy->executable_fd);
  syscall(SYS_execveat,policy->executable_fd,"",argv,environment,AT_EMPTY_PATH);refused();
}
