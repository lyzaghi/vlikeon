#import <Foundation/Foundation.h>
#include "session.hpp"
#include "catalog.hpp"
#include "process.hpp"
#include "boot.hpp"
#include <CommonCrypto/CommonDigest.h>
#include <algorithm>
#include <array>
#include <cctype>
#include <cerrno>
#include <cstdint>
#include <cstring>
#include <dirent.h>
#include <fcntl.h>
#include <filesystem>
#include <initializer_list>
#include <limits>
#include <mutex>
#include <set>
#include <stdio.h>
#include <string>
#include <sys/stat.h>
#include <sys/file.h>
#include <unistd.h>
#include <utility>
#include <vector>

namespace omniplay::macos {
namespace {
constexpr size_t kMaximumAssets = 144;
constexpr uint64_t kMaximumAssetBytes = 4ULL * 1024 * 1024 * 1024;
constexpr uint64_t kMaximumSessionBytes = 8ULL * 1024 * 1024 * 1024;
constexpr size_t kMaximumSessions = 4;
constexpr size_t kMaximumRootEntries = 32;
std::mutex g_sessions;

struct Asset {
  std::string name;std::string hash;uint64_t bytes;int descriptor=-1;
  Asset(std::string asset_name,std::string asset_hash,uint64_t asset_bytes,int asset_descriptor):name(std::move(asset_name)),hash(std::move(asset_hash)),bytes(asset_bytes),descriptor(asset_descriptor){}
  Asset(const Asset&)=delete;Asset& operator=(const Asset&)=delete;
  Asset(Asset&& other)noexcept:name(std::move(other.name)),hash(std::move(other.hash)),bytes(other.bytes),descriptor(other.descriptor){other.descriptor=-1;}
  Asset& operator=(Asset&& other)noexcept{if(this!=&other){if(descriptor>=0)close(descriptor);name=std::move(other.name);hash=std::move(other.hash);bytes=other.bytes;descriptor=other.descriptor;other.descriptor=-1;}return *this;}
  ~Asset(){if(descriptor>=0)close(descriptor);}
};
struct Descriptor { int value = -1; ~Descriptor(){if(value >= 0) close(value);} Descriptor()=default; Descriptor(const Descriptor&)=delete; Descriptor& operator=(const Descriptor&)=delete; };

bool uuid(const char* value) {
  if (!value || strnlen(value, 37) != 36) return false;
  bool nonzero=false;
  for (size_t index = 0; index < 36; ++index) {
    if (index == 8 || index == 13 || index == 18 || index == 23) { if (value[index] != '-') return false; }
    else {if (!((value[index] >= '0' && value[index] <= '9') || (value[index] >= 'a' && value[index] <= 'f'))) return false;nonzero|=value[index]!='0';}
  }
  return nonzero;
}
bool hash(const char* value) {
  if (!value || strnlen(value, 65) != 64) return false;
  for (size_t index=0;index<64;++index)if(!((value[index]>='0'&&value[index]<='9')||(value[index]>='a'&&value[index]<='f')))return false;
  return true;
}
bool name(const char* value) {
  auto alnum=[](unsigned char byte){return (byte>='a'&&byte<='z')||(byte>='A'&&byte<='Z')||(byte>='0'&&byte<='9');};
  if (!value) return false;size_t length=strnlen(value,129);if(length==0||length>128||!alnum(value[0])||value[length-1]=='.'||strcmp(value,"receipt.json")==0)return false;
  for(size_t index=0;index<length;++index){unsigned char byte=value[index];if(!(alnum(byte)||byte=='_'||byte=='-'||byte=='.'||byte=='+'))return false;}return true;
}
bool exact(xpc_object_t dictionary,std::initializer_list<const char*> names){if(xpc_get_type(dictionary)!=XPC_TYPE_DICTIONARY||xpc_dictionary_get_count(dictionary)!=names.size())return false;for(const char* name:names)if(!xpc_dictionary_get_value(dictionary,name))return false;return true;}

bool private_directory(const std::filesystem::path& path) {
  struct stat shape{};return lstat(path.c_str(),&shape)==0&&S_ISDIR(shape.st_mode)&&shape.st_uid==geteuid()&&(shape.st_mode&07777)==0700;
}
bool sync_directory(const std::filesystem::path& path);
bool ensure_root(std::filesystem::path& result,const std::string& profile) {
  if(!uuid(profile.c_str()))return false;
  NSString* home=NSHomeDirectory();if(![home isKindOfClass:NSString.class]||home.length==0)return false;
  const auto library=std::filesystem::path(home.fileSystemRepresentation)/"Library";
  auto base=library/"Application Support";auto managed=base/"OmniPlay";result=managed/"NativeSessions";
  for(const auto& directory:{library,base}){
    if(mkdir(directory.c_str(),0700)!=0&&errno!=EEXIST)return false;
    struct stat shape{};if(lstat(directory.c_str(),&shape)!=0||!S_ISDIR(shape.st_mode)||shape.st_uid!=geteuid()||(shape.st_mode&0022)!=0||!sync_directory(directory.parent_path()))return false;
  }
  if(mkdir(managed.c_str(),0700)!=0&&errno!=EEXIST)return false;if(!private_directory(managed))return false;
  if(mkdir(result.c_str(),0700)!=0&&errno!=EEXIST)return false;if(!private_directory(result))return false;
  // Profile UUIDs come from the authenticated, immutable connection binding.
  // Legacy unscoped directories are retained and never adopted as this profile.
  const auto profiles=result/"Profiles";
  if(mkdir(profiles.c_str(),0700)!=0&&errno!=EEXIST)return false;
  if(!private_directory(profiles))return false;
  size_t count=0;std::error_code error;
  std::filesystem::directory_iterator item(profiles,error),end;if(error)return false;
  while(item!=end){if(++count>4||!uuid(item->path().filename().c_str())||!private_directory(item->path()))return false;item.increment(error);if(error)return false;}
  result=profiles/profile;
  struct stat existing{};
  if(lstat(result.c_str(),&existing)!=0){if(errno!=ENOENT||count>=4||mkdir(result.c_str(),0700)!=0||!sync_directory(profiles))return false;}
  if(!private_directory(result))return false;
  return sync_directory(base)&&sync_directory(managed)&&sync_directory(profiles.parent_path())&&sync_directory(profiles);
}
bool sync_directory(const std::filesystem::path& path){int descriptor=open(path.c_str(),O_RDONLY|O_DIRECTORY|O_CLOEXEC|O_NOFOLLOW);if(descriptor<0)return false;bool ok=fsync(descriptor)==0;close(descriptor);return ok;}
bool remove_flat_directory(const std::filesystem::path& path,bool require_lease,bool empty_retired=false){
  Descriptor directory;directory.value=open(path.c_str(),O_RDONLY|O_DIRECTORY|O_CLOEXEC|O_NOFOLLOW);
  struct stat original{};if(directory.value<0||fstat(directory.value,&original)!=0||!S_ISDIR(original.st_mode)||
      original.st_uid!=geteuid()||(original.st_mode&07777)!=0700)return false;
  // A persisted launch fence survives owner restart. Numeric process records
  // are never adopted as permission to signal or remove this directory.
  struct stat marker{};if(fstatat(directory.value,".launch-intent",&marker,AT_SYMLINK_NOFOLLOW)==0||errno!=ENOENT)return false;
  Descriptor lease;lease.value=openat(directory.value,".owner-lease",O_RDWR|O_CLOEXEC|O_NOFOLLOW);
  if(lease.value<0){if(errno!=ENOENT||(require_lease&&!empty_retired))return false;}else{
    struct stat shape{};if(fstat(lease.value,&shape)!=0||!S_ISREG(shape.st_mode)||shape.st_uid!=geteuid()||
        shape.st_nlink!=1||(shape.st_mode&07777)!=0600||shape.st_size!=0||flock(lease.value,LOCK_EX|LOCK_NB)!=0)return false;
  }
  const int duplicate=fcntl(directory.value,F_DUPFD_CLOEXEC,10);if(duplicate<0)return false;
  DIR* listing=fdopendir(duplicate);if(!listing){close(duplicate);return false;}
  std::vector<std::string> names;uint64_t total=0;bool valid=true;errno=0;
  while(auto* entry=readdir(listing)){
    const std::string file=entry->d_name;if(file=="."||file=="..")continue;
    if(names.size()>=kMaximumAssets+2||(!name(file.c_str())&&file!="receipt.json"&&file!=".owner-lease")){valid=false;break;}
    struct stat shape{};const uint64_t limit=file=="receipt.json"?65536:file==".owner-lease"?0:kMaximumAssetBytes;
    if(fstatat(directory.value,file.c_str(),&shape,AT_SYMLINK_NOFOLLOW)!=0||!S_ISREG(shape.st_mode)||
        shape.st_uid!=geteuid()||shape.st_nlink!=1||((shape.st_mode&07777)!=0400&&(shape.st_mode&07777)!=0600)||
        shape.st_size<0||static_cast<uint64_t>(shape.st_size)>limit||
        total>kMaximumSessionBytes+65536-static_cast<uint64_t>(shape.st_size)){valid=false;break;}
    total+=static_cast<uint64_t>(shape.st_size);names.push_back(file);errno=0;
  }
  const int error=errno;const bool closed=closedir(listing)==0;
  struct stat current{};if(!valid||error||!closed||lstat(path.c_str(),&current)!=0||!S_ISDIR(current.st_mode)||
      current.st_dev!=original.st_dev||current.st_ino!=original.st_ino)return false;
  // A missing lease is recoverable only for an empty managed pending/retired
  // directory. A nonempty unknown shape is never reclaimed.
  if(require_lease&&lease.value<0&&!names.empty())return false;
  // Flat bounded deletion only: never recurse into an unexpected directory or
  // follow links, and retain unknown/partial shapes for later explicit repair.
  std::sort(names.begin(),names.end());
  for(const auto& file:names)if(file!=".owner-lease"&&unlinkat(directory.value,file.c_str(),0)!=0)return false;
  if(lease.value>=0&&unlinkat(directory.value,".owner-lease",0)!=0)return false;
  return fsync(directory.value)==0&&rmdir(path.c_str())==0;
}
bool inventory(const std::filesystem::path& root,size_t& sessions){
  sessions=0;size_t entries=0;std::error_code error;
  std::vector<std::pair<std::filesystem::path,bool>> abandoned;
  std::filesystem::directory_iterator item(root,error),end;if(error)return false;
  while(item!=end){if(++entries>kMaximumRootEntries)return false;
    const auto path=item->path();const auto filename=path.filename().native();
    const bool final=uuid(filename.c_str());
    const bool pending=filename.size()==45&&filename.front()=='.'&&filename.ends_with(".pending")&&uuid(filename.substr(1,36).c_str());
    const bool retired=filename.size()==46&&filename.front()=='.'&&filename.ends_with(".retiring")&&uuid(filename.substr(1,36).c_str());
    if(!final&&!pending&&!retired)return false;
    if(!private_directory(path))return false;
    if(pending||retired)abandoned.emplace_back(path,retired);else if(++sessions>kMaximumSessions)return false;
    item.increment(error);if(error)return false;
  }
  for(const auto& entry:abandoned)if(!remove_flat_directory(entry.first,true,true))return false;
  return sync_directory(root);
}
bool retire_directory(const std::filesystem::path& target){
  Descriptor directory;directory.value=open(target.c_str(),O_RDONLY|O_DIRECTORY|O_CLOEXEC|O_NOFOLLOW);
  if(directory.value<0||!private_directory(target))return false;
  struct stat intent{};if(fstatat(directory.value,".launch-intent",&intent,AT_SYMLINK_NOFOLLOW)==0||errno!=ENOENT)return false;
  Descriptor lease;lease.value=openat(directory.value,".owner-lease",O_RDWR|O_CLOEXEC|O_NOFOLLOW);
  struct stat shape{};if(lease.value<0||fstat(lease.value,&shape)!=0||!S_ISREG(shape.st_mode)||shape.st_uid!=geteuid()||shape.st_nlink!=1||(shape.st_mode&07777)!=0600||shape.st_size!=0||flock(lease.value,LOCK_EX|LOCK_NB)!=0)return false;
  const auto retired=target.parent_path()/("."+target.filename().native()+".retiring");
  struct stat exists{};if(lstat(retired.c_str(),&exists)==0||errno!=ENOENT)return false;
  // The rename, flushed in the parent, is the durable reclamation journal.
  // Admission only accepts final UUID paths, never a partially deleted tree.
  if(renameatx_np(AT_FDCWD,target.c_str(),AT_FDCWD,retired.c_str(),RENAME_EXCL)!=0||!sync_directory(target.parent_path()))return false;
  close(lease.value);lease.value=-1;
  return remove_flat_directory(retired,true,true)&&sync_directory(target.parent_path());
}
std::string hex(const std::array<unsigned char,CC_SHA256_DIGEST_LENGTH>& value){static constexpr char digits[]="0123456789abcdef";std::string result(value.size()*2,'0');for(size_t i=0;i<value.size();++i){result[i*2]=digits[value[i]>>4];result[i*2+1]=digits[value[i]&15];}return result;}

bool copy_asset(Asset& asset,const std::filesystem::path& target) {
  Descriptor input;input.value=asset.descriptor;asset.descriptor=-1;struct stat before{};int flags=fcntl(input.value,F_GETFL);
  if(fstat(input.value,&before)!=0||!S_ISREG(before.st_mode)||before.st_size<0||static_cast<uint64_t>(before.st_size)!=asset.bytes||flags<0||(flags&O_ACCMODE)!=O_RDONLY)return false;
  Descriptor output;output.value=open(target.c_str(),O_WRONLY|O_CREAT|O_EXCL|O_CLOEXEC|O_NOFOLLOW,0600);if(output.value<0)return false;
  CC_SHA256_CTX context;if(CC_SHA256_Init(&context)!=1)return false;std::array<unsigned char,65536> buffer{};uint64_t offset=0;
  while(offset<asset.bytes){size_t wanted=static_cast<size_t>(std::min<uint64_t>(buffer.size(),asset.bytes-offset));ssize_t count=pread(input.value,buffer.data(),wanted,static_cast<off_t>(offset));if(count<=0)return false;if(CC_SHA256_Update(&context,buffer.data(),static_cast<CC_LONG>(count))!=1)return false;size_t written=0;while(written<static_cast<size_t>(count)){ssize_t value=write(output.value,buffer.data()+written,static_cast<size_t>(count)-written);if(value<=0)return false;written+=static_cast<size_t>(value);}offset+=static_cast<uint64_t>(count);}
  unsigned char extra=0;if(pread(input.value,&extra,1,static_cast<off_t>(asset.bytes))!=0)return false;std::array<unsigned char,CC_SHA256_DIGEST_LENGTH> digest{};if(CC_SHA256_Final(digest.data(),&context)!=1||hex(digest)!=asset.hash)return false;
  struct stat after{};if(fstat(input.value,&after)!=0||before.st_dev!=after.st_dev||before.st_ino!=after.st_ino||before.st_size!=after.st_size||fchmod(output.value,0400)!=0||fsync(output.value)!=0)return false;return true;
}
bool write_receipt(const std::filesystem::path& directory,const char* session,const char* runtime,const std::string& profile,const std::vector<Asset>& assets){
  Descriptor lease;lease.value=open((directory/".owner-lease").c_str(),O_RDWR|O_CLOEXEC|O_NOFOLLOW);
  struct stat shape{};
  if(lease.value<0||fstat(lease.value,&shape)!=0||!S_ISREG(shape.st_mode)||shape.st_uid!=geteuid()||shape.st_nlink!=1||(shape.st_mode&07777)!=0600||shape.st_size!=0)return false;
  std::string raw="{\"schema_version\":3,\"profile_id\":\""+profile+"\",\"session_id\":\""+std::string(session)+"\",\"runtime_id\":\""+std::string(runtime)+"\",\"assets\":[";
  for(size_t i=0;i<assets.size();++i){if(i)raw+=',';raw+="{\"name\":\""+assets[i].name+"\",\"bytes\":"+std::to_string(assets[i].bytes)+",\"sha256\":\""+assets[i].hash+"\"}";}
  raw+="]}\n";if(raw.size()>65536)return false;
  Descriptor file;file.value=open((directory/"receipt.json").c_str(),O_WRONLY|O_CREAT|O_EXCL|O_CLOEXEC|O_NOFOLLOW,0400);
  if(file.value<0)return false;size_t offset=0;
  while(offset<raw.size()){ssize_t count=write(file.value,raw.data()+offset,raw.size()-offset);if(count<0&&errno==EINTR)continue;if(count<=0)return false;offset+=static_cast<size_t>(count);}
  return fsync(file.value)==0;
}

bool regular(int fd,mode_t mode,uint64_t maximum,struct stat& result){
  return fstat(fd,&result)==0&&S_ISREG(result.st_mode)&&result.st_uid==geteuid()&&result.st_nlink==1&&
      (result.st_mode&07777)==mode&&result.st_size>=0&&static_cast<uint64_t>(result.st_size)<=maximum;
}
bool unchanged(int fd,const struct stat& before){
  struct stat after{};return fstat(fd,&after)==0&&after.st_dev==before.st_dev&&after.st_ino==before.st_ino&&
      after.st_size==before.st_size&&after.st_mode==before.st_mode&&after.st_nlink==before.st_nlink&&
      after.st_mtimespec.tv_sec==before.st_mtimespec.tv_sec&&after.st_mtimespec.tv_nsec==before.st_mtimespec.tv_nsec&&
      after.st_ctimespec.tv_sec==before.st_ctimespec.tv_sec&&after.st_ctimespec.tv_nsec==before.st_ctimespec.tv_nsec;
}
bool number(id value,uint64_t minimum,uint64_t maximum){
  return [value isKindOfClass:NSNumber.class]&&CFGetTypeID((__bridge CFTypeRef)value)!=CFBooleanGetTypeID()&&
      !CFNumberIsFloatType((__bridge CFNumberRef)value)&&[value longLongValue]>=0&&
      [value unsignedLongLongValue]>=minimum&&[value unsignedLongLongValue]<=maximum;
}
bool dictionary(id value,NSArray<NSString*>* keys){
  return [value isKindOfClass:NSDictionary.class]&&[value count]==keys.count&&
      [[NSSet setWithArray:[value allKeys]] isEqualToSet:[NSSet setWithArray:keys]];
}
bool checked_asset(int directory,NSString* asset_name,uint64_t bytes,NSString* expected){
  Descriptor file;file.value=openat(directory,asset_name.UTF8String,O_RDONLY|O_CLOEXEC|O_NOFOLLOW);
  struct stat before{};if(file.value<0||!regular(file.value,0400,kMaximumAssetBytes,before)||static_cast<uint64_t>(before.st_size)!=bytes)return false;
  CC_SHA256_CTX context;if(CC_SHA256_Init(&context)!=1)return false;
  std::array<unsigned char,65536> buffer{};uint64_t offset=0;
  while(offset<bytes){const size_t wanted=static_cast<size_t>(std::min<uint64_t>(buffer.size(),bytes-offset));
    const ssize_t count=pread(file.value,buffer.data(),wanted,static_cast<off_t>(offset));
    if(count<0&&errno==EINTR)continue;if(count<=0||CC_SHA256_Update(&context,buffer.data(),static_cast<CC_LONG>(count))!=1)return false;offset+=static_cast<uint64_t>(count);}
  std::array<unsigned char,CC_SHA256_DIGEST_LENGTH> digest{};
  return CC_SHA256_Final(digest.data(),&context)==1&&hex(digest)==expected.UTF8String&&unchanged(file.value,before);
}
}

SessionLease::~SessionLease(){if(lease_>=0)close(lease_);if(directory_>=0)close(directory_);}
SessionLease::SessionLease(SessionLease&& other)noexcept
  :directory_(std::exchange(other.directory_,-1)),lease_(std::exchange(other.lease_,-1)),
   device_(other.device_),inode_(other.inode_),lease_device_(other.lease_device_),lease_inode_(other.lease_inode_),
   marker_device_(other.marker_device_),marker_inode_(other.marker_inode_),marked_(other.marked_),
   marker_unlinked_(other.marker_unlinked_),session_(std::move(other.session_)),runtime_(std::move(other.runtime_)),profile_(std::move(other.profile_)),path_(std::move(other.path_)){}
bool SessionLease::valid()const{
  struct stat directory{},path{},lease{},named{};
  return directory_>=0&&lease_>=0&&fstat(directory_,&directory)==0&&directory.st_dev==device_&&directory.st_ino==inode_&&
      S_ISDIR(directory.st_mode)&&directory.st_uid==geteuid()&&(directory.st_mode&07777)==0700&&
      lstat(path_.c_str(),&path)==0&&S_ISDIR(path.st_mode)&&path.st_dev==directory.st_dev&&path.st_ino==directory.st_ino&&
      regular(lease_,0600,0,lease)&&lease.st_dev==lease_device_&&lease.st_ino==lease_inode_&&
      fstatat(directory_,".owner-lease",&named,AT_SYMLINK_NOFOLLOW)==0&&named.st_dev==lease.st_dev&&named.st_ino==lease.st_ino;
}
bool SessionLease::begin_launch(){
  if(marked_||!valid())return false;
  const auto boot=boot_session();if(!boot)return false;
  Descriptor marker;marker.value=openat(directory_,".launch-intent",O_WRONLY|O_CREAT|O_EXCL|O_CLOEXEC|O_NOFOLLOW,0600);
  if(marker.value<0)return false;marked_=true;struct stat shape{};
  if(!regular(marker.value,0600,0,shape))return false;
  marker_device_=shape.st_dev;marker_inode_=shape.st_ino;
  const std::string raw="OPMACLAUNCH2\n"+profile_+"\n"+session_+"\n"+runtime_+"\n"+*boot+"\n";size_t offset=0;
  while(offset<raw.size()){const ssize_t count=write(marker.value,raw.data()+offset,raw.size()-offset);if(count<0&&errno==EINTR)continue;if(count<=0)return false;offset+=static_cast<size_t>(count);}
  return fsync(marker.value)==0&&fsync(directory_)==0;
}
bool SessionLease::finish_launch(){
  if(!valid())return false;if(!marked_)return true;
  if(!marker_unlinked_){struct stat current{};
    if(fstatat(directory_,".launch-intent",&current,AT_SYMLINK_NOFOLLOW)!=0||!S_ISREG(current.st_mode)||
       current.st_dev!=marker_device_||current.st_ino!=marker_inode_||current.st_uid!=geteuid()||
       current.st_nlink!=1||(current.st_mode&07777)!=0600||unlinkat(directory_,".launch-intent",0)!=0)return false;
    marker_unlinked_=true;
  }
  if(fsync(directory_)!=0)return false;marked_=false;return true;
}

std::optional<SessionLease> admit_staged_session(const std::string& session,const std::string& runtime,const std::string& profile){
  if(!uuid(session.c_str())||!admit_runtime(runtime.c_str()))return std::nullopt;
  std::lock_guard lock(g_sessions);std::filesystem::path root;if(!ensure_root(root,profile))return std::nullopt;
  SessionLease result;result.path_=root/session;result.session_=session;result.runtime_=runtime;result.profile_=profile;
  result.directory_=open(result.path_.c_str(),O_RDONLY|O_DIRECTORY|O_CLOEXEC|O_NOFOLLOW);
  struct stat directory{};if(result.directory_<0||fstat(result.directory_,&directory)!=0||!S_ISDIR(directory.st_mode)||
      directory.st_uid!=geteuid()||(directory.st_mode&07777)!=0700)return std::nullopt;
  result.device_=directory.st_dev;result.inode_=directory.st_ino;
  result.lease_=openat(result.directory_,".owner-lease",O_RDWR|O_CLOEXEC|O_NOFOLLOW);
  struct stat lease{};if(result.lease_<0||!regular(result.lease_,0600,0,lease)||flock(result.lease_,LOCK_EX|LOCK_NB)!=0)return std::nullopt;
  result.lease_device_=lease.st_dev;result.lease_inode_=lease.st_ino;
  struct stat intent{};if(fstatat(result.directory_,".launch-intent",&intent,AT_SYMLINK_NOFOLLOW)==0||errno!=ENOENT)return std::nullopt;
  Descriptor receipt;receipt.value=openat(result.directory_,"receipt.json",O_RDONLY|O_CLOEXEC|O_NOFOLLOW);
  struct stat shape{};if(receipt.value<0||!regular(receipt.value,0400,65536,shape)||shape.st_size==0)return std::nullopt;
  std::vector<unsigned char> raw(static_cast<size_t>(shape.st_size));size_t offset=0;
  while(offset<raw.size()){const ssize_t count=pread(receipt.value,raw.data()+offset,raw.size()-offset,static_cast<off_t>(offset));if(count<0&&errno==EINTR)continue;if(count<=0)return std::nullopt;offset+=static_cast<size_t>(count);}
  NSData* data=[NSData dataWithBytes:raw.data() length:raw.size()];
  id decoded=[NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
  if(!dictionary(decoded,@[@"schema_version",@"profile_id",@"session_id",@"runtime_id",@"assets"])||
      !number(decoded[@"schema_version"],3,3)||![decoded[@"profile_id"] isEqual:[NSString stringWithUTF8String:profile.c_str()]]||![decoded[@"session_id"] isEqual:[NSString stringWithUTF8String:session.c_str()]]||
      ![decoded[@"runtime_id"] isEqual:[NSString stringWithUTF8String:runtime.c_str()]])return std::nullopt;
  NSArray* assets=decoded[@"assets"];if(![assets isKindOfClass:NSArray.class]||!assets.count||assets.count>kMaximumAssets)return std::nullopt;
  std::set<std::string> names{"receipt.json",".owner-lease"};uint64_t total=0;
  for(id asset in assets){if(!dictionary(asset,@[@"name",@"bytes",@"sha256"]))return std::nullopt;
    NSString* asset_name=asset[@"name"];NSString* expected=asset[@"sha256"];
    if(![asset_name isKindOfClass:NSString.class]||![expected isKindOfClass:NSString.class]||
       !name(asset_name.UTF8String)||asset_name.length!=strlen(asset_name.UTF8String)||
       expected.length!=64||!hash(expected.UTF8String)||!number(asset[@"bytes"],1,kMaximumAssetBytes)||
       !names.insert(asset_name.UTF8String).second)return std::nullopt;
    const uint64_t bytes=[asset[@"bytes"] unsignedLongLongValue];if(total>kMaximumSessionBytes-bytes)return std::nullopt;total+=bytes;
    if(!checked_asset(result.directory_,asset_name,bytes,expected))return std::nullopt;
  }
  const int duplicate=fcntl(result.directory_,F_DUPFD_CLOEXEC,10);if(duplicate<0)return std::nullopt;
  DIR* listing=fdopendir(duplicate);if(!listing){close(duplicate);return std::nullopt;}
  size_t count=0;bool exact_inventory=true;errno=0;
  while(auto* entry=readdir(listing)){
    if(!strcmp(entry->d_name,".")||!strcmp(entry->d_name,".."))continue;
    if(++count>kMaximumAssets+2||!names.contains(entry->d_name)){exact_inventory=false;break;}
  }
  const int error=errno;const bool closed=closedir(listing)==0;
  if(!exact_inventory||error||!closed||count!=names.size()||!unchanged(receipt.value,shape)||!result.valid())return std::nullopt;
  return std::optional<SessionLease>(std::move(result));
}

StageStatus recover_session(xpc_object_t request,const std::string& profile){
  if(!exact(request,{"protocol","operation","request_id","profile_id","session_id","runtime_id","prior_boot"}))return StageStatus::rejected;
  const char* selected=xpc_dictionary_get_string(request,"profile_id");
  const char* session=xpc_dictionary_get_string(request,"session_id");
  const char* runtime=xpc_dictionary_get_string(request,"runtime_id");
  const char* prior=xpc_dictionary_get_string(request,"prior_boot");
  if(!selected||profile!=selected||!uuid(session)||!uuid(prior)||!runtime)return StageStatus::rejected;
  const size_t runtime_bytes=strnlen(runtime,65);if(!runtime_bytes||runtime_bytes>64)return StageStatus::rejected;
  for(size_t i=0;i<runtime_bytes;++i)if(!((runtime[i]>='A'&&runtime[i]<='Z')||(runtime[i]>='a'&&runtime[i]<='z')||(runtime[i]>='0'&&runtime[i]<='9')||runtime[i]=='_'||runtime[i]=='-'))return StageStatus::rejected;
  // prior comes from the authenticated broker's protected journal OUTSIDE this
  // inherited container. A writable marker alone does not establish a reboot.
  const auto current=boot_session();if(!current||*current==prior)return StageStatus::rejected;
  std::lock_guard lock(g_sessions);if(session_active(session))return StageStatus::rejected;
  std::filesystem::path root;if(!ensure_root(root,profile))return StageStatus::storage_failure;
  const auto final=root/session,retired=root/("."+std::string(session)+".retiring"),pending=root/("."+std::string(session)+".pending");
  std::filesystem::path target;unsigned found=0;
  for(const auto& path:{final,retired,pending}){struct stat shape{};
    if(lstat(path.c_str(),&shape)==0){if(!S_ISDIR(shape.st_mode)||shape.st_uid!=geteuid()||(shape.st_mode&07777)!=0700)return StageStatus::rejected;target=path;++found;}
    else if(errno!=ENOENT)return StageStatus::storage_failure;
  }
  if(!found)return sync_directory(root)?StageStatus::ok:StageStatus::storage_failure;
  if(found!=1)return StageStatus::rejected;
  if(target==pending)return remove_flat_directory(target,true,true)&&sync_directory(root)?StageStatus::ok:StageStatus::storage_failure;
  Descriptor directory;directory.value=open(target.c_str(),O_RDONLY|O_DIRECTORY|O_CLOEXEC|O_NOFOLLOW);
  struct stat original{};if(directory.value<0||fstat(directory.value,&original)!=0)return StageStatus::storage_failure;
  Descriptor lease;lease.value=openat(directory.value,".owner-lease",O_RDWR|O_CLOEXEC|O_NOFOLLOW);
  struct stat lease_shape{};
  if(lease.value<0){
    if(target!=retired||errno!=ENOENT)return StageStatus::rejected;
    return remove_flat_directory(target,true,true)&&sync_directory(root)?StageStatus::ok:StageStatus::storage_failure;
  }
  if(!regular(lease.value,0600,0,lease_shape)||flock(lease.value,LOCK_EX|LOCK_NB)!=0)return StageStatus::rejected;
  Descriptor marker;marker.value=openat(directory.value,".launch-intent",O_RDONLY|O_CLOEXEC|O_NOFOLLOW);
  if(marker.value<0){
    if(errno!=ENOENT)return StageStatus::rejected;close(lease.value);lease.value=-1;
    const bool removed=target==retired?remove_flat_directory(target,true,true):retire_directory(target);
    return removed&&sync_directory(root)?StageStatus::ok:StageStatus::storage_failure;
  }
  const std::string expected="OPMACLAUNCH2\n"+profile+"\n"+session+"\n"+runtime+"\n"+prior+"\n";
  struct stat marker_shape{};
  if(!regular(marker.value,0600,256,marker_shape)||static_cast<size_t>(marker_shape.st_size)!=expected.size())return StageStatus::rejected;
  std::array<char,256> raw{};size_t offset=0;
  while(offset<expected.size()){const auto count=pread(marker.value,raw.data()+offset,expected.size()-offset,static_cast<off_t>(offset));if(count<0&&errno==EINTR)continue;if(count<=0)return StageStatus::storage_failure;offset+=static_cast<size_t>(count);}
  if(std::string(raw.data(),offset)!=expected||!unchanged(marker.value,marker_shape))return StageStatus::rejected;
  struct stat named{},lease_named{},marker_named{};
  if(lstat(target.c_str(),&named)!=0||named.st_dev!=original.st_dev||named.st_ino!=original.st_ino||
      fstatat(directory.value,".owner-lease",&lease_named,AT_SYMLINK_NOFOLLOW)!=0||lease_named.st_dev!=lease_shape.st_dev||lease_named.st_ino!=lease_shape.st_ino||
      fstatat(directory.value,".launch-intent",&marker_named,AT_SYMLINK_NOFOLLOW)!=0||marker_named.st_dev!=marker_shape.st_dev||marker_named.st_ino!=marker_shape.st_ino)return StageStatus::rejected;
  // Rename and sync before clearing the fence. An interrupted retirement must
  // not leave a launchable final UUID between fence removal and reclamation.
  if(target==final){if(renameatx_np(AT_FDCWD,final.c_str(),AT_FDCWD,retired.c_str(),RENAME_EXCL)!=0||!sync_directory(root))return StageStatus::storage_failure;}
  if(unlinkat(directory.value,".launch-intent",0)!=0||fsync(directory.value)!=0)return StageStatus::storage_failure;
  close(lease.value);lease.value=-1;
  return remove_flat_directory(retired,true,true)&&sync_directory(root)?StageStatus::ok:StageStatus::storage_failure;
}

StageStatus stage_session(xpc_object_t request,const std::string& profile) {
  if(!exact(request,{"protocol","operation","request_id","profile_id","session_id","runtime_id","assets"}))return StageStatus::rejected;
  const char* selected_profile=xpc_dictionary_get_string(request,"profile_id");
  if(!selected_profile||profile!=selected_profile)return StageStatus::rejected;
  const char* session=xpc_dictionary_get_string(request,"session_id");const char* runtime=xpc_dictionary_get_string(request,"runtime_id");xpc_object_t list=xpc_dictionary_get_value(request,"assets");if(!uuid(session)||!admit_runtime(runtime).has_value()||!list||xpc_get_type(list)!=XPC_TYPE_ARRAY)return StageStatus::rejected;
  size_t count=xpc_array_get_count(list);if(count==0||count>kMaximumAssets)return StageStatus::rejected;std::vector<Asset> assets;assets.reserve(count);std::set<std::string> names;uint64_t total=0;
  for(size_t index=0;index<count;++index){xpc_object_t item=xpc_array_get_value(list,index);if(!exact(item,{"name","bytes","sha256","fd"}))return StageStatus::rejected;const char* asset_name=xpc_dictionary_get_string(item,"name");const char* asset_hash=xpc_dictionary_get_string(item,"sha256");xpc_object_t fd=xpc_dictionary_get_value(item,"fd");if(!name(asset_name)||!hash(asset_hash)||!fd||xpc_get_type(fd)!=XPC_TYPE_FD||!names.insert(asset_name).second)return StageStatus::rejected;auto declared_bytes=xpc_dictionary_get_value(item,"bytes");if(!declared_bytes||xpc_get_type(declared_bytes)!=XPC_TYPE_UINT64)return StageStatus::rejected;uint64_t bytes=xpc_uint64_get_value(declared_bytes);if(bytes==0||bytes>kMaximumAssetBytes||total>kMaximumSessionBytes-bytes)return StageStatus::rejected;int descriptor=xpc_dictionary_dup_fd(item,"fd");if(descriptor<0)return StageStatus::rejected;total+=bytes;assets.push_back(Asset{asset_name,asset_hash,bytes,descriptor});}
  std::sort(assets.begin(),assets.end(),[](const Asset& left,const Asset& right){return left.name<right.name;});std::lock_guard<std::mutex> guard(g_sessions);std::filesystem::path root;if(!ensure_root(root,profile))return StageStatus::storage_failure;size_t sessions=0;if(!inventory(root,sessions))return StageStatus::storage_failure;if(sessions>=kMaximumSessions)return StageStatus::rejected;
  std::filesystem::path pending=root/("."+std::string(session)+".pending"),final=root/session;struct stat shape{};if(lstat(pending.c_str(),&shape)==0||errno!=ENOENT||lstat(final.c_str(),&shape)==0||errno!=ENOENT)return StageStatus::rejected;
  if(mkdir(pending.c_str(),0700)!=0)return StageStatus::storage_failure;Descriptor initial_lease;initial_lease.value=open((pending/".owner-lease").c_str(),O_RDWR|O_CREAT|O_EXCL|O_CLOEXEC|O_NOFOLLOW,0600);bool ok=initial_lease.value>=0&&fsync(initial_lease.value)==0&&sync_directory(pending)&&sync_directory(root),published=false;for(auto& asset:assets)if(ok)ok=copy_asset(asset,pending/asset.name);if(ok)ok=write_receipt(pending,session,runtime,profile,assets);if(ok)ok=sync_directory(pending);if(ok){ok=renameatx_np(AT_FDCWD,pending.c_str(),AT_FDCWD,final.c_str(),RENAME_EXCL)==0;published=ok;}if(ok)ok=sync_directory(root);if(!ok){remove_flat_directory(published?final:pending,false);sync_directory(root);return StageStatus::storage_failure;}return StageStatus::ok;
}

StageStatus discard_session(xpc_object_t request,const std::string& profile){if(!exact(request,{"protocol","operation","request_id","profile_id","session_id"}))return StageStatus::rejected;const char* selected_profile=xpc_dictionary_get_string(request,"profile_id");if(!selected_profile||profile!=selected_profile)return StageStatus::rejected;const char* session=xpc_dictionary_get_string(request,"session_id");if(!uuid(session))return StageStatus::rejected;std::lock_guard<std::mutex> guard(g_sessions);if(session_active(session))return StageStatus::rejected;std::filesystem::path root;if(!ensure_root(root,profile))return StageStatus::storage_failure;size_t sessions=0;if(!inventory(root,sessions))return StageStatus::storage_failure;auto target=root/session;struct stat shape{};if(lstat(target.c_str(),&shape)!=0){return errno==ENOENT?StageStatus::ok:StageStatus::storage_failure;}if(!S_ISDIR(shape.st_mode)||shape.st_uid!=geteuid()||(shape.st_mode&07777)!=0700)return StageStatus::rejected;if(!retire_directory(target)||!sync_directory(root))return StageStatus::storage_failure;return StageStatus::ok;}
}
