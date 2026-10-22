#include "namespaces.h"
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/prctl.h>
#include <sys/syscall.h>
#include <unistd.h>

enum{plan_header_bytes=96,plan_entry_bytes=536};
static _Noreturn void refused(void){_exit(125);}
static void require(bool condition){if(!condition)refused();}
static uint32_t u32(const unsigned char* value){return(uint32_t)value[0]|((uint32_t)value[1]<<8)|((uint32_t)value[2]<<16)|((uint32_t)value[3]<<24);}
static uint64_t u64(const unsigned char* value){return(uint64_t)u32(value)|((uint64_t)u32(value+4)<<32);}
static void read_at(void* output,size_t count,uint64_t offset){
  size_t at=0;while(at<count){const ssize_t read=pread(5,(unsigned char*)output+at,count-at,(off_t)(offset+at));if(read<0&&errno==EINTR)continue;require(read>0&&(size_t)read<=count-at);at+=(size_t)read;}
}
static unsigned digit(char value){if(value>='0'&&value<='9')return(unsigned)(value-'0');if(value>='a'&&value<='f')return(unsigned)(value-'a'+10);refused();}
static void session(const char* raw,unsigned char output[16]){
  require(raw!=NULL&&strnlen(raw,37)==36);size_t at=0;bool nonzero=false;
  for(size_t i=0;i<36;){if(i==8||i==13||i==18||i==23){require(raw[i++]=='-');continue;}require(i+1<36&&at<16);output[at]=(unsigned char)(digit(raw[i])*16+digit(raw[i+1]));nonzero|=output[at++]!=0;i+=2;}
  require(at==16&&nonzero);
}
static void closed(int fd){require(close(fd)==0);}
static void read_plan(struct omni_linux_namespace_plan* namespace,struct omni_linux_root_plan* root,const char* uuid){
  struct stat shape;const int flags=fcntl(5,F_GETFL),seals=fcntl(5,F_GET_SEALS),required=F_SEAL_SEAL|F_SEAL_WRITE|F_SEAL_SHRINK|F_SEAL_GROW|F_SEAL_EXEC;
  // Inherited descriptors cannot arrive CLOEXEC across this bootstrap exec.
  // Mark the admitted plan and source descriptors before any later exec.
  require(flags>=0&&(flags&O_ACCMODE)==O_RDONLY&&(flags&O_PATH)==0&&fcntl(5,F_SETFD,FD_CLOEXEC)==0&&seals>=0&&(seals&required)==required&&fstat(5,&shape)==0&&S_ISREG(shape.st_mode)&&(shape.st_mode&0111)==0&&shape.st_size>=plan_header_bytes&&shape.st_size<=plan_header_bytes+OMNI_LINUX_ROOT_FILES*plan_entry_bytes);
  unsigned char header[plan_header_bytes];read_at(header,sizeof(header),0);const unsigned char magic[8]={'O','P','N','S','B','X','0','3'};require(memcmp(header,magic,sizeof(magic))==0&&u32(header+8)==3);
  const uint32_t count=u32(header+12),parent=u32(header+16),stage=u32(header+20);require(count>=2&&count<=OMNI_LINUX_ROOT_FILES&&parent>1&&parent<=INT_MAX&&stage>=8&&stage<1024&&(uint64_t)shape.st_size==plan_header_bytes+(uint64_t)count*plan_entry_bytes);
  unsigned char identity[16];session(uuid,identity);require(memcmp(header+64,identity,sizeof(identity))==0);
  const uint32_t cgroup=u32(header+80),tasks=u32(header+84),cpu=u32(header+88);require(cgroup>=8&&cgroup<1024&&cgroup!=stage&&tasks==64&&cpu>=1&&cpu<=8&&u32(header+92)==0);
  memset(namespace,0,sizeof(*namespace));memset(root,0,sizeof(*root));namespace->expected_parent=(pid_t)parent;memcpy(namespace->session,uuid,37);namespace->memory_bytes=u64(header+24);namespace->maximum_file_bytes=u64(header+32);namespace->root_bytes=u64(header+40);namespace->scratch_bytes=u64(header+48);namespace->save_bytes=u64(header+56);namespace->root_plan=root;root->empty_directory_fd=(int)stage;root->file_count=count;
  for(size_t i=0;i<count;i++){
    unsigned char raw[plan_entry_bytes];read_at(raw,sizeof(raw),plan_header_bytes+i*plan_entry_bytes);
    const uint32_t role=u32(raw),fd=u32(raw+4),executable=u32(raw+16);const size_t length=(size_t)raw[20]|((size_t)raw[21]<<8);
    require((role==OMNI_LINUX_IMAGE_FILE||role==OMNI_LINUX_GAME_INPUT)&&fd>=8&&fd<1024&&fd!=stage&&fd!=cgroup&&executable<=1&&length>0&&length<512&&raw[22]==0&&raw[23]==0);
    for(size_t at=0;at<512;at++)require(at<length?raw[24+at]!=0:raw[24+at]==0);
    struct omni_linux_root_file* file=&root->files[i];file->role=(enum omni_linux_root_role)role;file->fd=(int)fd;file->bytes=u64(raw+8);file->executable=executable!=0;memcpy(file->path,raw+24,length);
    for(size_t previous=0;previous<i;previous++)require(root->files[previous].fd!=file->fd&&strcmp(root->files[previous].path,file->path)!=0);
  }
  namespace->cgroup_fd=(int)cgroup;namespace->cpu_cores=cpu;require(fcntl(namespace->cgroup_fd,F_SETFD,FD_CLOEXEC)==0&&fcntl(root->empty_directory_fd,F_SETFD,FD_CLOEXEC)==0);
  for(size_t i=0;i<root->file_count;i++)require(fcntl(root->files[i].fd,F_SETFD,FD_CLOEXEC)==0);
  closed(5);
}
static void scrub(const struct omni_linux_root_plan* root,int cgroup){
  bool keep[1024]={false};keep[6]=true;keep[cgroup]=true;keep[root->empty_directory_fd]=true;for(size_t i=0;i<root->file_count;i++)keep[root->files[i].fd]=true;
  for(int fd=5;fd<1024;fd++)if(!keep[fd]){if(close(fd)<0)require(errno==EBADF);}
  require(syscall(SYS_close_range,1024u,UINT_MAX,0)==0);
}
int main(int argc,char** argv){
  require(argc==3&&strcmp(argv[1],"--stdio-isolation-v3")==0&&prctl(PR_GET_NO_NEW_PRIVS,0UL,0UL,0UL,0UL)==1);umask(0077);closed(7);
  struct omni_linux_namespace_plan namespace;struct omni_linux_root_plan root;read_plan(&namespace,&root,argv[2]);scrub(&root,namespace.cgroup_fd);omni_linux_namespace_supervise(&namespace);
}
