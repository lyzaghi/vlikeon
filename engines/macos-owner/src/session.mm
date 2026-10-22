#import <Foundation/Foundation.h>
#include "session.hpp"
#include "catalog.hpp"
#include "process.hpp"
#include <CommonCrypto/CommonDigest.h>
#include <algorithm>
#include <array>
#include <cctype>
#include <cerrno>
#include <cstdint>
#include <cstring>
#include <fcntl.h>
#include <filesystem>
#include <initializer_list>
#include <limits>
#include <mutex>
#include <set>
#include <string>
#include <sys/stat.h>
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
  for (size_t index = 0; index < 36; ++index) {
    if (index == 8 || index == 13 || index == 18 || index == 23) { if (value[index] != '-') return false; }
    else if (!((value[index] >= '0' && value[index] <= '9') || (value[index] >= 'a' && value[index] <= 'f'))) return false;
  }
  return true;
}
bool hash(const char* value) {
  if (!value || strnlen(value, 65) != 64) return false;
  for (size_t index=0;index<64;++index)if(!((value[index]>='0'&&value[index]<='9')||(value[index]>='a'&&value[index]<='f')))return false;
  return true;
}
bool name(const char* value) {
  if (!value) return false;size_t length=strnlen(value,129);if(length==0||length>128||!std::isalnum(static_cast<unsigned char>(value[0]))||value[length-1]=='.'||strcmp(value,"receipt.json")==0)return false;
  for(size_t index=0;index<length;++index){unsigned char byte=value[index];if(!(std::isalnum(byte)||byte=='_'||byte=='-'||byte=='.'||byte=='+'))return false;}return true;
}
bool exact(xpc_object_t dictionary,std::initializer_list<const char*> names){if(xpc_get_type(dictionary)!=XPC_TYPE_DICTIONARY||xpc_dictionary_get_count(dictionary)!=names.size())return false;for(const char* name:names)if(!xpc_dictionary_get_value(dictionary,name))return false;return true;}

bool private_directory(const std::filesystem::path& path) {
  struct stat shape{};return lstat(path.c_str(),&shape)==0&&S_ISDIR(shape.st_mode)&&shape.st_uid==geteuid()&&(shape.st_mode&07777)==0700;
}
bool ensure_root(std::filesystem::path& result) {
  NSString* home=NSHomeDirectory();if(![home isKindOfClass:NSString.class]||home.length==0)return false;
  auto base=std::filesystem::path(home.fileSystemRepresentation)/"Library/Application Support";auto managed=base/"OmniPlay";result=managed/"NativeSessions";
  struct stat base_shape{};if(lstat(base.c_str(),&base_shape)!=0||!S_ISDIR(base_shape.st_mode)||base_shape.st_uid!=geteuid()||(base_shape.st_mode&0022)!=0)return false;
  if(mkdir(managed.c_str(),0700)!=0&&errno!=EEXIST)return false;if(!private_directory(managed))return false;
  if(mkdir(result.c_str(),0700)!=0&&errno!=EEXIST)return false;if(!private_directory(result))return false;
  return true;
}
bool sync_directory(const std::filesystem::path& path){int descriptor=open(path.c_str(),O_RDONLY|O_DIRECTORY|O_CLOEXEC|O_NOFOLLOW);if(descriptor<0)return false;bool ok=fsync(descriptor)==0;close(descriptor);return ok;}
bool inventory(const std::filesystem::path& root,size_t& sessions){sessions=0;size_t entries=0;std::error_code error;std::vector<std::filesystem::path> abandoned;std::filesystem::directory_iterator item(root,error),end;if(error)return false;while(item!=end){if(++entries>kMaximumRootEntries)return false;auto path=item->path();auto filename=path.filename().native();bool final=uuid(filename.c_str()),pending=filename.size()==45&&filename.front()=='.'&&filename.ends_with(".pending")&&uuid(filename.substr(1,36).c_str());if(!final&&!pending)return false;struct stat shape{};if(lstat(path.c_str(),&shape)!=0||!S_ISDIR(shape.st_mode)||shape.st_uid!=geteuid()||(shape.st_mode&07777)!=0700)return false;if(pending)abandoned.push_back(path);else if(++sessions>kMaximumSessions)return false;item.increment(error);if(error)return false;}for(const auto& path:abandoned){std::filesystem::remove_all(path,error);if(error)return false;}return sync_directory(root);}
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
bool write_receipt(const std::filesystem::path& directory,const char* session,const char* runtime,const std::vector<Asset>& assets){std::string raw="{\"schema_version\":1,\"session_id\":\""+std::string(session)+"\",\"runtime_id\":\""+std::string(runtime)+"\",\"assets\":[";for(size_t i=0;i<assets.size();++i){if(i)raw+=',';raw+="{\"name\":\""+assets[i].name+"\",\"bytes\":"+std::to_string(assets[i].bytes)+",\"sha256\":\""+assets[i].hash+"\"}";}raw+="]}\n";if(raw.size()>65536)return false;Descriptor file;file.value=open((directory/"receipt.json").c_str(),O_WRONLY|O_CREAT|O_EXCL|O_CLOEXEC|O_NOFOLLOW,0400);if(file.value<0)return false;size_t offset=0;while(offset<raw.size()){ssize_t count=write(file.value,raw.data()+offset,raw.size()-offset);if(count<=0)return false;offset+=static_cast<size_t>(count);}return fsync(file.value)==0;}
}

StageStatus stage_session(xpc_object_t request) {
  if(!exact(request,{"protocol","operation","request_id","session_id","runtime_id","assets"}))return StageStatus::rejected;
  const char* session=xpc_dictionary_get_string(request,"session_id");const char* runtime=xpc_dictionary_get_string(request,"runtime_id");xpc_object_t list=xpc_dictionary_get_value(request,"assets");if(!uuid(session)||!admit_runtime(runtime).has_value()||!list||xpc_get_type(list)!=XPC_TYPE_ARRAY)return StageStatus::rejected;
  size_t count=xpc_array_get_count(list);if(count==0||count>kMaximumAssets)return StageStatus::rejected;std::vector<Asset> assets;assets.reserve(count);std::set<std::string> names;uint64_t total=0;
  for(size_t index=0;index<count;++index){xpc_object_t item=xpc_array_get_value(list,index);if(!exact(item,{"name","bytes","sha256","fd"}))return StageStatus::rejected;const char* asset_name=xpc_dictionary_get_string(item,"name");const char* asset_hash=xpc_dictionary_get_string(item,"sha256");xpc_object_t fd=xpc_dictionary_get_value(item,"fd");if(!name(asset_name)||!hash(asset_hash)||!fd||xpc_get_type(fd)!=XPC_TYPE_FD||!names.insert(asset_name).second)return StageStatus::rejected;uint64_t bytes=xpc_dictionary_get_uint64(item,"bytes");if(bytes==0||bytes>kMaximumAssetBytes||total>kMaximumSessionBytes-bytes)return StageStatus::rejected;int descriptor=xpc_dictionary_dup_fd(item,"fd");if(descriptor<0)return StageStatus::rejected;total+=bytes;assets.push_back(Asset{asset_name,asset_hash,bytes,descriptor});}
  std::sort(assets.begin(),assets.end(),[](const Asset& left,const Asset& right){return left.name<right.name;});std::lock_guard<std::mutex> guard(g_sessions);std::filesystem::path root;if(!ensure_root(root))return StageStatus::storage_failure;size_t sessions=0;if(!inventory(root,sessions))return StageStatus::storage_failure;if(sessions>=kMaximumSessions)return StageStatus::rejected;
  std::filesystem::path pending=root/("."+std::string(session)+".pending"),final=root/session;struct stat shape{};if(lstat(pending.c_str(),&shape)==0||errno!=ENOENT||lstat(final.c_str(),&shape)==0||errno!=ENOENT)return StageStatus::rejected;
  if(mkdir(pending.c_str(),0700)!=0)return StageStatus::storage_failure;bool ok=true,published=false;for(auto& asset:assets)if(ok)ok=copy_asset(asset,pending/asset.name);if(ok)ok=write_receipt(pending,session,runtime,assets);if(ok)ok=sync_directory(pending);if(ok){ok=rename(pending.c_str(),final.c_str())==0;published=ok;}if(ok)ok=sync_directory(root);if(!ok){std::error_code error;std::filesystem::remove_all(published?final:pending,error);sync_directory(root);return StageStatus::storage_failure;}return StageStatus::ok;
}

StageStatus discard_session(xpc_object_t request){if(!exact(request,{"protocol","operation","request_id","session_id"}))return StageStatus::rejected;const char* session=xpc_dictionary_get_string(request,"session_id");if(!uuid(session))return StageStatus::rejected;std::lock_guard<std::mutex> guard(g_sessions);if(session_active(session))return StageStatus::rejected;std::filesystem::path root;if(!ensure_root(root))return StageStatus::storage_failure;size_t sessions=0;if(!inventory(root,sessions))return StageStatus::storage_failure;auto target=root/session;struct stat shape{};if(lstat(target.c_str(),&shape)!=0){return errno==ENOENT?StageStatus::ok:StageStatus::storage_failure;}if(!S_ISDIR(shape.st_mode)||shape.st_uid!=geteuid()||(shape.st_mode&07777)!=0700)return StageStatus::rejected;std::error_code error;std::filesystem::remove_all(target,error);if(error||!sync_directory(root))return StageStatus::storage_failure;return StageStatus::ok;}
}
