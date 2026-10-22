#include "cgroup.h"
#include <errno.h>
#include <fcntl.h>
#include <linux/magic.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/statfs.h>
#include <unistd.h>

static _Noreturn void refused(void){_exit(125);}
static void require(bool condition){if(!condition)refused();}
static void closed(int fd){require(close(fd)==0);}
static size_t text(int root,const char* name,char* output,size_t capacity){
  const int fd=openat(root,name,O_RDONLY|O_CLOEXEC|O_NOFOLLOW);require(fd>=0);size_t at=0;
  for(;;){require(at+1<capacity);const ssize_t count=read(fd,output+at,capacity-at-1);if(count<0&&errno==EINTR)continue;require(count>=0);if(!count)break;at+=(size_t)count;}
  output[at]=0;closed(fd);return at;
}
static void exact(int root,const char* name,const char* expected){char actual[128];text(root,name,actual,sizeof(actual));require(strcmp(actual,expected)==0);}
static void written(int root,const char* name,const char* value){
  const size_t length=strlen(value);require(length>0&&length<96);const int fd=openat(root,name,O_WRONLY|O_CLOEXEC|O_NOFOLLOW);require(fd>=0);ssize_t count;
  do{count=write(fd,value,length);}while(count<0&&errno==EINTR);require(count==(ssize_t)length);closed(fd);
}
static void setting(int root,const char* name,const char* value){
  written(root,name,value);char expected[128];const int length=snprintf(expected,sizeof(expected),"%s\n",value);require(length>0&&(size_t)length<sizeof(expected));exact(root,name,expected);
}
static void number(int root,const char* name,uint64_t value){char raw[32];const int length=snprintf(raw,sizeof(raw),"%llu",(unsigned long long)value);require(length>0&&(size_t)length<sizeof(raw));setting(root,name,raw);}
static void no_descendants(int root){
  char raw[1024];text(root,"cgroup.stat",raw,sizeof(raw));bool descendants=false,dying=false;const char* at=raw;
  while(*at){const char* key=at;while((*at>='a'&&*at<='z')||*at=='_')at++;require(at>key&&at-key<=64&&*at==' ');const size_t length=(size_t)(at-key);at++;require(*at>='0'&&*at<='9');bool nonzero=false;size_t digits=0;while(*at>='0'&&*at<='9'){nonzero|=*at!='0';at++;require(++digits<=20);}require(*at++=='\n');
    if(length==14&&memcmp(key,"nr_descendants",14)==0){require(!descendants&&!nonzero);descendants=true;}
    if(length==20&&memcmp(key,"nr_dying_descendants",20)==0){require(!dying&&!nonzero);dying=true;}
  }
  require(descendants&&dying);
}
void omni_linux_cgroup_enter(int fd,const char* session,uint64_t memory,uint64_t root_bytes,uint64_t scratch,uint64_t save,unsigned cpu){
  require(fd>=6&&fd<1024&&session!=NULL&&strlen(session)==36&&cpu>=1&&cpu<=8&&memory>=256ull*1024*1024&&memory<=8ull*1024*1024*1024&&root_bytes>=16ull*1024*1024&&root_bytes<=1024ull*1024*1024&&scratch>=4ull*1024*1024&&scratch<=512ull*1024*1024&&save>=4ull*1024*1024&&save<=512ull*1024*1024&&((memory|root_bytes|scratch|save)&4095)==0);
  struct stat identity;struct statfs filesystem;const int flags=fcntl(fd,F_GETFL);require(flags>=0&&(flags&O_PATH)==O_PATH&&fstat(fd,&identity)==0&&S_ISDIR(identity.st_mode)&&identity.st_uid==getuid()&&(identity.st_mode&07777)==0700&&fstatfs(fd,&filesystem)==0&&filesystem.f_type==CGROUP2_SUPER_MAGIC);
  char link[64],path[4096],name[64];int length=snprintf(link,sizeof(link),"/proc/self/fd/%d",fd);require(length>0&&(size_t)length<sizeof(link));const ssize_t count=readlink(link,path,sizeof(path)-1);require(count>0&&(size_t)count<sizeof(path)-1);path[count]=0;const char* leaf=strrchr(path,'/');require(leaf!=NULL);length=snprintf(name,sizeof(name),"omniplay-%s",session);require(length>0&&(size_t)length<sizeof(name)&&strcmp(leaf+1,name)==0);
  exact(fd,"cgroup.type","domain\n");exact(fd,"cgroup.procs","");exact(fd,"pids.current","0\n");exact(fd,"cgroup.freeze","0\n");char subtree[64];text(fd,"cgroup.subtree_control",subtree,sizeof(subtree));require(subtree[0]==0||strcmp(subtree,"\n")==0);no_descendants(fd);
  // The sealed plan admits only this new leaf. Parent delegation/controller
  // provisioning and eventual cgroup.kill/empty removal belong to the broker.
  number(fd,"memory.max",memory+root_bytes+scratch+save+128ull*1024*1024);number(fd,"memory.swap.max",0);number(fd,"memory.oom.group",1);number(fd,"pids.max",64);
  char quota[64];length=snprintf(quota,sizeof(quota),"%u 100000",cpu*100000u);require(length>0&&(size_t)length<sizeof(quota));setting(fd,"cpu.max",quota);
  // Avoid inheriting an OOM exception from a protected launcher. An ordinary
  // workload may increase its adjustment to zero but cannot regain protection.
  written(AT_FDCWD,"/proc/self/oom_score_adj","0");exact(AT_FDCWD,"/proc/self/oom_score_adj","0\n");
  written(fd,"cgroup.procs","0");number(fd,"cgroup.max.depth",0);number(fd,"cgroup.max.descendants",0);exact(fd,"pids.current","1\n");
  char own[32];length=snprintf(own,sizeof(own),"%ld\n",(long)getpid());require(length>0&&(size_t)length<sizeof(own));exact(fd,"cgroup.procs",own);closed(fd);
}
