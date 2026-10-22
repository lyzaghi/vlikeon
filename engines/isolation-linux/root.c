#include "root.h"
#if !defined(__linux__) || !defined(__x86_64__) || defined(__ILP32__)
#error This root constructor supports only native Linux x86-64
#endif
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <linux/mount.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/sysmacros.h>
#include <unistd.h>

static _Noreturn void refused(void){_exit(125);}
/* Linux umount(2) UAPI: lazy detach. linux/mount.h supplies the fd-mount
 * constants; do not mix it with libc's duplicate mount attribute declarations. */
enum{omni_mnt_detach=2};
static void require(bool condition){if(!condition)refused();}
static void closed(int fd){require(close(fd)==0);}
static int descriptor(long value){require(value>=5&&value<=INT_MAX);return(int)value;}
static void parameter(int context,const char* name,const char* value){require(syscall(SYS_fsconfig,context,FSCONFIG_SET_STRING,name,value,0)==0);}
static void number(int context,const char* name,uint64_t value){char raw[32];const int length=snprintf(raw,sizeof(raw),"%llu",(unsigned long long)value);require(length>0&&(size_t)length<sizeof(raw));parameter(context,name,raw);}
static int filesystem(const char* kind,uint64_t size,bool noexec){
  const int context=descriptor(syscall(SYS_fsopen,kind,FSOPEN_CLOEXEC));
  if(size){number(context,"size",size);number(context,"nr_inodes",8192);parameter(context,"mode","0700");}
  require(syscall(SYS_fsconfig,context,FSCONFIG_CMD_CREATE,NULL,NULL,0)==0);
  const int result=descriptor(syscall(SYS_fsmount,context,FSMOUNT_CLOEXEC,MOUNT_ATTR_NOSUID|MOUNT_ATTR_NODEV|(noexec?MOUNT_ATTR_NOEXEC:0)));closed(context);return result;
}
static void attributes(int fd,uint64_t set,uint64_t clear){
  struct mount_attr attr;memset(&attr,0,sizeof(attr));attr.attr_set=set;attr.attr_clr=clear;
  require(syscall(SYS_mount_setattr,fd,"",AT_EMPTY_PATH,&attr,sizeof(attr))==0);
}
static void attach(int source,int target,const char* name){require(syscall(SYS_move_mount,source,"",target,name,MOVE_MOUNT_F_EMPTY_PATH)==0);}
static void attach_empty(int source,int target){require(syscall(SYS_move_mount,source,"",target,"",MOVE_MOUNT_F_EMPTY_PATH|MOVE_MOUNT_T_EMPTY_PATH)==0);}
static void clone_at(int source,const char* source_path,int target,const char* target_path,uint64_t rights,uint64_t clear){
  const int tree=descriptor(syscall(SYS_open_tree,source,source_path,OPEN_TREE_CLONE|OPEN_TREE_CLOEXEC|(*source_path?0:AT_EMPTY_PATH)));
  attributes(tree,rights,clear);attach(tree,target,target_path);closed(tree);
}
static void empty_stage(int fd){
  struct stat value;const int flags=fcntl(fd,F_GETFL),descriptor_flags=fcntl(fd,F_GETFD);
  require(fd>=5&&fd<1024&&flags>=0&&(flags&O_PATH)==O_PATH&&descriptor_flags>=0&&(descriptor_flags&FD_CLOEXEC)!=0&&fstat(fd,&value)==0&&S_ISDIR(value.st_mode)&&value.st_uid==0&&(value.st_mode&07777)==0700);
  const int readable=openat(fd,".",O_RDONLY|O_DIRECTORY|O_CLOEXEC|O_NOFOLLOW);require(readable>=5);DIR* entries=fdopendir(readable);require(entries!=NULL);struct dirent* entry;errno=0;
  while((entry=readdir(entries))!=NULL)require(strcmp(entry->d_name,".")==0||strcmp(entry->d_name,"..")==0);
  const int error=errno;require(closedir(entries)==0&&error==0);
}
static bool valid_path(const char path[512]){
  const size_t length=strnlen(path,512);if(length==0||length>=512||path[0]=='/'||path[length-1]=='/')return false;
  size_t part=0,depth=1;
  for(size_t i=0;i<length;i++){
    const unsigned char byte=(unsigned char)path[i];
    if(byte=='/'){if(part==0||part>127||++depth>8)return false;part=0;continue;}
    if(!((byte>='a'&&byte<='z')||(byte>='A'&&byte<='Z')||(byte>='0'&&byte<='9')||byte=='_'||byte=='-'||((byte=='.'||byte=='+')&&part>0)))return false;
    part++;
  }
  return part>0&&part<=127;
}
static bool image_path(const struct omni_linux_root_file* file){
  const bool worker=strcmp(file->path,"runtime/omniplay-native-worker")==0,loader=strcmp(file->path,"lib64/ld-linux-x86-64.so.2")==0;
  require(file->executable==(worker||loader));return strncmp(file->path,"runtime/",8)==0||loader;
}
static bool game_path(const struct omni_linux_root_file* file){
  if(strncmp(file->path,"work/",5)!=0||file->executable)return false;const char* name=file->path+5;
  return *name&&strchr(name,'/')==NULL&&strcmp(name,"scratch")!=0&&strcmp(name,"save")!=0;
}
static void inputs(const struct omni_linux_root_plan* plan,uint64_t root_bytes){
  require(plan!=NULL&&plan->file_count>=2&&plan->file_count<=OMNI_LINUX_ROOT_FILES);empty_stage(plan->empty_directory_fd);
  bool worker=false,loader=false;uint64_t image_bytes=0,game_bytes=0;
  for(size_t i=0;i<plan->file_count;i++){
    const struct omni_linux_root_file* file=&plan->files[i];require(valid_path(file->path)&&file->fd>=5&&file->fd<1024&&file->fd!=plan->empty_directory_fd&&file->bytes>0);
    for(size_t before=0;before<i;before++)require(plan->files[before].fd!=file->fd&&strcmp(plan->files[before].path,file->path)!=0);
    struct stat value;const int flags=fcntl(file->fd,F_GETFL),descriptor_flags=fcntl(file->fd,F_GETFD);
    require(flags>=0&&(flags&O_ACCMODE)==O_RDONLY&&(flags&O_PATH)==0&&descriptor_flags>=0&&(descriptor_flags&FD_CLOEXEC)!=0&&fstat(file->fd,&value)==0&&S_ISREG(value.st_mode)&&value.st_size>0&&(uint64_t)value.st_size==file->bytes);
    if(file->role==OMNI_LINUX_IMAGE_FILE){
      require(image_path(file)&&file->bytes<=256ull*1024*1024&&(value.st_mode&0111)==0);const int seals=fcntl(file->fd,F_GET_SEALS),required=F_SEAL_WRITE|F_SEAL_SHRINK|F_SEAL_GROW|F_SEAL_SEAL|F_SEAL_EXEC;require(seals>=0&&(seals&required)==required);
      require(file->bytes<=root_bytes&&image_bytes<=root_bytes-file->bytes);image_bytes+=file->bytes;
      worker|=strcmp(file->path,"runtime/omniplay-native-worker")==0;loader|=strcmp(file->path,"lib64/ld-linux-x86-64.so.2")==0;
    }else{
      require(file->role==OMNI_LINUX_GAME_INPUT&&game_path(file)&&file->bytes<=4ull*1024*1024*1024&&value.st_uid==0&&value.st_nlink==1&&(value.st_mode&0022)==0&&game_bytes<=32ull*1024*1024*1024-file->bytes);game_bytes+=file->bytes;
    }
  }
  require(worker&&loader&&root_bytes>=2ull*1024*1024&&image_bytes<=root_bytes-2ull*1024*1024);
}
static void directory(int root,const char* name){
  if(mkdirat(root,name,0700)<0)require(errno==EEXIST);struct stat value;require(fstatat(root,name,&value,AT_SYMLINK_NOFOLLOW)==0&&S_ISDIR(value.st_mode)&&value.st_uid==0);
}
static void parents(int root,const char* name){char path[512];const size_t length=strlen(name);require(length<sizeof(path));memcpy(path,name,length+1);for(size_t at=0;at<length;at++)if(path[at]=='/'){path[at]=0;directory(root,path);path[at]='/';}}
static int placeholder(int root,const struct omni_linux_root_file* file){parents(root,file->path);const int fd=openat(root,file->path,O_WRONLY|O_CREAT|O_EXCL|O_NOFOLLOW|O_CLOEXEC,0600);require(fd>=5);return fd;}
static void copy(int root,const struct omni_linux_root_file* file){
  const int output=placeholder(root,file);unsigned char buffer[65536];uint64_t at=0;
  while(at<file->bytes){const size_t count=(size_t)(file->bytes-at<sizeof(buffer)?file->bytes-at:sizeof(buffer));ssize_t read_count;
    do{read_count=pread(file->fd,buffer,count,(off_t)at);}while(read_count<0&&errno==EINTR);require(read_count>0&&(size_t)read_count<=count);
    size_t written=0;while(written<(size_t)read_count){const ssize_t result=write(output,buffer+written,(size_t)read_count-written);if(result<0&&errno==EINTR)continue;require(result>0);written+=(size_t)result;}at+=(uint64_t)read_count;
  }
  require(fchmod(output,file->executable?0500:0400)==0);closed(output);
}
void omni_linux_construct_root(const struct omni_linux_root_plan* plan,uint64_t root_bytes,uint64_t scratch_bytes,uint64_t save_bytes){
  require(root_bytes>=16ull*1024*1024&&root_bytes<=1024ull*1024*1024&&scratch_bytes>=4ull*1024*1024&&scratch_bytes<=512ull*1024*1024&&save_bytes>=4ull*1024*1024&&save_bytes<=512ull*1024*1024&&((root_bytes|scratch_bytes|save_bytes)&4095)==0);
  inputs(plan,root_bytes);const int root=filesystem("tmpfs",root_bytes,false);attach_empty(root,plan->empty_directory_fd);
  const char*const directories[]={"runtime","lib64","work","work/scratch","work/save","proc","dev",".old-root"};
  for(size_t i=0;i<sizeof(directories)/sizeof(directories[0]);i++)directory(root,directories[i]);
  for(size_t i=0;i<plan->file_count;i++){const struct omni_linux_root_file* file=&plan->files[i];if(file->role==OMNI_LINUX_IMAGE_FILE)copy(root,file);else closed(placeholder(root,file));}
  const int null=openat(root,"dev/null",O_WRONLY|O_CREAT|O_EXCL|O_NOFOLLOW|O_CLOEXEC,0600);require(null>=5);closed(null);
  const uint64_t immutable=MOUNT_ATTR_RDONLY|MOUNT_ATTR_NOSUID|MOUNT_ATTR_NODEV|MOUNT_ATTR_NOEXEC;
  clone_at(root,"work",root,"work",immutable,0);
  for(size_t i=0;i<plan->file_count;i++)if(plan->files[i].role==OMNI_LINUX_GAME_INPUT)clone_at(plan->files[i].fd,"",root,plan->files[i].path,immutable,0);
  const int scratch=filesystem("tmpfs",scratch_bytes,true);attach(scratch,root,"work/scratch");closed(scratch);
  const int saves=filesystem("tmpfs",save_bytes,true);attach(saves,root,"work/save");closed(saves);
  const int proc=filesystem("proc",0,true);attributes(proc,immutable,0);attach(proc,root,"proc");closed(proc);
  clone_at(2,"",root,"dev/null",MOUNT_ATTR_NOSUID|MOUNT_ATTR_NOEXEC,MOUNT_ATTR_NODEV);
  require(fchdir(root)==0&&syscall(SYS_pivot_root,".",".old-root")==0&&chdir("/")==0&&syscall(SYS_umount2,"/.old-root",omni_mnt_detach)==0&&rmdir("/.old-root")==0);
  attributes(root,MOUNT_ATTR_RDONLY|MOUNT_ATTR_NOSUID|MOUNT_ATTR_NODEV,0);closed(root);
}
